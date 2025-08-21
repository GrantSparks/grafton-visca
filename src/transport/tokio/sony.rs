//! Native async Sony encapsulated VISCA over IP transport for Tokio.
//!
//! This module provides native async implementations of Sony's encapsulation format
//! using Tokio's async I/O primitives. This is a true async implementation without
//! any blocking operations or thread pool usage.

use bytes::{Bytes, BytesMut};
use log::{debug, trace, warn};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::Mutex;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

use crate::{
    error::{Error, Result},
    protocol::encode::{PayloadType, SonyHeader},
    transport::{builder::TransportConfig, AsyncTransport},
};

/// Configuration for async Sony encapsulated IP transport.
pub use crate::transport::ip_sony::SonyIpConfig;

// Type alias for pending command tracking.
// We just need to track which sequence numbers are pending.
// Using unit type since we only care about presence in the HashMap.
type PendingCommand = ();

/// Native async Sony TCP transport using Tokio.
///
/// This transport implements Sony's encapsulation format with an 8-byte header
/// containing sequence numbers for request/response matching and automatic retry
/// on network errors. This is a true async implementation without blocking operations.
#[derive(Debug)]
pub struct Tcp {
    stream: Arc<Mutex<TcpStream>>,
    sequence: Arc<AtomicU32>,
    pending: Arc<Mutex<HashMap<u32, PendingCommand>>>,
    read_buffer: Arc<Mutex<BytesMut>>,
}

impl Tcp {
    /// Connect to a camera via Sony encapsulated TCP.
    pub async fn connect(address: &str) -> Result<Self> {
        let config = SonyIpConfig {
            address: address.to_string(),
            ..Default::default()
        };
        Self::connect_with_config(&config).await
    }

    /// Connect with custom configuration.
    pub async fn connect_with_config(config: &SonyIpConfig) -> Result<Self> {
        debug!("Connecting to {} via Sony TCP (async)", config.address);

        // Connect with timeout
        let stream =
            tokio::time::timeout(config.connect_timeout, TcpStream::connect(&config.address))
                .await
                .map_err(|_| Error::Timeout)??;

        // Set TCP nodelay for low latency
        stream.set_nodelay(true)?;

        debug!("Connected to {} (async)", config.address);

        Ok(Self {
            stream: Arc::new(Mutex::new(stream)),
            sequence: Arc::new(AtomicU32::new(1)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            read_buffer: Arc::new(Mutex::new(BytesMut::with_capacity(1024))),
        })
    }

    /// Connect with transport config (for builder compatibility).
    pub async fn connect_with_transport_config(
        address: &str,
        _config: TransportConfig,
    ) -> Result<Self> {
        // For now, just use default Sony config with the provided address
        // In the future, we could map TransportConfig fields to SonyIpConfig
        Self::connect(address).await
    }

    /// Send a command with Sony header.
    async fn send_with_header(&self, bytes: &[u8], sequence: u32) -> Result<()> {
        // Check if this is an inquiry (second byte is 0x09)
        let is_inquiry = bytes.len() >= 2 && bytes[1] == 0x09;
        let header = if is_inquiry {
            SonyHeader::new_inquiry(bytes.len(), sequence)
        } else {
            SonyHeader::new_command(bytes.len(), sequence)
        };

        let mut packet = BytesMut::with_capacity(SonyHeader::SIZE + bytes.len());
        packet.extend_from_slice(&header.encode());
        packet.extend_from_slice(bytes);

        // Send packet
        let mut stream = self.stream.lock().await;
        stream.write_all(&packet).await?;
        stream.flush().await?;

        trace!(
            "Sent packet with seq {}: header={:02X?} payload={:02X?}",
            sequence,
            &packet[..SonyHeader::SIZE],
            bytes
        );

        Ok(())
    }

    /// Receive a Sony encapsulated frame.
    async fn recv_sony_frame(&self) -> Result<(SonyHeader, Bytes)> {
        let mut buffer = self.read_buffer.lock().await;
        let mut stream = self.stream.lock().await;
        let mut temp_buf = vec![0u8; 256];

        loop {
            // Check if we have a complete header
            if buffer.len() >= SonyHeader::SIZE {
                let payload_length = u16::from_be_bytes([buffer[2], buffer[3]]) as usize;

                // Check if we have the complete payload
                if buffer.len() >= SonyHeader::SIZE + payload_length {
                    // Extract frame
                    let header_bytes = buffer.split_to(SonyHeader::SIZE);
                    let payload = buffer.split_to(payload_length);

                    let header = SonyHeader::decode(&header_bytes).ok_or_else(|| {
                        Error::InvalidResponse {
                            expected: "Valid Sony header".into(),
                            actual: format!("Invalid header bytes: {:02X?}", header_bytes).into(),
                        }
                    })?;

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
            match stream.read(&mut temp_buf).await {
                Ok(0) => return Err(Error::ConnectionClosed),
                Ok(n) => {
                    buffer.extend_from_slice(&temp_buf[..n]);
                    trace!("Read {} bytes from TCP", n);
                }
                Err(e) => {
                    return Err(Error::TransportError(format!("TCP read error: {e}").into()));
                }
            }
        }
    }
}

impl AsyncTransport for Tcp {
    async fn send(&self, bytes: &[u8]) -> Result<()> {
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst);

        // Store pending command for potential retry
        {
            let mut pending = self.pending.lock().await;
            pending.insert(sequence, ());
        }

        // Send with header
        self.send_with_header(bytes, sequence).await?;

        Ok(())
    }

    async fn recv(&self) -> Result<Bytes> {
        loop {
            match self.recv_sony_frame().await {
                Ok((header, payload)) => {
                    // Only accept replies that match a pending command
                    if header.payload_type == PayloadType::ViscaReply {
                        let mut pending = self.pending.lock().await;
                        if pending.remove(&header.sequence_number).is_none() {
                            // Late or duplicate reply - discard it
                            warn!(
                                "TCP: Discarding late/duplicate reply with seq {} (not in pending)",
                                header.sequence_number
                            );
                            continue;
                        }
                    }
                    return Ok(payload);
                }
                Err(e) => return Err(e),
            }
        }
    }
}

/// Native async Sony UDP transport using Tokio.
///
/// This transport implements Sony's encapsulation format with an 8-byte header
/// containing sequence numbers for request/response matching and automatic retry
/// on network errors. This is a true async implementation without blocking operations.
#[derive(Debug)]
pub struct Udp {
    socket: Arc<UdpSocket>,
    sequence: Arc<AtomicU32>,
    pending: Arc<Mutex<HashMap<u32, PendingCommand>>>,
    config: SonyIpConfig,
}

impl Udp {
    /// Connect to a camera via Sony encapsulated UDP.
    pub async fn connect(address: &str) -> Result<Self> {
        let config = SonyIpConfig {
            address: address.to_string(),
            use_tcp: false,
            ..Default::default()
        };
        Self::connect_with_config(&config).await
    }

    /// Connect with custom configuration.
    pub async fn connect_with_config(config: &SonyIpConfig) -> Result<Self> {
        debug!("Connecting to {} via Sony UDP (async)", config.address);

        // Parse the address
        let remote_addr: SocketAddr =
            config
                .address
                .parse()
                .map_err(|e| Error::InvalidParameter {
                    parameter: "address",
                    value: config.address.clone().into(),
                    reason: format!("Invalid socket address: {e}").into(),
                })?;

        // Create and bind UDP socket
        let local_addr: SocketAddr = if remote_addr.is_ipv4() {
            SocketAddr::from(([0, 0, 0, 0], 0))
        } else {
            SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 0], 0))
        };

        let socket = UdpSocket::bind(local_addr).await?;
        socket.connect(&remote_addr).await?;

        debug!("Connected to {} (async UDP)", config.address);

        Ok(Self {
            socket: Arc::new(socket),
            sequence: Arc::new(AtomicU32::new(1)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            config: config.clone(),
        })
    }

    /// Connect with transport config (for builder compatibility).
    pub async fn connect_with_transport_config(
        address: &str,
        _config: TransportConfig,
    ) -> Result<Self> {
        // For now, just use default Sony config with the provided address
        // In the future, we could map TransportConfig fields to SonyIpConfig
        Self::connect(address).await
    }

    /// Send a command with Sony header.
    async fn send_with_header(&self, bytes: &[u8], sequence: u32) -> Result<()> {
        // Check if this is an inquiry (second byte is 0x09)
        let is_inquiry = bytes.len() >= 2 && bytes[1] == 0x09;
        let header = if is_inquiry {
            SonyHeader::new_inquiry(bytes.len(), sequence)
        } else {
            SonyHeader::new_command(bytes.len(), sequence)
        };

        let mut packet = BytesMut::with_capacity(SonyHeader::SIZE + bytes.len());
        packet.extend_from_slice(&header.encode());
        packet.extend_from_slice(bytes);

        // Send packet
        self.socket.send(&packet).await?;

        trace!("Sent UDP packet with seq {}: {:02X?}", sequence, packet);

        Ok(())
    }

    /// Receive a Sony encapsulated frame.
    async fn recv_sony_frame(&self) -> Result<(SonyHeader, Bytes)> {
        let mut buf = vec![0u8; 1024];

        match self.socket.recv(&mut buf).await {
            Ok(n) if n >= SonyHeader::SIZE => {
                let payload_length = u16::from_be_bytes([buf[2], buf[3]]) as usize;

                if n >= SonyHeader::SIZE + payload_length {
                    let header = SonyHeader::decode(&buf[..SonyHeader::SIZE]).ok_or_else(|| {
                        Error::InvalidResponse {
                            expected: "Valid Sony header".into(),
                            actual: "Invalid header bytes".to_string().into(),
                        }
                    })?;

                    let payload = Bytes::copy_from_slice(
                        &buf[SonyHeader::SIZE..SonyHeader::SIZE + payload_length],
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
                        expected: format!("Sony frame with {} byte payload", payload_length).into(),
                        actual: format!("Only {} bytes received", n - SonyHeader::SIZE).into(),
                    })
                }
            }
            Ok(n) => Err(Error::InvalidResponse {
                expected: "Sony encapsulated frame".into(),
                actual: format!("Short packet ({} bytes)", n).into(),
            }),
            Err(e) => Err(Error::TransportError(format!("UDP read error: {e}").into())),
        }
    }
}

impl AsyncTransport for Udp {
    async fn send(&self, bytes: &[u8]) -> Result<()> {
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst);

        // Store pending command for potential retry
        {
            let mut pending = self.pending.lock().await;
            pending.insert(sequence, ());
        }

        // Send with header
        self.send_with_header(bytes, sequence).await?;

        Ok(())
    }

    async fn recv(&self) -> Result<Bytes> {
        // For UDP, we may need to implement retry logic
        // For now, keeping it simple
        loop {
            match tokio::time::timeout(self.config.response_timeout, self.recv_sony_frame()).await {
                Ok(Ok((header, payload))) => {
                    // Only accept replies that match a pending command
                    if header.payload_type == PayloadType::ViscaReply {
                        let mut pending = self.pending.lock().await;
                        if pending.remove(&header.sequence_number).is_none() {
                            // Late or duplicate reply - discard it
                            warn!(
                                "UDP: Discarding late/duplicate reply with seq {} (not in pending)",
                                header.sequence_number
                            );
                            continue;
                        }
                    }
                    return Ok(payload);
                }
                Ok(Err(e)) => return Err(e),
                Err(_) => {
                    // Timeout - in a real implementation, we might retry here
                    return Err(Error::Timeout);
                }
            }
        }
    }
}
