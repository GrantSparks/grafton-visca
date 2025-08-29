//! Transport builder pattern for flexible transport configuration.
//!
//! This module provides a unified builder API for creating and configuring
//! all transport types with a consistent interface.
//!
//! ## New Uniform API
//!
//! The library now provides a unified async transport API that automatically
//! selects the appropriate runtime implementation based on enabled features:
//!
//! ```rust,no_run
//! # #[cfg(all(feature = "async", any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")))]
//! use grafton_visca::transport::Transport;
//!
//! # #[cfg(all(feature = "async", any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")))]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Uniform API - runtime is automatically selected
//! let transport = Transport::tcp()
//!     .address("192.168.0.110:5678")
//!     .connect_timeout(std::time::Duration::from_secs(10))
//!     .tcp_nodelay(true)
//!     .connect()
//!     .await?;
//! # Ok(())
//! # }
//! ```

use std::time::Duration;

use crate::transport::buffer::BufferConfig;
#[cfg(not(feature = "async"))]
use crate::transport::BlockingTransport;
use crate::transport::RetryConfig;
#[cfg(any(
    not(feature = "async"),
    feature = "rt-tokio",
    feature = "rt-async-std",
    feature = "rt-smol"
))]
use crate::Error;

/// Common configuration options for all transport types.
#[derive(Debug, Clone, Copy)]
pub struct TransportConfig {
    /// Connection timeout duration.
    pub connect_timeout: Duration,
    /// Read timeout for receive operations.
    pub read_timeout: Duration,
    /// Write timeout for send operations.
    pub write_timeout: Duration,
    /// Retry configuration for operations.
    pub retry_config: RetryConfig,
    /// Buffer configuration for managing buffers.
    pub buffer_config: BufferConfig,
    /// Buffer size for buffered transports (e.g., TCP).
    pub buffer_size: Option<usize>,
    /// Whether to enable TCP nodelay (disable Nagle's algorithm).
    pub tcp_nodelay: Option<bool>,
    /// TTL (Time To Live) for packets.
    pub ttl: Option<u32>,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(5),
            write_timeout: Duration::from_secs(5),
            retry_config: RetryConfig::default(),
            buffer_config: BufferConfig::default(),
            buffer_size: None,
            tcp_nodelay: None,
            ttl: None,
        }
    }
}

/// Builder for creating configured transport instances.
///
/// This builder provides a fluent API for configuring transport parameters
/// before establishing a connection.
///
/// # Examples
///
/// ```rust,no_run
/// # #[cfg(not(feature = "async"))]
/// use grafton_visca::transport::builder::TransportBuilder;
/// # #[cfg(not(feature = "async"))]
/// use std::time::Duration;
///
/// # #[cfg(not(feature = "async"))]
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # #[cfg(not(feature = "async"))]
/// let transport = TransportBuilder::tcp()
///     .address("192.168.0.110:5678")
///     .connect_timeout(Duration::from_secs(10))
///     .read_timeout(Duration::from_secs(2))
///     .tcp_nodelay(true)
///     .max_retries(5)
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct TransportBuilder {
    #[cfg(not(feature = "async"))]
    transport_type: TransportType,
    address: Option<String>,
    config: TransportConfig,
}

/// The type of transport to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportType {
    /// TCP transport (blocking).
    #[cfg(not(feature = "async"))]
    Tcp,
    /// UDP transport (blocking).
    #[cfg(not(feature = "async"))]
    Udp,
}

impl TransportBuilder {
    /// Create a new builder for a TCP transport.
    #[cfg(not(feature = "async"))]
    pub fn tcp() -> Self {
        Self {
            transport_type: TransportType::Tcp,
            address: None,
            config: TransportConfig::default(),
        }
    }

    /// Create a new builder for a UDP transport.
    #[cfg(not(feature = "async"))]
    pub fn udp() -> Self {
        Self {
            transport_type: TransportType::Udp,
            address: None,
            config: TransportConfig::default(),
        }
    }

    /// Set the address to connect to.
    ///
    /// This can be a hostname with port (e.g., "camera.local:5678")
    /// or an IP address with port (e.g., "192.168.0.110:5678").
    pub fn address(mut self, address: impl Into<String>) -> Self {
        self.address = Some(address.into());
        self
    }

    /// Set the connection timeout.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.config.connect_timeout = timeout;
        self
    }

    /// Set the read timeout for receive operations.
    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        self.config.read_timeout = timeout;
        self
    }

    /// Set the write timeout for send operations.
    pub fn write_timeout(mut self, timeout: Duration) -> Self {
        self.config.write_timeout = timeout;
        self
    }

    /// Set all timeouts to the same value.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.connect_timeout = timeout;
        self.config.read_timeout = timeout;
        self.config.write_timeout = timeout;
        self
    }

    /// Set the retry configuration.
    pub fn retry_config(mut self, config: RetryConfig) -> Self {
        self.config.retry_config = config;
        self
    }

    /// Set the maximum number of retries.
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.config.retry_config.max_retries = max_retries;
        self
    }

    /// Set the base retry delay.
    pub fn retry_delay(mut self, delay: Duration) -> Self {
        self.config.retry_config.base_retry_delay = delay;
        self
    }

    /// Set the maximum retry duration.
    pub fn max_retry_duration(mut self, duration: Duration) -> Self {
        self.config.retry_config.max_retry_duration = duration;
        self
    }

    /// Enable or disable exponential backoff for retries.
    pub fn exponential_backoff(mut self, enabled: bool) -> Self {
        self.config.retry_config.exponential_backoff = enabled;
        self
    }

    /// Set the buffer size for buffered transports (TCP).
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_size = Some(size);
        // Also update the buffer config
        self.config.buffer_config.recv_buffer_size = size;
        self.config.buffer_config.send_buffer_size = size;
        self
    }

    /// Set the receive buffer size.
    pub fn recv_buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_config.recv_buffer_size = size;
        self
    }

    /// Set the send buffer size.
    pub fn send_buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_config.send_buffer_size = size;
        self
    }

    /// Set the maximum buffer size to prevent unbounded growth.
    pub fn max_buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_config.max_buffer_size = size;
        self
    }

    /// Use optimized buffer configuration for UDP transports.
    pub fn udp_buffers(mut self) -> Self {
        self.config.buffer_config = BufferConfig::for_udp();
        self
    }

    /// Use optimized buffer configuration for Sony IP protocol.
    pub fn sony_ip_buffers(mut self) -> Self {
        self.config.buffer_config = BufferConfig::for_sony_ip();
        self
    }

    /// Use optimized buffer configuration for raw IP protocol.
    pub fn raw_ip_buffers(mut self) -> Self {
        self.config.buffer_config = BufferConfig::for_raw_ip();
        self
    }

    /// Enable or disable TCP nodelay (Nagle's algorithm).
    ///
    /// This option only affects TCP transports.
    pub fn tcp_nodelay(mut self, enabled: bool) -> Self {
        self.config.tcp_nodelay = Some(enabled);
        self
    }

    /// Set the TTL (Time To Live) for packets.
    pub fn ttl(mut self, ttl: u32) -> Self {
        self.config.ttl = Some(ttl);
        self
    }

    /// Build the blocking transport.
    ///
    /// This method establishes the connection and returns a configured transport instance.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No address has been set
    /// - Connection fails
    /// - Socket configuration fails
    #[cfg(not(feature = "async"))]
    pub fn build(self) -> Result<Box<dyn BlockingTransport>, Error> {
        let address = self.address.ok_or_else(|| Error::InvalidParameter {
            parameter: "address",
            value: "None".into(),
            reason: "No address specified for transport".into(),
        })?;

        match self.transport_type {
            TransportType::Tcp => {
                // Use connect_with_config to apply all settings at once
                let transport =
                    crate::transport::blocking::Tcp::connect_with_config(&address, self.config)?;
                Ok(Box::new(transport))
            }
            TransportType::Udp => {
                // Use connect_with_config to apply all settings at once
                let transport =
                    crate::transport::blocking::Udp::connect_with_config(&address, self.config)?;
                Ok(Box::new(transport))
            }
        }
    }
}

/// Unified transport wrapper that can hold any transport type.
///
/// This enum allows the uniform transport API to return different concrete
/// transport types while maintaining type safety and avoiding trait objects.
///
/// This type is not exported from the public API of the library.
/// Use `BoxAsyncTransport` for stable dynamic transport handles.
#[derive(Debug)]
#[cfg(all(
    feature = "async",
    any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")
))]
pub enum UnifiedTransport {
    /// Tokio TCP transport.
    #[cfg(feature = "rt-tokio")]
    TokioTcp(crate::runtime_adapters::tokio::TcpTransport),
    /// Tokio UDP transport.
    #[cfg(feature = "rt-tokio")]
    TokioUdp(crate::runtime_adapters::tokio::UdpTransport),
    /// async-std TCP transport.
    #[cfg(feature = "rt-async-std")]
    AsyncStdTcp(crate::runtime_adapters::async_std::TcpTransport),
    /// async-std UDP transport.
    #[cfg(feature = "rt-async-std")]
    AsyncStdUdp(crate::runtime_adapters::async_std::UdpTransport),
    /// smol TCP transport.
    #[cfg(feature = "rt-smol")]
    SmolTcp(crate::runtime_adapters::smol::TcpTransport),
    /// smol UDP transport.
    #[cfg(feature = "rt-smol")]
    SmolUdp(crate::runtime_adapters::smol::UdpTransport),
}

#[cfg(all(
    feature = "async",
    any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")
))]
impl crate::transport::AsyncTransport for UnifiedTransport {
    async fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
        match self {
            #[cfg(feature = "rt-tokio")]
            UnifiedTransport::TokioTcp(transport) => transport.send(bytes).await,
            #[cfg(feature = "rt-tokio")]
            UnifiedTransport::TokioUdp(transport) => transport.send(bytes).await,
            #[cfg(feature = "rt-async-std")]
            UnifiedTransport::AsyncStdTcp(transport) => transport.send(bytes).await,
            #[cfg(feature = "rt-async-std")]
            UnifiedTransport::AsyncStdUdp(transport) => transport.send(bytes).await,
            #[cfg(feature = "rt-smol")]
            UnifiedTransport::SmolTcp(transport) => transport.send(bytes).await,
            #[cfg(feature = "rt-smol")]
            UnifiedTransport::SmolUdp(transport) => transport.send(bytes).await,
        }
    }

    async fn recv(&mut self) -> Result<bytes::Bytes, Error> {
        match self {
            #[cfg(feature = "rt-tokio")]
            UnifiedTransport::TokioTcp(transport) => transport.recv().await,
            #[cfg(feature = "rt-tokio")]
            UnifiedTransport::TokioUdp(transport) => transport.recv().await,
            #[cfg(feature = "rt-async-std")]
            UnifiedTransport::AsyncStdTcp(transport) => transport.recv().await,
            #[cfg(feature = "rt-async-std")]
            UnifiedTransport::AsyncStdUdp(transport) => transport.recv().await,
            #[cfg(feature = "rt-smol")]
            UnifiedTransport::SmolTcp(transport) => transport.recv().await,
            #[cfg(feature = "rt-smol")]
            UnifiedTransport::SmolUdp(transport) => transport.recv().await,
        }
    }
}

/// Uniform transport builder that automatically selects the runtime implementation.
///
/// This provides a clean API where users don't need to specify the runtime
/// (tokio, async-std, smol) - the library picks the right one based on enabled features.
#[derive(Debug, Clone)]
#[cfg(all(
    feature = "async",
    any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")
))]
pub struct UniformTransportBuilder {
    protocol: Protocol,
    address: Option<String>,
    config: TransportConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(all(
    feature = "async",
    any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")
))]
enum Protocol {
    Tcp,
    Udp,
}

#[cfg(all(
    feature = "async",
    any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")
))]
impl UniformTransportBuilder {
    /// Create a new TCP transport builder.
    fn new_tcp() -> Self {
        Self {
            protocol: Protocol::Tcp,
            address: None,
            config: TransportConfig::default(),
        }
    }

    /// Create a new UDP transport builder.
    fn new_udp() -> Self {
        Self {
            protocol: Protocol::Udp,
            address: None,
            config: TransportConfig::default(),
        }
    }

    /// Set the address to connect to.
    pub fn address(mut self, address: impl Into<String>) -> Self {
        self.address = Some(address.into());
        self
    }

    /// Set the connection timeout.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.config.connect_timeout = timeout;
        self
    }

    /// Set the read timeout for receive operations.
    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        self.config.read_timeout = timeout;
        self
    }

    /// Set the write timeout for send operations.
    pub fn write_timeout(mut self, timeout: Duration) -> Self {
        self.config.write_timeout = timeout;
        self
    }

    /// Set all timeouts to the same value.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.connect_timeout = timeout;
        self.config.read_timeout = timeout;
        self.config.write_timeout = timeout;
        self
    }

    /// Set the retry configuration.
    pub fn retry_config(mut self, config: RetryConfig) -> Self {
        self.config.retry_config = config;
        self
    }

    /// Set the maximum number of retries.
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.config.retry_config.max_retries = max_retries;
        self
    }

    /// Set the base retry delay.
    pub fn retry_delay(mut self, delay: Duration) -> Self {
        self.config.retry_config.base_retry_delay = delay;
        self
    }

    /// Set the maximum retry duration.
    pub fn max_retry_duration(mut self, duration: Duration) -> Self {
        self.config.retry_config.max_retry_duration = duration;
        self
    }

    /// Enable or disable exponential backoff for retries.
    pub fn exponential_backoff(mut self, enabled: bool) -> Self {
        self.config.retry_config.exponential_backoff = enabled;
        self
    }

    /// Set the buffer size for buffered transports (TCP).
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_size = Some(size);
        self.config.buffer_config.recv_buffer_size = size;
        self.config.buffer_config.send_buffer_size = size;
        self
    }

    /// Set the receive buffer size.
    pub fn recv_buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_config.recv_buffer_size = size;
        self
    }

    /// Set the send buffer size.
    pub fn send_buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_config.send_buffer_size = size;
        self
    }

    /// Set the maximum buffer size to prevent unbounded growth.
    pub fn max_buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_config.max_buffer_size = size;
        self
    }

    /// Use optimized buffer configuration for UDP transports.
    pub fn udp_buffers(mut self) -> Self {
        self.config.buffer_config = BufferConfig::for_udp();
        self
    }

    /// Use optimized buffer configuration for Sony IP protocol.
    pub fn sony_ip_buffers(mut self) -> Self {
        self.config.buffer_config = BufferConfig::for_sony_ip();
        self
    }

    /// Use optimized buffer configuration for raw IP protocol.
    pub fn raw_ip_buffers(mut self) -> Self {
        self.config.buffer_config = BufferConfig::for_raw_ip();
        self
    }

    /// Enable or disable TCP nodelay (Nagle's algorithm).
    /// This option only affects TCP transports.
    pub fn tcp_nodelay(mut self, enabled: bool) -> Self {
        self.config.tcp_nodelay = Some(enabled);
        self
    }

    /// Set the TTL (Time To Live) for packets.
    pub fn ttl(mut self, ttl: u32) -> Self {
        self.config.ttl = Some(ttl);
        self
    }

    /// Connect to the transport.
    ///
    /// This method automatically selects the appropriate runtime implementation
    /// based on enabled features and establishes the connection.
    ///
    /// # Priority Order
    ///
    /// When multiple runtime features are enabled, the selection priority is:
    /// 1. `rt-tokio` - Most common, well-tested
    /// 2. `rt-async-std` - Alternative async runtime
    /// 3. `rt-smol` - Lightweight runtime
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No address has been set
    /// - No async runtime features are enabled
    /// - Connection fails
    /// - Socket configuration fails
    #[cfg(any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol"))]
    #[allow(unreachable_code)]
    pub async fn connect(self) -> Result<UnifiedTransport, Error> {
        let address = self.address.ok_or_else(|| Error::InvalidParameter {
            parameter: "address",
            value: "None".into(),
            reason: "No address specified for transport".into(),
        })?;

        // Priority-based runtime selection when multiple runtimes are available
        #[cfg(feature = "rt-tokio")]
        {
            return match self.protocol {
                Protocol::Tcp => {
                    let transport =
                        crate::runtime_adapters::tokio::TcpTransport::connect_with_config(
                            &address,
                            self.config,
                        )
                        .await?;
                    Ok(UnifiedTransport::TokioTcp(transport))
                }
                Protocol::Udp => {
                    let transport =
                        crate::runtime_adapters::tokio::UdpTransport::connect_with_config(
                            &address,
                            self.config,
                        )
                        .await?;
                    Ok(UnifiedTransport::TokioUdp(transport))
                }
            };
        }

        #[cfg(feature = "rt-async-std")]
        #[cfg(not(feature = "rt-tokio"))]
        {
            return match self.protocol {
                Protocol::Tcp => {
                    let transport =
                        crate::runtime_adapters::async_std::TcpTransport::connect_with_config(
                            &address,
                            self.config,
                        )
                        .await?;
                    Ok(UnifiedTransport::AsyncStdTcp(transport))
                }
                Protocol::Udp => {
                    let transport =
                        crate::runtime_adapters::async_std::UdpTransport::connect_with_config(
                            &address,
                            self.config,
                        )
                        .await?;
                    Ok(UnifiedTransport::AsyncStdUdp(transport))
                }
            };
        }

        #[cfg(feature = "rt-smol")]
        #[cfg(not(feature = "rt-tokio"))]
        #[cfg(not(feature = "rt-async-std"))]
        {
            return match self.protocol {
                Protocol::Tcp => {
                    let transport =
                        crate::runtime_adapters::smol::TcpTransport::connect_with_config(
                            &address,
                            self.config,
                        )
                        .await?;
                    Ok(UnifiedTransport::SmolTcp(transport))
                }
                Protocol::Udp => {
                    let transport =
                        crate::runtime_adapters::smol::UdpTransport::connect_with_config(
                            &address,
                            self.config,
                        )
                        .await?;
                    Ok(UnifiedTransport::SmolUdp(transport))
                }
            };
        }

        // This should never be reached due to the cfg guard at the function level
        unreachable!("No runtime available - this should be prevented by cfg guard")
    }

    /// Connect to the transport with automatic protocol detection.
    ///
    /// This method implements EPIC task B3: automatic detection of Sony encapsulated
    /// vs raw VISCA protocol modes. It probes the camera with both formats and
    /// returns a transport configured for the detected protocol.
    ///
    /// # Protocol Detection Process
    ///
    /// 1. Try Sony encapsulated format first (8-byte header)
    /// 2. If no response, fallback to raw VISCA format
    /// 3. If neither works, return error
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No address has been set
    /// - No async runtime features are enabled
    /// - Connection fails
    /// - Socket configuration fails
    /// - No protocol response detected from camera
    #[cfg(any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol"))]
    #[allow(unreachable_code)]
    pub async fn connect_with_auto_detection(
        self,
    ) -> Result<(UnifiedTransport, super::DetectionResult), Error> {
        let address = self
            .address
            .clone()
            .ok_or_else(|| Error::InvalidParameter {
                parameter: "address",
                value: "None".into(),
                reason: "No address specified for transport".into(),
            })?;

        // First establish the connection
        let mut transport = self.connect().await?;

        // Perform protocol detection
        let detector = super::ProtocolDetector::new();
        let detection_result = detector.detect_protocol(&mut transport).await?;

        match detection_result {
            super::DetectionResult::SonyEncapsulated | super::DetectionResult::RawVisca => {
                Ok((transport, detection_result))
            }
            super::DetectionResult::NoResponse => {
                Err(Error::ConnectionFailed {
                    addr: address.into(),
                    source: std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "No VISCA protocol response detected from camera - verify camera is powered on and address is correct"
                    ),
                })
            }
        }
    }

    /// Connect to the transport and return a stable dynamic handle.
    ///
    /// Unlike [`connect()`](Self::connect), this method returns a [`BoxAsyncTransport`](super::BoxAsyncTransport)
    /// which provides a stable type across feature configurations. The returned handle
    /// uses heap allocation for each send/recv operation, making it unsuitable for
    /// performance-critical code paths.
    ///
    /// This method selects the enabled runtime implementation and wraps it in
    /// a boxed dynamic transport handle. Use this for plugin architectures,
    /// dependency injection, or when you need a stable transport type.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No address has been set
    /// - No async runtime features are enabled
    /// - Connection fails
    /// - Socket configuration fails
    #[cfg(any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol"))]
    #[allow(unreachable_code)]
    pub async fn connect_dyn(self) -> Result<super::BoxAsyncTransport, Error> {
        let address = self.address.ok_or_else(|| Error::InvalidParameter {
            parameter: "address",
            value: "None".into(),
            reason: "No address specified for transport".into(),
        })?;

        // Priority-based runtime selection when multiple runtimes are available
        #[cfg(feature = "rt-tokio")]
        {
            return match self.protocol {
                Protocol::Tcp => {
                    let transport =
                        crate::runtime_adapters::tokio::TcpTransport::connect_with_config(
                            &address,
                            self.config,
                        )
                        .await?;
                    Ok(Box::new(transport))
                }
                Protocol::Udp => {
                    let transport =
                        crate::runtime_adapters::tokio::UdpTransport::connect_with_config(
                            &address,
                            self.config,
                        )
                        .await?;
                    Ok(Box::new(transport))
                }
            };
        }

        #[cfg(feature = "rt-async-std")]
        #[cfg(not(feature = "rt-tokio"))]
        {
            return match self.protocol {
                Protocol::Tcp => {
                    let transport =
                        crate::runtime_adapters::async_std::TcpTransport::connect_with_config(
                            &address,
                            self.config,
                        )
                        .await?;
                    Ok(Box::new(transport))
                }
                Protocol::Udp => {
                    let transport =
                        crate::runtime_adapters::async_std::UdpTransport::connect_with_config(
                            &address,
                            self.config,
                        )
                        .await?;
                    Ok(Box::new(transport))
                }
            };
        }

        #[cfg(feature = "rt-smol")]
        #[cfg(not(feature = "rt-tokio"))]
        #[cfg(not(feature = "rt-async-std"))]
        {
            return match self.protocol {
                Protocol::Tcp => {
                    let transport =
                        crate::runtime_adapters::smol::TcpTransport::connect_with_config(
                            &address,
                            self.config,
                        )
                        .await?;
                    Ok(Box::new(transport))
                }
                Protocol::Udp => {
                    let transport =
                        crate::runtime_adapters::smol::UdpTransport::connect_with_config(
                            &address,
                            self.config,
                        )
                        .await?;
                    Ok(Box::new(transport))
                }
            };
        }

        // This should never be reached due to the cfg guard at the function level
        unreachable!("No runtime available - this should be prevented by cfg guard")
    }
}

/// Uniform transport API.
///
/// Provides a clean interface for creating transports without exposing runtime details.
/// The runtime implementation is automatically selected based on enabled features.
#[derive(Debug, Copy, Clone)]
#[cfg(all(
    feature = "async",
    any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")
))]
pub struct Transport;

#[cfg(all(
    feature = "async",
    any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")
))]
impl Transport {
    /// Create a TCP transport builder.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "async", any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")))]
    /// use grafton_visca::transport::Transport;
    ///
    /// # #[cfg(all(feature = "async", any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")))]
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let transport = Transport::tcp()
    ///     .address("192.168.0.110:5678")
    ///     .tcp_nodelay(true)
    ///     .connect()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn tcp() -> UniformTransportBuilder {
        UniformTransportBuilder::new_tcp()
    }

    /// Create a UDP transport builder.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "async", any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")))]
    /// use grafton_visca::transport::Transport;
    ///
    /// # #[cfg(all(feature = "async", any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")))]
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let transport = Transport::udp()
    ///     .address("192.168.0.110:5678")
    ///     .max_retries(5)
    ///     .connect()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn udp() -> UniformTransportBuilder {
        UniformTransportBuilder::new_udp()
    }

    /// Connect to a camera with automatic protocol detection (EPIC task B3).
    ///
    /// This is a convenience method that automatically detects whether the camera
    /// uses Sony encapsulated format (8-byte header) or raw VISCA format.
    ///
    /// Defaults to TCP transport on the provided address.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "async", any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")))]
    /// use grafton_visca::transport::Transport;
    ///
    /// # #[cfg(all(feature = "async", any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")))]
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Auto-detect protocol for camera (could be Sony or PTZOptics)
    /// let (transport, detected_protocol) = Transport::auto_detect("192.168.0.110:5678").await?;
    /// println!("Detected protocol: {:?}", detected_protocol);
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol"))]
    pub async fn auto_detect(
        address: impl Into<String>,
    ) -> Result<(UnifiedTransport, super::DetectionResult), Error> {
        Self::tcp()
            .address(address)
            .connect_with_auto_detection()
            .await
    }
}

/// Extension trait for creating transports with a builder pattern.
pub trait TransportBuilderExt: Sized {
    /// Create a builder for this transport type.
    fn builder() -> TransportBuilder;
}

#[cfg(not(feature = "async"))]
impl TransportBuilderExt for crate::transport::blocking::Tcp {
    fn builder() -> TransportBuilder {
        TransportBuilder::tcp()
    }
}

#[cfg(not(feature = "async"))]
impl TransportBuilderExt for crate::transport::blocking::Udp {
    fn builder() -> TransportBuilder {
        TransportBuilder::udp()
    }
}

#[cfg(all(test, not(feature = "async")))]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_default_config() {
        let builder = TransportBuilder::tcp();
        assert_eq!(builder.transport_type, TransportType::Tcp);
        assert_eq!(builder.config.connect_timeout, Duration::from_secs(5));
        assert_eq!(builder.config.retry_config.max_retries, 3);
    }

    #[test]
    fn test_builder_fluent_api() {
        let builder = TransportBuilder::udp()
            .address("192.168.0.110:5678")
            .connect_timeout(Duration::from_secs(10))
            .max_retries(5)
            .tcp_nodelay(true);

        assert_eq!(builder.transport_type, TransportType::Udp);
        assert_eq!(builder.address, Some("192.168.0.110:5678".to_string()));
        assert_eq!(builder.config.connect_timeout, Duration::from_secs(10));
        assert_eq!(builder.config.retry_config.max_retries, 5);
        assert_eq!(builder.config.tcp_nodelay, Some(true));
    }

    #[test]
    fn test_builder_timeout_convenience() {
        let builder = TransportBuilder::tcp().timeout(Duration::from_secs(3));

        assert_eq!(builder.config.connect_timeout, Duration::from_secs(3));
        assert_eq!(builder.config.read_timeout, Duration::from_secs(3));
        assert_eq!(builder.config.write_timeout, Duration::from_secs(3));
    }

    #[test]
    fn test_builder_requires_address() {
        let builder = TransportBuilder::tcp();
        let result = builder.build();
        assert!(result.is_err());

        // Check that we get the correct error type
        let is_correct_error = matches!(
            result,
            Err(Error::InvalidParameter {
                parameter,
                reason,
                ..
            }) if parameter == "address" && reason.contains("No address specified")
        );
        assert!(
            is_correct_error,
            "Expected InvalidParameter error with address parameter"
        );
    }

    #[test]
    fn test_retry_config_builder() {
        let builder = TransportBuilder::tcp()
            .max_retries(10)
            .retry_delay(Duration::from_millis(500))
            .max_retry_duration(Duration::from_secs(30))
            .exponential_backoff(false);

        assert_eq!(builder.config.retry_config.max_retries, 10);
        assert_eq!(
            builder.config.retry_config.base_retry_delay,
            Duration::from_millis(500)
        );
        assert_eq!(
            builder.config.retry_config.max_retry_duration,
            Duration::from_secs(30)
        );
        assert!(!builder.config.retry_config.exponential_backoff);
    }
}
