//! Unified async I/O helpers for transport implementations.
//!
//! This module provides shared functionality across different runtime transports
//! to reduce code duplication while maintaining zero-cost abstractions.

use std::future::Future;

use crate::{transport::builder::TransportConfig, Error};

/// Trait abstracting async read operations across different runtimes.
///
/// This trait unifies the async read capabilities needed for VISCA communication
/// across tokio, async-std, and smol runtimes. Not all methods will be used by
/// all runtimes, which is expected for a unified interface.
pub trait AsyncReadExt {
    /// Read data into a buffer, returning the number of bytes read.
    ///
    /// Returns 0 when the stream is closed.
    fn read(&mut self, buf: &mut [u8]) -> impl Future<Output = Result<usize, Error>> + Send;
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

/// Trait abstracting async datagram (UDP) operations across different runtimes.
///
/// This trait unifies the async UDP socket capabilities needed for VISCA communication
/// across tokio, async-std, and smol runtimes, enabling zero-cost abstractions
/// through monomorphization.
pub trait AsyncDatagram: Send + Sync {
    /// Send data on the socket to the connected remote address.
    ///
    /// Returns the number of bytes written on success.
    fn send(&self, buf: &[u8]) -> impl Future<Output = Result<usize, Error>> + Send;

    /// Receive data from the socket.
    ///
    /// Returns the number of bytes read on success.
    fn recv(&self, buf: &mut [u8]) -> impl Future<Output = Result<usize, Error>> + Send;
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
    /// Whether to enable TCP_NODELAY (Nagle's algorithm disable)
    pub nodelay: Option<bool>,
    /// Time-to-live for packets
    pub ttl: Option<u32>,
}

impl Default for TcpConnectionConfig {
    fn default() -> Self {
        Self {
            nodelay: Some(true), // Default to low latency
            ttl: None,
        }
    }
}

impl From<TransportConfig> for TcpConnectionConfig {
    fn from(config: TransportConfig) -> Self {
        Self {
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

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
mod tests {
    // The framing is now handled by ProtocolFramer in the runtime loop,
    // and transports only return raw bytes.
}
