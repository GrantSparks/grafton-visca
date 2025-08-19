//! Transport builder pattern for flexible transport configuration.
//!
//! This module provides a unified builder API for creating and configuring
//! all transport types with a consistent interface.

use std::time::Duration;

use crate::transport::buffer::BufferConfig;
use crate::transport::{BlockingTransport, RetryConfig};
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
/// use grafton_visca::transport::builder::TransportBuilder;
/// use std::time::Duration;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
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
    transport_type: TransportType,
    address: Option<String>,
    config: TransportConfig,
}

/// The type of transport to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportType {
    /// TCP transport (blocking).
    Tcp,
    /// UDP transport (blocking).
    Udp,
    /// TCP transport (async with tokio).
    #[cfg(feature = "rt-tokio")]
    TokioTcp,
    /// UDP transport (async with tokio).
    #[cfg(feature = "rt-tokio")]
    TokioUdp,
}

impl TransportBuilder {
    /// Create a new builder for a TCP transport.
    pub fn tcp() -> Self {
        Self {
            transport_type: TransportType::Tcp,
            address: None,
            config: TransportConfig::default(),
        }
    }

    /// Create a new builder for a UDP transport.
    pub fn udp() -> Self {
        Self {
            transport_type: TransportType::Udp,
            address: None,
            config: TransportConfig::default(),
        }
    }

    /// Create a new builder for a tokio TCP transport.
    #[cfg(feature = "rt-tokio")]
    pub fn tokio_tcp() -> Self {
        Self {
            transport_type: TransportType::TokioTcp,
            address: None,
            config: TransportConfig::default(),
        }
    }

    /// Create a new builder for a tokio UDP transport.
    #[cfg(feature = "rt-tokio")]
    pub fn tokio_udp() -> Self {
        Self {
            transport_type: TransportType::TokioUdp,
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
    pub fn build(self) -> Result<Box<dyn BlockingTransport>, Error> {
        let address = self.address.ok_or_else(|| Error::InvalidParameter {
            parameter: "address",
            value: "None".into(),
            reason: "No address specified for transport".into(),
        })?;

        match self.transport_type {
            TransportType::Tcp => {
                let mut transport = crate::transport::blocking::Tcp::connect_timeout(
                    &address,
                    self.config.connect_timeout,
                )?;

                // Apply retry configuration
                transport.set_retry_config(self.config.retry_config);

                // Apply socket options if the transport supports them
                if let Some(nodelay) = self.config.tcp_nodelay {
                    transport.set_nodelay(nodelay)?;
                }
                if let Some(ttl) = self.config.ttl {
                    transport.set_ttl(ttl)?;
                }

                Ok(Box::new(transport))
            }
            TransportType::Udp => {
                let mut transport = crate::transport::blocking::Udp::connect(&address)?;

                // Apply retry configuration
                transport.set_retry_config(self.config.retry_config);

                // Apply socket options
                if let Some(ttl) = self.config.ttl {
                    transport.set_ttl(ttl)?;
                }

                // Set timeouts
                transport.set_read_timeout(Some(self.config.read_timeout))?;
                transport.set_write_timeout(Some(self.config.write_timeout))?;

                Ok(Box::new(transport))
            }
            #[cfg(feature = "rt-tokio")]
            TransportType::TokioTcp | TransportType::TokioUdp => {
                // For async transports, we can't return them as BlockingTransport
                // This would require a separate build_async() method
                Err(Error::InvalidParameter {
                    parameter: "transport_type",
                    value: format!("{:?}", self.transport_type).into(),
                    reason: "Use build_async() for async transports".into(),
                })
            }
        }
    }

    /// Build a blocking transport wrapped for async usage.
    ///
    /// This method creates a blocking transport and wraps it with an `AsyncWrapper`
    /// to enable its use in async contexts. This is useful for:
    /// - Using blocking transports in async code
    /// - Gradual migration from blocking to async
    /// - Testing async code with simpler blocking implementations
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No address has been set
    /// - Connection fails
    /// - Socket configuration fails
    /// - The transport type is already async (use `build_async()` instead)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "async")]
    /// use grafton_visca::transport::builder::TransportBuilder;
    /// # #[cfg(feature = "async")]
    /// use grafton_visca::transport::AsyncTransport;
    ///
    /// # #[cfg(feature = "async")]
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let async_transport = TransportBuilder::tcp()
    ///     .address("192.168.0.110:5678")
    ///     .build_async_wrapper()?;
    ///
    /// // Now can be used as an AsyncTransport
    /// async_transport.send(b"\x81\x01\x04\x00\x02\xFF").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async")]
    pub fn build_async_wrapper(
        self,
    ) -> Result<crate::transport::AsyncWrapper<Box<dyn BlockingTransport>>, Error> {
        match self.transport_type {
            TransportType::Tcp | TransportType::Udp => {
                let blocking_transport = self.build()?;
                Ok(crate::transport::AsyncWrapper::new(blocking_transport))
            }
            #[cfg(feature = "rt-tokio")]
            TransportType::TokioTcp | TransportType::TokioUdp => Err(Error::InvalidParameter {
                parameter: "transport_type",
                value: format!("{:?}", self.transport_type).into(),
                reason: "Already an async transport, use build_async() instead".into(),
            }),
        }
    }
}

/// Extension trait for creating transports with a builder pattern.
pub trait TransportBuilderExt: Sized {
    /// Create a builder for this transport type.
    fn builder() -> TransportBuilder;
}

impl TransportBuilderExt for crate::transport::blocking::Tcp {
    fn builder() -> TransportBuilder {
        TransportBuilder::tcp()
    }
}

impl TransportBuilderExt for crate::transport::blocking::Udp {
    fn builder() -> TransportBuilder {
        TransportBuilder::udp()
    }
}

#[cfg(feature = "rt-tokio")]
impl TransportBuilderExt for crate::transport::tokio::tcp::Tcp {
    fn builder() -> TransportBuilder {
        TransportBuilder::tokio_tcp()
    }
}

#[cfg(feature = "rt-tokio")]
impl TransportBuilderExt for crate::transport::tokio::udp::Udp {
    fn builder() -> TransportBuilder {
        TransportBuilder::tokio_udp()
    }
}

#[cfg(test)]
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
