//! Simple tokio transport implementations without runtime abstraction overhead.
//!
//! These implementations work directly with tokio types without unnecessary
//! Arc<Mutex<>> wrapping or unsafe code.

use crate::error::Error;
use crate::transport::async_transport::AsyncTransport;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};

/// Simple TCP transport using tokio.
///
/// This implementation works directly with tokio's TcpStream without
/// unnecessary wrapping or complexity.
#[derive(Debug)]
pub struct SimpleTokioTcpTransport {
    stream: TcpStream,
    description: String,
}

impl SimpleTokioTcpTransport {
    /// Create a new TCP transport by connecting to the given address.
    pub async fn connect(addr: &str) -> Result<Self, Error> {
        let stream = TcpStream::connect(addr).await?;
        Ok(Self {
            stream,
            description: format!("TCP connection to {}", addr),
        })
    }

    /// Get a description of this transport.
    pub fn description(&self) -> &str {
        &self.description
    }
}

impl AsyncTransport for SimpleTokioTcpTransport {
    type SendFuture<'a> =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Error>> + Send + 'a>>;
    type ReceiveFuture<'a> =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;

    fn send<'a>(&'a mut self, data: &'a [u8]) -> Self::SendFuture<'a> {
        Box::pin(async move {
            self.stream.write_all(data).await?;
            self.stream.flush().await?;
            Ok(())
        })
    }

    fn receive(&mut self) -> Self::ReceiveFuture<'_> {
        Box::pin(async move {
            let mut buf = vec![0; 1024];
            let n = self.stream.read(&mut buf).await?;
            if n == 0 {
                return Err(Error::Io(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "Connection closed",
                )));
            }
            buf.truncate(n);
            Ok(buf)
        })
    }
}

/// Simple UDP transport using tokio.
///
/// This implementation works directly with tokio's UdpSocket without
/// unnecessary wrapping or complexity.
#[derive(Debug)]
pub struct SimpleTokioUdpTransport {
    socket: UdpSocket,
    remote_addr: SocketAddr,
    description: String,
}

impl SimpleTokioUdpTransport {
    /// Create a new UDP transport.
    ///
    /// Binds to a local address and sets the remote address for sending.
    pub async fn new(local_addr: &str, remote_addr: &str) -> Result<Self, Error> {
        let socket = UdpSocket::bind(local_addr).await?;
        let remote_addr: SocketAddr = remote_addr.parse().map_err(|e| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid remote address: {}", e),
            ))
        })?;

        Ok(Self {
            socket,
            remote_addr,
            description: format!("UDP {} -> {}", local_addr, remote_addr),
        })
    }

    /// Get a description of this transport.
    pub fn description(&self) -> &str {
        &self.description
    }
}

impl AsyncTransport for SimpleTokioUdpTransport {
    type SendFuture<'a> =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Error>> + Send + 'a>>;
    type ReceiveFuture<'a> =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;

    fn send<'a>(&'a mut self, data: &'a [u8]) -> Self::SendFuture<'a> {
        Box::pin(async move {
            self.socket.send_to(data, self.remote_addr).await?;
            Ok(())
        })
    }

    fn receive(&mut self) -> Self::ReceiveFuture<'_> {
        Box::pin(async move {
            let mut buf = vec![0; 1024];
            let (n, _addr) = self.socket.recv_from(&mut buf).await?;
            buf.truncate(n);
            Ok(buf)
        })
    }
}
