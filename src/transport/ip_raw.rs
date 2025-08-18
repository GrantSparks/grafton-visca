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
use std::time::{Duration, Instant};

use crate::error::{Error, Result};
use crate::protocol::encode::VISCA_TERMINATOR;
use crate::transport::{BlockingTransport, RetryConfig};

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
    stream: Arc<Mutex<TcpStream>>,
    read_buffer: Arc<Mutex<BytesMut>>,
    config: RawIpConfig,
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
            config,
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
        let mut attempts = 0;
        let start_time = Instant::now();

        loop {
            let mut stream = self
                .stream
                .lock()
                .map_err(|_| Error::LockPoisoned("transport mutex"))?;

            let result = stream
                .write_all(bytes)
                .and_then(|_| stream.flush())
                .map_err(|e| Error::TransportError(format!("TCP write error: {}", e).into()));

            drop(stream);

            match result {
                Ok(()) => {
                    trace!("Sent {} bytes: {:02X?}", bytes.len(), bytes);
                    return Ok(());
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

                    debug!("Retrying TCP send (attempt {}): {:?}", attempts, e);
                    std::thread::sleep(delay);
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn recv_blocking(&self) -> Result<Bytes> {
        let mut attempts = 0;
        let start_time = Instant::now();

        loop {
            match self.recv_frame() {
                Ok(frame) => return Ok(frame),
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

                    debug!("Retrying TCP receive (attempt {}): {:?}", attempts, e);
                    std::thread::sleep(delay);
                }
                Err(e) => return Err(e),
            }
        }
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
    config: RawIpConfig,
    /// Track last sent command for retry on timeout
    last_command: Arc<Mutex<Option<Vec<u8>>>>,
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
            config,
            last_command: Arc::new(Mutex::new(None)),
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
        // Store the command for potential retry on receive timeout
        {
            let mut last_cmd = self
                .last_command
                .lock()
                .map_err(|_| Error::LockPoisoned("transport mutex"))?;
            *last_cmd = Some(bytes.to_vec());
        }

        let mut attempts = 0;
        let start_time = Instant::now();

        loop {
            let result = self
                .socket
                .send(bytes)
                .map_err(|e| Error::TransportError(format!("UDP send error: {}", e).into()));

            match result {
                Ok(_) => {
                    trace!("Sent {} bytes: {:02X?}", bytes.len(), bytes);
                    return Ok(());
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

                    debug!("Retrying UDP send (attempt {}): {:?}", attempts, e);
                    std::thread::sleep(delay);
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn recv_blocking(&self) -> Result<Bytes> {
        let mut attempts = 0;
        let start_time = Instant::now();

        loop {
            match self.recv_frame() {
                Ok(frame) => return Ok(frame),
                Err(Error::Timeout)
                    if self.config.retry_config.should_retry(attempts, start_time) =>
                {
                    // For UDP, timeout might mean packet loss - resend last command
                    let last_cmd = self
                        .last_command
                        .lock()
                        .map_err(|_| Error::LockPoisoned("transport mutex"))?
                        .clone();

                    if let Some(cmd) = last_cmd {
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

                        debug!(
                            "UDP receive timeout, resending command (attempt {})",
                            attempts
                        );

                        // Resend the command
                        self.socket.send(&cmd).map_err(|e| {
                            Error::TransportError(format!("UDP resend error: {}", e).into())
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

                    debug!("Retrying UDP receive (attempt {}): {:?}", attempts, e);
                    std::thread::sleep(delay);
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
    retry_config: RetryConfig,
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
            retry_config: config.retry_config,
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

        let start_time = Instant::now();
        let mut attempt = 0;
        let mut last_error = None;

        while self.retry_config.should_retry(attempt, start_time) {
            let mut stream = self.stream.lock().await;

            let result: Result<()> =
                async {
                    stream.write_all(bytes).await.map_err(|e| {
                        Error::TransportError(format!("TCP write error: {}", e).into())
                    })?;
                    stream.flush().await.map_err(|e| {
                        Error::TransportError(format!("TCP flush error: {}", e).into())
                    })?;
                    trace!("Sent {} bytes: {:02X?}", bytes.len(), bytes);
                    Ok(())
                }
                .await;

            match result {
                Ok(()) => return Ok(()),
                Err(e) if e.is_retryable() => {
                    attempt += 1;

                    if self.retry_config.should_retry(attempt, start_time) {
                        let delay = self
                            .retry_config
                            .calculate_delay(attempt, e.suggested_retry_delay());
                        debug!(
                            "Retrying send after {:?} (attempt {}/{})",
                            delay, attempt, self.retry_config.max_retries
                        );
                        tokio::time::sleep(delay).await;
                    }
                    last_error = Some(e);
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_error.unwrap_or(Error::Timeout))
    }

    async fn recv(&self) -> Result<Bytes> {
        let start_time = Instant::now();
        let mut attempt = 0;
        let mut last_error = None;

        while self.retry_config.should_retry(attempt, start_time) {
            match self.recv_frame().await {
                Ok(bytes) => return Ok(bytes),
                Err(e) if e.is_retryable() => {
                    attempt += 1;

                    if self.retry_config.should_retry(attempt, start_time) {
                        let delay = self
                            .retry_config
                            .calculate_delay(attempt, e.suggested_retry_delay());
                        debug!(
                            "Retrying recv after {:?} (attempt {}/{})",
                            delay, attempt, self.retry_config.max_retries
                        );
                        tokio::time::sleep(delay).await;
                    }
                    last_error = Some(e);
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_error.unwrap_or(Error::Timeout))
    }
}

/// Async raw UDP transport using tokio.
#[cfg(feature = "rt-tokio")]
#[derive(Debug)]
pub struct AsyncRawUdpTransport {
    socket: Arc<tokio::net::UdpSocket>,
    read_buffer: Arc<tokio::sync::Mutex<BytesMut>>,
    retry_config: RetryConfig,
    last_sent_command: Arc<tokio::sync::Mutex<Option<Vec<u8>>>>,
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
            retry_config: config.retry_config,
            last_sent_command: Arc::new(tokio::sync::Mutex::new(None)),
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
        let start_time = Instant::now();
        let mut attempt = 0;
        let mut last_error = None;

        // Store command for potential resend on receive timeout
        {
            let mut last_cmd = self.last_sent_command.lock().await;
            *last_cmd = Some(bytes.to_vec());
        }

        while self.retry_config.should_retry(attempt, start_time) {
            match self.socket.send(bytes).await {
                Ok(_) => {
                    trace!("Sent {} bytes: {:02X?}", bytes.len(), bytes);
                    return Ok(());
                }
                Err(e) => {
                    let error = Error::TransportError(format!("UDP send error: {}", e).into());
                    if error.is_retryable() {
                        attempt += 1;

                        if self.retry_config.should_retry(attempt, start_time) {
                            let delay = self
                                .retry_config
                                .calculate_delay(attempt, error.suggested_retry_delay());
                            debug!(
                                "Retrying UDP send after {:?} (attempt {}/{})",
                                delay, attempt, self.retry_config.max_retries
                            );
                            tokio::time::sleep(delay).await;
                        }
                        last_error = Some(error);
                    } else {
                        return Err(error);
                    }
                }
            }
        }

        Err(last_error.unwrap_or(Error::Timeout))
    }

    async fn recv(&self) -> Result<Bytes> {
        let start_time = Instant::now();
        let mut attempt = 0;
        let mut last_error = None;

        while self.retry_config.should_retry(attempt, start_time) {
            match self.recv_frame().await {
                Ok(bytes) => return Ok(bytes),
                Err(Error::Timeout) => {
                    // On timeout, resend the last command (handle packet loss)
                    if let Some(last_cmd) = &*self.last_sent_command.lock().await {
                        debug!("Receive timeout, resending last command");
                        if let Err(e) = self.socket.send(last_cmd).await {
                            warn!("Failed to resend command: {}", e);
                        }
                    }

                    attempt += 1;

                    if self.retry_config.should_retry(attempt, start_time) {
                        let delay = self
                            .retry_config
                            .calculate_delay(attempt, Some(Duration::from_millis(200)));
                        debug!(
                            "Retrying UDP recv after {:?} (attempt {}/{})",
                            delay, attempt, self.retry_config.max_retries
                        );
                        tokio::time::sleep(delay).await;
                    }
                    last_error = Some(Error::Timeout);
                }
                Err(e) if e.is_retryable() => {
                    attempt += 1;

                    if self.retry_config.should_retry(attempt, start_time) {
                        let delay = self
                            .retry_config
                            .calculate_delay(attempt, e.suggested_retry_delay());
                        debug!(
                            "Retrying UDP recv after {:?} (attempt {}/{})",
                            delay, attempt, self.retry_config.max_retries
                        );
                        tokio::time::sleep(delay).await;
                    }
                    last_error = Some(e);
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_error.unwrap_or(Error::Timeout))
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
