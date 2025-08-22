//! Tokio TCP transport implementation with zero-cost async.

use bytes::Bytes;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

use std::borrow::Cow;
use std::time::Duration;

use crate::command::const_encoding::VISCA_TERMINATOR;
use crate::transport::{builder::TransportConfig, AsyncTransport};
use crate::Error;

/// TCP transport for async VISCA communication using tokio.
///
/// This transport uses native async functions without boxing, providing
/// zero-cost async transport operations.
#[derive(Debug)]
pub struct Tcp {
    reader: BufReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
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

        // Split into read and write halves
        let (read_half, write_half) = stream.into_split();

        Ok(Self {
            reader: BufReader::new(read_half),
            writer: write_half,
        })
    }

    /// Connect with a full configuration.
    ///
    /// This method provides full control over connection and socket parameters.
    pub async fn connect_with_config(
        address: &str,
        config: TransportConfig,
    ) -> Result<Self, Error> {
        // TcpStream::connect already handles DNS resolution and IPv6
        let stream = tokio::time::timeout(config.connect_timeout, TcpStream::connect(address))
            .await
            .map_err(|_| Error::Timeout)??;

        // Apply socket options
        if let Some(nodelay) = config.tcp_nodelay {
            stream.set_nodelay(nodelay)?;
        } else {
            // Default to nodelay for low latency
            stream.set_nodelay(true)?;
        }

        if let Some(ttl) = config.ttl {
            stream.set_ttl(ttl)?;
        }

        // Note: keepalive configuration would require platform-specific code

        // Split into read and write halves
        let (read_half, write_half) = stream.into_split();

        Ok(Self {
            reader: BufReader::new(read_half),
            writer: write_half,
        })
    }

    /// Split the TCP transport into separate reader and writer halves.
    ///
    /// This allows for concurrent reading and writing without needing mutable
    /// access to the entire transport. Useful for full-duplex communication patterns.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::runtime_adapters::tokio::TcpTransport as Tcp;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let transport = Tcp::connect("192.168.1.100:5678").await?;
    /// let (mut reader, mut writer) = transport.split();
    ///
    /// // Can now read and write concurrently
    /// tokio::spawn(async move {
    ///     // Use writer in one task
    ///     writer.send(&[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]).await.unwrap();
    /// });
    ///
    /// // Use reader in another task
    /// let response = reader.recv().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn split(self) -> (TcpReader, TcpWriter) {
        let reader = TcpReader {
            reader: self.reader,
        };

        let writer = TcpWriter {
            writer: self.writer,
        };

        (reader, writer)
    }
}

#[allow(clippy::manual_async_fn)]
impl AsyncTransport for Tcp {
    fn send(&mut self, data: &[u8]) -> impl std::future::Future<Output = Result<(), Error>> + Send {
        async move {
            self.writer.write_all(data).await?;
            self.writer.flush().await?;
            Ok(())
        }
    }

    fn recv(&mut self) -> impl std::future::Future<Output = Result<Bytes, Error>> + Send {
        async move {
            let mut buf = Vec::with_capacity(64);

            // Use buffered read_until to find VISCA terminator
            let n = self.reader.read_until(VISCA_TERMINATOR, &mut buf).await?;

            if n == 0 {
                return Err(Error::ConnectionLost {
                    reason: Cow::Borrowed("peer closed connection"),
                });
            }

            Ok(Bytes::from(buf))
        }
    }
}

/// Reader half of a split TCP transport.
///
/// This type allows reading from an async TCP connection that has been split
/// into separate reader and writer halves.
#[derive(Debug)]
pub struct TcpReader {
    reader: BufReader<OwnedReadHalf>,
}

impl TcpReader {
    /// Receive data from the TCP connection.
    pub async fn recv(&mut self) -> Result<Bytes, Error> {
        let mut buf = Vec::with_capacity(64);

        // Use buffered read_until to find VISCA terminator
        let n = self.reader.read_until(VISCA_TERMINATOR, &mut buf).await?;

        if n == 0 {
            return Err(Error::ConnectionLost {
                reason: Cow::Borrowed("peer closed connection"),
            });
        }

        Ok(Bytes::from(buf))
    }
}

/// Writer half of a split TCP transport.
///
/// This type allows writing to an async TCP connection that has been split
/// into separate reader and writer halves.
#[derive(Debug)]
pub struct TcpWriter {
    writer: OwnedWriteHalf,
}

impl TcpWriter {
    /// Send data over the TCP connection.
    pub async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        self.writer.write_all(data).await?;
        self.writer.flush().await?;
        Ok(())
    }
}
