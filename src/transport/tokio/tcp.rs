//! Clean TCP transport implementation for tokio.

use crate::error::Error;
use crate::transport::AsyncTransport;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// TCP transport implementation for tokio.
///
/// This is a clean implementation that works directly with AsyncTransport
/// without requiring adapters or complex trait hierarchies.
#[derive(Debug)]
pub struct TcpTransport {
    stream: std::sync::Arc<tokio::sync::Mutex<TcpStream>>,
    description: String,
}

impl TcpTransport {
    /// Create a new TCP transport by connecting to the given address.
    pub async fn connect(addr: &str) -> Result<Self, Error> {
        let stream = TcpStream::connect(addr).await?;
        Ok(Self {
            stream: std::sync::Arc::new(tokio::sync::Mutex::new(stream)),
            description: format!("TCP connection to {}", addr),
        })
    }

    /// Create a new TCP transport with a connection timeout.
    pub async fn connect_timeout(addr: &str, timeout: Duration) -> Result<Self, Error> {
        let stream = tokio::time::timeout(timeout, TcpStream::connect(addr))
            .await
            .map_err(|_| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Connection timed out",
                ))
            })??;
        Ok(Self {
            stream: std::sync::Arc::new(tokio::sync::Mutex::new(stream)),
            description: format!("TCP connection to {} (timeout: {:?})", addr, timeout),
        })
    }

    /// Get a description of this transport.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Check if the connection is still alive.
    pub fn is_connected(&self) -> bool {
        // For TCP, we assume the connection is alive unless we get an error
        // during I/O operations. This is a simplification.
        true
    }
}

impl AsyncTransport for TcpTransport {
    type SendFuture<'a> =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Error>> + Send + 'a>>;
    type ReceiveFuture<'a> =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;

    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFuture<'a> {
        Box::pin(async move {
            let mut stream = self.stream.lock().await;
            stream.write_all(data).await?;
            stream.flush().await?;
            Ok(())
        })
    }

    fn receive(&self) -> Self::ReceiveFuture<'_> {
        Box::pin(async move {
            let mut stream = self.stream.lock().await;
            let mut buf = vec![0; 1024];
            let n = stream.read(&mut buf).await?;
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
