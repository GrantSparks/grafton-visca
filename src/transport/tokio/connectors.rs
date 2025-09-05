//! Tokio-specific implementations of unified async I/O connectors.

use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpStream, UdpSocket};

use crate::transport::address::AddressResolver;
use crate::transport::async_io::{
    AsyncDatagram, AsyncReadExt as AsyncReadExtTrait, AsyncWriteExt as AsyncWriteExtTrait,
    TcpConnectionConfig, UdpSocketConfig,
};
use crate::Error;

/// Wrapper around tokio's BufReader to implement our AsyncReadExt trait.
#[derive(Debug)]
pub struct TokioBufferedReader<R> {
    inner: BufReader<R>,
}

impl<R: AsyncReadExt + Unpin> TokioBufferedReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            inner: BufReader::new(reader),
        }
    }
}

impl<R: AsyncReadExt + Unpin + Send> AsyncReadExtTrait for TokioBufferedReader<R> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        Ok(self.inner.read(buf).await?)
    }
}

/// Wrapper around tokio streams to implement our AsyncWriteExt trait.
#[derive(Debug)]
pub struct TokioWriter<W> {
    inner: W,
}

impl<W> TokioWriter<W> {
    pub fn new(writer: W) -> Self {
        Self { inner: writer }
    }
}

impl<W: AsyncWriteExt + Unpin + Send> AsyncWriteExtTrait for TokioWriter<W> {
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        Ok(self.inner.write_all(buf).await?)
    }

    async fn flush(&mut self) -> Result<(), Error> {
        Ok(self.inner.flush().await?)
    }
}

/// Combined TCP stream wrapper that implements both read and write traits.
#[derive(Debug)]
pub struct TokioTcpStream {
    pub reader: TokioBufferedReader<tokio::net::tcp::OwnedReadHalf>,
    pub writer: TokioWriter<tokio::net::tcp::OwnedWriteHalf>,
}

impl AsyncReadExtTrait for TokioTcpStream {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        self.reader.read(buf).await
    }
}

impl AsyncWriteExtTrait for TokioTcpStream {
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        self.writer.write_all(buf).await
    }

    async fn flush(&mut self) -> Result<(), Error> {
        self.writer.flush().await
    }
}

/// Create a configured TCP connection using unified helpers.
pub async fn connect_tcp(
    address: &str,
    config: TcpConnectionConfig,
) -> Result<TokioTcpStream, Error> {
    // Connect with timeout
    let stream = tokio::time::timeout(config.connect_timeout, TcpStream::connect(address))
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

    // Split and wrap
    let (read_half, write_half) = stream.into_split();

    Ok(TokioTcpStream {
        reader: TokioBufferedReader::new(read_half),
        writer: TokioWriter::new(write_half),
    })
}

/// Create a configured UDP socket using unified helpers.
pub async fn connect_udp(address: &str, config: UdpSocketConfig) -> Result<UdpSocket, Error> {
    // Use the common address resolver
    let resolver = AddressResolver::new();
    let target_addr = resolver.resolve_first(address)?;

    // Bind to the appropriate unspecified address based on target family
    let bind_addr = resolver.bind_address_for(&target_addr);

    let socket = UdpSocket::bind(bind_addr).await?;
    socket.connect(target_addr).await?;

    // Apply socket configuration
    if let Some(ttl) = config.ttl {
        socket.set_ttl(ttl)?;
    }

    Ok(socket)
}

/// Implement AsyncDatagram for tokio's UdpSocket
impl AsyncDatagram for UdpSocket {
    async fn send(&self, buf: &[u8]) -> Result<usize, Error> {
        Ok(UdpSocket::send(self, buf).await?)
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<usize, Error> {
        Ok(UdpSocket::recv(self, buf).await?)
    }
}

// Serial port adapters are defined in the serial_async module where tokio_serial is available
