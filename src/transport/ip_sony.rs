//! Sony encapsulated VISCA over IP transport.
//!
//! This module provides VISCA communication using Sony's encapsulation format,
//! which adds an 8-byte header containing sequence numbers for request/response
//! matching and automatic retry on network errors.

use bytes::{BufMut, Bytes, BytesMut};
use log::{debug, error, trace, warn};
use std::collections::HashMap;
use std::net::ToSocketAddrs;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::error::{Error, Result};
use crate::protocol::encode::{PayloadType, SonyHeader};
use crate::transport::{async_transport::AsyncTransport, BlockingTransport};

/// Configuration for Sony encapsulated IP transport.
#[derive(Debug, Clone)]
pub struct SonyIpConfig {
    /// Remote address and port.
    pub address: String,
    /// Connection timeout.
    pub connect_timeout: Duration,
    /// Read timeout.
    pub read_timeout: Duration,
    /// Write timeout.
    pub write_timeout: Duration,
    /// Maximum retries for failed commands.
    pub max_retries: u32,
    /// Timeout for waiting for a response.
    pub response_timeout: Duration,
    /// Whether to use TCP (true) or UDP (false).
    pub use_tcp: bool,
}

impl Default for SonyIpConfig {
    fn default() -> Self {
        Self {
            address: "192.168.0.110:52381".to_string(), // Sony default port
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_millis(100),
            write_timeout: Duration::from_millis(100),
            max_retries: 3,
            response_timeout: Duration::from_secs(2),
            use_tcp: true,
        }
    }
}

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
    stream: Arc<Mutex<std::net::TcpStream>>,
    sequence: Arc<AtomicU32>,
    pending: Arc<Mutex<HashMap<u32, PendingCommand>>>,
    read_buffer: Arc<Mutex<BytesMut>>,
    config: SonyIpConfig,
}

impl SonyTcpTransport {
    /// Connect to a camera via Sony encapsulated TCP.
    pub fn connect(config: SonyIpConfig) -> Result<Self> {
        let addr = config
            .address
            .to_socket_addrs()
            .map_err(|e| Error::TransportError(format!("Invalid address: {}", e).into()))?
            .next()
            .ok_or_else(|| Error::TransportError("No valid address".into()))?;

        debug!("Connecting to {} via Sony TCP", addr);

        let stream = std::net::TcpStream::connect_timeout(&addr, config.connect_timeout)
            .map_err(|e| Error::TransportError(format!("TCP connect failed: {}", e).into()))?;

        stream
            .set_read_timeout(Some(config.read_timeout))
            .map_err(|e| {
                Error::TransportError(format!("Failed to set read timeout: {}", e).into())
            })?;

        stream
            .set_write_timeout(Some(config.write_timeout))
            .map_err(|e| {
                Error::TransportError(format!("Failed to set write timeout: {}", e).into())
            })?;

        debug!("Connected to {}", addr);

        Ok(Self {
            stream: Arc::new(Mutex::new(stream)),
            sequence: Arc::new(AtomicU32::new(1)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            read_buffer: Arc::new(Mutex::new(BytesMut::with_capacity(512))),
            config,
        })
    }

    /// Send a command with Sony header.
    fn send_with_header(&self, bytes: &[u8], sequence: u32) -> Result<()> {
        use std::io::Write;

        let header = SonyHeader::new_command(bytes.len(), sequence);
        let mut packet = BytesMut::with_capacity(SonyHeader::SIZE + bytes.len());

        // Encode header
        packet.put_u16(header.payload_type as u16);
        packet.put_u16(header.payload_length);
        packet.put_u32(header.sequence_number);

        // Add payload
        packet.extend_from_slice(bytes);

        // Send packet
        let mut stream = self.stream.lock().unwrap();
        stream
            .write_all(&packet)
            .map_err(|e| Error::TransportError(format!("TCP write error: {}", e).into()))?;
        stream
            .flush()
            .map_err(|e| Error::TransportError(format!("TCP flush error: {}", e).into()))?;

        trace!(
            "Sent packet with seq {}: header={:02X?} payload={:02X?}",
            sequence,
            &packet[..SonyHeader::SIZE],
            bytes
        );

        Ok(())
    }

    /// Receive a Sony encapsulated frame.
    fn recv_sony_frame(&self) -> Result<(SonyHeader, Bytes)> {
        use std::io::Read;

        let mut buffer = self.read_buffer.lock().unwrap();
        let mut stream = self.stream.lock().unwrap();
        let mut temp_buf = [0u8; 512];

        loop {
            // Check if we have a complete header
            if buffer.len() >= SonyHeader::SIZE {
                // Parse header
                let payload_type = u16::from_be_bytes([buffer[0], buffer[1]]);
                let payload_length = u16::from_be_bytes([buffer[2], buffer[3]]) as usize;
                let sequence_number =
                    u32::from_be_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);

                // Check if we have the complete payload
                if buffer.len() >= SonyHeader::SIZE + payload_length {
                    // Extract frame - skip header and take payload
                    let _ = buffer.split_to(SonyHeader::SIZE);
                    let payload = buffer.split_to(payload_length);

                    let header = SonyHeader {
                        payload_type: match payload_type {
                            0x01 => PayloadType::ViscaCommand,
                            0x11 => PayloadType::ViscaReply,
                            0x02 => PayloadType::ViscaDeviceSetting,
                            0x20 => PayloadType::ControlCommand,
                            0x21 => PayloadType::ControlReply,
                            _ => {
                                warn!("Unknown payload type: 0x{:04X}", payload_type);
                                PayloadType::ViscaReply
                            }
                        },
                        payload_length: payload_length as u16,
                        sequence_number,
                    };

                    trace!(
                        "Received Sony frame: seq={} type={:?} len={} payload={:02X?}",
                        sequence_number,
                        header.payload_type,
                        payload_length,
                        payload
                    );

                    return Ok((header, payload.freeze()));
                }
            }

            // Read more data
            match stream.read(&mut temp_buf) {
                Ok(n) if n > 0 => {
                    buffer.extend_from_slice(&temp_buf[..n]);
                    trace!("Read {} bytes from TCP", n);
                }
                Ok(_) => {
                    return Err(Error::ConnectionClosed);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    return Err(Error::Timeout);
                }
                Err(e) => {
                    return Err(Error::TransportError(
                        format!("TCP read error: {}", e).into(),
                    ));
                }
            }
        }
    }

    /// Handle retries for a command.
    fn handle_retry(&self, sequence: u32) -> Result<()> {
        let mut pending = self.pending.lock().unwrap();

        if let Some(cmd) = pending.get_mut(&sequence) {
            if cmd.retries < self.config.max_retries {
                cmd.retries += 1;
                let retry_count = cmd.retries;
                cmd.sent_at = Instant::now();
                let bytes = cmd.bytes.clone();
                drop(pending); // Release lock before sending

                warn!(
                    "Retrying command seq {} (attempt {})",
                    sequence, retry_count
                );
                self.send_with_header(&bytes, sequence)?;
            } else {
                error!("Max retries exceeded for seq {}", sequence);
                pending.remove(&sequence);
                return Err(Error::MaxRetriesExceeded);
            }
        }

        Ok(())
    }

    /// Clean up old pending commands.
    fn cleanup_pending(&self) {
        let mut pending = self.pending.lock().unwrap();
        let now = Instant::now();
        let timeout = self.config.response_timeout;

        pending.retain(|seq, cmd| {
            if now.duration_since(cmd.sent_at) > timeout {
                warn!("Command seq {} timed out", seq);
                false
            } else {
                true
            }
        });
    }
}

impl BlockingTransport for SonyTcpTransport {
    fn send_blocking(&self, bytes: &[u8]) -> Result<()> {
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst);

        // Store pending command for potential retry
        {
            let mut pending = self.pending.lock().unwrap();
            pending.insert(
                sequence,
                PendingCommand {
                    bytes: bytes.to_vec(),
                    retries: 0,
                    sent_at: Instant::now(),
                },
            );
        }

        // Send with header
        self.send_with_header(bytes, sequence)?;

        // Clean up old pending commands
        self.cleanup_pending();

        Ok(())
    }

    fn recv_blocking(&self) -> Result<Bytes> {
        loop {
            match self.recv_sony_frame() {
                Ok((header, payload)) => {
                    // Remove from pending if this is a response
                    if header.payload_type == PayloadType::ViscaReply {
                        let mut pending = self.pending.lock().unwrap();
                        pending.remove(&header.sequence_number);
                    }

                    return Ok(payload);
                }
                Err(e) => {
                    // Check if we should retry any pending commands
                    if matches!(e, Error::Timeout) {
                        // Check for timed out commands
                        let pending = self.pending.lock().unwrap();
                        let now = Instant::now();
                        let timed_out: Vec<u32> = pending
                            .iter()
                            .filter(|(_, cmd)| {
                                now.duration_since(cmd.sent_at) > self.config.response_timeout
                            })
                            .map(|(seq, _)| *seq)
                            .collect();
                        drop(pending);

                        for seq in timed_out {
                            self.handle_retry(seq)?;
                        }
                    }

                    return Err(e);
                }
            }
        }
    }

    fn recv_blocking_with_timeout(&self, timeout: Duration) -> Result<Bytes> {
        // Temporarily set the timeout on the stream
        let stream = self.stream.lock().unwrap();
        let original_read_timeout = stream
            .read_timeout()
            .map_err(|e| Error::TransportError(format!("Failed to get timeout: {}", e).into()))?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|e| Error::TransportError(format!("Failed to set timeout: {}", e).into()))?;
        drop(stream);

        let result = self.recv_blocking();

        // Restore original timeout
        let stream = self.stream.lock().unwrap();
        stream
            .set_read_timeout(original_read_timeout)
            .map_err(|e| {
                Error::TransportError(format!("Failed to restore timeout: {}", e).into())
            })?;

        result
    }
}

/// Sony UDP transport for blocking I/O.
#[derive(Debug)]
pub struct SonyUdpTransport {
    socket: Arc<std::net::UdpSocket>,
    sequence: Arc<AtomicU32>,
    pending: Arc<Mutex<HashMap<u32, PendingCommand>>>,
    config: SonyIpConfig,
}

impl SonyUdpTransport {
    /// Connect to a camera via Sony encapsulated UDP.
    pub fn connect(config: SonyIpConfig) -> Result<Self> {
        let addr = config
            .address
            .to_socket_addrs()
            .map_err(|e| Error::TransportError(format!("Invalid address: {}", e).into()))?
            .next()
            .ok_or_else(|| Error::TransportError("No valid address".into()))?;

        debug!("Connecting to {} via Sony UDP", addr);

        let socket = std::net::UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| Error::TransportError(format!("UDP bind failed: {}", e).into()))?;

        socket
            .connect(&addr)
            .map_err(|e| Error::TransportError(format!("UDP connect failed: {}", e).into()))?;

        socket
            .set_read_timeout(Some(config.read_timeout))
            .map_err(|e| {
                Error::TransportError(format!("Failed to set read timeout: {}", e).into())
            })?;

        socket
            .set_write_timeout(Some(config.write_timeout))
            .map_err(|e| {
                Error::TransportError(format!("Failed to set write timeout: {}", e).into())
            })?;

        debug!("Connected to {}", addr);

        Ok(Self {
            socket: Arc::new(socket),
            sequence: Arc::new(AtomicU32::new(1)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            config,
        })
    }

    /// Send a command with Sony header.
    fn send_with_header(&self, bytes: &[u8], sequence: u32) -> Result<()> {
        let header = SonyHeader::new_command(bytes.len(), sequence);
        let mut packet = BytesMut::with_capacity(SonyHeader::SIZE + bytes.len());

        // Encode header
        packet.put_u16(header.payload_type as u16);
        packet.put_u16(header.payload_length);
        packet.put_u32(header.sequence_number);

        // Add payload
        packet.extend_from_slice(bytes);

        // Send packet
        self.socket
            .send(&packet)
            .map_err(|e| Error::TransportError(format!("UDP send error: {}", e).into()))?;

        trace!("Sent UDP packet with seq {}: {:02X?}", sequence, packet);

        Ok(())
    }

    /// Receive a Sony encapsulated frame.
    fn recv_sony_frame(&self) -> Result<(SonyHeader, Bytes)> {
        let mut temp_buf = [0u8; 1500]; // UDP MTU

        match self.socket.recv(&mut temp_buf) {
            Ok(n) if n >= SonyHeader::SIZE => {
                // Parse header
                let payload_type = u16::from_be_bytes([temp_buf[0], temp_buf[1]]);
                let payload_length = u16::from_be_bytes([temp_buf[2], temp_buf[3]]) as usize;
                let sequence_number =
                    u32::from_be_bytes([temp_buf[4], temp_buf[5], temp_buf[6], temp_buf[7]]);

                if n >= SonyHeader::SIZE + payload_length {
                    let header = SonyHeader {
                        payload_type: match payload_type {
                            0x01 => PayloadType::ViscaCommand,
                            0x11 => PayloadType::ViscaReply,
                            0x02 => PayloadType::ViscaDeviceSetting,
                            0x20 => PayloadType::ControlCommand,
                            0x21 => PayloadType::ControlReply,
                            _ => {
                                warn!("Unknown payload type: 0x{:04X}", payload_type);
                                PayloadType::ViscaReply
                            }
                        },
                        payload_length: payload_length as u16,
                        sequence_number,
                    };

                    let payload = Bytes::copy_from_slice(
                        &temp_buf[SonyHeader::SIZE..SonyHeader::SIZE + payload_length],
                    );

                    trace!(
                        "Received UDP frame: seq={} type={:?} payload={:02X?}",
                        sequence_number,
                        header.payload_type,
                        payload
                    );

                    return Ok((header, payload));
                } else {
                    return Err(Error::InvalidResponse {
                        expected: format!("Sony frame with {} byte payload", payload_length).into(),
                        actual: format!("Only {} bytes received", n - SonyHeader::SIZE).into(),
                    });
                }
            }
            Ok(n) => {
                return Err(Error::InvalidResponse {
                    expected: "Sony encapsulated frame".into(),
                    actual: format!("Short packet ({} bytes)", n).into(),
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                return Err(Error::Timeout);
            }
            Err(e) => {
                return Err(Error::TransportError(
                    format!("UDP read error: {}", e).into(),
                ));
            }
        }
    }
}

impl BlockingTransport for SonyUdpTransport {
    fn send_blocking(&self, bytes: &[u8]) -> Result<()> {
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst);

        // Store pending command for potential retry
        {
            let mut pending = self.pending.lock().unwrap();
            pending.insert(
                sequence,
                PendingCommand {
                    bytes: bytes.to_vec(),
                    retries: 0,
                    sent_at: Instant::now(),
                },
            );
        }

        // Send with header
        self.send_with_header(bytes, sequence)?;

        Ok(())
    }

    fn recv_blocking(&self) -> Result<Bytes> {
        loop {
            match self.recv_sony_frame() {
                Ok((header, payload)) => {
                    // Remove from pending if this is a response
                    if header.payload_type == PayloadType::ViscaReply {
                        let mut pending = self.pending.lock().unwrap();
                        pending.remove(&header.sequence_number);
                    }

                    return Ok(payload);
                }
                Err(Error::Timeout) => {
                    // Implement retry logic for UDP
                    let pending = self.pending.lock().unwrap();
                    let now = Instant::now();

                    // Find commands that need retry
                    let to_retry: Vec<(u32, PendingCommand)> = pending
                        .iter()
                        .filter(|(_, cmd)| {
                            now.duration_since(cmd.sent_at) > Duration::from_millis(500)
                                && cmd.retries < self.config.max_retries
                        })
                        .map(|(seq, cmd)| (*seq, cmd.clone()))
                        .collect();
                    drop(pending);

                    // Retry commands
                    for (seq, mut cmd) in to_retry {
                        cmd.retries += 1;
                        warn!("Retrying UDP command seq {} (attempt {})", seq, cmd.retries);
                        self.send_with_header(&cmd.bytes, seq)?;

                        let mut pending = self.pending.lock().unwrap();
                        cmd.sent_at = Instant::now();
                        pending.insert(seq, cmd);
                    }

                    // Continue waiting for response
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn recv_blocking_with_timeout(&self, timeout: Duration) -> Result<Bytes> {
        // Temporarily set the timeout on the socket
        let original_read_timeout = self
            .socket
            .read_timeout()
            .map_err(|e| Error::TransportError(format!("Failed to get timeout: {}", e).into()))?;
        self.socket
            .set_read_timeout(Some(timeout))
            .map_err(|e| Error::TransportError(format!("Failed to set timeout: {}", e).into()))?;

        let result = self.recv_blocking();

        // Restore original timeout
        self.socket
            .set_read_timeout(original_read_timeout)
            .map_err(|e| {
                Error::TransportError(format!("Failed to restore timeout: {}", e).into())
            })?;

        result
    }
}

/// Create a Sony transport based on configuration.
pub fn create_transport(config: SonyIpConfig) -> Result<Box<dyn BlockingTransport>> {
    if config.use_tcp {
        Ok(Box::new(SonyTcpTransport::connect(config)?))
    } else {
        Ok(Box::new(SonyUdpTransport::connect(config)?))
    }
}

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

// Async implementations

/// Async Sony TCP transport.
#[cfg(feature = "rt-tokio")]
#[derive(Clone, Debug)]
pub struct AsyncSonyTcpTransport {
    inner: Arc<SonyTcpTransport>,
}

#[cfg(feature = "rt-tokio")]
impl AsyncSonyTcpTransport {
    /// Connect to a camera via Sony encapsulated TCP.
    pub async fn connect(config: SonyIpConfig) -> Result<Self> {
        // Create blocking transport in a blocking task
        let transport = tokio::task::spawn_blocking(move || SonyTcpTransport::connect(config))
            .await
            .map_err(|e| {
                Error::TransportError(format!("Failed to spawn blocking task: {}", e).into())
            })??;

        Ok(Self {
            inner: Arc::new(transport),
        })
    }
}

#[cfg(feature = "rt-tokio")]
impl AsyncTransport for AsyncSonyTcpTransport {
    async fn send(&self, bytes: &[u8]) -> Result<()> {
        let inner = self.inner.clone();
        let bytes = bytes.to_vec();

        tokio::task::spawn_blocking(move || inner.send_blocking(&bytes))
            .await
            .map_err(|e| Error::TransportError(format!("Async send error: {}", e).into()))?
    }

    async fn recv(&self) -> Result<Bytes> {
        let inner = self.inner.clone();

        tokio::task::spawn_blocking(move || inner.recv_blocking())
            .await
            .map_err(|e| Error::TransportError(format!("Async recv error: {}", e).into()))?
    }
}

/// Async Sony UDP transport.
#[cfg(feature = "rt-tokio")]
#[derive(Clone, Debug)]
pub struct AsyncSonyUdpTransport {
    inner: Arc<SonyUdpTransport>,
}

#[cfg(feature = "rt-tokio")]
impl AsyncSonyUdpTransport {
    /// Connect to a camera via Sony encapsulated UDP.
    pub async fn connect(config: SonyIpConfig) -> Result<Self> {
        // Create blocking transport in a blocking task
        let transport = tokio::task::spawn_blocking(move || SonyUdpTransport::connect(config))
            .await
            .map_err(|e| {
                Error::TransportError(format!("Failed to spawn blocking task: {}", e).into())
            })??;

        Ok(Self {
            inner: Arc::new(transport),
        })
    }
}

#[cfg(feature = "rt-tokio")]
impl AsyncTransport for AsyncSonyUdpTransport {
    async fn send(&self, bytes: &[u8]) -> Result<()> {
        let inner = self.inner.clone();
        let bytes = bytes.to_vec();

        tokio::task::spawn_blocking(move || inner.send_blocking(&bytes))
            .await
            .map_err(|e| Error::TransportError(format!("Async send error: {}", e).into()))?
    }

    async fn recv(&self) -> Result<Bytes> {
        let inner = self.inner.clone();

        tokio::task::spawn_blocking(move || inner.recv_blocking())
            .await
            .map_err(|e| Error::TransportError(format!("Async recv error: {}", e).into()))?
    }
}
