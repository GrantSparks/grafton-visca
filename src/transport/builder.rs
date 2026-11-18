//! Checked owner I/O bounds and per-kind connector settings.

use std::time::{Duration, Instant};

use crate::{timeout::NonZeroDuration, transport::buffer::BufferConfig, Error};

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

/// Default connect, read and write timeout for built-in transports.
const DEFAULT_TIMEOUT: Duration = NonZeroDuration::from_secs::<5>().get();

fn validate_timeout(name: &str, parameter: &'static str, duration: Duration) -> crate::Result<()> {
    if NonZeroDuration::checked(duration).is_none() {
        return Err(Error::InvalidRequest {
            reason: format!("transport {name} timeout must be non-zero").into(),
        });
    }
    crate::timeout::instant_after(Instant::now(), duration, parameter).map(drop)
}

fn validate_ttl(value: Option<u32>) -> crate::Result<()> {
    if value.is_some_and(|ttl| !(1..=255).contains(&ttl)) {
        return Err(Error::InvalidRequest {
            reason: "transport TTL must be in 1..=255".into(),
        });
    }
    Ok(())
}

fn validate_keepalive(value: Option<TcpKeepaliveConfig>) -> crate::Result<()> {
    if let Some(keepalive) = value {
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
    Ok(())
}

/// Checked read and write timeouts and receive-buffer limits used by the owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "IoBoundsData")
)]
#[cfg_attr(
    feature = "schemars",
    derive(schemars::JsonSchema),
    schemars(deny_unknown_fields)
)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct IoBounds {
    /// Read timeout for receive operations.
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number; nanos: number }"))]
    read_timeout: Duration,
    /// Write timeout for send operations.
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number; nanos: number }"))]
    write_timeout: Duration,
    /// Frame and retention limits for received data.
    buffer_config: BufferConfig,
}

impl Default for IoBounds {
    fn default() -> Self {
        Self::with_known_buffers(BufferConfig::DEFAULT)
    }
}

impl IoBounds {
    pub(crate) const TCP_DEFAULT: Self = Self::with_known_buffers(BufferConfig::for_tcp());
    pub(crate) const UDP_DEFAULT: Self = Self::with_known_buffers(BufferConfig::for_udp());
    pub(crate) const SERIAL_DEFAULT: Self = Self::with_known_buffers(BufferConfig::for_serial());

    const fn with_known_buffers(buffer_config: BufferConfig) -> Self {
        Self {
            read_timeout: DEFAULT_TIMEOUT,
            write_timeout: DEFAULT_TIMEOUT,
            buffer_config,
        }
    }

    /// Checks the supplied timeouts and buffer limits.
    ///
    /// # Errors
    ///
    /// Returns an error if a timeout is zero or not representable, or if the
    /// frame and retention bounds exceed their supported limits.
    pub fn new(
        read_timeout: Duration,
        write_timeout: Duration,
        buffer_config: BufferConfig,
    ) -> crate::Result<Self> {
        Self::default()
            .with_read_timeout(read_timeout)?
            .with_write_timeout(write_timeout)?
            .with_buffer_config(buffer_config)
    }

    /// Returns the receive timeout.
    #[must_use]
    pub const fn read_timeout(&self) -> Duration {
        self.read_timeout
    }

    /// Selects a nonzero receive timeout.
    ///
    /// # Errors
    ///
    /// Returns an error if the timeout is zero or not representable.
    pub fn with_read_timeout(mut self, value: Duration) -> crate::Result<Self> {
        validate_timeout("read", "read_timeout", value)?;
        self.read_timeout = value;
        Ok(self)
    }

    /// Returns the send timeout.
    #[must_use]
    pub const fn write_timeout(&self) -> Duration {
        self.write_timeout
    }

    /// Selects a nonzero send timeout.
    ///
    /// # Errors
    ///
    /// Returns an error if the timeout is zero or not representable.
    pub fn with_write_timeout(mut self, value: Duration) -> crate::Result<Self> {
        validate_timeout("write", "write_timeout", value)?;
        self.write_timeout = value;
        Ok(self)
    }

    /// Returns the receive frame and retention limits.
    #[must_use]
    pub const fn buffer_config(&self) -> BufferConfig {
        self.buffer_config
    }

    /// Selects checked receive frame and retention limits.
    ///
    /// # Errors
    ///
    /// Returns an error if either bound exceeds its supported limits or
    /// one complete frame would not fit within the retention limit.
    pub fn with_buffer_config(mut self, value: BufferConfig) -> crate::Result<Self> {
        let reason = if value.recv_buffer_size < BufferConfig::MIN_RECV_BUFFER_SIZE {
            format!(
                "transport receive buffer must hold the largest VISCA reply ({} bytes)",
                BufferConfig::MIN_RECV_BUFFER_SIZE
            )
            .into()
        } else if value.max_buffer_size == 0 {
            "transport maximum buffer must be non-zero".into()
        } else if value.recv_buffer_size > value.max_buffer_size {
            "transport receive buffer cannot exceed maximum buffer".into()
        } else if value.recv_buffer_size > BufferConfig::MAX_RECV_BUFFER_SIZE {
            "transport receive buffer cannot exceed 65535 bytes".into()
        } else if value.max_buffer_size > BufferConfig::MAX_RETENTION_SIZE {
            "transport maximum buffer cannot exceed 1048576 bytes".into()
        } else {
            self.buffer_config = value;
            return Ok(self);
        };
        Err(Error::InvalidRequest { reason })
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[allow(clippy::unwrap_used)]
    pub(crate) const fn receive_capacity(&self) -> std::num::NonZeroUsize {
        std::num::NonZeroUsize::new(self.buffer_config.recv_buffer_size()).unwrap()
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[allow(clippy::unwrap_used)]
    pub(crate) const fn read_timeout_bound(&self) -> NonZeroDuration {
        NonZeroDuration::checked(self.read_timeout).unwrap()
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[allow(clippy::unwrap_used)]
    pub(crate) const fn write_timeout_bound(&self) -> NonZeroDuration {
        NonZeroDuration::checked(self.write_timeout).unwrap()
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
struct IoBoundsData {
    read_timeout: Duration,
    write_timeout: Duration,
    buffer_config: BufferConfig,
}

#[cfg(feature = "serde")]
impl TryFrom<IoBoundsData> for IoBounds {
    type Error = Error;

    fn try_from(data: IoBoundsData) -> crate::Result<Self> {
        Self::new(data.read_timeout, data.write_timeout, data.buffer_config)
    }
}

/// Settings applied while opening a TCP connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct TcpConnect {
    /// Shared DNS and connection timeout.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "deserialize_connect_timeout")
    )]
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number; nanos: number }"))]
    connect_timeout: Duration,
    /// TCP_NODELAY, with `None` retaining the operating system default.
    nodelay: Option<bool>,
    /// IP TTL or IPv6 hop limit.
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deserialize_ttl"))]
    ttl: Option<u32>,
    /// OS-level TCP keepalive probes.
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deserialize_keepalive"))]
    keepalive: Option<TcpKeepaliveConfig>,
}

impl Default for TcpConnect {
    fn default() -> Self {
        Self {
            connect_timeout: DEFAULT_TIMEOUT,
            nodelay: Some(true),
            ttl: None,
            keepalive: Some(DEFAULT_TCP_KEEPALIVE),
        }
    }
}

impl TcpConnect {
    // Test connector error mapping with a setting the OS will reject.
    #[cfg(all(test, any(feature = "blocking", feature = "runtime-tokio")))]
    pub(crate) const fn with_unchecked_ttl(mut self, ttl: Option<u32>) -> Self {
        self.ttl = ttl;
        self
    }

    /// Returns the DNS and connection timeout.
    #[must_use]
    pub const fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    /// Selects a nonzero timeout for DNS and connection setup.
    ///
    /// # Errors
    ///
    /// Returns an error if the timeout is zero or not representable.
    pub fn with_connect_timeout(mut self, value: Duration) -> crate::Result<Self> {
        validate_timeout("connect", "connect_timeout", value)?;
        self.connect_timeout = value;
        Ok(self)
    }

    /// Returns whether TCP_NODELAY is requested, or `None` for the OS default.
    #[must_use]
    pub const fn nodelay(&self) -> Option<bool> {
        self.nodelay
    }

    /// Selects the TCP_NODELAY setting.
    #[must_use]
    pub const fn with_nodelay(mut self, value: Option<bool>) -> Self {
        self.nodelay = value;
        self
    }

    /// Returns the IPv4 TTL or IPv6 hop limit, or `None` for the OS default.
    #[must_use]
    pub const fn ttl(&self) -> Option<u32> {
        self.ttl
    }

    /// Selects the IP hop limit used from the first packet, including the SYN.
    ///
    /// # Errors
    ///
    /// Returns an error if a specified limit is outside `1..=255`.
    pub fn with_ttl(mut self, value: Option<u32>) -> crate::Result<Self> {
        validate_ttl(value)?;
        self.ttl = value;
        Ok(self)
    }

    /// Returns the OS-level TCP keepalive policy.
    #[must_use]
    pub const fn keepalive(&self) -> Option<TcpKeepaliveConfig> {
        self.keepalive
    }

    /// Selects OS-level keepalive probes, or `None` for the OS default.
    ///
    /// # Errors
    ///
    /// Returns an error if the idle time or interval is outside whole seconds
    /// in `1..=32767`.
    pub fn with_keepalive(mut self, value: Option<TcpKeepaliveConfig>) -> crate::Result<Self> {
        validate_keepalive(value)?;
        self.keepalive = value;
        Ok(self)
    }
}

/// Settings applied while opening a UDP connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct UdpConnect {
    /// Shared DNS and socket-setup timeout.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "deserialize_connect_timeout")
    )]
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number; nanos: number }"))]
    connect_timeout: Duration,
    /// IP TTL or IPv6 hop limit.
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deserialize_ttl"))]
    ttl: Option<u32>,
}

impl Default for UdpConnect {
    fn default() -> Self {
        Self {
            connect_timeout: DEFAULT_TIMEOUT,
            ttl: None,
        }
    }
}

impl UdpConnect {
    // Test connector error mapping with a setting the OS will reject.
    #[cfg(all(test, feature = "runtime-tokio"))]
    pub(crate) const fn with_unchecked_ttl(mut self, ttl: Option<u32>) -> Self {
        self.ttl = ttl;
        self
    }

    /// Returns the DNS and socket-setup timeout.
    #[must_use]
    pub const fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    /// Selects a nonzero timeout for DNS and socket setup.
    ///
    /// # Errors
    ///
    /// Returns an error if the timeout is zero or not representable.
    pub fn with_connect_timeout(mut self, value: Duration) -> crate::Result<Self> {
        validate_timeout("connect", "connect_timeout", value)?;
        self.connect_timeout = value;
        Ok(self)
    }

    /// Returns the IPv4 TTL or IPv6 hop limit, or `None` for the OS default.
    #[must_use]
    pub const fn ttl(&self) -> Option<u32> {
        self.ttl
    }

    /// Selects the IPv4 TTL or IPv6 hop limit.
    ///
    /// # Errors
    ///
    /// Returns an error if a specified limit is outside `1..=255`.
    pub fn with_ttl(mut self, value: Option<u32>) -> crate::Result<Self> {
        validate_ttl(value)?;
        self.ttl = value;
        Ok(self)
    }
}

#[cfg(feature = "serde")]
fn deserialize_connect_timeout<'de, D: serde::Deserializer<'de>>(
    de: D,
) -> Result<Duration, D::Error> {
    let value = serde::Deserialize::deserialize(de)?;
    validate_timeout("connect", "connect_timeout", value).map_err(serde::de::Error::custom)?;
    Ok(value)
}

#[cfg(feature = "serde")]
fn deserialize_ttl<'de, D: serde::Deserializer<'de>>(de: D) -> Result<Option<u32>, D::Error> {
    let value = serde::Deserialize::deserialize(de)?;
    validate_ttl(value).map_err(serde::de::Error::custom)?;
    Ok(value)
}

#[cfg(feature = "serde")]
fn deserialize_keepalive<'de, D: serde::Deserializer<'de>>(
    de: D,
) -> Result<Option<TcpKeepaliveConfig>, D::Error> {
    let value = serde::Deserialize::deserialize(de)?;
    validate_keepalive(value).map_err(serde::de::Error::custom)?;
    Ok(value)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn configuration_preserves_independent_socket_options_and_io_bounds() {
        let keepalive = TcpKeepaliveConfig::new(Duration::from_secs(30))
            .with_interval(Some(Duration::from_secs(7)));
        let tcp = TcpConnect::default()
            .with_keepalive(Some(keepalive))
            .expect("keepalive")
            .with_nodelay(None)
            .with_ttl(Some(32))
            .expect("TTL");
        let udp = UdpConnect::default().with_ttl(Some(32)).expect("TTL");
        assert_eq!(tcp.keepalive(), Some(keepalive));
        assert_eq!(tcp.nodelay(), None);
        assert_eq!(tcp.ttl(), udp.ttl());
        assert_eq!(
            IoBounds::TCP_DEFAULT
                .with_buffer_config(BufferConfig::default())
                .expect("explicit buffer selection"),
            IoBounds::default()
        );
    }

    #[test]
    fn settings_reject_invalid_budgets_and_buffer_bounds() {
        for timeout in [Duration::ZERO, Duration::MAX] {
            assert!(TcpConnect::default().with_connect_timeout(timeout).is_err());
            assert!(UdpConnect::default().with_connect_timeout(timeout).is_err());
            assert!(IoBounds::default().with_read_timeout(timeout).is_err());
            assert!(IoBounds::default().with_write_timeout(timeout).is_err());
        }
        for ttl in [0, 256, u32::MAX] {
            assert!(TcpConnect::default().with_ttl(Some(ttl)).is_err());
            assert!(UdpConnect::default().with_ttl(Some(ttl)).is_err());
        }
        for duration in [
            Duration::ZERO,
            Duration::from_millis(500),
            Duration::from_millis(1500),
            Duration::from_secs(32_768),
            Duration::MAX,
        ] {
            assert!(TcpConnect::default()
                .with_keepalive(Some(TcpKeepaliveConfig::new(duration)))
                .is_err());
            assert!(TcpConnect::default()
                .with_keepalive(Some(
                    TcpKeepaliveConfig::default().with_interval(Some(duration))
                ))
                .is_err());
        }
        for (receive, retention) in [
            (0, 64),
            (BufferConfig::MIN_RECV_BUFFER_SIZE - 1, 64),
            (64, 0),
            (65, 64),
            (
                BufferConfig::MAX_RECV_BUFFER_SIZE + 1,
                BufferConfig::MAX_RETENTION_SIZE,
            ),
            (64, BufferConfig::MAX_RETENTION_SIZE + 1),
            (usize::MAX, usize::MAX),
        ] {
            let buffers = BufferConfig::default()
                .with_recv_buffer_size(receive)
                .with_max_buffer_size(retention);
            assert!(IoBounds::default().with_buffer_config(buffers).is_err());
        }
        let buffers = BufferConfig::default()
            .with_recv_buffer_size(BufferConfig::MAX_RECV_BUFFER_SIZE)
            .with_max_buffer_size(BufferConfig::MAX_RETENTION_SIZE);
        assert!(IoBounds::default().with_buffer_config(buffers).is_ok());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_validates_bounds_and_kind_specific_settings() -> Result<(), serde_json::Error> {
        let bounds = IoBounds::default()
            .with_read_timeout(Duration::from_millis(37))
            .expect("valid timeout");
        let value = serde_json::to_value(bounds)?;
        assert_eq!(
            value["read_timeout"],
            serde_json::json!({"secs": 0, "nanos": 37_000_000})
        );
        assert_eq!(serde_json::from_value::<IoBounds>(value.clone())?, bounds);
        let mut invalid = value.clone();
        invalid["read_timeout"] = serde_json::json!({"secs": 0, "nanos": 0});
        assert!(serde_json::from_value::<IoBounds>(invalid).is_err());
        let mut invalid = value.clone();
        invalid["buffer_config"]["recv_buffer_size"] = 0.into();
        assert!(serde_json::from_value::<IoBounds>(invalid).is_err());
        let mut invalid = value;
        invalid["connect_timeout"] = serde_json::json!({"secs": 1, "nanos": 0});
        assert!(serde_json::from_value::<IoBounds>(invalid).is_err());

        let tcp = TcpConnect::default().with_nodelay(None);
        let value = serde_json::to_value(tcp)?;
        assert_eq!(value["nodelay"], serde_json::Value::Null);
        assert_eq!(serde_json::from_value::<TcpConnect>(value.clone())?, tcp);
        let mut invalid = value;
        invalid["keepalive"]["idle"] = serde_json::json!({"secs": 0, "nanos": 0});
        assert!(serde_json::from_value::<TcpConnect>(invalid).is_err());

        let udp = UdpConnect::default();
        let value = serde_json::to_value(udp)?;
        assert_eq!(serde_json::from_value::<UdpConnect>(value.clone())?, udp);
        let mut invalid = value.clone();
        invalid["ttl"] = 256.into();
        assert!(serde_json::from_value::<UdpConnect>(invalid).is_err());
        let mut invalid = value;
        invalid["nodelay"] = true.into();
        assert!(serde_json::from_value::<UdpConnect>(invalid).is_err());
        Ok(())
    }

    #[cfg(all(feature = "schemars", feature = "ts-rs"))]
    #[test]
    fn schema_and_typescript_describe_each_checked_settings_type() {
        use ts_rs::TS;

        let schema = schemars::schema_for!(IoBounds);
        assert_eq!(
            schema.get("additionalProperties"),
            Some(&serde_json::json!(false))
        );
        assert!(schema
            .get("properties")
            .and_then(|props| props.get("read_timeout"))
            .is_some());
        let ts = ts_rs::Config::default();
        for binding in [
            IoBounds::decl(&ts),
            TcpConnect::decl(&ts),
            UdpConnect::decl(&ts),
        ] {
            assert!(binding.contains("secs: number"));
            assert!(binding.contains("nanos: number"));
        }
        assert!(TcpConnect::decl(&ts).contains("nodelay"));
        assert!(TcpConnect::decl(&ts).contains("keepalive"));
        assert!(!UdpConnect::decl(&ts).contains("nodelay"));
        assert!(!UdpConnect::decl(&ts).contains("keepalive"));
        assert!(TcpKeepaliveConfig::decl(&ts).contains("| null"));
    }
}
