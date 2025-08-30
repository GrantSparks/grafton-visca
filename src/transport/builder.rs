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
//!     .build_async()
//!     .await?;
//! # Ok(())
//! # }
//! ```

use std::time::Duration;

#[cfg(not(feature = "async"))]
use crate::transport::SyncTransport;
use crate::transport::{buffer::BufferConfig, RetryConfig};
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
    pub fn build(self) -> Result<Box<dyn SyncTransport>, Error> {
        let address = self.address.ok_or_else(|| Error::InvalidParameter {
            parameter: "address",
            value: "None".into(),
            reason: "No address specified for transport".into(),
        })?;

        match self.transport_type {
            TransportType::Tcp => {
                // Use connect_with_config to apply all settings at once
                #[cfg(not(feature = "async"))]
                let transport =
                    crate::transport::blocking::Tcp::connect_with_config(&address, self.config)?;
                #[cfg(feature = "async")]
                let transport = {
                    return Err(crate::Error::InvalidState(
                        "Blocking transport not available in async mode".into(),
                    ));
                };
                Ok(Box::new(transport))
            }
            TransportType::Udp => {
                // Use connect_with_config to apply all settings at once
                #[cfg(not(feature = "async"))]
                let transport =
                    crate::transport::blocking::Udp::connect_with_config(&address, self.config)?;
                #[cfg(feature = "async")]
                let transport = {
                    return Err(crate::Error::InvalidState(
                        "Blocking transport not available in async mode".into(),
                    ));
                };
                Ok(Box::new(transport))
            }
        }
    }
}

/// Any transport wrapper that can hold any transport type.
///
/// This enum allows the transport API to return different concrete
/// transport types while maintaining type safety and avoiding trait objects.
///
/// This type is not exported from the public API of the library.
/// Use generic transport types for zero-cost abstractions.
#[derive(Debug)]
#[cfg(all(
    feature = "async",
    any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")
))]
pub enum AnyTransport {
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
impl crate::transport::AsyncTransport for AnyTransport {
    async fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
        match self {
            #[cfg(feature = "rt-tokio")]
            AnyTransport::TokioTcp(transport) => transport.send(bytes).await,
            #[cfg(feature = "rt-tokio")]
            AnyTransport::TokioUdp(transport) => transport.send(bytes).await,
            #[cfg(feature = "rt-async-std")]
            AnyTransport::AsyncStdTcp(transport) => transport.send(bytes).await,
            #[cfg(feature = "rt-async-std")]
            AnyTransport::AsyncStdUdp(transport) => transport.send(bytes).await,
            #[cfg(feature = "rt-smol")]
            AnyTransport::SmolTcp(transport) => transport.send(bytes).await,
            #[cfg(feature = "rt-smol")]
            AnyTransport::SmolUdp(transport) => transport.send(bytes).await,
        }
    }

    async fn recv(&mut self) -> Result<bytes::Bytes, Error> {
        match self {
            #[cfg(feature = "rt-tokio")]
            AnyTransport::TokioTcp(transport) => transport.recv().await,
            #[cfg(feature = "rt-tokio")]
            AnyTransport::TokioUdp(transport) => transport.recv().await,
            #[cfg(feature = "rt-async-std")]
            AnyTransport::AsyncStdTcp(transport) => transport.recv().await,
            #[cfg(feature = "rt-async-std")]
            AnyTransport::AsyncStdUdp(transport) => transport.recv().await,
            #[cfg(feature = "rt-smol")]
            AnyTransport::SmolTcp(transport) => transport.recv().await,
            #[cfg(feature = "rt-smol")]
            AnyTransport::SmolUdp(transport) => transport.recv().await,
        }
    }
}

/// Uniform transport API.
///
/// Provides a clean interface for creating transports without exposing runtime details.
/// The runtime implementation is automatically selected based on enabled features.
#[derive(Debug, Copy, Clone)]
pub struct Transport;

impl Transport {
    /// Create a TCP transport builder.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use grafton_visca::transport::Transport;
    /// # use std::time::Duration;
    ///
    /// # #[cfg(not(feature = "async"))]
    /// # fn blocking_example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Building blocking transports
    /// let transport = Transport::tcp()
    ///     .address("192.168.0.110:5678")
    ///     .tcp_nodelay(true)
    ///     .build_blocking()?;
    /// # Ok(())
    /// # }
    ///
    /// # #[cfg(feature = "async")]
    /// # async fn async_example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Building async transports
    /// let transport = Transport::tcp()
    ///     .address("192.168.0.110:5678")
    ///     .tcp_nodelay(true)
    ///     .build_async()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn tcp() -> NetTransportBuilder {
        NetTransportBuilder::tcp()
    }

    /// Create a UDP transport builder.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use grafton_visca::transport::Transport;
    /// # use std::time::Duration;
    ///
    /// # #[cfg(not(feature = "async"))]
    /// # fn blocking_example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Building blocking transports
    /// let transport = Transport::udp()
    ///     .address("192.168.0.110:5678")
    ///     .max_retries(5)
    ///     .build_blocking()?;
    /// # Ok(())
    /// # }
    ///
    /// # #[cfg(feature = "async")]
    /// # async fn async_example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Building async transports
    /// let transport = Transport::udp()
    ///     .address("192.168.0.110:5678")
    ///     .max_retries(5)
    ///     .build_async()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn udp() -> NetTransportBuilder {
        NetTransportBuilder::udp()
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
    #[cfg(all(
        feature = "async",
        any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")
    ))]
    pub async fn auto_detect(
        address: impl Into<String>,
    ) -> Result<(AnyTransport, super::DetectionResult), Error> {
        Self::tcp()
            .address(address)
            .build_async_with_auto_detection()
            .await
    }
}

/// Unified transport builder that can create both blocking and async transports.
///
/// This builder eliminates the need for separate `TransportBuilder` and `AnyTransportBuilder`
/// types by providing `.build_blocking()` and `.build_async()` methods on a single builder.
///
/// # Example
///
/// ```rust,no_run
/// # #[cfg(any(feature = "async", not(feature = "async")))]
/// use grafton_visca::transport::NetTransportBuilder;
/// # use std::time::Duration;
///
/// # #[cfg(not(feature = "async"))]
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Building blocking transports
/// let blocking_transport = NetTransportBuilder::tcp()
///     .address("192.168.0.110:5678")
///     .connect_timeout(Duration::from_secs(10))
///     .build_blocking()?;
/// # Ok(())
/// # }
///
/// # #[cfg(feature = "async")]
/// # async fn async_example() -> Result<(), Box<dyn std::error::Error>> {
/// // Building async transports
/// let async_transport = NetTransportBuilder::tcp()
///     .address("192.168.0.110:5678")
///     .connect_timeout(Duration::from_secs(10))
///     .build_async()
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct NetTransportBuilder {
    #[cfg_attr(
        not(any(
            not(feature = "async"),
            feature = "rt-tokio",
            feature = "rt-async-std",
            feature = "rt-smol",
            test
        )),
        allow(dead_code)
    )]
    protocol: Protocol,
    address: Option<String>,
    config: TransportConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Protocol {
    Tcp,
    Udp,
}

impl NetTransportBuilder {
    /// Create a new TCP transport builder.
    pub fn tcp() -> Self {
        Self {
            protocol: Protocol::Tcp,
            address: None,
            config: TransportConfig::default(),
        }
    }

    /// Create a new UDP transport builder.
    pub fn udp() -> Self {
        Self {
            protocol: Protocol::Udp,
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

    /// Build a blocking transport.
    ///
    /// This method establishes the connection and returns a configured blocking transport instance.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No address has been set
    /// - Connection fails
    /// - Socket configuration fails
    /// - Async features are enabled without blocking support
    #[cfg(not(feature = "async"))]
    pub fn build_blocking(self) -> Result<Box<dyn SyncTransport>, Error> {
        let address = self.address.ok_or_else(|| Error::InvalidParameter {
            parameter: "address",
            value: "None".into(),
            reason: "No address specified for transport".into(),
        })?;

        match self.protocol {
            Protocol::Tcp => {
                let transport =
                    crate::transport::blocking::Tcp::connect_with_config(&address, self.config)?;
                Ok(Box::new(transport))
            }
            Protocol::Udp => {
                let transport =
                    crate::transport::blocking::Udp::connect_with_config(&address, self.config)?;
                Ok(Box::new(transport))
            }
        }
    }

    /// Build a blocking transport.
    ///
    /// This method returns an error when async features are enabled, since blocking
    /// transport creation is not supported in async mode.
    ///
    /// # Errors
    ///
    /// Returns an error indicating that blocking transport creation is not available in async mode.
    #[cfg(feature = "async")]
    pub fn build_blocking(self) -> Result<(), Error> {
        Err(Error::InvalidState(
            "Blocking transport not available when async features are enabled".into(),
        ))
    }

    /// Build an async transport.
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
    pub async fn build_async(self) -> Result<AnyTransport, Error> {
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
                    Ok(AnyTransport::TokioTcp(transport))
                }
                Protocol::Udp => {
                    let transport =
                        crate::runtime_adapters::tokio::UdpTransport::connect_with_config(
                            &address,
                            self.config,
                        )
                        .await?;
                    Ok(AnyTransport::TokioUdp(transport))
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
                    Ok(AnyTransport::AsyncStdTcp(transport))
                }
                Protocol::Udp => {
                    let transport =
                        crate::runtime_adapters::async_std::UdpTransport::connect_with_config(
                            &address,
                            self.config,
                        )
                        .await?;
                    Ok(AnyTransport::AsyncStdUdp(transport))
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
                    Ok(AnyTransport::SmolTcp(transport))
                }
                Protocol::Udp => {
                    let transport =
                        crate::runtime_adapters::smol::UdpTransport::connect_with_config(
                            &address,
                            self.config,
                        )
                        .await?;
                    Ok(AnyTransport::SmolUdp(transport))
                }
            };
        }

        // This should never be reached due to the cfg guard at the function level
        unreachable!("No runtime available - this should be prevented by cfg guard")
    }

    /// Build an async transport with automatic protocol detection (EPIC task B3).
    ///
    /// This method implements automatic detection of Sony encapsulated
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
    #[cfg(all(
        feature = "async",
        any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")
    ))]
    #[allow(unreachable_code)]
    pub async fn build_async_with_auto_detection(
        self,
    ) -> Result<(AnyTransport, super::DetectionResult), Error> {
        let address = self
            .address
            .clone()
            .ok_or_else(|| Error::InvalidParameter {
                parameter: "address",
                value: "None".into(),
                reason: "No address specified for transport".into(),
            })?;

        // First establish the connection
        let mut transport = self.build_async().await?;

        // For now, use a feature-specific executor approach
        // This will be improved when we have more specific builder methods
        #[cfg(feature = "rt-tokio")]
        {
            let executor = crate::executor::TokioExecutor::from_current()
                .map_err(|_| Error::MissingRuntime)?;
            let detector = super::ProtocolDetector::new();
            let detection_result = detector.detect_protocol(&mut transport, &executor).await?;
            match detection_result {
                super::DetectionResult::SonyEncapsulated | super::DetectionResult::RawVisca => {
                    return Ok((transport, detection_result));
                }
                super::DetectionResult::NoResponse => {
                    return Err(Error::ConnectionFailed {
                        addr: address.into(),
                        source: std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "No VISCA protocol response detected from camera - verify camera is powered on and address is correct"
                        ),
                    });
                }
            }
        }

        #[cfg(all(feature = "rt-async-std", not(feature = "rt-tokio")))]
        {
            let executor = crate::executor::AsyncStdExecutor::new();
            let detector = super::ProtocolDetector::new();
            let detection_result = detector.detect_protocol(&mut transport, &executor).await?;
            match detection_result {
                super::DetectionResult::SonyEncapsulated | super::DetectionResult::RawVisca => {
                    return Ok((transport, detection_result));
                }
                super::DetectionResult::NoResponse => {
                    return Err(Error::ConnectionFailed {
                        addr: address.into(),
                        source: std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "No VISCA protocol response detected from camera - verify camera is powered on and address is correct"
                        ),
                    });
                }
            }
        }

        #[cfg(all(
            feature = "rt-smol",
            not(any(feature = "rt-tokio", feature = "rt-async-std"))
        ))]
        {
            let executor = crate::executor::SmolExecutor::new();
            let detector = super::ProtocolDetector::new();
            let detection_result = detector.detect_protocol(&mut transport, &executor).await?;
            match detection_result {
                super::DetectionResult::SonyEncapsulated | super::DetectionResult::RawVisca => {
                    return Ok((transport, detection_result));
                }
                super::DetectionResult::NoResponse => {
                    return Err(Error::ConnectionFailed {
                        addr: address.into(),
                        source: std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "No VISCA protocol response detected from camera - verify camera is powered on and address is correct"
                        ),
                    });
                }
            }
        }

        // If no runtime features are enabled, return an error
        Err(Error::MissingRuntime)
    }

    /// Build an async transport (not available without async runtime features).
    ///
    /// This method returns an error when no async runtime features are enabled.
    ///
    /// # Errors
    ///
    /// Returns an error indicating that async transport creation requires runtime features.
    #[cfg(all(
        feature = "async",
        not(any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol"))
    ))]
    pub async fn build_async(self) -> Result<(), Error> {
        Err(Error::MissingRuntime)
    }

    /// Build an async transport (not available without async features).
    ///
    /// This method is not available when async features are disabled.
    #[cfg(not(feature = "async"))]
    pub async fn build_async(self) -> Result<(), Error> {
        Err(Error::InvalidState(
            "Async transport not available without async features enabled".into(),
        ))
    }

    /// Build an async transport with auto-detection (not available without async runtime features).
    ///
    /// This method returns an error when no async runtime features are enabled.
    ///
    /// # Errors
    ///
    /// Returns an error indicating that async transport creation requires runtime features.
    #[cfg(all(
        feature = "async",
        not(any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol"))
    ))]
    pub async fn build_async_with_auto_detection(
        self,
    ) -> Result<((), super::DetectionResult), Error> {
        Err(Error::MissingRuntime)
    }

    /// Build an async transport with auto-detection (not available without async features).
    ///
    /// This method is not available when async features are disabled.
    #[cfg(not(feature = "async"))]
    pub async fn build_async_with_auto_detection(self) -> Result<(), Error> {
        Err(Error::InvalidState(
            "Async transport with auto-detection not available without async features enabled"
                .into(),
        ))
    }
}

/// Extension trait for creating transports with a builder pattern.
pub trait TransportBuilderExt: Sized {
    /// Create a builder for this transport type.
    fn builder() -> NetTransportBuilder;
}

#[cfg(not(feature = "async"))]
impl TransportBuilderExt for crate::transport::blocking::Tcp {
    fn builder() -> NetTransportBuilder {
        NetTransportBuilder::tcp()
    }
}

#[cfg(not(feature = "async"))]
impl TransportBuilderExt for crate::transport::blocking::Udp {
    fn builder() -> NetTransportBuilder {
        NetTransportBuilder::udp()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_net_builder_default_config() {
        let builder = NetTransportBuilder::tcp();
        assert_eq!(builder.protocol, Protocol::Tcp);
        assert_eq!(builder.config.connect_timeout, Duration::from_secs(5));
        assert_eq!(builder.config.retry_config.max_retries, 3);
    }

    #[test]
    fn test_net_builder_fluent_api() {
        let builder = NetTransportBuilder::udp()
            .address("192.168.0.110:5678")
            .connect_timeout(Duration::from_secs(10))
            .max_retries(5)
            .tcp_nodelay(true);

        assert_eq!(builder.protocol, Protocol::Udp);
        assert_eq!(builder.address, Some("192.168.0.110:5678".to_string()));
        assert_eq!(builder.config.connect_timeout, Duration::from_secs(10));
        assert_eq!(builder.config.retry_config.max_retries, 5);
        assert_eq!(builder.config.tcp_nodelay, Some(true));
    }

    #[test]
    fn test_net_builder_timeout_convenience() {
        let builder = NetTransportBuilder::tcp().timeout(Duration::from_secs(3));

        assert_eq!(builder.config.connect_timeout, Duration::from_secs(3));
        assert_eq!(builder.config.read_timeout, Duration::from_secs(3));
        assert_eq!(builder.config.write_timeout, Duration::from_secs(3));
    }

    #[cfg(not(feature = "async"))]
    #[test]
    fn test_net_builder_blocking_requires_address() {
        let builder = NetTransportBuilder::tcp();
        let result = builder.build_blocking();
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

    #[cfg(feature = "async")]
    #[test]
    fn test_net_builder_blocking_not_available_in_async_mode() {
        let builder = NetTransportBuilder::tcp().address("192.168.0.110:5678");
        let result = builder.build_blocking();
        assert!(result.is_err());

        let is_correct_error = matches!(result, Err(Error::InvalidState(_)));
        assert!(
            is_correct_error,
            "Expected InvalidState error when building blocking transport in async mode"
        );
    }

    #[test]
    fn test_net_builder_retry_config() {
        let builder = NetTransportBuilder::tcp()
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

    #[test]
    fn test_transport_api_convenience() {
        let tcp_builder = Transport::tcp();
        assert_eq!(tcp_builder.protocol, Protocol::Tcp);

        let udp_builder = Transport::udp();
        assert_eq!(udp_builder.protocol, Protocol::Udp);
    }
}
