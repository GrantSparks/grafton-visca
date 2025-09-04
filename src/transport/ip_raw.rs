//! Raw VISCA over IP transport (PtzOptics style).
//!
//! This module provides raw VISCA communication over TCP/UDP without
//! any additional encapsulation. This is the format used by PtzOptics cameras.

use bytes::{Bytes, BytesMut};
use std::{
    borrow::Cow,
    io::{BufReader, Read, Write},
    net::{TcpStream, UdpSocket},
    time::{Duration, Instant},
};
use tracing::{debug, trace};

#[cfg(not(feature = "async"))]
use crate::transport::SyncTransport;
use crate::{
    command::{bytes::VISCA_TERMINATOR, CommandKind},
    error::{Error, Result},
    transport::{
        address::AddressResolver,
        buffer::{BufferConfig, BufferManager},
        retry::RetryExecutor,
        RetryConfig,
    },
};

/// Configuration for raw IP transport.
#[derive(Debug, Clone)]
pub struct RawIpConfig {
    /// Remote address and port.
    pub address: String,
    /// Connection timeout.
    pub connect_timeout: Duration,
    /// Read timeout.
    pub read_timeout: Duration,
    /// Write timeout.
    pub write_timeout: Duration,
    /// Retry configuration for network operations.
    pub retry_config: RetryConfig,
}

impl Default for RawIpConfig {
    fn default() -> Self {
        Self {
            address: "192.168.0.110:5678".to_string(),
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_millis(100),
            write_timeout: Duration::from_millis(100),
            retry_config: RetryConfig::default(),
        }
    }
}

/// Raw TCP transport for blocking I/O.
#[derive(Debug)]
pub struct RawTcpTransport {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    buffer_manager: BufferManager,
    read_buffer: BytesMut,
    retry_executor: RetryExecutor,
}

impl RawTcpTransport {
    /// Connect to a camera via raw TCP.
    pub fn connect(config: RawIpConfig) -> Result<Self> {
        let resolver = AddressResolver::new();
        let addr = resolver
            .resolve_first(&config.address)
            .map_err(|e| Error::TransportError(format!("Invalid address: {e}").into()))?;

        debug!("Connecting to {addr} via raw TCP");

        let stream = TcpStream::connect_timeout(&addr, config.connect_timeout)
            .map_err(|e| Error::TransportError(format!("TCP connect failed: {e}").into()))?;

        stream
            .set_read_timeout(Some(config.read_timeout))
            .map_err(|e| {
                Error::TransportError(format!("Failed to set read timeout: {e}").into())
            })?;

        stream
            .set_write_timeout(Some(config.write_timeout))
            .map_err(|e| {
                Error::TransportError(format!("Failed to set write timeout: {e}").into())
            })?;

        debug!("Connected to {addr}");

        // Clone the stream for reader/writer split
        let writer = stream
            .try_clone()
            .map_err(|e| Error::TransportError(format!("Failed to clone stream: {e}").into()))?;
        let reader = BufReader::new(stream);

        // Create buffer manager with raw IP optimized sizes
        let buffer_manager = BufferManager::new(BufferConfig::for_raw_ip());

        // Create retry executor with the configured retry settings
        let retry_executor = RetryExecutor::new(config.retry_config);

        Ok(Self {
            reader,
            writer,
            buffer_manager,
            read_buffer: buffer_manager.alloc_recv_buffer(),
            retry_executor,
        })
    }

    /// Receive a complete VISCA frame.
    fn recv_frame(&mut self) -> Result<Bytes> {
        let mut temp_buf = self.buffer_manager.alloc_vec_buffer();

        loop {
            // Check if we have a complete frame in the buffer
            if let Some(pos) = self.read_buffer.iter().position(|&b| b == VISCA_TERMINATOR) {
                let frame = self.read_buffer.split_to(pos + 1);
                trace!("Received frame: {frame:02X?}");
                return Ok(frame.freeze());
            }

            // Read more data
            match self.reader.read(&mut temp_buf) {
                Ok(n) if n > 0 => {
                    self.read_buffer.extend_from_slice(&temp_buf[..n]);
                    trace!("Read {n} bytes from TCP");
                }
                Ok(_) => {
                    return Err(Error::ConnectionClosed {
                        reason: Some(Cow::Borrowed("peer closed connection")),
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    return Err(Error::Timeout);
                }
                Err(e) => {
                    return Err(Error::TransportError(format!("TCP read error: {e}").into()));
                }
            }
        }
    }
}

#[cfg(not(feature = "async"))]
impl SyncTransport for RawTcpTransport {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<()> {
        // Clone bytes for the closure
        let bytes_vec = bytes.to_vec();

        // Create a mutable reference to writer for the retry closure
        let writer = &mut self.writer;

        // Use the retry executor for automatic retry handling
        self.retry_executor.execute(|| {
            writer
                .write_all(&bytes_vec)
                .and_then(|_| writer.flush())
                .map_err(|e| Error::TransportError(format!("TCP write error: {e}").into()))?;

            trace!(
                "Sent {len} bytes: {bytes:02X?}",
                len = bytes_vec.len(),
                bytes = bytes_vec
            );
            Ok(())
        })
    }

    fn recv(&mut self) -> Result<Bytes> {
        // Don't retry receive operations to avoid protocol confusion
        self.recv_frame()
    }

    fn recv_with_timeout(&mut self, timeout: Duration) -> Result<Bytes> {
        // We need to temporarily modify the timeout on the stream
        // Since we can't get a mutable reference while recv_frame borrows self mutably,
        // we'll use a different approach: set timeout before recv and restore after

        // Get the stream reference through the writer (which is a clone of the same stream)
        let original_read_timeout = self
            .writer
            .read_timeout()
            .map_err(|e| Error::TransportError(format!("Failed to get timeout: {e}").into()))?;

        self.writer
            .set_read_timeout(Some(timeout))
            .map_err(|e| Error::TransportError(format!("Failed to set timeout: {e}").into()))?;

        let result = self.recv_frame();

        // Restore original timeout
        self.writer
            .set_read_timeout(original_read_timeout)
            .map_err(|e| Error::TransportError(format!("Failed to restore timeout: {e}").into()))?;

        result
    }
}

/// Raw UDP transport for blocking I/O.
#[derive(Debug)]
pub struct RawUdpTransport {
    socket: UdpSocket,
    buffer_manager: BufferManager,
    read_buffer: BytesMut,
    retry_executor: RetryExecutor,
    config: RawIpConfig,
    /// Track last sent command for retry on timeout
    last_command: Option<Vec<u8>>,
}

impl RawUdpTransport {
    /// Connect to a camera via raw UDP.
    pub fn connect(config: RawIpConfig) -> Result<Self> {
        let resolver = AddressResolver::new();
        let addr = resolver
            .resolve_first(&config.address)
            .map_err(|e| Error::TransportError(format!("Invalid address: {e}").into()))?;

        debug!("Connecting to {addr} via raw UDP");

        let bind_addr = resolver.bind_address_for(&addr);
        let socket = UdpSocket::bind(bind_addr)
            .map_err(|e| Error::TransportError(format!("UDP bind failed: {e}").into()))?;

        socket
            .connect(addr)
            .map_err(|e| Error::TransportError(format!("UDP connect failed: {e}").into()))?;

        socket
            .set_read_timeout(Some(config.read_timeout))
            .map_err(|e| {
                Error::TransportError(format!("Failed to set read timeout: {e}").into())
            })?;

        socket
            .set_write_timeout(Some(config.write_timeout))
            .map_err(|e| {
                Error::TransportError(format!("Failed to set write timeout: {e}").into())
            })?;

        debug!("Connected to {addr}");

        // Create buffer manager with UDP optimized sizes
        let buffer_manager = BufferManager::new(BufferConfig::for_udp());

        // Create retry executor with the configured retry settings
        let retry_executor = RetryExecutor::new(config.retry_config);

        Ok(Self {
            socket,
            buffer_manager,
            read_buffer: buffer_manager.alloc_recv_buffer(),
            retry_executor,
            config,
            last_command: None,
        })
    }

    /// Receive a complete VISCA frame.
    fn recv_frame(&mut self) -> Result<Bytes> {
        let mut temp_buf = self.buffer_manager.alloc_vec_buffer();

        loop {
            // Check if we have a complete frame in the buffer
            if let Some(pos) = self.read_buffer.iter().position(|&b| b == VISCA_TERMINATOR) {
                let frame = self.read_buffer.split_to(pos + 1);
                trace!("Received frame: {frame:02X?}");
                return Ok(frame.freeze());
            }

            // Read more data
            match self.socket.recv(&mut temp_buf) {
                Ok(n) if n > 0 => {
                    self.read_buffer.extend_from_slice(&temp_buf[..n]);
                    trace!("Read {n} bytes from UDP");
                }
                Ok(_) => {
                    return Err(Error::Timeout);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    return Err(Error::Timeout);
                }
                Err(e) => {
                    return Err(Error::TransportError(format!("UDP read error: {e}").into()));
                }
            }
        }
    }
}

#[cfg(not(feature = "async"))]
impl SyncTransport for RawUdpTransport {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<()> {
        // Store the command for potential retry on receive timeout
        self.last_command = Some(bytes.to_vec());

        // Clone bytes for the closure
        let bytes_vec = bytes.to_vec();

        // Create a reference to socket for the retry closure
        let socket = &self.socket;

        // Use the retry executor for automatic retry handling
        self.retry_executor.execute(|| {
            socket
                .send(&bytes_vec)
                .map_err(|e| Error::TransportError(format!("UDP send error: {e}").into()))?;

            trace!(
                "Sent {len} bytes: {bytes:02X?}",
                len = bytes_vec.len(),
                bytes = bytes_vec
            );
            Ok(())
        })
    }

    fn recv(&mut self) -> Result<Bytes> {
        let mut attempts = 0;
        let start_time = Instant::now();

        loop {
            match self.recv_frame() {
                Ok(frame) => return Ok(frame),
                Err(Error::Timeout)
                    if self.config.retry_config.should_retry(attempts, start_time) =>
                {
                    // For UDP, timeout might mean packet loss - resend last command
                    if let Some(cmd) = self.last_command.clone() {
                        attempts += 1;
                        let delay = self
                            .config
                            .retry_config
                            .calculate_delay(attempts, Error::Timeout.suggested_retry_delay());

                        if start_time.elapsed() + delay
                            > self.config.retry_config.max_retry_duration
                        {
                            return Err(Error::MaxRetriesExceeded);
                        }

                        debug!("UDP receive timeout, resending command (attempt {attempts})");

                        // Resend the command
                        self.socket.send(&cmd).map_err(|e| {
                            Error::TransportError(format!("UDP resend error: {e}").into())
                        })?;

                        std::thread::sleep(delay);
                    } else {
                        return Err(Error::Timeout);
                    }
                }
                Err(e)
                    if e.is_retryable()
                        && self.config.retry_config.should_retry(attempts, start_time) =>
                {
                    attempts += 1;
                    let delay = self
                        .config
                        .retry_config
                        .calculate_delay(attempts, e.suggested_retry_delay());

                    if start_time.elapsed() + delay > self.config.retry_config.max_retry_duration {
                        return Err(Error::MaxRetriesExceeded);
                    }

                    debug!("Retrying UDP receive (attempt {attempts}): {e:?}");
                    std::thread::sleep(delay);
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn recv_with_timeout(&mut self, timeout: Duration) -> Result<Bytes> {
        // Temporarily set the timeout on the socket
        let original_read_timeout = self
            .socket
            .read_timeout()
            .map_err(|e| Error::TransportError(format!("Failed to get timeout: {e}").into()))?;

        self.socket
            .set_read_timeout(Some(timeout))
            .map_err(|e| Error::TransportError(format!("Failed to set timeout: {e}").into()))?;

        let result = self.recv_frame();

        // Restore original timeout
        self.socket
            .set_read_timeout(original_read_timeout)
            .map_err(|e| Error::TransportError(format!("Failed to restore timeout: {e}").into()))?;

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_ip_config_default() {
        let config = RawIpConfig::default();
        assert_eq!(config.address, "192.168.0.110:5678");
        assert_eq!(config.connect_timeout, Duration::from_secs(5));
        assert_eq!(config.read_timeout, Duration::from_millis(100));
    }
}
