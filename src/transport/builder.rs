//! Shared transport configuration and validation.
//!
//! Start from the appropriate transport defaults, then select overrides:
//!
//! ```rust
//! use grafton_visca::{transport::TcpKeepaliveConfig, transport::TransportConfig};
//! use std::time::Duration;
//!
//! let config = TransportConfig::for_tcp()
//!     .with_tcp_keepalive(Some(TcpKeepaliveConfig::new(Duration::from_secs(30))));
//! assert_eq!(config.read_timeout(), Duration::from_secs(5));
//! ```

use std::time::Duration;
#[cfg(any(feature = "async", feature = "blocking", test))]
use std::time::Instant;

use crate::transport::buffer::BufferConfig;
#[cfg(any(feature = "async", feature = "blocking", test))]
use crate::Error;

/// Addressing mode for VISCA communication.
///
/// Determines how device addresses are handled in VISCA frames.
/// - Serial: Device addresses (0x81-0x88) are preserved as-is
/// - IP: Device address is always normalized to 0x81 (per VISCA-over-IP spec)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub struct TcpKeepaliveConfig {
    /// Idle time before the first keepalive probe is sent.
    ///
    /// Must be a whole number of seconds in `1..=32_767`. The shared limit
    /// is portable across the supported socket implementations and avoids
    /// silently truncating subsecond values.
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number; nanos: number }"))]
    pub(crate) idle: Duration,
    /// Optional spacing between subsequent keepalive probes.
    ///
    /// When `None`, the OS default is used.
    /// Otherwise, must be a whole number of seconds in `1..=32_767`.
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number; nanos: number } | null"))]
    pub(crate) interval: Option<Duration>,
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

    /// Returns the idle period before the first keepalive probe.
    #[must_use]
    pub const fn idle(&self) -> Duration {
        self.idle
    }

    /// Returns the probe interval, or `None` for the operating-system default.
    #[must_use]
    pub const fn interval(&self) -> Option<Duration> {
        self.interval
    }

    /// Selects the initial idle period. Opening validates whole seconds in
    /// `1..=32_767` before any connection I/O.
    #[must_use]
    pub const fn with_idle(mut self, idle: Duration) -> Self {
        self.idle = idle;
        self
    }

    /// Selects the probe interval, or `None` for the operating-system default.
    /// Opening validates whole seconds in `1..=32_767`.
    #[must_use]
    pub const fn with_interval(mut self, interval: Option<Duration>) -> Self {
        self.interval = interval;
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
/// built-in transport uses. Every built-in entry point — [`crate::config::CameraConfig`],
/// the runtime connectors and the direct
/// transport constructors — starts from them, so the largest reply a session
/// accepts never depends on which entry point opened the transport.
///
/// | Constructor | `recv_buffer_size` | TCP fields |
/// |---|---|---|
/// | [`for_tcp`](Self::for_tcp) | 256 ([`BufferConfig::for_tcp`]) | nodelay on, keepalive on |
/// | [`for_udp`](Self::for_udp) | 1024 ([`BufferConfig::for_udp`]) | `None` |
/// | [`for_serial`](Self::for_serial) | 256 ([`BufferConfig::for_serial`]) | `None` |
///
/// [`TransportConfig::default`] is the transport-neutral base the three
/// constructors refine. It is the right starting point only for a custom
/// transport that has no built-in kind.
///
/// [`crate::config::CameraConfig`]: crate::config::CameraConfig
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub struct TransportConfig {
    /// Connection timeout duration.
    ///
    /// Bounds name resolution plus connection setup for the IP transports on
    /// every facade. Serial devices have no connect phase.
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number; nanos: number }"))]
    pub(crate) connect_timeout: Duration,
    /// Read timeout for receive operations.
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number; nanos: number }"))]
    pub(crate) read_timeout: Duration,
    /// Write timeout for send operations.
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number; nanos: number }"))]
    pub(crate) write_timeout: Duration,
    /// Frame and retention limits for received data.
    pub(crate) buffer_config: BufferConfig,

    /// TCP nodelay (Nagle's algorithm disabled when `Some(true)`).
    ///
    /// `None` leaves the operating-system default in place.
    pub(crate) tcp_nodelay: Option<bool>,
    /// IPv4 TTL / IPv6 hop limit (Time To Live) for packets, in `1..=255`.
    /// `None` keeps the OS default.
    pub(crate) ttl: Option<u32>,
    /// TCP keepalive policy. When set, enables OS-level probes that detect
    /// broken peers and may preserve idle network-path state. These probes do
    /// not send VISCA traffic or guarantee that a camera application keeps its
    /// session open.
    pub(crate) tcp_keepalive: Option<TcpKeepaliveConfig>,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            connect_timeout: DEFAULT_TRANSPORT_TIMEOUT,
            read_timeout: DEFAULT_TRANSPORT_TIMEOUT,
            write_timeout: DEFAULT_TRANSPORT_TIMEOUT,
            buffer_config: BufferConfig::default(),
            tcp_nodelay: Some(true),
            ttl: None,
            tcp_keepalive: Some(DEFAULT_TCP_KEEPALIVE),
        }
    }
}

/// The default connect, read, and write timeout of every built-in transport.
const DEFAULT_TRANSPORT_TIMEOUT: Duration = Duration::from_secs(5);

impl TransportConfig {
    /// Returns the connection timeout.
    #[must_use]
    pub const fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    /// Selects the connection timeout. Transport opening validates the configuration.
    #[must_use]
    pub const fn with_connect_timeout(mut self, value: Duration) -> Self {
        self.connect_timeout = value;
        self
    }

    /// Returns the receive timeout.
    #[must_use]
    pub const fn read_timeout(&self) -> Duration {
        self.read_timeout
    }

    /// Selects the receive timeout. Transport opening validates the configuration.
    #[must_use]
    pub const fn with_read_timeout(mut self, value: Duration) -> Self {
        self.read_timeout = value;
        self
    }

    /// Returns the send timeout.
    #[must_use]
    pub const fn write_timeout(&self) -> Duration {
        self.write_timeout
    }

    /// Selects the send timeout. Transport opening validates the configuration.
    #[must_use]
    pub const fn with_write_timeout(mut self, value: Duration) -> Self {
        self.write_timeout = value;
        self
    }

    /// Returns the receive frame and retention limits.
    #[must_use]
    pub const fn buffer_config(&self) -> BufferConfig {
        self.buffer_config
    }

    /// Selects the receive frame and retention limits. Transport opening validates the configuration.
    #[must_use]
    pub const fn with_buffer_config(mut self, value: BufferConfig) -> Self {
        self.buffer_config = value;
        self
    }

    /// Returns the TCP nodelay setting, or `None` for the OS default.
    #[must_use]
    pub const fn tcp_nodelay(&self) -> Option<bool> {
        self.tcp_nodelay
    }

    /// Selects the TCP nodelay setting, or `None` for the OS default. Transport opening validates the configuration.
    #[must_use]
    pub const fn with_tcp_nodelay(mut self, value: Option<bool>) -> Self {
        self.tcp_nodelay = value;
        self
    }

    /// Returns the IPv4 TTL / IPv6 hop limit, or `None` for the OS default.
    #[must_use]
    pub const fn ttl(&self) -> Option<u32> {
        self.ttl
    }

    /// Selects the IPv4 TTL / IPv6 hop limit, or `None` for the OS default. Transport opening validates the configuration.
    #[must_use]
    pub const fn with_ttl(mut self, value: Option<u32>) -> Self {
        self.ttl = value;
        self
    }

    /// Returns the TCP keepalive policy, or `None` for the OS default.
    #[must_use]
    pub const fn tcp_keepalive(&self) -> Option<TcpKeepaliveConfig> {
        self.tcp_keepalive
    }

    /// Selects the TCP keepalive policy, or `None` for the OS default.
    /// Transport opening validates the configuration.
    #[must_use]
    pub const fn with_tcp_keepalive(mut self, value: Option<TcpKeepaliveConfig>) -> Self {
        self.tcp_keepalive = value;
        self
    }

    /// The configuration every built-in TCP transport starts from.
    ///
    /// Raw VISCA over TCP: 256-byte frame limit, `TCP_NODELAY`
    /// and the long-lived keepalive policy.
    #[must_use]
    pub fn for_tcp() -> Self {
        Self {
            buffer_config: BufferConfig::for_tcp(),
            ..Self::default()
        }
    }

    /// The configuration every built-in UDP transport starts from.
    ///
    /// One datagram per receive with a 1024-byte limit; no
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
    /// A 256-byte frame limit and no TCP socket options.
    #[must_use]
    pub fn for_serial() -> Self {
        Self {
            buffer_config: BufferConfig::for_serial(),
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
    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub(crate) fn validate(&self) -> crate::Result<()> {
        let now = Instant::now();
        for (name, parameter, timeout) in [
            ("connect", "connect_timeout", self.connect_timeout),
            ("read", "read_timeout", self.read_timeout),
            ("write", "write_timeout", self.write_timeout),
        ] {
            if timeout.is_zero() {
                return Err(Error::InvalidRequest {
                    reason: format!("transport {name} timeout must be non-zero").into(),
                });
            }
            crate::timeout::instant_after(now, timeout, parameter)?;
        }
        if self.ttl.is_some_and(|ttl| !(1..=255).contains(&ttl)) {
            return Err(Error::InvalidRequest {
                reason: "transport TTL must be in 1..=255".into(),
            });
        }
        if let Some(keepalive) = self.tcp_keepalive {
            for (name, duration) in std::iter::once(("idle", keepalive.idle))
                .chain(keepalive.interval.map(|interval| ("interval", interval)))
            {
                if duration.subsec_nanos() != 0 || !(1..=32_767).contains(&duration.as_secs()) {
                    return Err(Error::InvalidRequest {
                        reason: format!("TCP keepalive {name} must be whole seconds in 1..=32767")
                            .into(),
                    });
                }
            }
        }
        if self.buffer_config.recv_buffer_size < BufferConfig::MIN_RECV_BUFFER_SIZE {
            return Err(Error::InvalidRequest {
                reason: format!(
                    "transport receive buffer must hold the largest VISCA reply ({} bytes)",
                    BufferConfig::MIN_RECV_BUFFER_SIZE
                )
                .into(),
            });
        }
        if self.buffer_config.max_buffer_size == 0 {
            return Err(Error::InvalidRequest {
                reason: "transport maximum buffer must be non-zero".into(),
            });
        }
        if self.buffer_config.recv_buffer_size > self.buffer_config.max_buffer_size {
            return Err(Error::InvalidRequest {
                reason: "transport receive buffer cannot exceed maximum buffer".into(),
            });
        }
        if self.buffer_config.recv_buffer_size > BufferConfig::MAX_RECV_BUFFER_SIZE {
            return Err(Error::InvalidRequest {
                reason: "transport receive buffer cannot exceed 65535 bytes".into(),
            });
        }
        if self.buffer_config.max_buffer_size > BufferConfig::MAX_RETENTION_SIZE {
            return Err(Error::InvalidRequest {
                reason: "transport maximum buffer cannot exceed 1048576 bytes".into(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_defaults_and_optional_overrides_keep_their_meanings() {
        let tcp = TransportConfig::for_tcp();
        assert_eq!(tcp.connect_timeout(), Duration::from_secs(5));
        assert_eq!(tcp.buffer_config(), BufferConfig::for_tcp());
        assert_eq!(tcp.tcp_nodelay(), Some(true));
        assert_eq!(tcp.tcp_keepalive(), Some(TcpKeepaliveConfig::default()));
        for config in [TransportConfig::for_udp(), TransportConfig::for_serial()] {
            assert_eq!(config.tcp_nodelay(), None);
            assert_eq!(config.tcp_keepalive(), None);
            assert_eq!(config.ttl(), None);
        }
        let config = tcp
            .with_tcp_keepalive(None)
            .with_tcp_nodelay(None)
            .with_ttl(Some(32));
        assert_eq!(config.tcp_keepalive(), None);
        assert_eq!(config.tcp_nodelay(), None);
        assert_eq!(config.ttl(), Some(32));
        assert!(config.validate().is_ok());
        assert_eq!(
            TcpKeepaliveConfig::new(Duration::from_secs(30))
                .with_interval(Some(Duration::from_secs(7)))
                .with_interval(None)
                .interval(),
            None
        );
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
                Err(Error::InvalidRequest { reason: actual }) if actual.as_ref() == message
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

    #[cfg(feature = "serde")]
    #[test]
    fn serde_preserves_policy_values_and_rejects_unknown_nested_fields(
    ) -> Result<(), serde_json::Error> {
        let config = TransportConfig::for_tcp()
            .with_read_timeout(Duration::from_millis(37))
            .with_tcp_nodelay(None)
            .with_tcp_keepalive(Some(
                TcpKeepaliveConfig::new(Duration::from_secs(30))
                    .with_interval(Some(Duration::from_secs(7))),
            ))
            .with_buffer_config(
                BufferConfig::for_tcp()
                    .with_recv_buffer_size(64)
                    .with_max_buffer_size(256),
            );
        let value = serde_json::to_value(config)?;
        assert_eq!(
            value["read_timeout"],
            serde_json::json!({"secs": 0, "nanos": 37_000_000})
        );
        assert_eq!(value["tcp_nodelay"], serde_json::Value::Null);
        assert_eq!(
            serde_json::from_value::<TransportConfig>(value.clone())?,
            config
        );
        for path in [None, Some("buffer_config"), Some("tcp_keepalive")] {
            let mut unknown = value.clone();
            let object = match path {
                None => &mut unknown,
                Some(field) => &mut unknown[field],
            };
            object["misspelled_option"] = serde_json::json!(true);
            assert!(serde_json::from_value::<TransportConfig>(unknown).is_err());
        }
        // Deserialization preserves selected data; opening still enforces the
        // exact existing timeout and receive-buffer preflight rules.
        let invalid = config.with_read_timeout(Duration::ZERO);
        let restored: TransportConfig = serde_json::from_value(serde_json::to_value(invalid)?)?;
        assert_eq!(restored, invalid);
        assert!(restored.validate().is_err());
        Ok(())
    }

    #[cfg(all(feature = "schemars", feature = "ts-rs"))]
    #[test]
    fn schemas_and_bindings_describe_the_serialized_duration_shape() {
        let schema = schemars::schema_for!(TransportConfig);
        assert_eq!(
            schema.get("additionalProperties"),
            Some(&serde_json::json!(false))
        );
        let binding = <TransportConfig as ts_rs::TS>::decl(&ts_rs::Config::default());
        assert!(binding.contains("secs: number"));
        assert!(binding.contains("nanos: number"));
        let keepalive = <TcpKeepaliveConfig as ts_rs::TS>::decl(&ts_rs::Config::default());
        assert!(keepalive.contains("| null"));
    }
}
