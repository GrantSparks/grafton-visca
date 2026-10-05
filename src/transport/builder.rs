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
//! // Start from the TCP defaults every TCP entry point uses, then adjust.
//! let mut config = TransportConfig::for_tcp();
//! config.tcp_keepalive = Some(TcpKeepaliveConfig::new(std::time::Duration::from_secs(30)));
//! let transport = runtime.connect_tcp("192.168.0.110:5678", config).await?;
//! let _session = Session::open(
//!     transport,
//!     SessionConfig::from_compile_time::<GenericVisca>()?,
//!     runtime,
//! )
//! .await?;
//! # Ok(())
//! # }
//! ```

use std::time::{Duration, Instant};

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
#[non_exhaustive]
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
///
/// # Per-transport defaults
///
/// [`TransportConfig::for_tcp`], [`TransportConfig::for_udp`] and
/// [`TransportConfig::for_serial`] are the single source of the defaults each
/// built-in transport uses. Every built-in entry point — [`CameraConfig`],
/// the blocking `Transport` builders, the runtime connectors and the direct
/// transport constructors — starts from them, so the largest reply a session
/// accepts never depends on which entry point opened the transport.
///
/// | Constructor | `recv_buffer_size` | `addressing` | TCP fields |
/// |---|---|---|---|
/// | [`for_tcp`](Self::for_tcp) | 256 ([`BufferConfig::for_raw_ip`]) | `Ip` | nodelay on, keepalive on |
/// | [`for_udp`](Self::for_udp) | 1024 ([`BufferConfig::for_udp`]) | `Ip` | `None` |
/// | [`for_serial`](Self::for_serial) | 256 ([`BufferConfig::for_serial`]) | `Serial` | `None` |
///
/// [`TransportConfig::default`] is the transport-neutral base the three
/// constructors refine. It is the right starting point only for a custom
/// transport that has no built-in kind.
///
/// [`CameraConfig`]: crate::camera::CameraConfig
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct TransportConfig {
    /// Connection timeout duration.
    ///
    /// Bounds name resolution plus connection setup for the IP transports on
    /// every facade. Serial devices have no connect phase.
    pub connect_timeout: Duration,
    /// Read timeout for receive operations.
    pub read_timeout: Duration,
    /// Write timeout for send operations.
    pub write_timeout: Duration,
    /// Frame and retention limits for received data.
    pub buffer_config: BufferConfig,
    /// Addressing mode (Serial vs IP) for VISCA frames.
    pub addressing: AddressingMode,
    /// TCP nodelay (Nagle's algorithm disabled when `Some(true)`).
    ///
    /// `None` leaves the operating-system default in place.
    pub tcp_nodelay: Option<bool>,
    /// IPv4 TTL (Time To Live) for packets. `None` keeps the OS default.
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
            connect_timeout: DEFAULT_TRANSPORT_TIMEOUT,
            read_timeout: DEFAULT_TRANSPORT_TIMEOUT,
            write_timeout: DEFAULT_TRANSPORT_TIMEOUT,
            buffer_config: BufferConfig::default(),
            addressing: AddressingMode::default(),
            tcp_nodelay: Some(true),
            ttl: None,
            tcp_keepalive: Some(DEFAULT_TCP_KEEPALIVE),
        }
    }
}

/// The default connect, read, and write timeout of every built-in transport.
const DEFAULT_TRANSPORT_TIMEOUT: Duration = Duration::from_secs(5);

impl TransportConfig {
    /// The configuration every built-in TCP transport starts from.
    ///
    /// Raw VISCA over TCP: 256-byte frame limit, IP addressing, `TCP_NODELAY`
    /// and the long-lived keepalive policy.
    #[must_use]
    pub fn for_tcp() -> Self {
        Self {
            buffer_config: BufferConfig::for_raw_ip(),
            ..Self::default()
        }
    }

    /// The configuration every built-in UDP transport starts from.
    ///
    /// One datagram per receive with a 1024-byte limit and IP addressing; no
    /// TCP socket options.
    #[must_use]
    pub fn for_udp() -> Self {
        Self {
            buffer_config: BufferConfig::for_udp(),
            tcp_nodelay: None,
            tcp_keepalive: None,
            ..Self::default()
        }
    }

    /// The configuration every built-in serial transport starts from.
    ///
    /// Serial addressing (device addresses `0x81..=0x88` are preserved on the
    /// wire), a 256-byte frame limit, and no TCP socket options.
    #[must_use]
    pub fn for_serial() -> Self {
        Self {
            buffer_config: BufferConfig::for_serial(),
            addressing: AddressingMode::Serial,
            tcp_nodelay: None,
            ttl: None,
            tcp_keepalive: None,
            ..Self::default()
        }
    }

    /// Validate I/O bounds that must hold before a transport is opened.
    ///
    /// The owner validates the same invariant when admitting a caller-owned
    /// transport. Standard construction also needs it here, before a network
    /// connector or serial-device initializer can perform I/O.
    pub(crate) fn validate(&self) -> crate::Result<()> {
        let now = Instant::now();
        for (name, parameter, timeout) in [
            ("connect", "connect_timeout", self.connect_timeout),
            ("read", "read_timeout", self.read_timeout),
            ("write", "write_timeout", self.write_timeout),
        ] {
            if timeout.is_zero() {
                return Err(Error::InvalidRequest(
                    format!("transport {name} timeout must be non-zero").into(),
                ));
            }
            crate::timeout::instant_after(now, timeout, parameter)?;
        }
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
    /// Create a new TCP transport builder starting from
    /// [`TransportConfig::for_tcp`].
    pub fn tcp() -> Self {
        Self {
            protocol: Protocol::Tcp,
            address: None,
            config: TransportConfig::for_tcp(),
        }
    }

    /// Create a new UDP transport builder starting from
    /// [`TransportConfig::for_udp`].
    pub fn udp() -> Self {
        Self {
            protocol: Protocol::Udp,
            address: None,
            config: TransportConfig::for_udp(),
        }
    }

    /// The configuration [`Self::build_blocking`] will connect with.
    pub fn config(&self) -> &TransportConfig {
        &self.config
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

    /// Set the largest accepted reply frame (see
    /// [`BufferConfig::recv_buffer_size`]).
    pub fn recv_buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_config.recv_buffer_size = size;
        self
    }

    /// Set the stream retention bound (see [`BufferConfig::max_buffer_size`]).
    pub fn max_buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_config.max_buffer_size = size;
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
        // Keep the builder's missing-address error ahead of configuration
        // validation. There is no connector to enter without an address, and
        // this preserves the established public error precedence.
        let address = self.address.ok_or_else(|| Error::InvalidParameter {
            parameter: "address",
            value: "None".into(),
            reason: "No address specified for transport".into(),
        })?;

        // The connector's preflight validates the configuration before any
        // I/O, exactly once.
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
    fn transport_config_rejects_invalid_timeouts() {
        for (config, message) in [
            (
                TransportConfig {
                    connect_timeout: Duration::ZERO,
                    ..TransportConfig::default()
                },
                "transport connect timeout must be non-zero",
            ),
            (
                TransportConfig {
                    read_timeout: Duration::ZERO,
                    ..TransportConfig::default()
                },
                "transport read timeout must be non-zero",
            ),
            (
                TransportConfig {
                    write_timeout: Duration::ZERO,
                    ..TransportConfig::default()
                },
                "transport write timeout must be non-zero",
            ),
        ] {
            assert!(matches!(
                config.validate(),
                Err(Error::InvalidRequest(actual)) if actual.as_ref() == message
            ));
        }
        // An unrepresentable timeout is the same error every runtime budget
        // reports.
        for (config, name) in [
            (
                TransportConfig {
                    connect_timeout: Duration::MAX,
                    ..TransportConfig::default()
                },
                "connect_timeout",
            ),
            (
                TransportConfig {
                    read_timeout: Duration::MAX,
                    ..TransportConfig::default()
                },
                "read_timeout",
            ),
            (
                TransportConfig {
                    write_timeout: Duration::MAX,
                    ..TransportConfig::default()
                },
                "write_timeout",
            ),
        ] {
            assert!(matches!(
                config.validate(),
                Err(Error::InvalidParameter { parameter, .. }) if parameter == name
            ));
        }
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
    fn missing_address_precedes_invalid_buffer_bounds() {
        let result = NetTransportBuilder::tcp()
            .recv_buffer_size(0)
            .build_blocking();

        // A builder without an address cannot reach a connector. Retaining
        // this precedence keeps its established argument error rather than
        // exposing a secondary configuration error first.
        assert!(matches!(
            result,
            Err(Error::InvalidParameter {
                parameter: "address",
                ..
            })
        ));
    }

    #[test]
    fn test_transport_api_convenience() {
        let tcp_builder = Transport::tcp();
        assert_eq!(tcp_builder.protocol, Protocol::Tcp);

        let udp_builder = Transport::udp();
        assert_eq!(udp_builder.protocol, Protocol::Udp);
    }
}
