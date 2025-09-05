//! Sony encapsulated VISCA over IP transport.
//!
//! This module provides VISCA communication using Sony's encapsulation format,
//! which adds an 8-byte header containing sequence numbers for request/response
//! matching and automatic retry on network errors.

use bytes::{Bytes, BytesMut};
use tracing::{debug, trace};

use std::{
    borrow::Cow,
    io::{BufReader, Read, Write},
    net::TcpStream,
    time::Duration,
};

pub use crate::transport::sony_config::SonyIpConfig;
#[cfg(not(feature = "async"))]
use crate::transport::SyncTransport;
use crate::{
    error::{Error, Result},
    protocol::sony::SonyHeader,
    transport::{
        address::AddressResolver,
        buffer::{BufferConfig, BufferManager},
    },
};

/// Sony TCP transport for blocking I/O.
///
/// This transport handles basic TCP I/O and Sony framing. Retry logic and sequence
/// management are handled by the BlockingRunner and SchedulerCore.
#[derive(Debug)]
pub struct SonyTcpTransport {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    buffer_manager: BufferManager,
    read_buffer: BytesMut,
}

impl SonyTcpTransport {
    /// Connect to a camera via Sony encapsulated TCP.
    pub fn connect(config: SonyIpConfig) -> Result<Self> {
        let resolver = AddressResolver::new();
        let addr = resolver
            .resolve_first(&config.address)
            .map_err(|e| Error::TransportError(format!("Invalid address: {e}").into()))?;

        debug!("Connecting to {addr} via Sony TCP");

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

        // Create buffer manager with Sony IP optimized sizes
        let buffer_manager = BufferManager::new(BufferConfig::for_sony_ip());

        Ok(Self {
            reader,
            writer,
            buffer_manager,
            read_buffer: buffer_manager.alloc_recv_buffer(),
        })
    }

    /// Receive raw bytes from TCP, assembling complete Sony frames.
    fn recv_frame(&mut self) -> Result<Bytes> {
        let mut temp_buf = self.buffer_manager.alloc_vec_buffer();

        loop {
            // Check if we have a complete header
            if self.read_buffer.len() >= SonyHeader::SIZE {
                // Parse header to get payload length
                let _payload_type = u16::from_be_bytes([self.read_buffer[0], self.read_buffer[1]]);
                let payload_length =
                    u16::from_be_bytes([self.read_buffer[2], self.read_buffer[3]]) as usize;

                // Check if we have the complete payload
                if self.read_buffer.len() >= SonyHeader::SIZE + payload_length {
                    // Extract complete frame
                    let frame_size = SonyHeader::SIZE + payload_length;
                    let frame_bytes = self.read_buffer.split_to(frame_size);

                    trace!("Received complete Sony frame: {} bytes", frame_size);
                    return Ok(Bytes::copy_from_slice(&frame_bytes));
                }
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
impl SyncTransport for SonyTcpTransport {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: crate::command::CommandKind) -> Result<()> {
        // The bytes should already be framed by the caller (BlockingRunner or Camera)
        // We just send them as-is
        self.writer
            .write_all(bytes)
            .map_err(|e| Error::TransportError(format!("TCP write error: {e}").into()))?;
        self.writer
            .flush()
            .map_err(|e| Error::TransportError(format!("TCP flush error: {e}").into()))?;

        trace!("Sent {} bytes over TCP", bytes.len());
        Ok(())
    }

    fn recv(&mut self) -> Result<Bytes> {
        // Return a complete Sony frame (with header)
        // The caller (BlockingRunner or Camera) will handle extraction
        self.recv_frame()
    }

    fn recv_with_timeout(&mut self, timeout: Duration) -> Result<Bytes> {
        // We need to temporarily modify the timeout on the stream
        // Since we can't get a mutable reference while recv borrows self mutably,
        // we'll use the writer (which is a clone of the same stream)

        let original_read_timeout = self
            .writer
            .read_timeout()
            .map_err(|e| Error::TransportError(format!("Failed to get timeout: {e}").into()))?;

        self.writer
            .set_read_timeout(Some(timeout))
            .map_err(|e| Error::TransportError(format!("Failed to set timeout: {e}").into()))?;

        let result = self.recv();

        // Restore original timeout
        self.writer
            .set_read_timeout(original_read_timeout)
            .map_err(|e| Error::TransportError(format!("Failed to restore timeout: {e}").into()))?;

        result
    }
}

/// Sony UDP transport for blocking I/O.
///
/// This transport handles basic UDP I/O and Sony framing. Retry logic and sequence
/// management are handled by the BlockingRunner and SchedulerCore.
#[derive(Debug)]
pub struct SonyUdpTransport {
    socket: std::net::UdpSocket,
    buffer_manager: BufferManager,
}

impl SonyUdpTransport {
    /// Connect to a camera via Sony encapsulated UDP.
    pub fn connect(config: SonyIpConfig) -> Result<Self> {
        let resolver = AddressResolver::new();
        let addr = resolver
            .resolve_first(&config.address)
            .map_err(|e| Error::TransportError(format!("Invalid address: {e}").into()))?;

        debug!("Connecting to {addr} via Sony UDP");

        let bind_addr = resolver.bind_address_for(&addr);
        let socket = std::net::UdpSocket::bind(bind_addr)
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

        // Create buffer manager with Sony IP optimized sizes
        let buffer_manager = BufferManager::new(BufferConfig::for_sony_ip());

        Ok(Self {
            socket,
            buffer_manager,
        })
    }

    /// Receive raw bytes from UDP, expecting complete Sony frames.
    fn recv_frame(&mut self) -> Result<Bytes> {
        let mut temp_buf = self.buffer_manager.alloc_vec_buffer();

        match self.socket.recv(&mut temp_buf) {
            Ok(n) if n >= SonyHeader::SIZE => {
                // Parse header to get payload length
                let _payload_type = u16::from_be_bytes([temp_buf[0], temp_buf[1]]);
                let payload_length = u16::from_be_bytes([temp_buf[2], temp_buf[3]]) as usize;

                if n >= SonyHeader::SIZE + payload_length {
                    // Return the complete frame
                    let frame_bytes = &temp_buf[..SonyHeader::SIZE + payload_length];
                    trace!("Received complete Sony UDP frame: {} bytes", n);
                    Ok(Bytes::copy_from_slice(frame_bytes))
                } else {
                    Err(Error::InvalidResponse {
                        expected: format!("Sony frame with {payload_length} byte payload").into(),
                        actual: format!(
                            "Only {bytes_received} bytes received",
                            bytes_received = n - SonyHeader::SIZE
                        )
                        .into(),
                    })
                }
            }
            Ok(n) => Err(Error::InvalidResponse {
                expected: "Sony encapsulated frame".into(),
                actual: format!("Short packet ({n} bytes)").into(),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Err(Error::Timeout),
            Err(e) => Err(Error::TransportError(format!("UDP read error: {e}").into())),
        }
    }
}

#[cfg(not(feature = "async"))]
impl SyncTransport for SonyUdpTransport {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: crate::command::CommandKind) -> Result<()> {
        // The bytes should already be framed by the caller (BlockingRunner or Camera)
        // We just send them as-is
        self.socket
            .send(bytes)
            .map_err(|e| Error::TransportError(format!("UDP send error: {e}").into()))?;

        trace!("Sent {} bytes over UDP", bytes.len());
        Ok(())
    }

    fn recv(&mut self) -> Result<Bytes> {
        // Return a complete Sony frame (with header)
        // The caller (BlockingRunner or Camera) will handle extraction
        self.recv_frame()
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

        let result = self.recv();

        // Restore original timeout
        self.socket
            .set_read_timeout(original_read_timeout)
            .map_err(|e| Error::TransportError(format!("Failed to restore timeout: {e}").into()))?;

        result
    }
}

/// Create a Sony transport based on configuration.
#[cfg(not(feature = "async"))]
pub fn create_transport(config: SonyIpConfig) -> Result<Box<dyn SyncTransport>> {
    if config.use_tcp {
        Ok(Box::new(SonyTcpTransport::connect(config)?))
    } else {
        Ok(Box::new(SonyUdpTransport::connect(config)?))
    }
}

// Async implementations

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sony_ip_config_default() {
        let config = SonyIpConfig::default();
        assert_eq!(config.address, "192.168.0.110:52381");
        assert_eq!(config.max_retries, 3);
        assert!(config.use_tcp);
    }

    #[test]
    fn test_sony_header_encoding() {
        use crate::protocol::sony::PayloadType;

        let header = SonyHeader::new_command(5, 42);
        assert_eq!(header.payload_type, PayloadType::ViscaCommand);
        assert_eq!(header.payload_length, 5);
        assert_eq!(header.sequence_number, 42);
    }
}
