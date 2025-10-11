//! async-std-specific implementations of unified async I/O connectors.

use async_std::{
    io::prelude::*,
    net::{TcpStream, ToSocketAddrs, UdpSocket},
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
pub struct AsyncStdTcpStream {
    stream: TcpStream,
}

impl AsyncStdTcpStream {
    pub fn new(stream: TcpStream) -> Self {
        Self { stream }
    }

    pub fn clone_stream(&self) -> TcpStream {
        self.stream.clone()
    }
}

impl AsyncReadExtTrait for AsyncStdTcpStream {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        Ok(self.stream.read(buf).await?)
    }
}

impl AsyncWriteExtTrait for AsyncStdTcpStream {
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
) -> Result<AsyncStdTcpStream, Error> {
    // Connect with timeout
    let stream = async_std::future::timeout(config.connect_timeout, TcpStream::connect(address))
        .await
        .map_err(|_| Error::Timeout)??;

    // Apply socket configuration
    if let Some(nodelay) = config.nodelay {
        stream.set_nodelay(nodelay)?;
    } else {
        stream.set_nodelay(true)?; // Default to low latency
    }

    if let Some(ttl) = config.ttl {
        stream.set_ttl(ttl)?;
    }

    Ok(AsyncStdTcpStream::new(stream))
}

/// Create a configured UDP socket using unified helpers.
pub async fn connect_udp(address: &str, config: UdpSocketConfig) -> Result<UdpSocket, Error> {
    // Perform async DNS resolution with timeout
    let mut addrs = async_std::future::timeout(config.connect_timeout, address.to_socket_addrs())
        .await
        .map_err(|_| Error::Timeout)??;
    let target_addr = addrs.next().ok_or_else(|| Error::InvalidAddress {
        reason: "No addresses resolved".into(),
    })?;

    // Bind to the appropriate unspecified address based on target family
    let resolver = AddressResolver::new();
    let bind_addr = resolver.bind_address_for(&target_addr);

    let socket = UdpSocket::bind(bind_addr).await?;

    // Connect with timeout
    async_std::future::timeout(config.connect_timeout, socket.connect(target_addr))
        .await
        .map_err(|_| Error::Timeout)??;

    // Apply socket options
    if let Some(ttl) = config.ttl {
        socket.set_ttl(ttl)?;
    }

    Ok(socket)
}

/// Implement AsyncDatagram for async-std's UdpSocket
impl AsyncDatagram for UdpSocket {
    async fn send(&self, buf: &[u8]) -> Result<usize, Error> {
        Ok(UdpSocket::send(self, buf).await?)
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<usize, Error> {
        Ok(UdpSocket::recv(self, buf).await?)
    }
}
