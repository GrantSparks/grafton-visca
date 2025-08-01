//! Blocking adapter for the new transport architecture.
//!
//! This module provides blocking transport implementations that implement
//! both the async Transport trait (with immediate futures) and the
//! BlockingTransport trait for synchronous operation.

use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;

use crate::{
    transport::async_trait_transport::{BlockingTransport, Transport},
    Error,
};

/// TCP transport for blocking VISCA communication.
#[derive(Debug)]
pub struct TcpTransportBlocking {
    stream: Mutex<TcpStream>,
}

impl TcpTransportBlocking {
    /// Connect to a TCP endpoint.
    pub fn connect(address: &str) -> Result<Self, Error> {
        Self::connect_timeout(address, Duration::from_secs(5))
    }

    /// Connect with a custom timeout.
    pub fn connect_timeout(address: &str, timeout: Duration) -> Result<Self, Error> {
        let addr = address
            .parse()
            .map_err(|e| Error::TransportError(format!("Invalid address: {e}").into()))?;

        let stream = TcpStream::connect_timeout(&addr, timeout)
            .map_err(|e| Error::TransportError(e.to_string().into()))?;

        // Set socket options
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| Error::TransportError(e.to_string().into()))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| Error::TransportError(e.to_string().into()))?;
        stream
            .set_nodelay(true)
            .map_err(|e| Error::TransportError(e.to_string().into()))?;

        Ok(Self {
            stream: Mutex::new(stream),
        })
    }
}

#[async_trait]
impl Transport for TcpTransportBlocking {
    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        self.send_blocking(bytes)
    }

    async fn recv(&self) -> Result<Bytes, Error> {
        self.recv_blocking()
    }
}

impl BlockingTransport for TcpTransportBlocking {
    fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error> {
        let mut stream = self
            .stream
            .lock()
            .map_err(|e| Error::TransportError(format!("Failed to lock stream: {e}").into()))?;

        stream
            .write_all(bytes)
            .map_err(|e| Error::TransportError(e.to_string().into()))?;
        stream
            .flush()
            .map_err(|e| Error::TransportError(e.to_string().into()))?;
        Ok(())
    }

    fn recv_blocking(&self) -> Result<Bytes, Error> {
        let mut stream = self
            .stream
            .lock()
            .map_err(|e| Error::TransportError(format!("Failed to lock stream: {e}").into()))?;

        let mut buffer = Vec::with_capacity(256);
        let mut byte = [0u8; 1];

        // Read until VISCA terminator
        loop {
            match stream.read_exact(&mut byte) {
                Ok(()) => {
                    buffer.push(byte[0]);
                    if byte[0] == 0xFF {
                        // Found terminator
                        return Ok(Bytes::from(buffer));
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Err(Error::TransportError("Connection closed".into()))
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    return Err(Error::Timeout)
                }
                Err(e) => return Err(Error::TransportError(e.to_string().into())),
            }

            // Check for buffer overflow
            if buffer.len() > 1024 {
                return Err(Error::TransportError("Response too large".into()));
            }
        }
    }
}

/// UDP transport for blocking VISCA communication.
#[derive(Debug)]
pub struct UdpTransportBlocking {
    socket: UdpSocket,
    #[allow(dead_code)]
    remote_addr: String,
}

impl UdpTransportBlocking {
    /// Connect to a UDP endpoint.
    pub fn connect(local_addr: &str, remote_addr: &str) -> Result<Self, Error> {
        let socket = UdpSocket::bind(local_addr)
            .map_err(|e| Error::TransportError(e.to_string().into()))?;

        socket
            .connect(&remote_addr)
            .map_err(|e| Error::TransportError(e.to_string().into()))?;

        // Set timeouts
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| Error::TransportError(e.to_string().into()))?;
        socket
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| Error::TransportError(e.to_string().into()))?;

        Ok(Self {
            socket,
            remote_addr: remote_addr.to_string(),
        })
    }
}

#[async_trait]
impl Transport for UdpTransportBlocking {
    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        self.send_blocking(bytes)
    }

    async fn recv(&self) -> Result<Bytes, Error> {
        self.recv_blocking()
    }
}

impl BlockingTransport for UdpTransportBlocking {
    fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error> {
        self.socket
            .send(bytes)
            .map_err(|e| Error::TransportError(e.to_string().into()))?;
        Ok(())
    }

    fn recv_blocking(&self) -> Result<Bytes, Error> {
        let mut buffer = vec![0u8; 1024];
        let n = self
            .socket
            .recv(&mut buffer)
            .map_err(|e| Error::TransportError(e.to_string().into()))?;

        buffer.truncate(n);
        Ok(Bytes::from(buffer))
    }
}

/// Helper functions for creating blocking transports.
pub mod factory {
    use super::*;

    /// Connect to a TCP endpoint and return a blocking Transport.
    pub fn connect_tcp(addr: &str) -> Result<impl BlockingTransport, Error> {
        TcpTransportBlocking::connect(addr)
    }

    /// Connect to a UDP endpoint and return a blocking Transport.
    pub fn connect_udp(local: &str, remote: &str) -> Result<impl BlockingTransport, Error> {
        UdpTransportBlocking::connect(local, remote)
    }
}