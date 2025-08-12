//! Tokio TCP transport implementation with zero-cost async.

use crate::transport::AsyncTransport;
use crate::Error;
use bytes::Bytes;
use std::borrow::Cow;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

/// TCP transport for async VISCA communication using tokio.
///
/// This transport uses native async functions without boxing, providing
/// zero-cost async transport operations.
#[derive(Debug)]
pub struct Tcp {
    reader: Mutex<BufReader<OwnedReadHalf>>,
    writer: Mutex<OwnedWriteHalf>,
}

impl Tcp {
    /// Connect to a TCP endpoint.
    ///
    /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
    pub async fn connect(address: &str) -> Result<Self, Error> {
        Self::connect_timeout(address, Duration::from_secs(5)).await
    }

    /// Connect with a custom timeout.
    ///
    /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
    pub async fn connect_timeout(address: &str, timeout: Duration) -> Result<Self, Error> {
        // TcpStream::connect already handles DNS resolution and IPv6
        let stream = tokio::time::timeout(timeout, TcpStream::connect(address))
            .await
            .map_err(|_| Error::Timeout)??;

        // Set TCP nodelay for low latency
        stream.set_nodelay(true)?;

        // Split into read and write halves for concurrent access
        let (read_half, write_half) = stream.into_split();

        Ok(Self {
            reader: Mutex::new(BufReader::new(read_half)),
            writer: Mutex::new(write_half),
        })
    }
}

impl AsyncTransport for Tcp {
    async fn send(&self, data: &[u8]) -> Result<(), Error> {
        let mut writer = self.writer.lock().await;
        writer.write_all(data).await?;
        writer.flush().await?;
        Ok(())
    }

    async fn recv(&self) -> Result<Bytes, Error> {
        let mut reader = self.reader.lock().await;
        let mut buf = Vec::with_capacity(64);

        // Use buffered read_until to find VISCA terminator (0xFF)
        let n = reader.read_until(0xFF, &mut buf).await?;

        if n == 0 {
            return Err(Error::ConnectionLost {
                reason: Cow::Borrowed("peer closed connection"),
            });
        }

        Ok(Bytes::from(buf))
    }
}
