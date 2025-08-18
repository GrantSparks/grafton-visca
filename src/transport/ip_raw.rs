//! Raw VISCA over IP transport (PtzOptics style).
//!
//! This module provides raw VISCA communication over TCP/UDP without
//! any additional encapsulation. This is the format used by PtzOptics cameras.

use bytes::{Bytes, BytesMut};
use log::{debug, trace};

use std::io::{Read, Write};

#[cfg(feature = "rt-tokio")]
use std::net::SocketAddr;
use std::net::{TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::error::{Error, Result};
use crate::protocol::encode::VISCA_TERMINATOR;
use crate::transport::BlockingTransport;

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
}

impl Default for RawIpConfig {
    fn default() -> Self {
        Self {
            address: "192.168.0.110:5678".to_string(),
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_millis(100),
            write_timeout: Duration::from_millis(100),
        }
    }
}

/// Raw TCP transport for blocking I/O.
#[derive(Debug)]
pub struct RawTcpTransport {
    stream: Arc<Mutex<TcpStream>>,
    read_buffer: Arc<Mutex<BytesMut>>,
}

impl RawTcpTransport {
    /// Connect to a camera via raw TCP.
    pub fn connect(config: RawIpConfig) -> Result<Self> {
        let addr = config
            .address
            .to_socket_addrs()
            .map_err(|e| Error::TransportError(format!("Invalid address: {}", e).into()))?
            .next()
            .ok_or_else(|| Error::TransportError("No valid address".into()))?;

        debug!("Connecting to {} via raw TCP", addr);

        let stream = TcpStream::connect_timeout(&addr, config.connect_timeout)
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
            read_buffer: Arc::new(Mutex::new(BytesMut::with_capacity(256))),
        })
    }

    /// Receive a complete VISCA frame.
    fn recv_frame(&self) -> Result<Bytes> {
        let mut buffer = self
            .read_buffer
            .lock()
            .map_err(|_| Error::LockPoisoned("transport mutex"))?;
        let mut stream = self
            .stream
            .lock()
            .map_err(|_| Error::LockPoisoned("transport mutex"))?;
        let mut temp_buf = [0u8; 256];

        loop {
            // Check if we have a complete frame in the buffer
            if let Some(pos) = buffer.iter().position(|&b| b == VISCA_TERMINATOR) {
                let frame = buffer.split_to(pos + 1);
                trace!("Received frame: {:02X?}", frame);
                return Ok(frame.freeze());
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
}

impl BlockingTransport for RawTcpTransport {
    fn send_blocking(&self, bytes: &[u8]) -> Result<()> {
        let mut stream = self
            .stream
            .lock()
            .map_err(|_| Error::LockPoisoned("transport mutex"))?;
        stream
            .write_all(bytes)
            .map_err(|e| Error::TransportError(format!("TCP write error: {}", e).into()))?;
        stream
            .flush()
            .map_err(|e| Error::TransportError(format!("TCP flush error: {}", e).into()))?;
        trace!("Sent {} bytes: {:02X?}", bytes.len(), bytes);
        Ok(())
    }

    fn recv_blocking(&self) -> Result<Bytes> {
        self.recv_frame()
    }

    fn recv_blocking_with_timeout(&self, timeout: Duration) -> Result<Bytes> {
        // Temporarily set the timeout on the stream
        let stream = self
            .stream
            .lock()
            .map_err(|_| Error::LockPoisoned("transport mutex"))?;
        let original_read_timeout = stream
            .read_timeout()
            .map_err(|e| Error::TransportError(format!("Failed to get timeout: {}", e).into()))?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|e| Error::TransportError(format!("Failed to set timeout: {}", e).into()))?;
        drop(stream);

        let result = self.recv_frame();

        // Restore original timeout
        let stream = self
            .stream
            .lock()
            .map_err(|_| Error::LockPoisoned("transport mutex"))?;
        stream
            .set_read_timeout(original_read_timeout)
            .map_err(|e| {
                Error::TransportError(format!("Failed to restore timeout: {}", e).into())
            })?;

        result
    }
}

/// Raw UDP transport for blocking I/O.
#[derive(Debug)]
pub struct RawUdpTransport {
    socket: Arc<UdpSocket>,
    read_buffer: Arc<Mutex<BytesMut>>,
}

impl RawUdpTransport {
    /// Connect to a camera via raw UDP.
    pub fn connect(config: RawIpConfig) -> Result<Self> {
        let addr = config
            .address
            .to_socket_addrs()
            .map_err(|e| Error::TransportError(format!("Invalid address: {}", e).into()))?
            .next()
            .ok_or_else(|| Error::TransportError("No valid address".into()))?;

        debug!("Connecting to {} via raw UDP", addr);

        let socket = UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| Error::TransportError(format!("UDP bind failed: {}", e).into()))?;

        socket
            .connect(addr)
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
            read_buffer: Arc::new(Mutex::new(BytesMut::with_capacity(256))),
        })
    }

    /// Receive a complete VISCA frame.
    fn recv_frame(&self) -> Result<Bytes> {
        let mut buffer = self
            .read_buffer
            .lock()
            .map_err(|_| Error::LockPoisoned("transport mutex"))?;
        let mut temp_buf = [0u8; 1500]; // UDP MTU

        loop {
            // Check if we have a complete frame in the buffer
            if let Some(pos) = buffer.iter().position(|&b| b == VISCA_TERMINATOR) {
                let frame = buffer.split_to(pos + 1);
                trace!("Received frame: {:02X?}", frame);
                return Ok(frame.freeze());
            }

            // Read more data
            match self.socket.recv(&mut temp_buf) {
                Ok(n) if n > 0 => {
                    buffer.extend_from_slice(&temp_buf[..n]);
                    trace!("Read {} bytes from UDP", n);
                }
                Ok(_) => {
                    return Err(Error::Timeout);
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
}

impl BlockingTransport for RawUdpTransport {
    fn send_blocking(&self, bytes: &[u8]) -> Result<()> {
        self.socket
            .send(bytes)
            .map_err(|e| Error::TransportError(format!("UDP send error: {}", e).into()))?;
        trace!("Sent {} bytes: {:02X?}", bytes.len(), bytes);
        Ok(())
    }

    fn recv_blocking(&self) -> Result<Bytes> {
        self.recv_frame()
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

        let result = self.recv_frame();

        // Restore original timeout
        self.socket
            .set_read_timeout(original_read_timeout)
            .map_err(|e| {
                Error::TransportError(format!("Failed to restore timeout: {}", e).into())
            })?;

        result
    }
}

/// Async raw TCP transport using tokio.
#[cfg(feature = "rt-tokio")]
#[derive(Debug)]
pub struct AsyncRawTcpTransport {
    stream: Arc<tokio::sync::Mutex<tokio::net::TcpStream>>,
    read_buffer: Arc<tokio::sync::Mutex<BytesMut>>,
}

#[cfg(feature = "rt-tokio")]
impl AsyncRawTcpTransport {
    /// Connect to a camera via raw TCP.
    pub async fn connect(config: RawIpConfig) -> Result<Self> {
        let addr = config
            .address
            .parse::<SocketAddr>()
            .map_err(|e| Error::TransportError(format!("Invalid address: {}", e).into()))?;

        debug!("Connecting to {} via raw TCP", addr);

        let stream =
            tokio::time::timeout(config.connect_timeout, tokio::net::TcpStream::connect(addr))
                .await
                .map_err(|_| Error::Timeout)?
                .map_err(|e| Error::TransportError(format!("TCP connect failed: {}", e).into()))?;

        debug!("Connected to {}", addr);

        Ok(Self {
            stream: Arc::new(tokio::sync::Mutex::new(stream)),
            read_buffer: Arc::new(tokio::sync::Mutex::new(BytesMut::with_capacity(256))),
        })
    }

    /// Receive a complete VISCA frame.
    async fn recv_frame(&self) -> Result<Bytes> {
        use tokio::io::AsyncReadExt;

        let mut buffer = self.read_buffer.lock().await;
        let mut stream = self.stream.lock().await;
        let mut temp_buf = [0u8; 256];

        loop {
            // Check if we have a complete frame in the buffer
            if let Some(pos) = buffer.iter().position(|&b| b == VISCA_TERMINATOR) {
                let frame = buffer.split_to(pos + 1);
                trace!("Received frame: {:02X?}", frame);
                return Ok(frame.freeze());
            }

            // Read more data
            match stream.read(&mut temp_buf).await {
                Ok(n) if n > 0 => {
                    buffer.extend_from_slice(&temp_buf[..n]);
                    trace!("Read {} bytes from TCP", n);
                }
                Ok(_) => {
                    return Err(Error::ConnectionClosed);
                }
                Err(e) => {
                    return Err(Error::TransportError(
                        format!("TCP read error: {}", e).into(),
                    ));
                }
            }
        }
    }
}

#[cfg(feature = "rt-tokio")]
impl crate::transport::AsyncTransport for AsyncRawTcpTransport {
    async fn send(&self, bytes: &[u8]) -> Result<()> {
        use tokio::io::AsyncWriteExt;

        let mut stream = self.stream.lock().await;
        stream
            .write_all(bytes)
            .await
            .map_err(|e| Error::TransportError(format!("TCP write error: {}", e).into()))?;
        stream
            .flush()
            .await
            .map_err(|e| Error::TransportError(format!("TCP flush error: {}", e).into()))?;
        trace!("Sent {} bytes: {:02X?}", bytes.len(), bytes);
        Ok(())
    }

    async fn recv(&self) -> Result<Bytes> {
        self.recv_frame().await
    }
}

/// Async raw UDP transport using tokio.
#[cfg(feature = "rt-tokio")]
#[derive(Debug)]
pub struct AsyncRawUdpTransport {
    socket: Arc<tokio::net::UdpSocket>,
    read_buffer: Arc<tokio::sync::Mutex<BytesMut>>,
}

#[cfg(feature = "rt-tokio")]
impl AsyncRawUdpTransport {
    /// Connect to a camera via raw UDP.
    pub async fn connect(config: RawIpConfig) -> Result<Self> {
        let addr = config
            .address
            .parse::<SocketAddr>()
            .map_err(|e| Error::TransportError(format!("Invalid address: {}", e).into()))?;

        debug!("Connecting to {} via raw UDP", addr);

        let socket = tokio::net::UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| Error::TransportError(format!("UDP bind failed: {}", e).into()))?;

        socket
            .connect(addr)
            .await
            .map_err(|e| Error::TransportError(format!("UDP connect failed: {}", e).into()))?;

        debug!("Connected to {}", addr);

        Ok(Self {
            socket: Arc::new(socket),
            read_buffer: Arc::new(tokio::sync::Mutex::new(BytesMut::with_capacity(256))),
        })
    }

    /// Receive a complete VISCA frame.
    async fn recv_frame(&self) -> Result<Bytes> {
        let mut buffer = self.read_buffer.lock().await;
        let mut temp_buf = [0u8; 1500]; // UDP MTU

        loop {
            // Check if we have a complete frame in the buffer
            if let Some(pos) = buffer.iter().position(|&b| b == VISCA_TERMINATOR) {
                let frame = buffer.split_to(pos + 1);
                trace!("Received frame: {:02X?}", frame);
                return Ok(frame.freeze());
            }

            // Read more data
            match self.socket.recv(&mut temp_buf).await {
                Ok(n) if n > 0 => {
                    buffer.extend_from_slice(&temp_buf[..n]);
                    trace!("Read {} bytes from UDP", n);
                }
                Ok(_) => {
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
}

#[cfg(feature = "rt-tokio")]
impl crate::transport::AsyncTransport for AsyncRawUdpTransport {
    async fn send(&self, bytes: &[u8]) -> Result<()> {
        self.socket
            .send(bytes)
            .await
            .map_err(|e| Error::TransportError(format!("UDP send error: {}", e).into()))?;
        trace!("Sent {} bytes: {:02X?}", bytes.len(), bytes);
        Ok(())
    }

    async fn recv(&self) -> Result<Bytes> {
        self.recv_frame().await
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
