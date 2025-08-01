//! Tokio adapter for async-trait based transport.
//!
//! This module provides tokio-specific transport implementations that use
//! the new async-trait based Transport interface.

#[cfg(feature = "tokio")]
use std::sync::Arc;
#[cfg(feature = "tokio")]
use std::time::Duration;

#[cfg(feature = "tokio")]
use async_trait::async_trait;
#[cfg(feature = "tokio")]
use bytes::Bytes;
#[cfg(feature = "tokio")]
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[cfg(feature = "tokio")]
use tokio::net::{TcpStream, UdpSocket};
#[cfg(feature = "tokio")]
use tokio::sync::Mutex;

#[cfg(feature = "tokio")]
use crate::{transport::async_trait_transport::Transport, Error};

/// TCP transport for async VISCA communication using tokio.
#[cfg(feature = "tokio")]
#[derive(Debug)]
pub struct TcpTransport {
    stream: Arc<Mutex<TcpStream>>,
}

#[cfg(feature = "tokio")]
impl TcpTransport {
    /// Connect to a TCP endpoint.
    pub async fn connect(address: &str) -> Result<Self, Error> {
        Self::connect_timeout(address, Duration::from_secs(5)).await
    }

    /// Connect with a custom timeout.
    pub async fn connect_timeout(address: &str, timeout: Duration) -> Result<Self, Error> {
        let stream = tokio::time::timeout(timeout, TcpStream::connect(address))
            .await
            .map_err(|_| Error::Timeout)?
            .map_err(|e| Error::TransportError(e.to_string().into()))?;

        // Set TCP nodelay for low latency
        stream
            .set_nodelay(true)
            .map_err(|e| Error::TransportError(e.to_string().into()))?;

        Ok(Self {
            stream: Arc::new(Mutex::new(stream)),
        })
    }
}

#[cfg(feature = "tokio")]
#[async_trait]
impl Transport for TcpTransport {
    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        let mut stream = self.stream.lock().await;
        stream
            .write_all(bytes)
            .await
            .map_err(|e| Error::TransportError(e.to_string().into()))?;
        stream
            .flush()
            .await
            .map_err(|e| Error::TransportError(e.to_string().into()))?;
        Ok(())
    }

    async fn recv(&self) -> Result<Bytes, Error> {
        let mut stream = self.stream.lock().await;
        let mut buffer = Vec::with_capacity(256);
        let mut temp_buf = vec![0u8; 128];

        loop {
            // Read efficiently in chunks
            let n = stream
                .read(&mut temp_buf)
                .await
                .map_err(|e| Error::TransportError(e.to_string().into()))?;

            if n == 0 {
                return Err(Error::TransportError("Connection closed".into()));
            }

            // Scan for VISCA terminator
            for i in 0..n {
                buffer.push(temp_buf[i]);
                if temp_buf[i] == 0xFF {
                    // Found terminator
                    return Ok(Bytes::from(buffer));
                }
            }

            // Check for buffer overflow
            if buffer.len() > 1024 {
                return Err(Error::TransportError("Response too large".into()));
            }
        }
    }
}

/// UDP transport for async VISCA communication using tokio.
#[cfg(feature = "tokio")]
#[derive(Debug)]
pub struct UdpTransport {
    socket: Arc<UdpSocket>,
    #[allow(dead_code)]
    remote_addr: String,
}

#[cfg(feature = "tokio")]
impl UdpTransport {
    /// Connect to a UDP endpoint.
    pub async fn connect(local_addr: &str, remote_addr: &str) -> Result<Self, Error> {
        let socket = UdpSocket::bind(local_addr)
            .await
            .map_err(|e| Error::TransportError(e.to_string().into()))?;

        socket
            .connect(&remote_addr)
            .await
            .map_err(|e| Error::TransportError(e.to_string().into()))?;

        Ok(Self {
            socket: Arc::new(socket),
            remote_addr: remote_addr.to_string(),
        })
    }
}

#[cfg(feature = "tokio")]
#[async_trait]
impl Transport for UdpTransport {
    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        self.socket
            .send(bytes)
            .await
            .map_err(|e| Error::TransportError(e.to_string().into()))?;
        Ok(())
    }

    async fn recv(&self) -> Result<Bytes, Error> {
        let mut buffer = vec![0u8; 1024];
        let n = self
            .socket
            .recv(&mut buffer)
            .await
            .map_err(|e| Error::TransportError(e.to_string().into()))?;

        buffer.truncate(n);
        Ok(Bytes::from(buffer))
    }
}

/// Helper functions for creating tokio-based transports.
#[cfg(feature = "tokio")]
pub mod factory {
    use super::*;

    /// Connect to a TCP endpoint and return a Transport.
    pub async fn connect_tcp(addr: &str) -> Result<impl Transport, Error> {
        TcpTransport::connect(addr).await
    }

    /// Connect to a UDP endpoint and return a Transport.
    pub async fn connect_udp(local: &str, remote: &str) -> Result<impl Transport, Error> {
        UdpTransport::connect(local, remote).await
    }
}

/// Timeout wrapper for any Transport implementation.
#[cfg(feature = "tokio")]
#[derive(Debug)]
pub struct TimeoutTransport<T> {
    inner: T,
    timeout: Duration,
}

#[cfg(feature = "tokio")]
impl<T: Transport> TimeoutTransport<T> {
    /// Create a new transport with timeout.
    pub fn new(inner: T, timeout: Duration) -> Self {
        Self { inner, timeout }
    }
}

#[cfg(feature = "tokio")]
#[async_trait]
impl<T: Transport> Transport for TimeoutTransport<T> {
    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        tokio::time::timeout(self.timeout, self.inner.send(bytes))
            .await
            .map_err(|_| Error::Timeout)?
    }

    async fn recv(&self) -> Result<Bytes, Error> {
        tokio::time::timeout(self.timeout, self.inner.recv())
            .await
            .map_err(|_| Error::Timeout)?
    }
}