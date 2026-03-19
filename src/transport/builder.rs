//! Transport builder pattern for flexible transport configuration.
//!
//! This module provides a unified builder API for creating and configuring
//! all transport types with a consistent interface.
//!
//! ## Blocking Transport Builder
//!
//! `Transport` and `NetTransportBuilder` are the blocking transport-construction
//! entry points. For async mode, use either:
//! - `CameraConfig::transport_config(...)` for high-level configuration, or
//! - runtime-specific transports plus `CameraBuilder::from_transport(...)` for
//!   advanced BYO-transport flows.
//!
//! ```rust,no_run
//! # #[cfg(feature = "runtime-tokio")]
//! use grafton_visca::camera::{CameraConfig, profiles::GenericVisca};
//! # #[cfg(feature = "runtime-tokio")]
//! use grafton_visca::runtime::TokioRuntime;
//! # #[cfg(feature = "runtime-tokio")]
//! use grafton_visca::transport::TransportConfig;
//! # #[cfg(feature = "runtime-tokio")]
//! use std::time::Duration;
//!
//! # #[cfg(feature = "runtime-tokio")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let runtime = TokioRuntime::from_current()?;
//! let config = CameraConfig::<GenericVisca>::new()
//!     .address("192.168.0.110:5678")
//!     .transport_config(TransportConfig {
//!         tcp_keepalive: Some(Duration::from_secs(30)),
//!         ..TransportConfig::default()
//!     });
//! let _camera = config.open_async(runtime).await?;
//! # Ok(())
//! # }
//! ```

use std::{num::NonZeroUsize, time::Duration};

#[cfg(not(feature = "mode-async"))]
use crate::transport::BackoffStrategy;
use crate::transport::{buffer::BufferConfig, RetryConfig};

#[cfg(not(feature = "mode-async"))]
use crate::Error;

/// Addressing mode for VISCA communication.
///
/// Determines how device addresses are handled in VISCA frames.
/// - Serial: Device addresses (0x81-0x88) are preserved as-is
/// - IP: Device address is always normalized to 0x81 (per VISCA-over-IP spec)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AddressingMode {
    /// Serial addressing - device IDs are meaningful (0x81-0x88)
    Serial,
    /// IP addressing - always uses ID 1 (0x81) per spec
    #[default]
    Ip,
}

/// Default maximum pending queue depth for runtime admission control.
///
/// This bounds the number of commands/inquiries that can be queued in the
/// runtime scheduler before new submissions are rejected with a retryable
/// `RuntimeQueueFull` error.
///
/// A depth of 64 provides reasonable headroom for bursty submission patterns
/// (UI scrubbing, multi-camera fanout, polling loops) while preventing
/// unbounded memory growth under sustained overload.
pub const DEFAULT_MAX_PENDING_QUEUE_DEPTH: usize = 64;

/// Default maximum pending queue depth as NonZeroUsize.
///
/// This is a compile-time constant for use in `TransportConfig::default()`.
#[allow(clippy::unwrap_used)]
pub const DEFAULT_MAX_PENDING_QUEUE_DEPTH_NONZERO: NonZeroUsize =
    NonZeroUsize::new(DEFAULT_MAX_PENDING_QUEUE_DEPTH).unwrap();

/// Default TCP keepalive interval for long-lived VISCA TCP connections.
pub(crate) const DEFAULT_TCP_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);

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
    /// Addressing mode (Serial vs IP) for VISCA frames.
    pub addressing: AddressingMode,
    /// Whether to enable TCP nodelay (disable Nagle's algorithm).
    pub tcp_nodelay: Option<bool>,
    /// TTL (Time To Live) for packets.
    pub ttl: Option<u32>,
    /// TCP keepalive interval. When set, enables OS-level TCP keepalive probes
    /// to prevent camera-side idle timeout on long-lived connections.
    pub tcp_keepalive: Option<Duration>,
    /// Maximum pending queue depth for runtime admission control and backpressure.
    ///
    /// This value provides a **hard memory/backpressure guarantee** by bounding:
    ///
    /// 1. **Submission channel capacity**: The channel from `RuntimeHandle` to the
    ///    runtime loop is bounded to this depth. When full, `send_async` calls
    ///    will await rather than buffer unboundedly, providing backpressure to
    ///    bursty producers.
    ///
    /// 2. **Adapter admission control**: Commands/inquiries that exceed this depth
    ///    after reaching the runtime loop are rejected with a retryable
    ///    `RuntimeQueueFull` error.
    ///
    /// Together, these bounds ensure worst-case memory usage is O(max_pending_queue_depth)
    /// rather than O(number of submitted commands), preventing OOM under sustained load.
    ///
    /// Defaults to [`DEFAULT_MAX_PENDING_QUEUE_DEPTH`] (64).
    pub max_pending_queue_depth: NonZeroUsize,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(5),
            write_timeout: Duration::from_secs(5),
            retry_config: RetryConfig::default(),
            buffer_config: BufferConfig::default(),
            addressing: AddressingMode::default(),
            tcp_nodelay: Some(true),
            ttl: None,
            tcp_keepalive: Some(DEFAULT_TCP_KEEPALIVE_INTERVAL),
            max_pending_queue_depth: DEFAULT_MAX_PENDING_QUEUE_DEPTH_NONZERO,
        }
    }
}

/// Uniform transport API (blocking mode only).
///
/// Provides a clean interface for creating transports without exposing runtime details.
/// This type is only available in blocking mode. For async mode, use `CameraConfig`
/// and `Connect` convenience methods instead.
#[cfg(not(feature = "mode-async"))]
#[derive(Debug, Copy, Clone)]
pub struct Transport;

#[cfg(not(feature = "mode-async"))]
impl Transport {
    /// Create a TCP transport builder.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use grafton_visca::transport::Transport;
    /// # use std::time::Duration;
    ///
    /// # #[cfg(not(feature = "mode-async"))]
    /// # fn blocking_example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Building blocking transports
    /// let transport = Transport::tcp()
    ///     .address("192.168.0.110:5678")
    ///     .tcp_nodelay(true)
    ///     .build_blocking()?;
    /// # Ok(())
    /// # }
    ///
    /// # #[cfg(feature = "runtime-tokio")]
    /// # async fn async_example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Building cameras with transport configuration
    /// use grafton_visca::{camera::{CameraConfig, Camera, Connect}, runtime::TokioRuntime};
    /// use grafton_visca::camera::profiles::GenericVisca;
    /// let runtime = TokioRuntime::from_current()?;
    /// // Use convenience method for quick setup
    /// let session = Connect::open_tcp_async::<GenericVisca, _>("192.168.0.110:5678", runtime).await?;
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
    /// # #[cfg(not(feature = "mode-async"))]
    /// # fn blocking_example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Building blocking transports
    /// let transport = Transport::udp()
    ///     .address("192.168.0.110:5678")
    ///     .max_retries(5)
    ///     .build_blocking()?;
    /// # Ok(())
    /// # }
    ///
    /// # #[cfg(feature = "runtime-tokio")]
    /// # async fn async_example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Building cameras with transport configuration
    /// use grafton_visca::{camera::{CameraConfig, Camera, Connect}, runtime::TokioRuntime};
    /// use grafton_visca::camera::profiles::GenericVisca;
    /// let runtime = TokioRuntime::from_current()?;
    /// // Use convenience method for quick setup
    /// let session = Connect::open_udp_async::<GenericVisca, _>("192.168.0.110:1259", runtime).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn udp() -> NetTransportBuilder {
        NetTransportBuilder::udp()
    }
}

/// Unified transport builder for blocking transports.
///
/// This builder provides `.build_blocking()` to create blocking transport instances.
/// This type is only available in blocking mode. For async mode, use `CameraConfig`
/// and `Connect` convenience methods instead.
///
/// # Example
///
/// ```rust,no_run
/// # #[cfg(not(feature = "mode-async"))]
/// use grafton_visca::transport::NetTransportBuilder;
/// # use std::time::Duration;
///
/// # #[cfg(not(feature = "mode-async"))]
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Building blocking transports
/// let blocking_transport = NetTransportBuilder::tcp()
///     .address("192.168.0.110:5678")
///     .connect_timeout(Duration::from_secs(10))
///     .build_blocking()?;
/// # Ok(())
/// # }
/// ```
#[cfg(not(feature = "mode-async"))]
#[derive(Debug, Clone)]
pub struct NetTransportBuilder {
    protocol: Protocol,
    address: Option<String>,
    config: TransportConfig,
}

#[cfg(not(feature = "mode-async"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Protocol {
    Tcp,
    Udp,
}

#[cfg(not(feature = "mode-async"))]
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

    /// Set the backoff strategy for retries.
    ///
    /// The backoff strategy determines how retry delays are calculated:
    /// - [`BackoffStrategy::Constant`]: Same delay for all retries
    /// - [`BackoffStrategy::Exponential`]: Delay doubles each attempt (default)
    pub fn backoff_strategy(mut self, strategy: BackoffStrategy) -> Self {
        self.config.retry_config.backoff_strategy = strategy;
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

    /// Set the TCP keepalive interval.
    ///
    /// This option only affects TCP transports. The interval controls both the
    /// initial idle period before keepalive probes begin and the spacing between
    /// subsequent probes.
    pub fn tcp_keepalive(mut self, interval: Duration) -> Self {
        self.config.tcp_keepalive = Some(interval);
        self
    }

    /// Disable TCP keepalive.
    ///
    /// This option only affects TCP transports.
    pub fn disable_tcp_keepalive(mut self) -> Self {
        self.config.tcp_keepalive = None;
        self
    }

    /// Set the maximum pending queue depth for runtime admission control.
    ///
    /// This bounds the number of commands/inquiries that can be queued
    /// in the runtime scheduler. When the queue is at capacity, new
    /// submissions are rejected with a retryable `RuntimeQueueFull` error.
    ///
    /// # Arguments
    ///
    /// * `depth` - Maximum number of pending commands/inquiries
    ///
    /// # Returns
    ///
    /// Returns `self` for chaining. If `depth` is 0, the configuration is
    /// unchanged (preserves current value).
    pub fn max_pending_queue_depth(mut self, depth: NonZeroUsize) -> Self {
        self.config.max_pending_queue_depth = depth;
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
    #[cfg(not(feature = "mode-async"))]
    pub fn build_blocking(self) -> Result<crate::transport::BlockingTransportHandle, Error> {
        let address = self.address.ok_or_else(|| Error::InvalidParameter {
            parameter: "address",
            value: "None".into(),
            reason: "No address specified for transport".into(),
        })?;

        match self.protocol {
            Protocol::Tcp => {
                let transport =
                    crate::transport::blocking::Tcp::connect_with_config(&address, self.config)?;
                Ok(crate::transport::BlockingTransportHandle::Tcp(transport))
            }
            Protocol::Udp => {
                let transport =
                    crate::transport::blocking::Udp::connect_with_config(&address, self.config)?;
                Ok(crate::transport::BlockingTransportHandle::Udp(transport))
            }
        }
    }
}

/// Extension trait for creating transports with a builder pattern.
///
/// This trait is only available in blocking mode.
#[cfg(not(feature = "mode-async"))]
pub trait TransportBuilderExt: Sized {
    /// Create a builder for this transport type.
    fn builder() -> NetTransportBuilder;
}

#[cfg(not(feature = "mode-async"))]
impl TransportBuilderExt for crate::transport::blocking::Tcp {
    fn builder() -> NetTransportBuilder {
        NetTransportBuilder::tcp()
    }
}

#[cfg(not(feature = "mode-async"))]
impl TransportBuilderExt for crate::transport::blocking::Udp {
    fn builder() -> NetTransportBuilder {
        NetTransportBuilder::udp()
    }
}

#[cfg(all(test, not(feature = "mode-async")))]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_net_builder_default_config() {
        let builder = NetTransportBuilder::tcp();
        assert_eq!(builder.protocol, Protocol::Tcp);
        assert_eq!(builder.config.connect_timeout, Duration::from_secs(5));
        assert_eq!(builder.config.retry_config.max_retries, 3);
        assert_eq!(builder.config.tcp_nodelay, Some(true));
        assert_eq!(
            builder.config.tcp_keepalive,
            Some(DEFAULT_TCP_KEEPALIVE_INTERVAL)
        );
    }

    #[test]
    fn test_net_builder_fluent_api() {
        let builder = NetTransportBuilder::udp()
            .address("192.168.0.110:5678")
            .connect_timeout(Duration::from_secs(10))
            .max_retries(5)
            .tcp_nodelay(true)
            .tcp_keepalive(Duration::from_secs(45));

        assert_eq!(builder.protocol, Protocol::Udp);
        assert_eq!(builder.address, Some("192.168.0.110:5678".to_string()));
        assert_eq!(builder.config.connect_timeout, Duration::from_secs(10));
        assert_eq!(builder.config.retry_config.max_retries, 5);
        assert_eq!(builder.config.tcp_nodelay, Some(true));
        assert_eq!(builder.config.tcp_keepalive, Some(Duration::from_secs(45)));
    }

    #[test]
    fn test_net_builder_timeout_convenience() {
        let builder = NetTransportBuilder::tcp().timeout(Duration::from_secs(3));

        assert_eq!(builder.config.connect_timeout, Duration::from_secs(3));
        assert_eq!(builder.config.read_timeout, Duration::from_secs(3));
        assert_eq!(builder.config.write_timeout, Duration::from_secs(3));
    }

    #[test]
    fn test_net_builder_disable_tcp_keepalive() {
        let builder = NetTransportBuilder::tcp().disable_tcp_keepalive();
        assert_eq!(builder.config.tcp_keepalive, None);
    }

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

    #[test]
    fn test_net_builder_retry_config() {
        use crate::transport::BackoffStrategy;

        let builder = NetTransportBuilder::tcp()
            .max_retries(10)
            .retry_delay(Duration::from_millis(500))
            .max_retry_duration(Duration::from_secs(30))
            .backoff_strategy(BackoffStrategy::Constant);

        assert_eq!(builder.config.retry_config.max_retries, 10);
        assert_eq!(
            builder.config.retry_config.base_retry_delay,
            Duration::from_millis(500)
        );
        assert_eq!(
            builder.config.retry_config.max_retry_duration,
            Duration::from_secs(30)
        );
        assert_eq!(
            builder.config.retry_config.backoff_strategy,
            BackoffStrategy::Constant
        );
    }

    #[test]
    fn test_transport_api_convenience() {
        let tcp_builder = Transport::tcp();
        assert_eq!(tcp_builder.protocol, Protocol::Tcp);

        let udp_builder = Transport::udp();
        assert_eq!(udp_builder.protocol, Protocol::Udp);
    }
}
