//! Unified async I/O helpers for transport implementations.
//!
//! This module provides shared functionality across different runtime transports
//! to reduce code duplication while maintaining zero-cost abstractions.

use bytes::Bytes;
use std::borrow::Cow;
use std::future::Future;
use std::time::Duration;

use crate::command::const_encoding::VISCA_TERMINATOR;
use crate::transport::builder::TransportConfig;
use crate::Error;

/// Trait abstracting async read operations across different runtimes.
///
/// This trait unifies the async read capabilities needed for VISCA communication
/// across tokio, async-std, and smol runtimes.
pub trait AsyncReadExt {
    /// Read data into a buffer, returning the number of bytes read.
    ///
    /// Returns 0 when the stream is closed.
    fn read(&mut self, buf: &mut [u8]) -> impl Future<Output = Result<usize, Error>> + Send;

    /// Read until a delimiter byte is encountered.
    ///
    /// The delimiter byte is included in the returned data.
    /// Returns the number of bytes read (including delimiter).
    fn read_until(
        &mut self,
        delimiter: u8,
        buf: &mut Vec<u8>,
    ) -> impl Future<Output = Result<usize, Error>> + Send;
}

/// Trait abstracting async write operations across different runtimes.
///
/// This trait unifies the async write capabilities needed for VISCA communication
/// across tokio, async-std, and smol runtimes.
pub trait AsyncWriteExt {
    /// Write all data in the buffer.
    ///
    /// This ensures all bytes are written before returning.
    fn write_all(&mut self, buf: &[u8]) -> impl Future<Output = Result<(), Error>> + Send;

    /// Flush any buffered data to the underlying transport.
    fn flush(&mut self) -> impl Future<Output = Result<(), Error>> + Send;
}

/// Unified helper for reading VISCA frames from async streams.
///
/// This function handles the common pattern of reading data until the VISCA
/// terminator (0xFF) is found, which is used across all transport implementations.
pub async fn read_until_terminator<R: AsyncReadExt>(reader: &mut R) -> Result<Bytes, Error> {
    let mut buf = Vec::with_capacity(64);

    let n = reader.read_until(VISCA_TERMINATOR, &mut buf).await?;

    if n == 0 {
        return Err(Error::ConnectionLost {
            reason: Cow::Borrowed("peer closed connection"),
        });
    }

    Ok(Bytes::from(buf))
}

/// Unified helper for reading VISCA frames with byte-by-byte fallback.
///
/// This function provides a fallback for runtimes that don't have efficient
/// read_until implementations (like smol), reading one byte at a time.
pub async fn read_until_terminator_fallback<R: AsyncReadExt>(
    reader: &mut R,
) -> Result<Bytes, Error> {
    let mut buf = Vec::with_capacity(64);

    loop {
        let mut byte = [0u8; 1];
        let n = reader.read(&mut byte).await?;
        if n == 0 {
            return Err(Error::ConnectionLost {
                reason: Cow::Borrowed("peer closed connection"),
            });
        }
        buf.push(byte[0]);
        if byte[0] == VISCA_TERMINATOR {
            break;
        }
    }

    Ok(Bytes::from(buf))
}

/// Unified helper for writing data with proper flushing.
///
/// This function handles the common pattern of write_all followed by flush
/// that is used across all transport implementations.
pub async fn write_all_flush<W: AsyncWriteExt>(writer: &mut W, data: &[u8]) -> Result<(), Error> {
    writer.write_all(data).await?;
    writer.flush().await?;
    Ok(())
}

/// Configuration for TCP connection behavior.
#[derive(Debug, Clone)]
pub struct TcpConnectionConfig {
    /// Connection timeout duration
    pub connect_timeout: Duration,
    /// Whether to enable TCP_NODELAY (Nagle's algorithm disable)
    pub nodelay: Option<bool>,
    /// Time-to-live for packets
    pub ttl: Option<u32>,
}

impl Default for TcpConnectionConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            nodelay: Some(true), // Default to low latency
            ttl: None,
        }
    }
}

impl From<TransportConfig> for TcpConnectionConfig {
    fn from(config: TransportConfig) -> Self {
        Self {
            connect_timeout: config.connect_timeout,
            nodelay: config.tcp_nodelay,
            ttl: config.ttl,
        }
    }
}

/// Configuration for UDP socket behavior.
#[derive(Debug, Clone, Default)]
pub struct UdpSocketConfig {
    /// Time-to-live for packets
    pub ttl: Option<u32>,
}

impl From<TransportConfig> for UdpSocketConfig {
    fn from(config: TransportConfig) -> Self {
        Self { ttl: config.ttl }
    }
}
