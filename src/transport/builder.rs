//! Transport builder pattern for flexible transport configuration.
//!
//! This module provides a unified builder API for creating and configuring
//! all transport types with a consistent interface.
//!
//! ## Blocking Transport Builder
//!
//! `Transport` and `NetTransportBuilder` are the blocking transport-construction
//! entry points. For canonical async mode, obtain a runtime-specific transport
//! and pass it to `Session::open`; `TransportConfig` is shared by both paths.
//!
//! ```rust,no_run
//! # #[cfg(feature = "runtime-tokio")]
//! use grafton_visca::{
//!     profiles::GenericVisca, Runtime, Session, SessionConfig, TokioRuntime,
//! };
//! # #[cfg(feature = "runtime-tokio")]
//! use grafton_visca::transport::{TcpKeepaliveConfig, TransportConfig};
//!
//! # #[cfg(feature = "runtime-tokio")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let runtime = TokioRuntime::from_current()?;
//! let transport = runtime
//!     .connect_tcp(
//!         "192.168.0.110:5678",
//!         TransportConfig {
//!             tcp_keepalive: Some(TcpKeepaliveConfig::default()),
//!             ..TransportConfig::default()
//!         },
//!     )
//!     .await?;
//! let _session = Session::open(
//!     transport,
//!     SessionConfig::from_compile_time::<GenericVisca>()?,
//!     runtime,
//! )
//! .await?;
//! # Ok(())
//! # }
//! ```

use std::time::Duration;

use crate::{transport::buffer::BufferConfig, Error};

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

/// Default initial idle period before TCP keepalive probes begin.
pub(crate) const DEFAULT_TCP_KEEPALIVE_IDLE: Duration = Duration::from_secs(10);

/// Default spacing between TCP keepalive probes.
pub(crate) const DEFAULT_TCP_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(10);

/// TCP keepalive policy for long-lived VISCA TCP connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpKeepaliveConfig {
    /// Idle time before the first keepalive probe is sent.
    pub idle: Duration,
    /// Optional spacing between subsequent keepalive probes.
    ///
    /// When `None`, the OS default is used.
    pub interval: Option<Duration>,
}

impl TcpKeepaliveConfig {
    /// Create a keepalive policy with a required idle period and OS defaults for
    /// the remaining parameters.
    pub const fn new(idle: Duration) -> Self {
        Self {
            idle,
            interval: None,
        }
    }

    /// Create the default keepalive policy used for long-lived VISCA TCP sessions.
    pub const fn for_visca_long_lived_tcp() -> Self {
        DEFAULT_TCP_KEEPALIVE
    }

    /// Override the spacing between keepalive probes.
    pub const fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = Some(interval);
        self
    }
}

impl Default for TcpKeepaliveConfig {
    fn default() -> Self {
        DEFAULT_TCP_KEEPALIVE
    }
}

/// Default TCP keepalive policy for long-lived VISCA TCP connections.
pub(crate) const DEFAULT_TCP_KEEPALIVE: TcpKeepaliveConfig = TcpKeepaliveConfig {
    idle: DEFAULT_TCP_KEEPALIVE_IDLE,
    interval: Some(DEFAULT_TCP_KEEPALIVE_INTERVAL),
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
    /// Buffer configuration for managing buffers.
    pub buffer_config: BufferConfig,
    /// Addressing mode (Serial vs IP) for VISCA frames.
    pub addressing: AddressingMode,
    /// Whether to enable TCP nodelay (disable Nagle's algorithm).
    pub tcp_nodelay: Option<bool>,
    /// TTL (Time To Live) for packets.
    pub ttl: Option<u32>,
    /// TCP keepalive policy. When set, enables OS-level probes that detect
    /// broken peers and may preserve idle network-path state. These probes do
    /// not send VISCA traffic or guarantee that a camera application keeps its
    /// session open.
    pub tcp_keepalive: Option<TcpKeepaliveConfig>,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(5),
            write_timeout: Duration::from_secs(5),
            buffer_config: BufferConfig::default(),
            addressing: AddressingMode::default(),
            tcp_nodelay: Some(true),
            ttl: None,
            tcp_keepalive: Some(DEFAULT_TCP_KEEPALIVE),
        }
    }
}

impl TransportConfig {
    /// Validate buffer bounds that must hold before a transport is opened.
    ///
    /// The owner validates the same invariant when admitting a caller-owned
    /// transport. Standard construction also needs it here, before a network
    /// connector or serial-device initializer can perform I/O.
    pub(crate) fn validate_buffer_bounds(&self) -> crate::Result<()> {
        if self.buffer_config.recv_buffer_size == 0 {
            return Err(Error::InvalidRequest(
                "transport receive buffer must be non-zero".into(),
            ));
        }
        if self.buffer_config.max_buffer_size == 0 {
            return Err(Error::InvalidRequest(
                "transport maximum buffer must be non-zero".into(),
            ));
        }
        if self.buffer_config.recv_buffer_size > self.buffer_config.max_buffer_size {
            return Err(Error::InvalidRequest(
                "transport receive buffer cannot exceed maximum buffer".into(),
            ));
        }
        Ok(())
    }
}

/// Uniform transport API (blocking mode only).
///
/// Provides a clean interface for creating transports without exposing runtime details.
/// This type is only available in blocking mode. For canonical async mode, use a
/// runtime-specific transport with `Session::open` instead.
#[cfg(feature = "blocking")]
#[derive(Debug, Copy, Clone)]
pub struct Transport;

#[cfg(feature = "blocking")]
impl Transport {
    /// Create a TCP transport builder.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use grafton_visca::transport::Transport;
    /// # use std::time::Duration;
    ///
    /// # #[cfg(feature = "blocking")]
    /// # fn blocking_example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Building blocking transports
    /// let transport = Transport::tcp()
    ///     .address("192.168.0.110:5678")
    ///     .tcp_nodelay(true)
    ///     .build_blocking()?;
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
    /// # #[cfg(feature = "blocking")]
    /// # fn blocking_example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Building blocking transports
    /// let transport = Transport::udp()
    ///     .address("192.168.0.110:5678")
    ///     .build_blocking()?;
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
/// This type is only available in blocking mode. For canonical async mode, use a
/// runtime-specific transport with `Session::open` instead.
///
/// # Example
///
/// ```rust,no_run
/// # #[cfg(feature = "blocking")]
/// use grafton_visca::transport::NetTransportBuilder;
/// # use std::time::Duration;
///
/// # #[cfg(feature = "blocking")]
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Building blocking transports
/// let blocking_transport = NetTransportBuilder::tcp()
///     .address("192.168.0.110:5678")
///     .connect_timeout(Duration::from_secs(10))
///     .build_blocking()?;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "blocking")]
#[derive(Debug, Clone)]
pub struct NetTransportBuilder {
    protocol: Protocol,
    address: Option<String>,
    config: TransportConfig,
}

#[cfg(feature = "blocking")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Protocol {
    Tcp,
    Udp,
}

#[cfg(feature = "blocking")]
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
    /// This builder has no profile default port, so the address must include an
    /// explicit port. Use a hostname with port (e.g., `"camera.local:5678"`), an
    /// IPv4 address with port (e.g., `"192.168.0.110:5678"`), or bracketed IPv6
    /// with port (e.g., `"[::1]:5678"`).
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

    /// Set the TCP keepalive policy.
    ///
    /// This option only affects TCP transports.
    pub fn tcp_keepalive(mut self, keepalive: TcpKeepaliveConfig) -> Self {
        self.config.tcp_keepalive = Some(keepalive);
        self
    }

    /// Disable TCP keepalive.
    ///
    /// This option only affects TCP transports.
    pub fn disable_tcp_keepalive(mut self) -> Self {
        self.config.tcp_keepalive = None;
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
    #[cfg(feature = "blocking")]
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
#[cfg(feature = "blocking")]
pub trait TransportBuilderExt: Sized {
    /// Create a builder for this transport type.
    fn builder() -> NetTransportBuilder;
}

#[cfg(feature = "blocking")]
impl TransportBuilderExt for crate::transport::blocking::Tcp {
    fn builder() -> NetTransportBuilder {
        NetTransportBuilder::tcp()
    }
}

#[cfg(feature = "blocking")]
impl TransportBuilderExt for crate::transport::blocking::Udp {
    fn builder() -> NetTransportBuilder {
        NetTransportBuilder::udp()
    }
}

#[cfg(all(test, feature = "blocking"))]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_net_builder_default_config() {
        let builder = NetTransportBuilder::tcp();
        assert_eq!(builder.protocol, Protocol::Tcp);
        assert_eq!(builder.config.connect_timeout, Duration::from_secs(5));
        assert_eq!(builder.config.tcp_nodelay, Some(true));
        assert_eq!(builder.config.tcp_keepalive, Some(DEFAULT_TCP_KEEPALIVE));
    }

    #[test]
    fn test_net_builder_fluent_api() {
        let builder = NetTransportBuilder::udp()
            .address("192.168.0.110:5678")
            .connect_timeout(Duration::from_secs(10))
            .tcp_nodelay(true)
            .tcp_keepalive(
                TcpKeepaliveConfig::new(Duration::from_secs(45))
                    .with_interval(Duration::from_secs(15)),
            );

        assert_eq!(builder.protocol, Protocol::Udp);
        assert_eq!(builder.address, Some("192.168.0.110:5678".to_string()));
        assert_eq!(builder.config.connect_timeout, Duration::from_secs(10));
        assert_eq!(builder.config.tcp_nodelay, Some(true));
        assert_eq!(
            builder.config.tcp_keepalive,
            Some(
                TcpKeepaliveConfig::new(Duration::from_secs(45))
                    .with_interval(Duration::from_secs(15))
            )
        );
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
    fn test_transport_api_convenience() {
        let tcp_builder = Transport::tcp();
        assert_eq!(tcp_builder.protocol, Protocol::Tcp);

        let udp_builder = Transport::udp();
        assert_eq!(udp_builder.protocol, Protocol::Udp);
    }
}
