//! Raw VISCA over IP transport (PtzOptics style).
//!
//! This module provides raw VISCA communication over TCP/UDP without
//! any additional encapsulation. This is the format used by PtzOptics cameras.

use bytes::{Bytes, BytesMut};
use log::{debug, trace};

use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::error::{Error, Result};
use crate::protocol::encode::VISCA_TERMINATOR;
use crate::transport::address::AddressResolver;
use crate::transport::buffer::{BufferConfig, BufferManager};
use crate::transport::retry::RetryExecutor;
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
    buffer_manager: Arc<BufferManager>,
    read_buffer: Arc<Mutex<BytesMut>>,
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

        // Create buffer manager with raw IP optimized sizes
        let buffer_manager = Arc::new(BufferManager::new(BufferConfig::for_raw_ip()));

        // Create retry executor with the configured retry settings
        let retry_executor = RetryExecutor::new(config.retry_config);

        Ok(Self {
            stream: Arc::new(Mutex::new(stream)),
            buffer_manager: buffer_manager.clone(),
            read_buffer: Arc::new(Mutex::new(buffer_manager.alloc_recv_buffer())),
            retry_executor,
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
        let mut temp_buf = self.buffer_manager.alloc_vec_buffer();

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
}

impl BlockingTransport for RawTcpTransport {
    fn send_blocking(&self, bytes: &[u8]) -> Result<()> {
        // Clone bytes for the closure
        let bytes_vec = bytes.to_vec();

        // Use the retry executor for automatic retry handling
        self.retry_executor.execute(|| {
            let mut stream = self
                .stream
                .lock()
                .map_err(|_| Error::LockPoisoned("transport mutex"))?;

            stream
                .write_all(&bytes_vec)
                .and_then(|_| stream.flush())
                .map_err(|e| Error::TransportError(format!("TCP write error: {e}").into()))?;

            trace!("Sent {} bytes: {:02X?}", bytes_vec.len(), bytes_vec);
            Ok(())
        })
    }

    fn recv_blocking(&self) -> Result<Bytes> {
        // Don't retry receive operations to avoid protocol confusion
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
            .map_err(|e| Error::TransportError(format!("Failed to get timeout: {e}").into()))?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|e| Error::TransportError(format!("Failed to set timeout: {e}").into()))?;
        drop(stream);

        let result = self.recv_frame();

        // Restore original timeout
        let stream = self
            .stream
            .lock()
            .map_err(|_| Error::LockPoisoned("transport mutex"))?;
        stream
            .set_read_timeout(original_read_timeout)
            .map_err(|e| Error::TransportError(format!("Failed to restore timeout: {e}").into()))?;

        result
    }
}

/// Raw UDP transport for blocking I/O.
#[derive(Debug)]
pub struct RawUdpTransport {
    socket: Arc<UdpSocket>,
    buffer_manager: Arc<BufferManager>,
    read_buffer: Arc<Mutex<BytesMut>>,
    retry_executor: RetryExecutor,
    config: RawIpConfig,
    /// Track last sent command for retry on timeout
    last_command: Arc<Mutex<Option<Vec<u8>>>>,
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
        let buffer_manager = Arc::new(BufferManager::new(BufferConfig::for_udp()));

        // Create retry executor with the configured retry settings
        let retry_executor = RetryExecutor::new(config.retry_config);

        Ok(Self {
            socket: Arc::new(socket),
            buffer_manager: buffer_manager.clone(),
            read_buffer: Arc::new(Mutex::new(buffer_manager.alloc_recv_buffer())),
            retry_executor,
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
        let mut temp_buf = self.buffer_manager.alloc_vec_buffer();

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

        // Clone bytes for the closure
        let bytes_vec = bytes.to_vec();

        // Use the retry executor for automatic retry handling
        self.retry_executor.execute(|| {
            self.socket
                .send(&bytes_vec)
                .map_err(|e| Error::TransportError(format!("UDP send error: {e}").into()))?;

            trace!("Sent {} bytes: {:02X?}", bytes_vec.len(), bytes_vec);
            Ok(())
        })
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

    fn recv_blocking_with_timeout(&self, timeout: Duration) -> Result<Bytes> {
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

/// Async raw TCP transport using AsyncWrapper.
///
/// This transport wraps the blocking RawTcpTransport to provide async operations
/// while reusing all the buffer management, retry logic, and error handling
/// from the blocking implementation.
#[cfg(feature = "async")]
#[derive(Clone, Debug)]
pub struct AsyncRawTcpTransport {
    inner: Arc<RawTcpTransport>,
}

#[cfg(feature = "async")]
impl AsyncRawTcpTransport {
    /// Connect to a camera via raw TCP.
    ///
    /// Creates a blocking transport and wraps it for async usage.
    pub async fn connect(config: RawIpConfig) -> Result<Self> {
        // Create blocking transport in a blocking task
        #[cfg(feature = "rt-tokio")]
        let transport = tokio::task::spawn_blocking(move || RawTcpTransport::connect(config))
            .await
            .map_err(|e| {
                Error::TransportError(format!("Failed to spawn blocking task: {e}").into())
            })??;

        #[cfg(all(feature = "rt-async-std", not(feature = "rt-tokio")))]
        let transport =
            async_std::task::spawn_blocking(move || RawTcpTransport::connect(config)).await?;

        #[cfg(all(
            feature = "rt-smol",
            not(feature = "rt-tokio"),
            not(feature = "rt-async-std")
        ))]
        let transport = smol::unblock(move || RawTcpTransport::connect(config)).await?;

        #[cfg(all(
            feature = "async",
            not(feature = "rt-tokio"),
            not(feature = "rt-async-std"),
            not(feature = "rt-smol")
        ))]
        let transport = std::thread::spawn(move || RawTcpTransport::connect(config))
            .join()
            .map_err(|_| Error::Io(std::io::Error::other("Thread panicked")))??;

        Ok(Self {
            inner: Arc::new(transport),
        })
    }
}

#[cfg(feature = "async")]
impl crate::transport::AsyncTransport for AsyncRawTcpTransport {
    async fn send(&self, bytes: &[u8]) -> Result<()> {
        let inner = self.inner.clone();
        let bytes = bytes.to_vec();

        // Use spawn_blocking to run the blocking send in a thread pool
        #[cfg(feature = "rt-tokio")]
        {
            tokio::task::spawn_blocking(move || inner.send_blocking(&bytes))
                .await
                .map_err(|e| Error::TransportError(format!("Async send error: {e}").into()))?
        }

        #[cfg(all(feature = "rt-async-std", not(feature = "rt-tokio")))]
        {
            async_std::task::spawn_blocking(move || inner.send_blocking(&bytes)).await
        }

        #[cfg(all(
            feature = "rt-smol",
            not(feature = "rt-tokio"),
            not(feature = "rt-async-std")
        ))]
        {
            smol::unblock(move || inner.send_blocking(&bytes)).await
        }

        #[cfg(all(
            feature = "async",
            not(feature = "rt-tokio"),
            not(feature = "rt-async-std"),
            not(feature = "rt-smol")
        ))]
        {
            std::thread::spawn(move || inner.send_blocking(&bytes))
                .join()
                .map_err(|_| Error::Io(std::io::Error::other("Thread panicked")))?
        }
    }

    async fn recv(&self) -> Result<Bytes> {
        let inner = self.inner.clone();

        // Use spawn_blocking to run the blocking recv in a thread pool
        #[cfg(feature = "rt-tokio")]
        {
            tokio::task::spawn_blocking(move || inner.recv_blocking())
                .await
                .map_err(|e| Error::TransportError(format!("Async recv error: {e}").into()))?
        }

        #[cfg(all(feature = "rt-async-std", not(feature = "rt-tokio")))]
        {
            async_std::task::spawn_blocking(move || inner.recv_blocking()).await
        }

        #[cfg(all(
            feature = "rt-smol",
            not(feature = "rt-tokio"),
            not(feature = "rt-async-std")
        ))]
        {
            smol::unblock(move || inner.recv_blocking()).await
        }

        #[cfg(all(
            feature = "async",
            not(feature = "rt-tokio"),
            not(feature = "rt-async-std"),
            not(feature = "rt-smol")
        ))]
        {
            std::thread::spawn(move || inner.recv_blocking())
                .join()
                .map_err(|_| Error::Io(std::io::Error::other("Thread panicked")))?
        }
    }
}

/// Async raw UDP transport using AsyncWrapper.
///
/// This transport wraps the blocking RawUdpTransport to provide async operations
/// while reusing all the buffer management, retry logic, packet loss handling,
/// and error handling from the blocking implementation.
#[cfg(feature = "async")]
#[derive(Clone, Debug)]
pub struct AsyncRawUdpTransport {
    inner: Arc<RawUdpTransport>,
}

#[cfg(feature = "async")]
impl AsyncRawUdpTransport {
    /// Connect to a camera via raw UDP.
    ///
    /// Creates a blocking transport and wraps it for async usage.
    /// The blocking transport handles all UDP-specific concerns like
    /// packet loss and automatic resending.
    pub async fn connect(config: RawIpConfig) -> Result<Self> {
        // Create blocking transport in a blocking task
        #[cfg(feature = "rt-tokio")]
        let transport = tokio::task::spawn_blocking(move || RawUdpTransport::connect(config))
            .await
            .map_err(|e| {
                Error::TransportError(format!("Failed to spawn blocking task: {e}").into())
            })??;

        #[cfg(all(feature = "rt-async-std", not(feature = "rt-tokio")))]
        let transport =
            async_std::task::spawn_blocking(move || RawUdpTransport::connect(config)).await?;

        #[cfg(all(
            feature = "rt-smol",
            not(feature = "rt-tokio"),
            not(feature = "rt-async-std")
        ))]
        let transport = smol::unblock(move || RawUdpTransport::connect(config)).await?;

        #[cfg(all(
            feature = "async",
            not(feature = "rt-tokio"),
            not(feature = "rt-async-std"),
            not(feature = "rt-smol")
        ))]
        let transport = std::thread::spawn(move || RawUdpTransport::connect(config))
            .join()
            .map_err(|_| Error::Io(std::io::Error::other("Thread panicked")))??;

        Ok(Self {
            inner: Arc::new(transport),
        })
    }
}

#[cfg(feature = "async")]
impl crate::transport::AsyncTransport for AsyncRawUdpTransport {
    async fn send(&self, bytes: &[u8]) -> Result<()> {
        let inner = self.inner.clone();
        let bytes = bytes.to_vec();

        // Use spawn_blocking to run the blocking send in a thread pool
        // The blocking implementation handles all UDP-specific concerns
        #[cfg(feature = "rt-tokio")]
        {
            tokio::task::spawn_blocking(move || inner.send_blocking(&bytes))
                .await
                .map_err(|e| Error::TransportError(format!("Async send error: {e}").into()))?
        }

        #[cfg(all(feature = "rt-async-std", not(feature = "rt-tokio")))]
        {
            async_std::task::spawn_blocking(move || inner.send_blocking(&bytes)).await
        }

        #[cfg(all(
            feature = "rt-smol",
            not(feature = "rt-tokio"),
            not(feature = "rt-async-std")
        ))]
        {
            smol::unblock(move || inner.send_blocking(&bytes)).await
        }

        #[cfg(all(
            feature = "async",
            not(feature = "rt-tokio"),
            not(feature = "rt-async-std"),
            not(feature = "rt-smol")
        ))]
        {
            std::thread::spawn(move || inner.send_blocking(&bytes))
                .join()
                .map_err(|_| Error::Io(std::io::Error::other("Thread panicked")))?
        }
    }

    async fn recv(&self) -> Result<Bytes> {
        let inner = self.inner.clone();

        // Use spawn_blocking to run the blocking recv in a thread pool
        // The blocking implementation handles packet loss and resending
        #[cfg(feature = "rt-tokio")]
        {
            tokio::task::spawn_blocking(move || inner.recv_blocking())
                .await
                .map_err(|e| Error::TransportError(format!("Async recv error: {e}").into()))?
        }

        #[cfg(all(feature = "rt-async-std", not(feature = "rt-tokio")))]
        {
            async_std::task::spawn_blocking(move || inner.recv_blocking()).await
        }

        #[cfg(all(
            feature = "rt-smol",
            not(feature = "rt-tokio"),
            not(feature = "rt-async-std")
        ))]
        {
            smol::unblock(move || inner.recv_blocking()).await
        }

        #[cfg(all(
            feature = "async",
            not(feature = "rt-tokio"),
            not(feature = "rt-async-std"),
            not(feature = "rt-smol")
        ))]
        {
            std::thread::spawn(move || inner.recv_blocking())
                .join()
                .map_err(|_| Error::Io(std::io::Error::other("Thread panicked")))?
        }
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
