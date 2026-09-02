//! Pure configuration for camera connection and behavior.
//!
//! This module provides a pure-data configuration struct that holds all the settings
//! needed to establish a camera connection. The configuration is separate from the
//! actual connection process, allowing for easy cloning, reuse, and modification.

use std::{marker::PhantomData, num::NonZeroUsize};

use crate::{
    camera_id::CameraId,
    capabilities::{Profile, SupportsSerial, SupportsTcp, SupportsUdp},
    error::Error,
    transport::builder::TransportConfig,
    OperationalTuning,
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

/// How a standard network constructor obtains a port for a host-only address.
///
/// An `Option<u16>` cannot distinguish an explicitly selected default from
/// the normal profile-default behavior, so keep those policies separate until
/// endpoint canonicalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NetworkDefaultPort {
    /// Use the selected profile's TCP or UDP default port.
    Profile,
    /// Use this constructor-selected default instead of looking up profile facts.
    Explicit(u16),
    /// Require an explicit port in the supplied endpoint.
    Disabled,
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
/// # Examples
///
/// A configuration is pure data and can be reused for either owner-backed
/// facade. The standard network constructors return a `CameraSession<P>`; obtain
/// its profile-bound `Camera` with `CameraSession<P>::camera()` before using
/// noun accessors.
///
/// ```rust,no_run
/// # #[cfg(feature = "blocking")]
/// # fn blocking_example() -> grafton_visca::Result<()> {
/// use grafton_visca::camera::{profiles::PtzOpticsG2, CameraConfig};
/// use grafton_visca::OperationalTuning;
///
/// let config = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110")
///     .with_tuning(OperationalTuning::new());
/// let session = config.open()?;
/// session.camera().power().on()?;
/// session.close()?;
/// # Ok(())
/// # }
/// ```
///
/// ```rust,no_run
/// # #[cfg(feature = "runtime-tokio")]
/// # async fn async_example() -> grafton_visca::Result<()> {
/// use grafton_visca::camera::{profiles::PtzOpticsG2, CameraConfig};
/// use grafton_visca::runtime::TokioRuntime;
///
/// let config = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110");
/// let runtime = TokioRuntime::from_current()?;
/// let session = config.open_async(runtime).await?;
/// session.camera().power().on().await?;
/// session.close().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct CameraConfig<P> {
    /// Transport configuration.
    pub(crate) transport: TransportOptions,
    /// Policy for resolving a missing TCP or UDP port.
    network_default_port: NetworkDefaultPort,
    /// Validated operational overrides applied to prepared requests.
    pub(crate) tuning: OperationalTuning,
    /// Immutable request-admission capacity passed to the owner session.
    pub(crate) admission_capacity: NonZeroUsize,
    /// Whether to reset Sony VISCA-over-IP sequence state during session open.
    pub(crate) sony_sequence_reset_on_connect: bool,
    /// Whether raw correlation uncertainty poisons the entire session.
    pub(crate) strict_unconfirmed_poison: bool,
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
            network_default_port: NetworkDefaultPort::Profile,
            tuning: OperationalTuning::new(),
            admission_capacity: crate::SessionConfig::default().admission_capacity(),
            sony_sequence_reset_on_connect: false,
            strict_unconfirmed_poison: false,
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
            NetworkDefaultPort::Explicit(<P as SupportsTcp>::DEFAULT_TCP_PORT),
        )
    }

    /// Create a UDP camera configuration.
    pub fn udp(address: impl Into<String>) -> Self
    where
        P: SupportsUdp,
    {
        Self::new().with_transport(
            TransportOptions::udp(address),
            NetworkDefaultPort::Explicit(<P as SupportsUdp>::DEFAULT_UDP_PORT),
        )
    }

    /// Create a serial camera configuration.
    pub fn serial(port: impl Into<String>, baud_rate: u32) -> Self
    where
        P: SupportsSerial,
    {
        Self::new().with_transport(
            TransportOptions::serial(port, baud_rate),
            NetworkDefaultPort::Disabled,
        )
    }

    fn with_transport(
        mut self,
        transport: TransportOptions,
        default_port: NetworkDefaultPort,
    ) -> Self {
        self.transport = transport;
        self.network_default_port = default_port;
        self
    }

    /// Set the connection address.
    ///
    /// For TCP/UDP, this may be a host:port string like `"192.168.0.110:5678"`
    /// or a host-only address when a profile/default port is available.
    ///
    /// IPv6 literals must be bracketed, whether the port is supplied explicitly
    /// or comes from the profile/default port. Bare IPv6 is rejected because its
    /// colons are ambiguous with an explicit port: `2001:db8::1` and
    /// `2001:db8::1:5678` are invalid, while `[2001:db8::1]` uses the default
    /// port and `[2001:db8::1]:5678` is an IPv6 address with port `5678`.
    ///
    /// Examples:
    /// - IPv4: `"192.168.1.1"`, `"192.168.1.1:5678"`
    /// - IPv6: `"[::1]"`, `"[2001:db8::1]"`, `"[::1]:5678"`
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
            TransportKind::Tcp | TransportKind::Udp => NetworkDefaultPort::Profile,
            TransportKind::Serial | TransportKind::Custom => NetworkDefaultPort::Disabled,
        };
        self.transport = transport;
        self
    }

    /// Set operational overrides for prepared requests.
    pub fn with_tuning(mut self, tuning: OperationalTuning) -> Self {
        self.tuning = tuning;
        self
    }

    /// Returns the operational overrides stored in this configuration.
    #[must_use]
    pub const fn tuning(&self) -> OperationalTuning {
        self.tuning
    }

    /// Sets the immutable request-admission capacity for sessions opened from
    /// this configuration.
    ///
    /// Admission is fail-fast once this many requests are pending or active;
    /// it is independent of transport buffers and per-camera VISCA socket
    /// capacity.
    #[must_use]
    pub const fn with_admission_capacity(mut self, capacity: NonZeroUsize) -> Self {
        self.admission_capacity = capacity;
        self
    }

    /// Returns the request-admission capacity stored in this configuration.
    #[must_use]
    pub const fn admission_capacity(&self) -> NonZeroUsize {
        self.admission_capacity
    }

    /// Opt in to sending Sony's sequence-number RESET control command when a
    /// session opened from this configuration connects.
    #[must_use]
    pub const fn with_sony_sequence_reset_on_connect(mut self, enabled: bool) -> Self {
        self.sony_sequence_reset_on_connect = enabled;
        self
    }

    /// Returns whether Sony sequence state is reset when the session connects.
    #[must_use]
    pub const fn sony_sequence_reset_on_connect(&self) -> bool {
        self.sony_sequence_reset_on_connect
    }

    /// Selects strict whole-session poisoning for unconfirmable raw commands.
    ///
    /// This is immutable construction policy; runtime tuning remains fully
    /// replaceable after the session opens. See
    /// [`crate::SessionConfig::with_strict_unconfirmed_poison`].
    #[must_use]
    pub const fn with_strict_unconfirmed_poison(mut self, enabled: bool) -> Self {
        self.strict_unconfirmed_poison = enabled;
        self
    }

    /// Returns whether raw correlation uncertainty poisons the whole session.
    #[must_use]
    pub const fn strict_unconfirmed_poison(&self) -> bool {
        self.strict_unconfirmed_poison
    }

    /// Set transport configuration.
    pub fn transport_config(mut self, transport_config: TransportConfig) -> Self {
        self.transport_config = transport_config;
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
    ) -> crate::Result<crate::transport::serial::Config> {
        let transport_config = self.serial_transport_config();
        transport_config.validate_buffer_bounds()?;

        Ok(crate::transport::serial::Config::new(port.to_string())
            .baud_rate(baud_rate)
            .camera_address(self.camera_id.id())
            .read_timeout(transport_config.read_timeout)
            .write_timeout(transport_config.write_timeout)
            .buffer_config(transport_config.buffer_config))
    }
}

// The owner-backed facades use the same public configuration values and lower
// them once into immutable session tuning at the construction boundary.
#[cfg(any(feature = "async", feature = "blocking"))]
impl<P> CameraConfig<P>
where
    P: crate::profile::CompileTimeProfile,
{
    /// Validate profile, tuning, and transport facts before any transport I/O.
    pub fn validate(&self) -> crate::Result<()> {
        let profile = crate::ProfileSpec::from_compile_time::<P>()?;
        profile.validate_tuning(self.tuning)?;
        if self.sony_sequence_reset_on_connect
            && profile.envelope() != crate::profile::ProfileEnvelope::SonyEncapsulated
        {
            return Err(Error::InvalidRequest(
                "Sony sequence reset on connect requires a Sony encapsulated profile".into(),
            ));
        }
        if let Some(profile_id) = P::PROFILE_ID {
            self.transport.validate_for_profile(profile_id)?;
        }
        Ok(())
    }

    pub(crate) fn owner_tuning(&self) -> OperationalTuning {
        self.tuning
    }

    pub(crate) fn owner_profile_config(&self) -> crate::Result<crate::SessionConfig> {
        let profile = crate::ProfileSpec::from_compile_time::<P>()?;
        let config = crate::SessionConfig::for_target(self.camera_id, profile)?
            .with_tuning(self.owner_tuning())?;
        Ok(config
            .with_admission_capacity(self.admission_capacity)
            .with_sony_sequence_reset_on_connect(self.sony_sequence_reset_on_connect)
            .with_strict_unconfirmed_poison(self.strict_unconfirmed_poison))
    }

    pub(crate) fn owner_default_port(&self, kind: TransportKind) -> Option<u16> {
        match self.network_default_port {
            NetworkDefaultPort::Profile => match kind {
                TransportKind::Tcp => P::TRANSPORTS.tcp_port(),
                TransportKind::Udp => P::TRANSPORTS.udp_port(),
                TransportKind::Serial | TransportKind::Custom => None,
            },
            NetworkDefaultPort::Explicit(port) => Some(port),
            NetworkDefaultPort::Disabled => None,
        }
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
        transport_config.validate_buffer_bounds()?;
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
    /// Opens a standard TCP or UDP connection and starts one single-camera
    /// owner session.
    pub async fn open_async<R>(&self, runtime: R) -> crate::Result<crate::CameraSession<P>>
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

        let session = crate::Session::open::<R, _>(transport, plan.session_config, runtime).await?;
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
            .connect_serial(self.serial_config(port, baud_rate)?)
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
    /// Opens a configured standard blocking TCP or UDP single-camera owner
    /// session.
    pub fn open(&self) -> crate::Result<crate::blocking::CameraSession<P>> {
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

        let session = crate::blocking::Session::open(transport, plan.session_config)?;
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
            self.serial_config(port, baud_rate)?,
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
    fn canonical_session_config_uses_stored_operational_tuning() {
        use std::time::Duration;

        use crate::{camera::CameraConfig, profile::OperationalTuning, profiles::PtzOpticsG2};

        let tuning = OperationalTuning::new()
            .ack_timeout(Duration::from_secs(61))
            .quick_timeout(Duration::from_secs(67))
            .movement_timeout(Duration::from_secs(71))
            .preset_timeout(Duration::from_secs(73))
            .long_running_timeout(Duration::from_secs(379))
            .network_timeout(Duration::from_secs(83))
            .settlement_timeout(Duration::from_secs(83));
        let config = CameraConfig::<PtzOpticsG2>::udp("camera.local").with_tuning(tuning);

        assert_eq!(
            config.session_config().expect("session config").tuning(),
            tuning
        );
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn canonical_session_config_preserves_validated_sony_reset_opt_in() {
        use crate::{
            camera::CameraConfig,
            profiles::{PtzOpticsG2, SonyFR7},
            Error,
        };

        let raw = CameraConfig::<PtzOpticsG2>::new().with_sony_sequence_reset_on_connect(true);
        assert!(matches!(raw.validate(), Err(Error::InvalidRequest(_))));

        let sony = CameraConfig::<SonyFR7>::new().with_sony_sequence_reset_on_connect(true);
        assert!(sony.sony_sequence_reset_on_connect());
        assert!(sony
            .session_config()
            .expect("Sony session config")
            .sony_sequence_reset_on_connect());
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn canonical_session_config_preserves_strict_recovery_policy_outside_tuning() {
        use crate::{camera::CameraConfig, profiles::PtzOpticsG2, OperationalTuning};

        let camera_config = CameraConfig::<PtzOpticsG2>::new().with_strict_unconfirmed_poison(true);
        assert!(camera_config.strict_unconfirmed_poison());

        let session_config = camera_config.session_config().expect("session config");
        assert!(session_config.strict_unconfirmed_poison());
        assert_eq!(session_config.tuning(), OperationalTuning::new());
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn invalid_buffer_bounds_fail_during_network_preflight() {
        use crate::{
            camera::CameraConfig,
            profiles::PtzOpticsG2,
            transport::{BufferConfig, TransportConfig},
            Error,
        };

        for (buffer_config, message) in [
            (
                BufferConfig {
                    recv_buffer_size: 0,
                    send_buffer_size: 64,
                    max_buffer_size: 64,
                },
                "transport receive buffer must be non-zero",
            ),
            (
                BufferConfig {
                    recv_buffer_size: 64,
                    send_buffer_size: 64,
                    max_buffer_size: 0,
                },
                "transport maximum buffer must be non-zero",
            ),
            (
                BufferConfig {
                    recv_buffer_size: 65,
                    send_buffer_size: 64,
                    max_buffer_size: 64,
                },
                "transport receive buffer cannot exceed maximum buffer",
            ),
        ] {
            for config in [
                CameraConfig::<PtzOpticsG2>::tcp("camera.local:5678"),
                CameraConfig::<PtzOpticsG2>::udp("camera.local:1259"),
            ] {
                let error = config
                    .transport_config(TransportConfig {
                        buffer_config,
                        ..TransportConfig::default()
                    })
                    .standard_connection_plan()
                    .expect_err("invalid buffer bounds must fail in preflight");
                assert!(matches!(
                    error,
                    Error::InvalidRequest(actual) if actual.as_ref() == message
                ));
            }
        }
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
            transport::{AddressingMode, BufferConfig, TransportConfig},
        };

        let transport_config = TransportConfig {
            read_timeout: Duration::from_millis(37),
            write_timeout: Duration::from_millis(41),
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
        let serial = config
            .serial_config("/dev/fake-visca", 38_400)
            .expect("valid serial config");
        assert_eq!(serial.port, "/dev/fake-visca");
        assert_eq!(serial.baud_rate, 38_400);
        assert_eq!(serial.read_timeout, Duration::from_millis(37));
        assert_eq!(serial.write_timeout, Duration::from_millis(41));
        assert_eq!(serial.buffer_config, transport_config.buffer_config);
        assert_eq!(
            config.serial_transport_config().addressing,
            AddressingMode::Serial
        );
    }

    #[cfg(any(
        feature = "transport-serial-tokio",
        all(feature = "blocking", feature = "transport-serial")
    ))]
    #[test]
    fn invalid_buffer_bounds_fail_during_serial_preflight() {
        use crate::{
            camera::CameraConfig,
            profiles::PtzOpticsG2,
            transport::{BufferConfig, TransportConfig},
            Error,
        };

        for (buffer_config, message) in [
            (
                BufferConfig {
                    recv_buffer_size: 0,
                    send_buffer_size: 64,
                    max_buffer_size: 64,
                },
                "transport receive buffer must be non-zero",
            ),
            (
                BufferConfig {
                    recv_buffer_size: 64,
                    send_buffer_size: 64,
                    max_buffer_size: 0,
                },
                "transport maximum buffer must be non-zero",
            ),
            (
                BufferConfig {
                    recv_buffer_size: 65,
                    send_buffer_size: 64,
                    max_buffer_size: 64,
                },
                "transport receive buffer cannot exceed maximum buffer",
            ),
        ] {
            let error = CameraConfig::<PtzOpticsG2>::serial("/dev/not-opened", 9_600)
                .transport_config(TransportConfig {
                    buffer_config,
                    ..TransportConfig::default()
                })
                .serial_config("/dev/not-opened", 9_600)
                .expect_err("invalid buffer bounds must fail before serial-device open");
            assert!(matches!(
                error,
                Error::InvalidRequest(actual) if actual.as_ref() == message
            ));
        }
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
