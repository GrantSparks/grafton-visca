//! Pure configuration for camera connection and behavior.
//!
//! This module provides a pure-data configuration struct that holds all the settings
//! needed to establish a camera connection. The configuration is separate from the
//! actual connection process, allowing for easy cloning, reuse, and modification.

use std::marker::PhantomData;

use crate::{
    camera_id::CameraId,
    capabilities::{Profile, SupportsSerial, SupportsTcp, SupportsUdp},
    error::Error,
    timeout::TimeoutConfig,
    transport::builder::TransportConfig,
};

#[cfg(any(feature = "async", feature = "blocking"))]
use crate::transport::{buffer::BufferConfig, builder::AddressingMode};

/// Standard transport kind used for profile support validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "kebab-case")
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum TransportKind {
    /// TCP network transport.
    Tcp,
    /// UDP network transport.
    Udp,
    /// Serial transport.
    Serial,
    /// Custom transport with no standard transport kind.
    Custom,
}

impl std::fmt::Display for TransportKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tcp => write!(f, "TCP"),
            Self::Udp => write!(f, "UDP"),
            Self::Serial => write!(f, "serial"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

/// Transport configuration options.
#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(tag = "type")
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum TransportOptions {
    /// TCP connection with address.
    #[cfg_attr(feature = "serde", serde(rename = "TCP"))]
    Tcp {
        /// Host:port string (e.g., "192.168.0.110:5678")
        address: String,
    },
    /// UDP connection with address.
    #[cfg_attr(feature = "serde", serde(rename = "UDP"))]
    Udp {
        /// Host:port string (e.g., "192.168.0.110:1259")
        address: String,
    },
    /// Serial connection.
    Serial {
        /// Serial port path (e.g., "/dev/ttyUSB0")
        port: String,
        /// Baud rate (default: 9600)
        baud_rate: u32,
    },
    /// Custom transport provided by user.
    Custom,
}

impl TransportOptions {
    /// Returns the selected transport kind.
    pub const fn kind(&self) -> TransportKind {
        match self {
            Self::Tcp { .. } => TransportKind::Tcp,
            Self::Udp { .. } => TransportKind::Udp,
            Self::Serial { .. } => TransportKind::Serial,
            Self::Custom => TransportKind::Custom,
        }
    }

    /// Validate this transport selection against built-in profile registry facts.
    pub fn validate_for_profile(
        &self,
        profile: crate::camera::profiles::ProfileId,
    ) -> Result<(), Error> {
        let transport = self.kind();
        if profile.supports_transport(transport) {
            Ok(())
        } else {
            Err(Error::UnsupportedTransport { profile, transport })
        }
    }

    /// Create TCP transport options.
    pub fn tcp(address: impl Into<String>) -> Self {
        Self::Tcp {
            address: address.into(),
        }
    }

    /// Create UDP transport options.
    pub fn udp(address: impl Into<String>) -> Self {
        Self::Udp {
            address: address.into(),
        }
    }

    /// Create serial transport options.
    pub fn serial(port: impl Into<String>, baud_rate: u32) -> Self {
        Self::Serial {
            port: port.into(),
            baud_rate,
        }
    }
}

/// Pure configuration for camera connection.
///
/// This struct holds all configuration needed to establish a camera connection
/// but performs no I/O itself. Configuration can be built, cloned, and reused.
///
/// # Example
///
/// ```ignore
/// use grafton_visca::camera::{CameraConfig, profiles::PtzOpticsG2};
/// use grafton_visca::timeout::TimeoutConfig;
/// use grafton_visca::runtime::TokioRuntime;
///
/// let config = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110")
///     .timeouts(TimeoutConfig::balanced());
///
/// // Configuration is pure data and can be cloned
/// let config2 = config.clone();
///
/// // Open async connection using the configuration
/// let runtime = TokioRuntime::from_current()?;
/// let camera1 = config.open_async(runtime).await?;
///
/// // Open blocking connection using the configuration
/// let camera2 = config2.open()?;
///
/// // Use accessor-style API
/// camera1.power().on().await?;
/// camera2.power().on()?;
/// ```
#[derive(Debug, Clone)]
pub struct CameraConfig<P> {
    /// Transport configuration.
    pub(crate) transport: TransportOptions,
    /// Default network port selected by the typed constructor or registry facts.
    pub(crate) network_default_port: Option<u16>,
    /// Command timeout configuration.
    pub(crate) timeouts: TimeoutConfig,
    /// Transport configuration for the underlying connection.
    pub(crate) transport_config: TransportConfig,
    /// Camera VISCA address (usually 1).
    pub(crate) camera_id: CameraId,
    /// Profile marker.
    pub(crate) _phantom: PhantomData<P>,
}

impl<P> CameraConfig<P>
where
    P: Profile,
{
    /// Create a new camera configuration for the specified profile.
    ///
    /// This initializes non-transport configuration with profile defaults.
    /// Select a standard transport explicitly with [`CameraConfig::tcp`],
    /// [`CameraConfig::udp`], or [`CameraConfig::serial`].
    pub fn new() -> Self {
        Self {
            transport: TransportOptions::Custom,
            network_default_port: None,
            timeouts: TimeoutConfig::default(),
            transport_config: TransportConfig::default(),
            camera_id: CameraId::new(P::DEFAULT_CAMERA_ID).unwrap_or_default(),
            _phantom: PhantomData,
        }
    }

    /// Convenience constructor using the `for` naming convention.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = CameraConfig::<SonyFR7>::udp("192.168.0.108");
    /// ```
    pub fn for_camera() -> Self {
        Self::new()
    }

    /// Create a TCP camera configuration.
    pub fn tcp(address: impl Into<String>) -> Self
    where
        P: SupportsTcp,
    {
        Self::new().with_transport(
            TransportOptions::tcp(address),
            Some(<P as SupportsTcp>::DEFAULT_TCP_PORT),
        )
    }

    /// Create a UDP camera configuration.
    pub fn udp(address: impl Into<String>) -> Self
    where
        P: SupportsUdp,
    {
        Self::new().with_transport(
            TransportOptions::udp(address),
            Some(<P as SupportsUdp>::DEFAULT_UDP_PORT),
        )
    }

    /// Create a serial camera configuration.
    pub fn serial(port: impl Into<String>, baud_rate: u32) -> Self
    where
        P: SupportsSerial,
    {
        Self::new().with_transport(TransportOptions::serial(port, baud_rate), None)
    }

    fn with_transport(mut self, transport: TransportOptions, default_port: Option<u16>) -> Self {
        self.transport = transport;
        self.network_default_port = default_port;
        self
    }

    /// Set the connection address.
    ///
    /// For TCP/UDP, this may be a host:port string like `"192.168.0.110:5678"`
    /// or a host-only address when a profile/default port is available.
    ///
    /// Bare IPv6 may be used only when the port comes from the profile/default
    /// port. Explicit IPv6 ports require brackets, so `2001:db8::1:5678` is a
    /// bare IPv6 literal and `[2001:db8::1]:5678` is an IPv6 address with port
    /// `5678`.
    ///
    /// Examples:
    /// - IPv4: `"192.168.1.1"`, `"192.168.1.1:5678"`
    /// - IPv6: `"::1"`, `"2001:db8::1"`, `"[::1]:5678"`
    /// - Hostnames: `"localhost"`, `"camera.local:5678"`
    pub fn address(mut self, address: impl Into<String>) -> Self {
        let addr = address.into();

        // Update transport with new address, preserving transport type
        self.transport = match self.transport {
            TransportOptions::Tcp { .. } => TransportOptions::tcp(addr),
            TransportOptions::Udp { .. } => TransportOptions::udp(addr),
            other => other,
        };
        self
    }

    /// Set custom transport options.
    pub fn transport(mut self, transport: TransportOptions) -> Self {
        self.network_default_port = match transport.kind() {
            TransportKind::Tcp => P::PROFILE_ID.and_then(|profile| profile.default_tcp_port()),
            TransportKind::Udp => P::PROFILE_ID.and_then(|profile| profile.default_udp_port()),
            TransportKind::Serial | TransportKind::Custom => None,
        };
        self.transport = transport;
        self
    }

    /// Clears the profile-derived network port.  This is crate-private because
    /// it is only needed by the typed connection builder's explicit
    /// `with_default_port` opt-in; [`Self::tcp`] and [`Self::udp`] retain their
    /// documented profile defaults.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn without_network_default_port(mut self) -> Self {
        self.network_default_port = None;
        self
    }

    /// Set timeout configuration.
    pub fn timeouts(mut self, timeouts: TimeoutConfig) -> Self {
        self.timeouts = timeouts;
        self
    }

    /// Set transport configuration.
    pub fn transport_config(mut self, transport_config: TransportConfig) -> Self {
        self.transport_config = transport_config;
        self
    }

    /// Set retry configuration.
    pub fn retry_config(mut self, retry_config: crate::transport::RetryConfig) -> Self {
        self.transport_config.retry_config = retry_config;
        self
    }

    /// Set camera VISCA address.
    pub fn camera_id(mut self, id: CameraId) -> Self {
        self.camera_id = id;
        self
    }

    /// Set camera VISCA address from a raw numeric ID.
    ///
    /// # Errors
    /// Returns an error if `id` is outside the supported VISCA camera ID range.
    pub fn try_camera_id(self, id: u8) -> Result<Self, Error> {
        Ok(self.camera_id(CameraId::new(id)?))
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    fn defaulted_buffer_config(&self, transport_default: BufferConfig) -> BufferConfig {
        if self.transport_config.buffer_config == BufferConfig::default() {
            transport_default
        } else {
            self.transport_config.buffer_config
        }
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn tcp_transport_config(&self) -> TransportConfig {
        TransportConfig {
            buffer_config: self.defaulted_buffer_config(BufferConfig::for_raw_ip()),
            addressing: AddressingMode::Ip,
            ..self.transport_config
        }
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn udp_transport_config(&self) -> TransportConfig {
        TransportConfig {
            buffer_config: self.defaulted_buffer_config(BufferConfig::for_udp()),
            addressing: AddressingMode::Ip,
            ..self.transport_config
        }
    }

    /// Validate this configuration before any transport I/O.
    pub fn validate(&self) -> Result<(), Error> {
        if let Some(profile) = P::PROFILE_ID {
            self.transport.validate_for_profile(profile)?;
        }
        Ok(())
    }

    #[cfg(any(
        feature = "transport-serial-tokio",
        all(feature = "blocking", feature = "transport-serial")
    ))]
    pub(crate) fn serial_transport_config(&self) -> TransportConfig {
        TransportConfig {
            buffer_config: self.defaulted_buffer_config(BufferConfig::for_serial()),
            addressing: AddressingMode::Serial,
            tcp_nodelay: None,
            ttl: None,
            tcp_keepalive: None,
            ..self.transport_config
        }
    }

    #[cfg(any(
        feature = "transport-serial-tokio",
        all(feature = "blocking", feature = "transport-serial")
    ))]
    pub(crate) fn serial_config(
        &self,
        port: &str,
        baud_rate: u32,
    ) -> crate::transport::serial::Config {
        let transport_config = self.serial_transport_config();

        crate::transport::serial::Config::new(port.to_string())
            .baud_rate(baud_rate)
            .camera_address(self.camera_id.id())
            .read_timeout(transport_config.read_timeout)
            .write_timeout(transport_config.write_timeout)
            .retry_config(transport_config.retry_config)
            .buffer_config(transport_config.buffer_config)
    }
}

// The owner-backed facades use the same public configuration values and lower
// them once into immutable session tuning at the construction boundary.
#[cfg(any(feature = "async", feature = "blocking"))]
impl<P> CameraConfig<P>
where
    P: crate::profile::CompileTimeProfile,
{
    pub(crate) fn owner_tuning(&self) -> crate::OperationalTuning {
        use crate::transport::BackoffStrategy;

        let timeout = self.timeouts;
        let retry = self.transport_config.retry_config;

        // The owner has one immutable completion budget and one immutable
        // settlement budget. Use the largest configured timeout so a long or
        // preset operation cannot be shortened when lowered into the final
        // request model.
        let completion = timeout
            .quick_timeout
            .max(timeout.movement_timeout)
            .max(timeout.preset_timeout)
            .max(timeout.long_timeout)
            .max(timeout.network_timeout)
            .max(timeout.default_timeout);
        let settlement = completion;

        let maximum_backoff = match retry.backoff_strategy {
            BackoffStrategy::Constant => retry.base_retry_delay,
            BackoffStrategy::Exponential => {
                let mut value = retry.base_retry_delay;
                for _ in 0..retry.max_retries.min(31) {
                    value = value.saturating_mul(2);
                }
                value.min(retry.max_retry_duration)
            }
        };

        crate::OperationalTuning::new()
            .ack_timeout(timeout.ack_timeout)
            .completion_timeout(completion)
            .settlement_timeout(settlement)
            .inquiry_timeout(timeout.quick_timeout.max(timeout.network_timeout))
            .retry_limit(retry.max_retries)
            .retry_timing(
                retry.base_retry_delay,
                maximum_backoff,
                retry.max_retry_duration,
            )
    }

    pub(crate) fn owner_profile_config(&self) -> crate::Result<crate::SessionConfig> {
        let profile = crate::ProfileSpec::from_compile_time::<P>()?;
        crate::SessionConfig::for_target(self.camera_id, profile)?.with_tuning(self.owner_tuning())
    }

    pub(crate) fn owner_default_port(&self, kind: TransportKind) -> Option<u16> {
        self.network_default_port.or_else(|| match kind {
            TransportKind::Tcp => P::TRANSPORTS.tcp_port(),
            TransportKind::Udp => P::TRANSPORTS.udp_port(),
            TransportKind::Serial | TransportKind::Custom => None,
        })
    }

    /// Returns the validated, pure [`crate::SessionConfig`] that standard
    /// construction will pass to the owner-backed session.
    ///
    /// This performs profile and tuning validation but no endpoint parsing,
    /// DNS lookup, device open, task spawn, or protocol I/O. It is useful when
    /// an application needs to inspect or compose the shared session policy
    /// before selecting an async or blocking transport.
    pub fn session_config(&self) -> crate::Result<crate::SessionConfig> {
        self.validate()?;
        self.owner_profile_config()
    }
}

/// Pure standard TCP/UDP construction state shared by the async and blocking
/// owner-backed constructors. No connector or socket is created while this
/// plan is being built.
#[derive(Debug, Clone)]
#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) struct StandardConnectionPlan {
    pub(crate) kind: TransportKind,
    pub(crate) endpoint: String,
    pub(crate) transport_config: TransportConfig,
    pub(crate) session_config: crate::SessionConfig,
}

#[cfg(any(feature = "async", feature = "blocking"))]
impl<P> CameraConfig<P>
where
    P: crate::profile::CompileTimeProfile,
{
    /// Resolves all standard profile/configuration/endpoint state before any
    /// runtime connector or blocking socket is entered.
    pub(crate) fn standard_connection_plan(&self) -> crate::Result<StandardConnectionPlan> {
        let session_config = self.session_config()?;
        let (kind, address, default_port, transport_config) = match &self.transport {
            TransportOptions::Tcp { address } => (
                TransportKind::Tcp,
                address.as_str(),
                self.owner_default_port(TransportKind::Tcp),
                self.tcp_transport_config(),
            ),
            TransportOptions::Udp { address } => (
                TransportKind::Udp,
                address.as_str(),
                self.owner_default_port(TransportKind::Udp),
                self.udp_transport_config(),
            ),
            TransportOptions::Serial { .. } => {
                return Err(Error::InvalidState(
                    "serial transport requires its serial construction path".into(),
                ));
            }
            TransportOptions::Custom => {
                return Err(Error::InvalidState(
                    "custom transport requires Session::open".into(),
                ));
            }
        };

        session_config.validate_for_transport(Some(kind))?;
        let endpoint = crate::transport::address::canonicalize_endpoint(address, default_port)?;
        Ok(StandardConnectionPlan {
            kind,
            endpoint,
            transport_config,
            session_config,
        })
    }
}

impl<P> Default for CameraConfig<P>
where
    P: Profile,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Canonical owner-backed standard async construction.
#[cfg(feature = "async")]
impl<P> CameraConfig<P>
where
    P: crate::profile::CompileTimeProfile,
{
    /// Opens a standard TCP or UDP connection and starts one owner session.
    pub async fn open_async<R>(&self, runtime: R) -> crate::Result<crate::Session>
    where
        R: crate::runtime::Runtime,
    {
        use crate::runtime::TransportHandle;

        let plan = self.standard_connection_plan()?;
        let transport = match plan.kind {
            TransportKind::Tcp => {
                let tcp = runtime
                    .connect_tcp(&plan.endpoint, plan.transport_config)
                    .await?;
                TransportHandle::<R>::Tcp(tcp)
            }
            TransportKind::Udp => {
                let udp = runtime
                    .connect_udp(&plan.endpoint, plan.transport_config)
                    .await?;
                TransportHandle::<R>::Udp(udp)
            }
            _ => unreachable!("standard connection plan only contains network transports"),
        };

        crate::Session::open::<R, _>(transport, plan.session_config, runtime).await
    }

    /// Opens a standard TCP or UDP connection and starts one single-camera
    /// owner session.
    ///
    /// This is the configured counterpart of
    /// [`Connect::open_tcp_camera`](crate::camera::Connect::open_tcp_camera):
    /// the profile is named once, by this configuration, and the returned
    /// [`crate::CameraSession`] owns the camera bound to that same `P`.
    pub async fn open_camera_async<R>(&self, runtime: R) -> crate::Result<crate::CameraSession<P>>
    where
        R: crate::runtime::Runtime,
    {
        let session = self.open_async(runtime).await?;
        crate::CameraSession::from_session(session, self.camera_id)
    }

    /// Opens the configured Tokio serial transport through the owner session.
    #[cfg(feature = "transport-serial-tokio")]
    pub async fn open_serial_async<R>(&self, runtime: R) -> crate::Result<crate::Session>
    where
        R: crate::runtime::Runtime
            + crate::runtime::RuntimeSerial<SerialTransport = crate::transport::tokio::serial::Serial>,
    {
        use crate::runtime::TransportHandle;

        let session_config = self.session_config()?;
        session_config.validate_for_transport(Some(TransportKind::Serial))?;

        let (port, baud_rate) = match &self.transport {
            TransportOptions::Serial { port, baud_rate } => (port, *baud_rate),
            _ => {
                return Err(Error::InvalidState(
                    "open_serial_async requires serial transport configuration".into(),
                ));
            }
        };
        let serial = runtime
            .connect_serial(self.serial_config(port, baud_rate))
            .await?;
        crate::Session::open::<R, _>(
            TransportHandle::<R>::Serial(Box::new(serial)),
            session_config,
            runtime,
        )
        .await
    }
}

/// Canonical owner-backed blocking construction.
#[cfg(feature = "blocking")]
impl<P> CameraConfig<P>
where
    P: crate::profile::CompileTimeProfile,
{
    /// Opens a configured standard blocking TCP or UDP owner session.
    pub fn open(&self) -> crate::Result<crate::blocking::Session> {
        use crate::transport::blocking::{Tcp, Udp};

        let plan = self.standard_connection_plan()?;
        let transport = match plan.kind {
            TransportKind::Tcp => crate::transport::BlockingTransportHandle::Tcp(
                Tcp::connect_with_config(&plan.endpoint, plan.transport_config)?,
            ),
            TransportKind::Udp => crate::transport::BlockingTransportHandle::Udp(
                Udp::connect_with_config(&plan.endpoint, plan.transport_config)?,
            ),
            _ => unreachable!("standard connection plan only contains network transports"),
        };

        crate::blocking::Session::open(transport, plan.session_config)
    }

    /// Opens a configured standard blocking TCP or UDP single-camera session.
    ///
    /// This is the configured counterpart of
    /// [`blocking::Connect::open_tcp_camera`](crate::blocking::Connect::open_tcp_camera):
    /// the profile is named once, by this configuration, and the returned
    /// [`crate::blocking::CameraSession`] hands out the camera bound to that
    /// same `P`.
    pub fn open_camera(&self) -> crate::Result<crate::blocking::CameraSession<P>> {
        let session = self.open()?;
        crate::blocking::CameraSession::from_session(session, self.camera_id)
    }

    /// Opens a configured blocking serial owner session.
    #[cfg(feature = "transport-serial")]
    pub fn open_serial(&self) -> crate::Result<crate::blocking::Session> {
        let session_config = self.session_config()?;
        session_config.validate_for_transport(Some(TransportKind::Serial))?;
        let (port, baud_rate) = match &self.transport {
            TransportOptions::Serial { port, baud_rate } => (port, *baud_rate),
            _ => {
                return Err(Error::InvalidState(
                    "open_serial requires serial transport configuration".into(),
                ));
            }
        };
        let serial = crate::transport::serial_blocking::SerialTransport::new(
            self.serial_config(port, baud_rate),
        )?;
        let transport = crate::transport::BlockingTransportHandle::Serial(serial);
        crate::blocking::Session::open(transport, session_config)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn canonical_standard_plan_records_defaults_and_transport_options_without_io() {
        use std::time::Duration;

        use crate::{
            camera::{CameraConfig, TransportKind, TransportOptions},
            profiles::{PtzOpticsG2, SonyFR7},
            transport::{AddressingMode, BufferConfig, TcpKeepaliveConfig, TransportConfig},
        };

        let transport_config = TransportConfig {
            connect_timeout: Duration::from_millis(17),
            read_timeout: Duration::from_millis(19),
            write_timeout: Duration::from_millis(23),
            buffer_config: BufferConfig {
                recv_buffer_size: 11,
                send_buffer_size: 13,
                max_buffer_size: 17,
            },
            tcp_nodelay: Some(false),
            tcp_keepalive: Some(TcpKeepaliveConfig::new(Duration::from_secs(4))),
            ttl: Some(41),
            addressing: AddressingMode::Ip,
            ..TransportConfig::default()
        };

        let tcp = CameraConfig::<PtzOpticsG2>::tcp("camera.local")
            .transport_config(transport_config)
            .standard_connection_plan()
            .expect("TCP plan");
        assert_eq!(tcp.kind, TransportKind::Tcp);
        assert_eq!(tcp.endpoint, "camera.local:5678");
        assert_eq!(
            tcp.transport_config.connect_timeout,
            Duration::from_millis(17)
        );
        assert_eq!(tcp.transport_config.read_timeout, Duration::from_millis(19));
        assert_eq!(
            tcp.transport_config.write_timeout,
            Duration::from_millis(23)
        );
        assert_eq!(
            tcp.transport_config.buffer_config,
            transport_config.buffer_config
        );
        assert_eq!(tcp.transport_config.tcp_nodelay, Some(false));
        assert_eq!(
            tcp.transport_config.tcp_keepalive,
            transport_config.tcp_keepalive
        );
        assert_eq!(tcp.transport_config.ttl, Some(41));
        assert_eq!(tcp.transport_config.addressing, AddressingMode::Ip);

        let udp = CameraConfig::<PtzOpticsG2>::udp("camera.local")
            .transport_config(transport_config)
            .standard_connection_plan()
            .expect("UDP plan");
        assert_eq!(udp.kind, TransportKind::Udp);
        assert_eq!(udp.endpoint, "camera.local:1259");
        assert_eq!(
            udp.transport_config.connect_timeout,
            Duration::from_millis(17)
        );
        assert_eq!(udp.transport_config.read_timeout, Duration::from_millis(19));
        assert_eq!(
            udp.transport_config.write_timeout,
            Duration::from_millis(23)
        );
        assert_eq!(
            udp.transport_config.buffer_config,
            transport_config.buffer_config
        );
        assert_eq!(udp.transport_config.ttl, Some(41));
        assert_eq!(udp.transport_config.addressing, AddressingMode::Ip);

        let sony = CameraConfig::<SonyFR7>::udp("camera.local")
            .standard_connection_plan()
            .expect("Sony UDP plan");
        assert_eq!(sony.endpoint, "camera.local:52381");

        let mut connector_calls = 0;
        let unsupported = CameraConfig::<SonyFR7>::new()
            .transport(TransportOptions::tcp("does-not-resolve.invalid"))
            .standard_connection_plan();
        if unsupported.is_ok() {
            connector_calls += 1;
        }
        assert!(unsupported.is_err());
        assert_eq!(connector_calls, 0);

        let malformed =
            CameraConfig::<PtzOpticsG2>::udp("invalid:address:format").standard_connection_plan();
        if malformed.is_ok() {
            connector_calls += 1;
        }
        assert!(malformed.is_err());
        assert_eq!(connector_calls, 0);
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn canonical_session_config_lowers_timeout_and_retry_policy() {
        use std::time::Duration;

        use crate::{
            camera::CameraConfig,
            profile::OperationalTuning,
            profiles::PtzOpticsG2,
            timeout::TimeoutConfig,
            transport::{BackoffStrategy, RetryConfig},
        };

        let timeouts = TimeoutConfig {
            ack_timeout: Duration::from_secs(61),
            quick_timeout: Duration::from_secs(67),
            movement_timeout: Duration::from_secs(71),
            preset_timeout: Duration::from_secs(73),
            long_timeout: Duration::from_secs(79),
            network_timeout: Duration::from_secs(83),
            default_timeout: Duration::from_secs(89),
        };
        let retry = RetryConfig {
            max_retries: 2,
            base_retry_delay: Duration::from_secs(3),
            max_retry_duration: Duration::from_secs(17),
            backoff_strategy: BackoffStrategy::Exponential,
        };
        let config = CameraConfig::<PtzOpticsG2>::udp("camera.local")
            .timeouts(timeouts)
            .retry_config(retry);

        let expected = OperationalTuning::new()
            .ack_timeout(Duration::from_secs(61))
            .completion_timeout(Duration::from_secs(89))
            .settlement_timeout(Duration::from_secs(89))
            .inquiry_timeout(Duration::from_secs(83))
            .retry_limit(2)
            .retry_timing(
                Duration::from_secs(3),
                Duration::from_secs(12),
                Duration::from_secs(17),
            );
        assert_eq!(
            config.session_config().expect("session config").tuning(),
            expected
        );
    }

    #[cfg(any(
        feature = "transport-serial-tokio",
        all(feature = "blocking", feature = "transport-serial")
    ))]
    #[test]
    fn serial_plan_preserves_device_config_before_open() {
        use std::time::Duration;

        use crate::{
            camera::CameraConfig,
            profiles::PtzOpticsG2,
            transport::{
                AddressingMode, BackoffStrategy, BufferConfig, RetryConfig, TransportConfig,
            },
        };

        let retry = RetryConfig {
            max_retries: 4,
            base_retry_delay: Duration::from_millis(7),
            max_retry_duration: Duration::from_millis(31),
            backoff_strategy: BackoffStrategy::Constant,
        };
        let transport_config = TransportConfig {
            read_timeout: Duration::from_millis(37),
            write_timeout: Duration::from_millis(41),
            retry_config: retry,
            buffer_config: BufferConfig {
                recv_buffer_size: 43,
                send_buffer_size: 47,
                max_buffer_size: 53,
            },
            addressing: AddressingMode::Ip,
            ..TransportConfig::default()
        };
        let config = CameraConfig::<PtzOpticsG2>::serial("/dev/fake-visca", 38_400)
            .transport_config(transport_config);
        let serial = config.serial_config("/dev/fake-visca", 38_400);
        assert_eq!(serial.port, "/dev/fake-visca");
        assert_eq!(serial.baud_rate, 38_400);
        assert_eq!(serial.read_timeout, Duration::from_millis(37));
        assert_eq!(serial.write_timeout, Duration::from_millis(41));
        assert_eq!(serial.retry_config.max_retries, 4);
        assert_eq!(
            serial.retry_config.base_retry_delay,
            Duration::from_millis(7)
        );
        assert_eq!(serial.buffer_config, transport_config.buffer_config);
        assert_eq!(
            config.serial_transport_config().addressing,
            AddressingMode::Serial
        );
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_transport_options_serialization() {
        use super::TransportOptions;

        // Test TCP serialization
        let tcp = TransportOptions::Tcp {
            address: "192.168.0.110:5678".to_string(),
        };
        if let Ok(json) = serde_json::to_value(&tcp) {
            assert_eq!(json["type"], "TCP");
            assert_eq!(json["address"], "192.168.0.110:5678");
            if let Ok(pretty) = serde_json::to_string_pretty(&tcp) {
                println!("TCP: {pretty}");
            }
        }

        // Test UDP serialization
        let udp = TransportOptions::Udp {
            address: "192.168.0.110:1259".to_string(),
        };
        if let Ok(json) = serde_json::to_value(&udp) {
            assert_eq!(json["type"], "UDP");
            assert_eq!(json["address"], "192.168.0.110:1259");
            if let Ok(pretty) = serde_json::to_string_pretty(&udp) {
                println!("UDP: {pretty}");
            }
        }

        // Test Serial serialization
        let serial = TransportOptions::Serial {
            port: "/dev/ttyUSB0".to_string(),
            baud_rate: 9600,
        };
        if let Ok(json) = serde_json::to_value(&serial) {
            assert_eq!(json["type"], "Serial");
            assert_eq!(json["port"], "/dev/ttyUSB0");
            assert_eq!(json["baud_rate"], 9600);
            if let Ok(pretty) = serde_json::to_string_pretty(&serial) {
                println!("Serial: {pretty}");
            }
        }
    }
}
