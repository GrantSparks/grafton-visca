//! smol-specific implementations of unified async I/O connectors.

use async_io::Timer;
use futures_lite::future::race;
use smol::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
};

use crate::{
    transport::{
        address::AddressResolver,
        async_io::{
            AsyncDatagram, AsyncReadExt as AsyncReadExtTrait, AsyncWriteExt as AsyncWriteExtTrait,
            TcpConnectionConfig, UdpSocketConfig,
        },
    },
    Error,
};

/// Combined TCP stream wrapper that implements both read and write traits.
#[derive(Debug)]
pub struct SmolTcpStream {
    stream: TcpStream,
}

impl SmolTcpStream {
    pub fn new(stream: TcpStream) -> Self {
        Self { stream }
    }

    pub fn clone_stream(&self) -> TcpStream {
        self.stream.clone()
    }
}

impl AsyncReadExtTrait for SmolTcpStream {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        Ok(self.stream.read(buf).await?)
    }
}

impl AsyncWriteExtTrait for SmolTcpStream {
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        Ok(self.stream.write_all(buf).await?)
    }

    async fn flush(&mut self) -> Result<(), Error> {
        Ok(self.stream.flush().await?)
    }
}

/// Create a configured TCP connection using unified helpers.
pub async fn connect_tcp(
    address: &str,
    config: TcpConnectionConfig,
) -> Result<SmolTcpStream, Error> {
    // Connect with timeout using race pattern
    let stream = race(async { Ok(TcpStream::connect(address).await?) }, async {
        Timer::after(config.connect_timeout).await;
        Err(Error::Timeout)
    })
    .await?;

    // Apply socket configuration
    if let Some(nodelay) = config.nodelay {
        stream.set_nodelay(nodelay)?;
    } else {
        stream.set_nodelay(true)?; // Default to low latency
    }

    if let Some(ttl) = config.ttl {
        stream.set_ttl(ttl)?;
    }

    Ok(SmolTcpStream::new(stream))
}

/// Create a configured UDP socket using unified helpers.
pub async fn connect_udp(address: &str, config: UdpSocketConfig) -> Result<UdpSocket, Error> {
    // Perform DNS resolution with timeout using unblock (smol doesn't have native async DNS)
    let address_owned = address.to_string();
    let target_addr = race(
        async {
            smol::unblock(move || {
                use std::net::ToSocketAddrs;
                address_owned
                    .to_socket_addrs()
                    .map_err(Error::from)?
                    .next()
                    .ok_or_else(|| Error::InvalidAddress {
                        reason: "No addresses resolved".into(),
                    })
            })
            .await
        },
        async {
            Timer::after(config.connect_timeout).await;
            Err(Error::Timeout)
        },
    )
    .await?;

    // Bind to the appropriate unspecified address based on target family
    let resolver = AddressResolver::new();
    let bind_addr = resolver.bind_address_for(&target_addr);

    let socket = UdpSocket::bind(bind_addr).await?;

    // Connect with timeout
    race(
        async {
            socket.connect(target_addr).await?;
            Ok(())
        },
        async {
            Timer::after(config.connect_timeout).await;
            Err(Error::Timeout)
        },
    )
    .await?;

    // Apply socket options
    if let Some(ttl) = config.ttl {
        socket.set_ttl(ttl)?;
    }

    Ok(socket)
}

/// Implement AsyncDatagram for smol's UdpSocket
impl AsyncDatagram for UdpSocket {
    async fn send(&self, buf: &[u8]) -> Result<usize, Error> {
        Ok(UdpSocket::send(self, buf).await?)
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<usize, Error> {
        Ok(UdpSocket::recv(self, buf).await?)
    }
}
