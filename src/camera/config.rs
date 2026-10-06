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
    transport::builder::TransportConfig,
};

#[cfg(any(feature = "async", feature = "blocking"))]
use crate::transport::builder::AddressingMode;

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
    #[non_exhaustive]
    Tcp {
        /// Host:port string (e.g., "192.168.0.110:5678")
        address: String,
    },
    /// UDP connection with address.
    #[cfg_attr(feature = "serde", serde(rename = "UDP"))]
    #[non_exhaustive]
    Udp {
        /// Host:port string (e.g., "192.168.0.110:1259")
        address: String,
    },
    /// Serial connection.
    #[non_exhaustive]
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
    /// The session policy passed to the owner session.
    policy: crate::session_config::SessionPolicy,
    /// Caller-supplied transport configuration; `None` selects the
    /// transport's own defaults ([`TransportConfig::for_tcp`] and friends).
    pub(crate) transport_config: Option<TransportConfig>,
    /// Bus writes a serial transport performs while opening.
    #[cfg(any(feature = "transport-serial", feature = "transport-serial-tokio"))]
    pub(crate) serial_startup: crate::transport::serial::Startup,
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
            policy: crate::session_config::SessionPolicy::DEFAULT,
            transport_config: None,
            #[cfg(any(feature = "transport-serial", feature = "transport-serial-tokio"))]
            serial_startup: crate::transport::serial::Startup::default(),
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
    ///
    /// A host-only address uses the profile's TCP port
    /// ([`crate::CompileTimeProfile::TRANSPORTS`]), exactly as
    /// `CameraConfig::new().transport(TransportOptions::tcp(address))` does.
    /// A profile that implements [`SupportsTcp`] without declaring a TCP port
    /// in `TRANSPORTS` fails to build at this call.
    pub fn tcp(address: impl Into<String>) -> Self
    where
        P: SupportsTcp + crate::CompileTimeProfile,
    {
        const {
            assert!(
                P::TRANSPORTS.tcp_port().is_some(),
                "a profile implementing `SupportsTcp` must declare a TCP port in `TRANSPORTS`"
            );
        }
        Self::new().transport(TransportOptions::tcp(address))
    }

    /// Create a UDP camera configuration.
    ///
    /// A host-only address uses the profile's UDP port
    /// ([`crate::CompileTimeProfile::TRANSPORTS`]), exactly as
    /// `CameraConfig::new().transport(TransportOptions::udp(address))` does.
    /// A profile that implements [`SupportsUdp`] without declaring a UDP port
    /// in `TRANSPORTS` fails to build at this call.
    pub fn udp(address: impl Into<String>) -> Self
    where
        P: SupportsUdp + crate::CompileTimeProfile,
    {
        const {
            assert!(
                P::TRANSPORTS.udp_port().is_some(),
                "a profile implementing `SupportsUdp` must declare a UDP port in `TRANSPORTS`"
            );
        }
        Self::new().transport(TransportOptions::udp(address))
    }

    /// Create a serial camera configuration.
    pub fn serial(port: impl Into<String>, baud_rate: u32) -> Self
    where
        P: SupportsSerial,
    {
        Self::new().transport(TransportOptions::serial(port, baud_rate))
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
        self.transport = transport;
        self
    }

    crate::session_config::session_policy_accessors!();

    /// Replace the selected transport's default configuration.
    ///
    /// Without this call each transport uses its own defaults:
    /// [`TransportConfig::for_tcp`], [`TransportConfig::for_udp`] or
    /// [`TransportConfig::for_serial`]. A supplied configuration is used as
    /// given, so start from the matching constructor to change one field.
    /// The transport kind still owns its wire facts: TCP and UDP always use
    /// IP addressing, and a serial transport uses only the timeouts and buffer
    /// limits (serial addressing, no TCP socket options).
    pub fn transport_config(mut self, transport_config: TransportConfig) -> Self {
        self.transport_config = Some(transport_config);
        self
    }

    /// Select the bus writes a serial transport performs while opening.
    ///
    /// The default writes nothing: opening only opens and configures the
    /// port. See [`crate::transport::serial::Startup`] for what Address Set and
    /// I/F Clear broadcast and when the VISCA serial guidance calls for them.
    #[cfg(any(feature = "transport-serial", feature = "transport-serial-tokio"))]
    pub fn serial_startup(mut self, startup: crate::transport::serial::Startup) -> Self {
        self.serial_startup = startup;
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

    /// The configuration a standard network transport opens with: the
    /// caller's configuration or `transport_default`, always with IP
    /// addressing.
    #[cfg(any(feature = "async", feature = "blocking"))]
    fn ip_transport_config(&self, transport_default: fn() -> TransportConfig) -> TransportConfig {
        let mut config = self.transport_config.unwrap_or_else(transport_default);
        config.addressing = AddressingMode::Ip;
        config
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn tcp_transport_config(&self) -> TransportConfig {
        self.ip_transport_config(TransportConfig::for_tcp)
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn udp_transport_config(&self) -> TransportConfig {
        self.ip_transport_config(TransportConfig::for_udp)
    }

    /// The serial device configuration a serial open uses: the caller's
    /// timeouts and buffer limits (or the serial defaults) and the selected
    /// startup writes.
    #[cfg(any(
        feature = "transport-serial-tokio",
        all(feature = "blocking", feature = "transport-serial")
    ))]
    pub(crate) fn serial_config(
        &self,
        port: &str,
        baud_rate: u32,
    ) -> crate::Result<crate::transport::serial::Config> {
        let transport = self
            .transport_config
            .unwrap_or_else(TransportConfig::for_serial);
        let config = crate::transport::serial::Config::new(port.to_string())
            .baud_rate(baud_rate)
            .startup(self.serial_startup)
            .read_timeout(transport.read_timeout)
            .write_timeout(transport.write_timeout)
            .buffer_config(transport.buffer_config);
        config.transport_config().validate()?;
        Ok(config)
    }
}

// The owner-backed facades use the same public configuration values and lower
// them once into immutable session tuning at the construction boundary.
#[cfg(any(feature = "async", feature = "blocking"))]
impl<P> CameraConfig<P>
where
    P: crate::profile::CompileTimeProfile,
{
    /// Validate profile, session policy, and transport facts before any
    /// transport I/O; see [`Self::session_config`].
    pub fn validate(&self) -> crate::Result<()> {
        self.validated_session_config().map(drop)
    }

    /// The session configuration this camera opens with, its policy
    /// validated once against `P`'s profile.
    pub(crate) fn validated_session_config(
        &self,
    ) -> crate::Result<crate::session_config::ValidatedSessionConfig> {
        let profile = crate::ProfileSpec::from_compile_time::<P>()?;
        let config = crate::SessionConfig::for_target(self.camera_id, profile)?
            .with_policy(self.policy)
            .validated()?;
        if let Some(profile_id) = P::PROFILE_ID {
            self.transport.validate_for_profile(profile_id)?;
        }
        Ok(config)
    }

    pub(crate) fn owner_default_port(&self, kind: TransportKind) -> Option<u16> {
        match kind {
            TransportKind::Tcp => P::TRANSPORTS.tcp_port(),
            TransportKind::Udp => P::TRANSPORTS.udp_port(),
            TransportKind::Serial | TransportKind::Custom => None,
        }
    }

    /// Returns the validated, pure [`crate::SessionConfig`] that standard
    /// construction will pass to the owner-backed session.
    ///
    /// This performs profile and session-policy validation but no endpoint
    /// parsing, DNS lookup, device open, task spawn, or protocol I/O. It is
    /// useful when an application needs to inspect or compose the shared
    /// session policy before selecting an async or blocking transport.
    pub fn session_config(&self) -> crate::Result<crate::SessionConfig> {
        self.validated_session_config()
            .map(crate::session_config::ValidatedSessionConfig::into_config)
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
    pub(crate) session_config: crate::session_config::ValidatedSessionConfig,
}

#[cfg(any(feature = "async", feature = "blocking"))]
impl<P> CameraConfig<P>
where
    P: crate::profile::CompileTimeProfile,
{
    /// Resolves all standard profile/configuration/endpoint state before any
    /// runtime connector or blocking socket is entered.
    pub(crate) fn standard_connection_plan(&self) -> crate::Result<StandardConnectionPlan> {
        let session_config = self.validated_session_config()?;
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
        transport_config.validate()?;
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

        let session =
            crate::Session::open_validated::<R, _>(transport, plan.session_config, runtime).await?;
        crate::CameraSession::from_session(session, self.camera_id)
    }

    /// Opens the configured Tokio serial transport through the owner session.
    ///
    /// Opening writes to the bus only the startup selected with
    /// [`CameraConfig::serial_startup`] (nothing by default), then the owner
    /// session starts without any further protocol write. When Address Set
    /// runs, the configured camera must be among the cameras it addressed.
    #[cfg(feature = "transport-serial-tokio")]
    pub async fn open_serial_async<R>(&self, runtime: R) -> crate::Result<crate::Session>
    where
        R: crate::runtime::Runtime
            + crate::runtime::RuntimeSerial<SerialTransport = crate::transport::tokio::serial::Serial>,
    {
        use crate::runtime::TransportHandle;

        let session_config = self.validated_session_config()?;
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
        crate::Session::open_validated::<R, _>(
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

        let session = crate::blocking::Session::open_validated(transport, plan.session_config)?;
        crate::blocking::CameraSession::from_session(session, self.camera_id)
    }

    /// Opens a configured blocking serial owner session.
    ///
    /// Opening writes to the bus only the startup selected with
    /// [`CameraConfig::serial_startup`] (nothing by default), then the owner
    /// session starts without any further protocol write. When Address Set
    /// runs, the configured camera must be among the cameras it addressed.
    #[cfg(feature = "transport-serial")]
    pub fn open_serial(&self) -> crate::Result<crate::blocking::Session> {
        let session_config = self.validated_session_config()?;
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
        crate::blocking::Session::open_validated(transport, session_config)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    /// #822: a downstream profile's host-only TCP/UDP address resolves to
    /// its `CompileTimeProfile::TRANSPORTS` port, whether the configuration is
    /// built with `CameraConfig::tcp`/`udp` or with `.transport(..)`.
    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn standard_constructors_and_transport_options_share_the_profile_port() {
        use std::time::Duration;

        use crate::{
            camera::{CameraConfig, TransportOptions},
            capabilities::{
                exposure::ShutterSpeedEntry, CapabilityDomain, CapabilityRange, Exposure, Focus,
                ImageProcessing, MenuCapability, MotionSyncMetadata, NdFilterMetadata, PanTilt,
                Power, Presets, ProfileMetadata, ProfileTypedSupport, SupportsTcp, SupportsUdp,
                Tally, TypedSupportSet, VariableSpeedMetadata, WhiteBalance, Zoom,
            },
            profiles::GenericVisca as Base,
            AffectedAxes, CompileTimeProfile, PositionInquirySupport, TransportCompatibility,
            WhiteBalanceMode,
        };

        macro_rules! downstream_profile {
            ($name:ident, $transports:expr) => {
                #[derive(Debug, Default, Clone, Copy)]
                struct $name;

                impl ProfileMetadata for $name {
                    const MODEL_NAME: &'static str = stringify!($name);
                    const DEFAULT_CAMERA_ID: u8 = 1;
                    type Envelope = crate::transport::RawVisca;
                    const ACK_TIMEOUT: Duration = <Base as ProfileMetadata>::ACK_TIMEOUT;
                    const COMMAND_TIMEOUTS: crate::CommandTimeouts =
                        crate::CommandTimeouts::DEFAULT;
                }
                impl PanTilt for $name {
                    const PAN_RANGE: CapabilityRange<i32> = <Base as PanTilt>::PAN_RANGE;
                    const TILT_RANGE: CapabilityRange<i32> = <Base as PanTilt>::TILT_RANGE;
                    const MAX_PAN_SPEED: u8 = <Base as PanTilt>::MAX_PAN_SPEED;
                    const MAX_TILT_SPEED: u8 = <Base as PanTilt>::MAX_TILT_SPEED;
                    const PAN_DEGREES_TO_UNITS: f32 = <Base as PanTilt>::PAN_DEGREES_TO_UNITS;
                    const TILT_DEGREES_TO_UNITS: f32 = <Base as PanTilt>::TILT_DEGREES_TO_UNITS;
                }
                impl Zoom for $name {
                    const OPTICAL_ZOOM_MAX: u16 = <Base as Zoom>::OPTICAL_ZOOM_MAX;
                    const DIGITAL_ZOOM_MAX: Option<u16> = None;
                    const ZOOM_SPEED_RANGE: CapabilityRange<u8> = <Base as Zoom>::ZOOM_SPEED_RANGE;
                    const OPTICAL_ZOOM_RATIO: Option<f32> = None;
                }
                impl Focus for $name {
                    const FOCUS_NEAR_LIMIT: u16 = <Base as Focus>::FOCUS_NEAR_LIMIT;
                    const FOCUS_FAR_LIMIT: u16 = <Base as Focus>::FOCUS_FAR_LIMIT;
                    const SUPPORTS_AUTO_FOCUS: bool = true;
                    const SUPPORTS_ONE_PUSH_FOCUS: bool = false;
                }
                impl Exposure for $name {
                    const IRIS_RANGE: Option<CapabilityDomain<u16>> = None;
                    const SHUTTER_SPEEDS: &'static [ShutterSpeedEntry] =
                        <Base as Exposure>::SHUTTER_SPEEDS;
                    const GAIN_RANGE: CapabilityRange<u8> = <Base as Exposure>::GAIN_RANGE;
                    const SUPPORTS_BACKLIGHT_COMP: bool = false;
                }
                impl WhiteBalance for $name {
                    const WB_MODES: &'static [WhiteBalanceMode] = &[WhiteBalanceMode::Auto];
                    const SUPPORTS_ONE_PUSH_WB: bool = false;
                    const RG_TUNING_RANGE: Option<CapabilityRange<i8>> = None;
                    const BG_TUNING_RANGE: Option<CapabilityRange<i8>> = None;
                }
                impl ImageProcessing for $name {
                    const CONTRAST_RANGE: Option<CapabilityRange<u8>> = None;
                    const SHARPNESS_RANGE: Option<CapabilityRange<u8>> = None;
                    const SATURATION_RANGE: Option<CapabilityRange<u8>> = None;
                    const SUPPORTS_FLIP: bool = false;
                    const SUPPORTS_MIRROR: bool = false;
                }
                impl Presets for $name {
                    const HIGHEST_PRESET: u8 = 6;
                    const PRESET_SPEED_RANGE: Option<CapabilityRange<u8>> =
                        <Base as Presets>::PRESET_SPEED_RANGE;
                    const SUPPORTS_PRESET_TOUR: bool = false;
                }
                impl Power for $name {
                    const POWER_ON_TIME: Duration = Duration::from_secs(5);
                    const SUPPORTS_STANDBY: bool = false;
                }
                impl MenuCapability for $name {}
                impl Tally for $name {}
                impl MotionSyncMetadata for $name {}
                impl NdFilterMetadata for $name {}
                impl VariableSpeedMetadata for $name {}
                impl ProfileTypedSupport for $name {
                    const TYPED_SUPPORT: TypedSupportSet = TypedSupportSet::empty();
                }
                impl CompileTimeProfile for $name {
                    const TRANSPORTS: TransportCompatibility = $transports;
                    const INQUIRY_TIMEOUT: Duration = Duration::from_secs(1);
                    const CANCELLATION_TIMEOUT: Duration = Duration::from_secs(1);
                    const AMBIGUITY_TIMEOUT: Duration = Duration::from_secs(1);
                    const MAXIMUM_COMMAND_SOCKETS: u8 = 2;
                    const PRESET_RECALL_AXES: Option<AffectedAxes> =
                        Some(AffectedAxes::PAN_TILT.union(AffectedAxes::ZOOM));
                    const POSITION_INQUIRIES: PositionInquirySupport =
                        PositionInquirySupport::new(true, true, true);
                }
            };
        }

        downstream_profile!(
            Downstream,
            TransportCompatibility::new(Some(9876), Some(9877), false)
        );
        downstream_profile!(
            UdpOnly,
            TransportCompatibility::new(None, Some(9877), false)
        );
        impl SupportsTcp for Downstream {}
        impl SupportsUdp for Downstream {}

        let endpoint = |config: CameraConfig<Downstream>| {
            config
                .standard_connection_plan()
                .expect("downstream plan")
                .endpoint
        };
        assert_eq!(
            endpoint(CameraConfig::tcp("camera.local")),
            "camera.local:9876"
        );
        assert_eq!(
            endpoint(CameraConfig::new().transport(TransportOptions::tcp("camera.local"))),
            "camera.local:9876"
        );
        assert_eq!(
            endpoint(CameraConfig::udp("camera.local")),
            "camera.local:9877"
        );
        assert_eq!(
            endpoint(CameraConfig::new().transport(TransportOptions::udp("camera.local"))),
            "camera.local:9877"
        );

        // A profile that declares no TCP port gets no `CameraConfig::tcp`
        // (it fails to build for a `SupportsTcp` implementor without one), and
        // an explicit TCP transport is refused before any I/O.
        impl SupportsUdp for UdpOnly {}
        assert!(CameraConfig::<UdpOnly>::new()
            .transport(TransportOptions::tcp("camera.local"))
            .standard_connection_plan()
            .is_err());
        assert_eq!(
            CameraConfig::<UdpOnly>::udp("camera.local")
                .standard_connection_plan()
                .expect("UDP plan")
                .endpoint,
            "camera.local:9877"
        );
    }

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
                recv_buffer_size: 31,
                max_buffer_size: 37,
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

        // Issue #805: the camera and session configurations reject the same
        // policy with the one session-policy message.
        let raw = CameraConfig::<PtzOpticsG2>::new().with_sony_sequence_reset_on_connect(true);
        let session_error = crate::SessionConfig::from_compile_time::<PtzOpticsG2>()
            .expect("G2 session config")
            .with_sony_sequence_reset_on_connect(true)
            .validated()
            .expect_err("a raw profile cannot reset Sony sequences");
        assert!(matches!(
            (raw.validate(), session_error),
            (Err(Error::InvalidRequest(camera)), Error::InvalidRequest(session))
                if camera == session
                    && camera == "Sony sequence reset on connect requires Sony encapsulated profiles"
        ));

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
                    max_buffer_size: 64,
                },
                "transport receive buffer must hold the largest VISCA reply (24 bytes)",
            ),
            (
                BufferConfig {
                    recv_buffer_size: BufferConfig::MIN_RECV_BUFFER_SIZE - 1,
                    max_buffer_size: 64,
                },
                "transport receive buffer must hold the largest VISCA reply (24 bytes)",
            ),
            (
                BufferConfig {
                    recv_buffer_size: 64,
                    max_buffer_size: 0,
                },
                "transport maximum buffer must be non-zero",
            ),
            (
                BufferConfig {
                    recv_buffer_size: 65,
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
        assert_eq!(serial.transport_config().addressing, AddressingMode::Serial);
        assert_eq!(serial.transport_config().tcp_keepalive, None);
    }

    /// #828: every `CameraConfig` serial open used to broadcast I/F Clear
    /// with no way to disable it. A serial open now writes only the startup
    /// the caller selects, and nothing by default.
    #[cfg(any(
        feature = "transport-serial-tokio",
        all(feature = "blocking", feature = "transport-serial")
    ))]
    #[test]
    fn serial_open_writes_only_the_selected_startup() {
        use crate::{camera::CameraConfig, profiles::PtzOpticsG2, transport::serial::Startup};

        let plain = CameraConfig::<PtzOpticsG2>::serial("/dev/fake-visca", 9_600)
            .serial_config("/dev/fake-visca", 9_600)
            .expect("valid serial config");
        assert_eq!(plain.startup, Startup::default());
        assert!(!plain.startup.address_set && !plain.startup.interface_clear);

        let selected = Startup::default()
            .with_address_set(true)
            .with_interface_clear(true);
        let configured = CameraConfig::<PtzOpticsG2>::serial("/dev/fake-visca", 9_600)
            .serial_startup(selected)
            .serial_config("/dev/fake-visca", 9_600)
            .expect("valid serial config");
        assert_eq!(configured.startup, selected);
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
                    max_buffer_size: 64,
                },
                "transport receive buffer must hold the largest VISCA reply (24 bytes)",
            ),
            (
                BufferConfig {
                    recv_buffer_size: BufferConfig::MIN_RECV_BUFFER_SIZE - 1,
                    max_buffer_size: 64,
                },
                "transport receive buffer must hold the largest VISCA reply (24 bytes)",
            ),
            (
                BufferConfig {
                    recv_buffer_size: 64,
                    max_buffer_size: 0,
                },
                "transport maximum buffer must be non-zero",
            ),
            (
                BufferConfig {
                    recv_buffer_size: 65,
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
