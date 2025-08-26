//! Sony encapsulated VISCA over IP transport.
//!
//! This module provides VISCA communication using Sony's encapsulation format,
//! which adds an 8-byte header containing sequence numbers for request/response
//! matching and automatic retry on network errors.

use bytes::{Bytes, BytesMut};
use tracing::{debug, error, trace, warn};

use std::{
    collections::HashMap,
    io::{BufReader, Read, Write},
    net::TcpStream,
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, Instant},
};

#[cfg(not(feature = "async"))]
use crate::protocol::encode::PayloadType;
#[cfg(not(feature = "async"))]
use crate::transport::BlockingTransport;
use crate::{
    error::{Error, Result},
    protocol::encode::SonyHeader,
    transport::{
        address::AddressResolver,
        buffer::{BufferConfig, BufferManager},
    },
};

pub use crate::transport::sony_config::SonyIpConfig;

/// Pending command information for retry handling.
#[derive(Debug, Clone)]
struct PendingCommand {
    /// Original command bytes (without header).
    bytes: Vec<u8>,
    /// Number of retries attempted.
    retries: u32,
    /// Timestamp when sent.
    sent_at: Instant,
}

/// Sony TCP transport for blocking I/O.
#[derive(Debug)]
pub struct SonyTcpTransport {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    sequence: AtomicU32,
    pending: HashMap<u32, PendingCommand>,
    buffer_manager: BufferManager,
    read_buffer: BytesMut,
    config: SonyIpConfig,
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
            sequence: AtomicU32::new(1),
            pending: HashMap::new(),
            buffer_manager,
            read_buffer: buffer_manager.alloc_recv_buffer(),
            config,
        })
    }

    /// Send a command with Sony header.
    fn send_with_header(&mut self, bytes: &[u8], sequence: u32) -> Result<()> {
        // Check if this is an inquiry (second byte is 0x09)
        let is_inquiry = bytes.len() >= 2 && bytes[1] == 0x09;
        let header = if is_inquiry {
            SonyHeader::new_inquiry(bytes.len(), sequence)
        } else {
            SonyHeader::new_command(bytes.len(), sequence)
        };
        let mut packet = self.buffer_manager.alloc_send_buffer();

        // Use the header's encode method for consistency
        packet.extend_from_slice(&header.encode());

        // Add payload
        packet.extend_from_slice(bytes);

        // Send packet
        self.writer
            .write_all(&packet)
            .map_err(|e| Error::TransportError(format!("TCP write error: {e}").into()))?;
        self.writer
            .flush()
            .map_err(|e| Error::TransportError(format!("TCP flush error: {e}").into()))?;

        trace!(
            "Sent packet with seq {}: header={:02X?} payload={:02X?}",
            sequence,
            &packet[..SonyHeader::SIZE],
            bytes
        );

        Ok(())
    }

    /// Receive a Sony encapsulated frame.
    fn recv_sony_frame(&mut self) -> Result<(SonyHeader, Bytes)> {
        let mut temp_buf = self.buffer_manager.alloc_vec_buffer();

        loop {
            // Check if we have a complete header
            if self.read_buffer.len() >= SonyHeader::SIZE {
                // Parse header
                let _payload_type = u16::from_be_bytes([self.read_buffer[0], self.read_buffer[1]]);
                let payload_length =
                    u16::from_be_bytes([self.read_buffer[2], self.read_buffer[3]]) as usize;
                let _sequence_number = u32::from_be_bytes([
                    self.read_buffer[4],
                    self.read_buffer[5],
                    self.read_buffer[6],
                    self.read_buffer[7],
                ]);

                // Check if we have the complete payload
                if self.read_buffer.len() >= SonyHeader::SIZE + payload_length {
                    // Extract frame - use decode method for consistency
                    let header_bytes = self.read_buffer.split_to(SonyHeader::SIZE);
                    let payload = self.read_buffer.split_to(payload_length);

                    let header = match SonyHeader::decode(&header_bytes) {
                        Some(h) => h,
                        None => {
                            warn!("Failed to decode Sony header: {:02X?}", header_bytes);
                            return Err(Error::InvalidResponse {
                                expected: "Valid Sony header".into(),
                                actual: format!("Invalid header bytes: {:02X?}", header_bytes)
                                    .into(),
                            });
                        }
                    };

                    trace!(
                        "Received Sony frame: seq={} type={:?} len={} payload={:02X?}",
                        header.sequence_number,
                        header.payload_type,
                        header.payload_length,
                        payload
                    );

                    return Ok((header, payload.freeze()));
                }
            }

            // Read more data
            match self.reader.read(&mut temp_buf) {
                Ok(n) if n > 0 => {
                    self.read_buffer.extend_from_slice(&temp_buf[..n]);
                    trace!("Read {n} bytes from TCP");
                }
                Ok(_) => {
                    return Err(Error::ConnectionClosed);
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

    /// Handle retries for a command.
    fn handle_retry(&mut self, old_sequence: u32) -> Result<()> {
        if let Some(mut cmd) = self.pending.remove(&old_sequence) {
            if cmd.retries < self.config.max_retries {
                cmd.retries += 1;
                let retry_count = cmd.retries;
                cmd.sent_at = Instant::now();
                let bytes = cmd.bytes.clone();

                // For UDP retries, allocate a new sequence number as per VISCA spec
                let new_sequence = self.sequence.fetch_add(1, Ordering::SeqCst);

                // Insert command with new sequence
                self.pending.insert(new_sequence, cmd);

                warn!(
                    "Retrying command (old seq: {old_sequence}, new seq: {new_sequence}, attempt {retry_count})"
                );
                self.send_with_header(&bytes, new_sequence)?;
            } else {
                error!("Max retries exceeded for seq {old_sequence}");
                return Err(Error::MaxRetriesExceeded);
            }
        }

        Ok(())
    }

    /// Clean up old pending commands.
    fn cleanup_pending(&mut self) {
        let now = Instant::now();
        let timeout = self.config.response_timeout;

        self.pending.retain(|seq, cmd| {
            if now.duration_since(cmd.sent_at) > timeout {
                warn!("Command seq {seq} timed out");
                false
            } else {
                true
            }
        });
    }
}

#[cfg(not(feature = "async"))]
impl BlockingTransport for SonyTcpTransport {
    fn send_blocking(&mut self, bytes: &[u8]) -> Result<()> {
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst);

        // Store pending command for potential retry
        self.pending.insert(
            sequence,
            PendingCommand {
                bytes: bytes.to_vec(),
                retries: 0,
                sent_at: Instant::now(),
            },
        );

        // Send with header
        self.send_with_header(bytes, sequence)?;

        // Clean up old pending commands
        self.cleanup_pending();

        Ok(())
    }

    fn recv_blocking(&mut self) -> Result<Bytes> {
        loop {
            match self.recv_sony_frame() {
                Ok((header, payload)) => {
                    // Only accept replies that match a pending command
                    if header.payload_type == PayloadType::ViscaReply
                        && self.pending.remove(&header.sequence_number).is_none()
                    {
                        // Late or duplicate reply - discard it
                        warn!(
                            "TCP: Discarding late/duplicate reply with seq {} (not in pending)",
                            header.sequence_number
                        );
                        continue; // Keep waiting for a valid response
                    }

                    return Ok(payload);
                }
                Err(e) => {
                    // Check if we should retry any pending commands
                    if matches!(e, Error::Timeout) {
                        // Check for timed out commands
                        let now = Instant::now();
                        let timed_out: Vec<u32> = self
                            .pending
                            .iter()
                            .filter(|(_, cmd)| {
                                now.duration_since(cmd.sent_at) > self.config.response_timeout
                            })
                            .map(|(seq, _)| *seq)
                            .collect();

                        for seq in timed_out {
                            self.handle_retry(seq)?;
                        }
                    }

                    return Err(e);
                }
            }
        }
    }

    fn recv_blocking_with_timeout(&mut self, timeout: Duration) -> Result<Bytes> {
        // We need to temporarily modify the timeout on the stream
        // Since we can't get a mutable reference while recv_blocking borrows self mutably,
        // we'll use the writer (which is a clone of the same stream)

        let original_read_timeout = self
            .writer
            .read_timeout()
            .map_err(|e| Error::TransportError(format!("Failed to get timeout: {e}").into()))?;

        self.writer
            .set_read_timeout(Some(timeout))
            .map_err(|e| Error::TransportError(format!("Failed to set timeout: {e}").into()))?;

        let result = self.recv_blocking();

        // Restore original timeout
        self.writer
            .set_read_timeout(original_read_timeout)
            .map_err(|e| Error::TransportError(format!("Failed to restore timeout: {e}").into()))?;

        result
    }
}

/// Sony UDP transport for blocking I/O.
#[derive(Debug)]
pub struct SonyUdpTransport {
    socket: std::net::UdpSocket,
    sequence: AtomicU32,
    pending: HashMap<u32, PendingCommand>,
    buffer_manager: BufferManager,
    config: SonyIpConfig,
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
            sequence: AtomicU32::new(1),
            pending: HashMap::new(),
            buffer_manager,
            config,
        })
    }

    /// Send a command with Sony header.
    fn send_with_header(&mut self, bytes: &[u8], sequence: u32) -> Result<()> {
        // Check if this is an inquiry (second byte is 0x09)
        let is_inquiry = bytes.len() >= 2 && bytes[1] == 0x09;
        let header = if is_inquiry {
            SonyHeader::new_inquiry(bytes.len(), sequence)
        } else {
            SonyHeader::new_command(bytes.len(), sequence)
        };
        let mut packet = self.buffer_manager.alloc_send_buffer();

        // Use the header's encode method for consistency
        packet.extend_from_slice(&header.encode());

        // Add payload
        packet.extend_from_slice(bytes);

        // Send packet
        self.socket
            .send(&packet)
            .map_err(|e| Error::TransportError(format!("UDP send error: {e}").into()))?;

        trace!("Sent UDP packet with seq {sequence}: {packet:02X?}");

        Ok(())
    }

    /// Receive a Sony encapsulated frame.
    fn recv_sony_frame(&mut self) -> Result<(SonyHeader, Bytes)> {
        let mut temp_buf = self.buffer_manager.alloc_vec_buffer();

        match self.socket.recv(&mut temp_buf) {
            Ok(n) if n >= SonyHeader::SIZE => {
                // Parse header
                let _payload_type = u16::from_be_bytes([temp_buf[0], temp_buf[1]]);
                let payload_length = u16::from_be_bytes([temp_buf[2], temp_buf[3]]) as usize;
                let _sequence_number =
                    u32::from_be_bytes([temp_buf[4], temp_buf[5], temp_buf[6], temp_buf[7]]);

                if n >= SonyHeader::SIZE + payload_length {
                    // Use decode method for consistency
                    let header = match SonyHeader::decode(&temp_buf[..SonyHeader::SIZE]) {
                        Some(h) => h,
                        None => {
                            warn!(
                                "Failed to decode Sony header from UDP: {:02X?}",
                                &temp_buf[..SonyHeader::SIZE]
                            );
                            return Err(Error::InvalidResponse {
                                expected: "Valid Sony header".into(),
                                actual: "Invalid header bytes".to_string().into(),
                            });
                        }
                    };

                    let payload = Bytes::copy_from_slice(
                        &temp_buf[SonyHeader::SIZE..SonyHeader::SIZE + payload_length],
                    );

                    trace!(
                        "Received UDP frame: seq={} type={:?} payload={:02X?}",
                        header.sequence_number,
                        header.payload_type,
                        payload
                    );

                    Ok((header, payload))
                } else {
                    Err(Error::InvalidResponse {
                        expected: format!("Sony frame with {payload_length} byte payload").into(),
                        actual: format!("Only {} bytes received", n - SonyHeader::SIZE).into(),
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
impl BlockingTransport for SonyUdpTransport {
    fn send_blocking(&mut self, bytes: &[u8]) -> Result<()> {
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst);

        // Store pending command for potential retry
        self.pending.insert(
            sequence,
            PendingCommand {
                bytes: bytes.to_vec(),
                retries: 0,
                sent_at: Instant::now(),
            },
        );

        // Send with header
        self.send_with_header(bytes, sequence)?;

        Ok(())
    }

    fn recv_blocking(&mut self) -> Result<Bytes> {
        loop {
            match self.recv_sony_frame() {
                Ok((header, payload)) => {
                    // Only accept replies that match a pending command
                    if header.payload_type == PayloadType::ViscaReply
                        && self.pending.remove(&header.sequence_number).is_none()
                    {
                        // Late or duplicate reply - discard it
                        warn!(
                            "UDP: Discarding late/duplicate reply with seq {} (not in pending)",
                            header.sequence_number
                        );
                        continue; // Keep waiting for a valid response
                    }

                    return Ok(payload);
                }
                Err(Error::Timeout) => {
                    // Implement retry logic for UDP
                    let now = Instant::now();

                    // Find commands that need retry
                    let to_retry: Vec<(u32, PendingCommand)> = self
                        .pending
                        .iter()
                        .filter(|(_, cmd)| {
                            now.duration_since(cmd.sent_at) > Duration::from_millis(500)
                                && cmd.retries < self.config.max_retries
                        })
                        .map(|(seq, cmd)| (*seq, cmd.clone()))
                        .collect();

                    // Retry commands with new sequence numbers (per Sony spec)
                    for (old_seq, mut cmd) in to_retry {
                        cmd.retries += 1;

                        // Allocate new sequence number for retry (Sony requirement)
                        let new_seq = self.sequence.fetch_add(1, Ordering::SeqCst);

                        warn!(
                            "Retrying UDP command (old seq {old_seq}, new seq {new_seq}, attempt {})",
                            cmd.retries
                        );

                        self.send_with_header(&cmd.bytes, new_seq)?;

                        // Remove old sequence entry
                        self.pending.remove(&old_seq);
                        // Insert with new sequence
                        cmd.sent_at = Instant::now();
                        self.pending.insert(new_seq, cmd);
                    }

                    // Continue waiting for response
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn recv_blocking_with_timeout(&mut self, timeout: Duration) -> Result<Bytes> {
        // Temporarily set the timeout on the socket
        let original_read_timeout = self
            .socket
            .read_timeout()
            .map_err(|e| Error::TransportError(format!("Failed to get timeout: {e}").into()))?;
        self.socket
            .set_read_timeout(Some(timeout))
            .map_err(|e| Error::TransportError(format!("Failed to set timeout: {e}").into()))?;

        let result = self.recv_blocking();

        // Restore original timeout
        self.socket
            .set_read_timeout(original_read_timeout)
            .map_err(|e| Error::TransportError(format!("Failed to restore timeout: {e}").into()))?;

        result
    }
}

/// Create a Sony transport based on configuration.
#[cfg(not(feature = "async"))]
pub fn create_transport(config: SonyIpConfig) -> Result<Box<dyn BlockingTransport>> {
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
        let header = SonyHeader::new_command(5, 42);
        assert_eq!(header.payload_type, PayloadType::ViscaCommand);
        assert_eq!(header.payload_length, 5);
        assert_eq!(header.sequence_number, 42);
    }
}
