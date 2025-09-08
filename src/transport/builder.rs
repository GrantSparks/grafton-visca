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
//! # #[cfg(feature = "rt-tokio")]
//! use grafton_visca::camera::{Camera, CameraConfig, profiles::GenericVisca};
//! # #[cfg(feature = "rt-tokio")]
//! use grafton_visca::runtime_trait::TokioRuntime;
//!
//! # #[cfg(feature = "rt-tokio")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // New session-centric API with convenience methods
//! let runtime = TokioRuntime::from_current()?;
//! let session = Camera::open_tcp_async::<GenericVisca, _>(
//!     "192.168.0.110:5678",
//!     runtime
//! ).await?;
//! # Ok(())
//! # }
//! ```

use std::time::Duration;

#[cfg(not(feature = "async"))]
use crate::transport::SyncTransport;
use crate::{
    transport::{buffer::BufferConfig, RetryConfig},
    Error,
};

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
    /// # #[cfg(feature = "rt-tokio")]
    /// # async fn async_example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Building cameras with transport configuration
    /// use grafton_visca::{camera::{CameraConfig, Camera}, runtime_trait::TokioRuntime};
    /// use grafton_visca::camera::profiles::GenericVisca;
    /// let runtime = TokioRuntime::from_current()?;
    /// // Use convenience method for quick setup
    /// let session = Camera::open_tcp_async::<GenericVisca, _>("192.168.0.110:5678", runtime).await?;
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
    /// # #[cfg(feature = "rt-tokio")]
    /// # async fn async_example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Building cameras with transport configuration
    /// use grafton_visca::{camera::{CameraConfig, Camera}, runtime_trait::TokioRuntime};
    /// use grafton_visca::camera::profiles::GenericVisca;
    /// let runtime = TokioRuntime::from_current()?;
    /// // Use convenience method for quick setup
    /// let session = Camera::open_udp_async::<GenericVisca, _>("192.168.0.110:1259", runtime).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn udp() -> NetTransportBuilder {
        NetTransportBuilder::udp()
    }
}

/// Unified transport builder that can create both blocking and async transports.
///
/// This builder eliminates the need for separate `TransportBuilder` and `AnyTransportBuilder`
/// types by providing `.build_blocking()` and `.open_async()` methods on a single builder.
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
/// # #[cfg(feature = "rt-tokio")]
/// # async fn async_example() -> Result<(), Box<dyn std::error::Error>> {
/// // Building cameras with custom transport configuration
/// use grafton_visca::{camera::{CameraConfig, profiles::GenericVisca}, runtime_trait::TokioRuntime};
/// let runtime = TokioRuntime::from_current()?;
/// let session = CameraConfig::<GenericVisca>::new()
///     .tcp()
///     .address("192.168.0.110:5678")
///     .open_async(runtime)
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct NetTransportBuilder {
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
        // Consume self to avoid dead code warning
        let _ = self.protocol;
        let _ = self.address;
        let _ = self.config;
        Err(Error::InvalidState(
            "Blocking transport not available when async features are enabled".into(),
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
