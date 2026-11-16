//! Pure configuration for camera connection and behavior.
//!
//! This module provides a pure-data configuration struct that holds all the settings
//! needed to establish a camera connection. The configuration is separate from the
//! actual connection process, allowing for easy cloning, reuse, and modification.

use crate::{
    camera_id::CameraId,
    capabilities::{SupportsSerial, SupportsTcp, SupportsUdp},
    error::Error,
    profiles::{
        AnyProfile, CameraProfile, CompileTimeProfile, ProfileSelection, TypedProfileSelection,
    },
    transport::{SerialStartup, TransportConfig},
};

#[cfg(any(feature = "async", feature = "blocking"))]
use crate::transport::SerialConfig;

/// Standard transport kind used for profile support validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
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
    /// Configuration selects a caller-owned transport, outside the standard support matrix.
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
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum TransportOptions {
    /// TCP connection with address.
    #[non_exhaustive]
    Tcp {
        /// Host:port string (e.g., "192.168.0.110:5678")
        address: String,
    },
    /// UDP connection with address.
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
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "deserialize_custom_transport")
    )]
    Custom,
}

// Serde's internally tagged unit-variant visitor otherwise ignores remaining
// map entries, even with the enum's deny_unknown_fields attribute. Deserialize
// this variant as an empty record so Custom has the same strict field contract.
#[cfg(feature = "serde")]
fn deserialize_custom_transport<'de, D>(deserializer: D) -> Result<(), D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Empty {}

    <Empty as serde::Deserialize>::deserialize(deserializer).map(|_| ())
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
        let supported = self.identity().is_none_or(|identity| {
            identity
                .standard_kind()
                .is_none_or(|kind| profile.transport_compatibility().supports(kind))
        });
        if supported {
            Ok(())
        } else {
            Err(Error::UnsupportedTransport {
                profile,
                transport: self.kind(),
            })
        }
    }

    /// The selected standard descriptor, or a caller-owned transport selection.
    pub(crate) const fn identity(&self) -> Option<crate::transport::TransportIdentity<'static>> {
        match self {
            Self::Tcp { .. } => Some(crate::transport::TransportIdentity::Tcp),
            Self::Udp { .. } => Some(crate::transport::TransportIdentity::Udp),
            Self::Serial { .. } => Some(crate::transport::TransportIdentity::Serial { bus: None }),
            Self::Custom => None,
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

/// Pure, reusable camera configuration for either execution facade.
///
/// Runtime configurations select a built-in ID or a custom specification with
/// [`crate::config::CameraConfig::new`]. Typed configurations use [`crate::config::CameraConfig::tcp`],
/// [`crate::config::CameraConfig::udp`] or [`crate::config::CameraConfig::serial`]. Profile and transport
/// validation run before a session performs connection I/O.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(
        deny_unknown_fields,
        bound(
            serialize = "P::Selection: serde::Serialize",
            deserialize = "P::Selection: serde::Deserialize<'de>"
        )
    )
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(
    feature = "schemars",
    schemars(deny_unknown_fields, bound = "P::Selection: schemars::JsonSchema")
)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-rs", ts(export, concrete(P = AnyProfile), bound = "P::Selection: ts_rs::TS"))]
pub struct CameraConfig<P: CameraProfile = AnyProfile> {
    profile: P::Selection,
    transport: TransportOptions,
    policy: crate::session_config::SessionPolicy,
    transport_config: Option<TransportConfig>,
    serial_startup: SerialStartup,
    camera_id: Option<CameraId>,
}

impl<P: CameraProfile> Clone for CameraConfig<P> {
    fn clone(&self) -> Self {
        Self {
            profile: self.profile.clone(),
            transport: self.transport.clone(),
            policy: self.policy,
            transport_config: self.transport_config,
            serial_startup: self.serial_startup,
            camera_id: self.camera_id,
        }
    }
}
impl<P: CameraProfile> std::fmt::Debug for CameraConfig<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraConfig")
            .field("profile", &self.profile)
            .field("transport", &self.transport)
            .field("policy", &self.policy)
            .field("transport_config", &self.transport_config)
            .field("serial_startup", &self.serial_startup)
            .field("camera_id", &self.camera_id)
            .finish()
    }
}

impl CameraConfig<AnyProfile> {
    /// Selects a runtime profile and its transport without performing I/O.
    pub fn new(profile: impl Into<ProfileSelection>, transport: TransportOptions) -> Self {
        Self::from_selection(profile.into(), transport)
    }
}
impl<P: CompileTimeProfile> CameraConfig<P> {
    fn typed(transport: TransportOptions) -> Self {
        Self::from_selection(TypedProfileSelection::<P>::deferred(), transport)
    }
    /// Selects TCP using the profile's default port for a host-only address.
    /// Profile resolution is deferred until validation or connection.
    pub fn tcp(address: impl Into<String>) -> Self
    where
        P: SupportsTcp,
    {
        Self::typed(TransportOptions::tcp(address))
    }
    /// Selects UDP using the profile's default port for a host-only address.
    pub fn udp(address: impl Into<String>) -> Self
    where
        P: SupportsUdp,
    {
        Self::typed(TransportOptions::udp(address))
    }
    /// Selects an individual camera on a serial VISCA bus.
    pub fn serial(port: impl Into<String>, baud_rate: u32) -> Self
    where
        P: SupportsSerial,
    {
        Self::typed(TransportOptions::serial(port, baud_rate))
    }
}
impl<P: CameraProfile> CameraConfig<P> {
    fn from_selection(profile: P::Selection, transport: TransportOptions) -> Self {
        Self {
            profile,
            transport,
            policy: crate::session_config::SessionPolicy::DEFAULT,
            transport_config: None,
            serial_startup: SerialStartup::default(),
            camera_id: None,
        }
    }
    /// Resolves the selected profile, including deferred community-profile validation.
    pub fn profile(&self) -> crate::Result<crate::profiles::ProfileSpec> {
        P::selected_spec(&self.profile)
    }
    /// Returns the selected standard or custom transport.
    pub const fn transport(&self) -> &TransportOptions {
        &self.transport
    }
    /// Replaces the transport selection without performing I/O.
    #[must_use]
    pub fn with_transport(mut self, transport: TransportOptions) -> Self {
        self.transport = transport;
        self
    }
    /// Returns the network endpoint for TCP or UDP, before default-port resolution.
    pub fn address(&self) -> Option<&str> {
        match &self.transport {
            TransportOptions::Tcp { address } | TransportOptions::Udp { address } => Some(address),
            _ => None,
        }
    }
    /// Replaces a TCP or UDP endpoint, preserving the selected transport kind.
    /// IPv6 literals must be bracketed; a host-only endpoint uses the profile port.
    #[must_use]
    pub fn with_address(mut self, address: impl Into<String>) -> Self {
        let address = address.into();
        self.transport = match self.transport {
            TransportOptions::Tcp { .. } => TransportOptions::tcp(address),
            TransportOptions::Udp { .. } => TransportOptions::udp(address),
            other => other,
        };
        self
    }
    /// Returns the explicit VISCA address override, or `None` for the profile default.
    pub const fn camera_id(&self) -> Option<CameraId> {
        self.camera_id
    }
    /// Selects an explicit individual VISCA target, validated when the config opens.
    #[must_use]
    pub const fn with_camera_id(mut self, id: CameraId) -> Self {
        self.camera_id = Some(id);
        self
    }
    /// Returns an explicit transport configuration, or `None` for transport defaults.
    pub const fn transport_config(&self) -> Option<TransportConfig> {
        self.transport_config
    }
    /// Replaces the selected transport's defaults with these explicit settings.
    #[must_use]
    pub const fn with_transport_config(mut self, config: TransportConfig) -> Self {
        self.transport_config = Some(config);
        self
    }
    /// Returns the serial startup writes selected by this configuration.
    pub const fn serial_startup(&self) -> SerialStartup {
        self.serial_startup
    }
    /// Selects optional Address Set and I/F Clear writes; the default writes nothing.
    #[must_use]
    pub const fn with_serial_startup(mut self, startup: SerialStartup) -> Self {
        self.serial_startup = startup;
        self
    }
    crate::session_config::session_policy_accessors!();

    /// Resolves a typed profile into an explicit runtime selection.
    pub fn into_any(self) -> crate::Result<CameraConfig> {
        let spec = P::selected_spec(&self.profile)?;
        Ok(self.reselect::<AnyProfile>(selection_for_spec(spec)))
    }
    /// Checks the complete selected specification against a compile-time profile.
    pub fn into_typed<Q: CompileTimeProfile>(self) -> crate::Result<CameraConfig<Q>> {
        let selected = P::selected_spec(&self.profile)?;
        let selection = Q::select(selection_for_spec(selected))?;
        Ok(self.reselect::<Q>(selection))
    }
    fn reselect<Q: CameraProfile>(self, profile: Q::Selection) -> CameraConfig<Q> {
        CameraConfig {
            profile,
            transport: self.transport,
            policy: self.policy,
            transport_config: self.transport_config,
            serial_startup: self.serial_startup,
            camera_id: self.camera_id,
        }
    }
    /// Lowers the selected profile into a target registry without transport I/O.
    #[cfg(any(feature = "async", feature = "blocking"))]
    fn session_config_data(&self) -> crate::Result<crate::session_config::SessionConfig> {
        let profile = P::selected_spec(&self.profile)?;
        let id = self
            .camera_id
            .unwrap_or_else(|| profile.capabilities().default_camera_id.into());
        Ok(
            crate::session_config::SessionConfig::for_target(id, profile)?
                .with_policy(self.policy)
                .with_transport(self.transport.clone())
                .with_serial_startup(self.serial_startup)
                .with_optional_transport_config(self.transport_config),
        )
    }
    /// Returns the session registry after profile, policy and transport validation.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub fn session_config(&self) -> crate::Result<crate::session_config::SessionConfig> {
        self.validated_session_config()
            .map(crate::session_config::ValidatedSessionConfig::into_config)
    }
    /// Validates the selected profile, policy and transport before any connection I/O.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub fn validate(&self) -> crate::Result<()> {
        self.validated_session_config().map(drop)
    }
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn validated_session_config(
        &self,
    ) -> crate::Result<crate::session_config::ValidatedSessionConfig> {
        let config = self.session_config_data()?.validated()?;
        validate_selected_transport(&config, &self.transport)?;
        Ok(config)
    }
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn standard_connection_plan(&self) -> crate::Result<StandardConnectionPlan> {
        let plan = self.session_config_data()?.standard_connection_plan()?;
        let camera_id = plan
            .session_config
            .sole_target_for("CameraConfig::camera_id")?;
        Ok(StandardConnectionPlan {
            kind: plan.kind,
            endpoint: plan.endpoint,
            transport_config: plan.transport_config,
            session_config: plan.session_config,
            camera_id,
        })
    }
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn serial_config(&self, port: &str, baud_rate: u32) -> crate::Result<SerialConfig> {
        serial_device_config(port, baud_rate, self.transport_config, self.serial_startup)
    }
}

pub(crate) fn selection_for_spec(spec: crate::profiles::ProfileSpec) -> ProfileSelection {
    match spec.capabilities().profile_id {
        Some(id) => ProfileSelection::Builtin(id),
        None => ProfileSelection::Custom(Box::new(spec)),
    }
}

#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) fn serial_device_config(
    port: &str,
    baud_rate: u32,
    transport: Option<TransportConfig>,
    startup: SerialStartup,
) -> crate::Result<SerialConfig> {
    let transport = transport.unwrap_or_else(TransportConfig::for_serial);
    let config = SerialConfig::new(port.to_owned())
        .with_baud_rate(baud_rate)
        .with_startup(startup)
        .with_read_timeout(transport.read_timeout())
        .with_write_timeout(transport.write_timeout())
        .with_buffer_config(transport.buffer_config());
    // Serial consumes only timeouts and buffers. TCP-only options do not
    // participate in serial validation, matching the opened device facts.
    config.transport_config().validate()?;
    Ok(config)
}

#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) fn validate_selected_transport(
    config: &crate::session_config::ValidatedSessionConfig,
    selected: &TransportOptions,
) -> crate::Result<()> {
    match selected.identity() {
        Some(identity) => config.validate_for_transport(identity),
        None => Ok(()),
    }
}

/// Pure network construction state shared by both execution facades.
#[derive(Debug, Clone)]
#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) struct SessionConnectionPlan {
    pub(crate) kind: TransportKind,
    pub(crate) endpoint: String,
    pub(crate) transport_config: TransportConfig,
    pub(crate) session_config: crate::session_config::ValidatedSessionConfig,
}
/// A single-camera network plan with its validated target identity.
#[derive(Debug, Clone)]
#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) struct StandardConnectionPlan {
    pub(crate) kind: TransportKind,
    pub(crate) endpoint: String,
    pub(crate) transport_config: TransportConfig,
    pub(crate) session_config: crate::session_config::ValidatedSessionConfig,
    pub(crate) camera_id: CameraId,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    #[cfg(any(feature = "blocking", feature = "async"))]
    use super::TransportOptions;
    /// #822: a downstream profile's host-only TCP/UDP address resolves to
    /// its validated profile port, whether the configuration is
    /// built with `CameraConfig::tcp`/`udp` or with `.with_transport(..)`.
    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn standard_constructors_and_transport_options_share_the_profile_port() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        use crate::{
            camera::{CameraConfig, TransportOptions},
            capabilities::{SupportsTcp, SupportsUdp, TypedSupportSet},
            profiles::GenericVisca as Base,
            CompileTimeProfile, ProfileSpec, TransportCompatibility,
        };

        static RESOLUTIONS: AtomicUsize = AtomicUsize::new(0);

        macro_rules! downstream_profile {
            ($name:ident, $transports:expr) => {
                #[derive(Debug, Default, Clone, Copy)]
                struct $name;

                impl CompileTimeProfile for $name {
                    const TYPED_SUPPORT: TypedSupportSet = TypedSupportSet::empty();
                    const ID: Option<crate::profiles::ProfileId> = None;

                    fn spec() -> crate::Result<ProfileSpec> {
                        RESOLUTIONS.fetch_add(1, Ordering::SeqCst);
                        let base = Base::spec()?;
                        let mut capabilities = base.capabilities().clone();
                        capabilities.profile_id = None;
                        capabilities.default_camera_id = crate::IndividualCameraId::new(2)?;
                        capabilities.model_name = stringify!($name).into();
                        capabilities.typed_support = Self::TYPED_SUPPORT;
                        ProfileSpec::builder(capabilities)
                            .transports($transports)
                            .envelope(base.envelope())
                            .timing(base.timing())
                            .maximum_command_sockets(base.maximum_command_sockets())
                            .supports_operation_complete(base.supports_operation_complete())
                            .supports_command_cancel(base.supports_command_cancel())
                            .position_inquiries(crate::AffectedAxes::MOVEMENT)
                            .build()
                    }
                }
            };
        }

        downstream_profile!(
            Downstream,
            TransportCompatibility::new().with_tcp(9876).with_udp(9877)
        );
        downstream_profile!(UdpOnly, TransportCompatibility::new().with_udp(9877));
        impl SupportsTcp for Downstream {}
        impl SupportsUdp for Downstream {}

        let config = CameraConfig::<Downstream>::tcp("camera.local");
        assert_eq!(RESOLUTIONS.load(Ordering::SeqCst), 0);
        let plan = config
            .standard_connection_plan()
            .expect("default target plan");
        assert_eq!(RESOLUTIONS.load(Ordering::SeqCst), 1);
        assert_eq!(plan.camera_id, crate::CameraId::CAMERA_2);
        assert_eq!(
            config
                .with_camera_id(crate::CameraId::CAMERA_3)
                .standard_connection_plan()
                .expect("address override")
                .camera_id,
            crate::CameraId::CAMERA_3
        );

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
            endpoint(
                CameraConfig::<Downstream>::typed(TransportOptions::Custom)
                    .with_transport(TransportOptions::tcp("camera.local"))
            ),
            "camera.local:9876"
        );
        assert_eq!(
            endpoint(CameraConfig::udp("camera.local")),
            "camera.local:9877"
        );
        assert_eq!(
            endpoint(
                CameraConfig::<Downstream>::typed(TransportOptions::Custom)
                    .with_transport(TransportOptions::udp("camera.local"))
            ),
            "camera.local:9877"
        );

        // A downstream marker declaration cannot bypass runtime profile facts.
        // Both typed and explicit transport selection are refused before I/O.
        impl SupportsTcp for UdpOnly {}
        impl SupportsUdp for UdpOnly {}
        assert!(CameraConfig::<UdpOnly>::tcp("/never-resolved")
            .validate()
            .is_err());
        for transport in [
            TransportOptions::tcp("/never-resolved"),
            TransportOptions::serial("/never-opened", 9600),
        ] {
            let config =
                CameraConfig::<UdpOnly>::typed(TransportOptions::Custom).with_transport(transport);
            let preflight = config.validate().expect_err("unsupported custom transport");
            let session = config.session_config().expect_err("same session preflight");
            assert_eq!(preflight.to_string(), session.to_string());
            assert!(matches!(preflight, crate::Error::InvalidRequest { .. }));
        }
        #[cfg(feature = "blocking")]
        {
            let config = CameraConfig::<UdpOnly>::typed(TransportOptions::Custom)
                .with_transport(TransportOptions::tcp("/never-resolved"));
            assert_eq!(
                config.validate().expect_err("preflight").to_string(),
                crate::blocking::CameraSession::connect(&config)
                    .expect_err("refused before DNS")
                    .to_string(),
            );
        }
        assert!(CameraConfig::<UdpOnly>::typed(TransportOptions::Custom)
            .with_transport(TransportOptions::tcp("camera.local"))
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
    fn malformed_downstream_profile_is_rejected_at_validation_before_endpoint_parsing() {
        use crate::{capabilities::SupportsTcp, CompileTimeProfile};

        #[derive(Debug, Clone, Copy)]
        struct Malformed;

        impl CompileTimeProfile for Malformed {
            const TYPED_SUPPORT: crate::capabilities::TypedSupportSet =
                crate::capabilities::TypedSupportSet::empty();
            const ID: Option<crate::profiles::ProfileId> = None;

            fn spec() -> crate::Result<crate::ProfileSpec> {
                Err(crate::Error::InvalidRequest {
                    reason: "invalid downstream profile".into(),
                })
            }
        }
        impl SupportsTcp for Malformed {}

        // These infallible builders cannot resolve a fallible profile.
        let _unselected = super::CameraConfig::<Malformed>::typed(TransportOptions::Custom);
        let config = super::CameraConfig::<Malformed>::tcp("/never-resolved");
        let error = config.validate().expect_err("invalid profile");
        assert!(
            matches!(&error, crate::Error::InvalidRequest { reason: message } if message == "invalid downstream profile")
        );
        assert_eq!(
            error.to_string(),
            config
                .standard_connection_plan()
                .expect_err("profile before endpoint")
                .to_string()
        );
        #[cfg(feature = "blocking")]
        assert_eq!(
            error.to_string(),
            crate::blocking::CameraSession::connect(&config)
                .expect_err("profile before DNS")
                .to_string()
        );
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn canonical_standard_plan_records_defaults_and_transport_options_without_io() {
        use std::time::Duration;

        use crate::{
            camera::{CameraConfig, TransportKind, TransportOptions},
            profiles::{PtzOpticsG2, SonyFr7},
            transport::{BufferConfig, TcpKeepaliveConfig, TransportConfig},
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
        };

        let tcp = CameraConfig::<PtzOpticsG2>::tcp("camera.local")
            .with_transport_config(transport_config)
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

        let udp = CameraConfig::<PtzOpticsG2>::udp("camera.local")
            .with_transport_config(transport_config)
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

        let sony = CameraConfig::<SonyFr7>::udp("camera.local")
            .standard_connection_plan()
            .expect("Sony UDP plan");
        assert_eq!(sony.endpoint, "camera.local:52381");

        let mut connector_calls = 0;
        let unsupported = CameraConfig::<SonyFr7>::typed(TransportOptions::Custom)
            .with_transport(TransportOptions::tcp("does-not-resolve.invalid"))
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

        use crate::{camera::CameraConfig, config::OperationalTuning, profiles::PtzOpticsG2};

        let tuning = OperationalTuning::new()
            .with_ack_timeout(Duration::from_secs(61))
            .with_quick_timeout(Duration::from_secs(67))
            .with_movement_timeout(Duration::from_secs(71))
            .with_preset_timeout(Duration::from_secs(73))
            .with_long_running_timeout(Duration::from_secs(379))
            .with_network_timeout(Duration::from_secs(83))
            // The longer movement/preset deadlines also raise their retry floor.
            .with_settlement_timeout(Duration::from_secs(600));
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
            profiles::{PtzOpticsG2, SonyFr7},
            Error,
        };

        // Issue #805: the camera and session configurations reject the same
        // policy with the one session-policy message.
        let raw = CameraConfig::<PtzOpticsG2>::typed(TransportOptions::Custom)
            .with_sony_sequence_reset_on_connect(true);
        let session_error = crate::SessionConfig::from_compile_time::<PtzOpticsG2>()
            .expect("G2 session config")
            .with_sony_sequence_reset_on_connect(true)
            .validated()
            .expect_err("a raw profile cannot reset Sony sequences");
        assert!(matches!(
            (raw.validate(), session_error),
            (Err(Error::InvalidRequest { reason: camera }), Error::InvalidRequest { reason: session })
                if camera == session
                    && camera == "Sony sequence reset on connect requires Sony encapsulated profiles"
        ));

        let sony = CameraConfig::<SonyFr7>::typed(TransportOptions::Custom)
            .with_sony_sequence_reset_on_connect(true);
        assert!(sony.sony_sequence_reset_on_connect());
        assert!(sony
            .session_config()
            .expect("Sony session config")
            .sony_sequence_reset_on_connect());
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn canonical_session_config_preserves_strict_recovery_policy_outside_tuning() {
        use crate::{camera::CameraConfig, config::OperationalTuning, profiles::PtzOpticsG2};

        let camera_config = CameraConfig::<PtzOpticsG2>::typed(TransportOptions::Custom)
            .with_strict_unconfirmed_poison(true);
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
                    .with_transport_config(TransportConfig {
                        buffer_config,
                        ..TransportConfig::default()
                    })
                    .standard_connection_plan()
                    .expect_err("invalid buffer bounds must fail in preflight");
                assert!(matches!(
                    error,
                    Error::InvalidRequest { reason: actual } if actual.as_ref() == message
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
            transport::{BufferConfig, TransportConfig},
        };

        let transport_config = TransportConfig {
            read_timeout: Duration::from_millis(37),
            write_timeout: Duration::from_millis(41),
            buffer_config: BufferConfig {
                recv_buffer_size: 43,
                max_buffer_size: 53,
            },

            ..TransportConfig::default()
        };
        let config = CameraConfig::<PtzOpticsG2>::serial("/dev/fake-visca", 38_400)
            .with_transport_config(transport_config);
        let serial = config
            .serial_config("/dev/fake-visca", 38_400)
            .expect("valid serial config");
        assert_eq!(serial.port, "/dev/fake-visca");
        assert_eq!(serial.baud_rate, 38_400);
        assert_eq!(serial.read_timeout, Duration::from_millis(37));
        assert_eq!(serial.write_timeout, Duration::from_millis(41));
        assert_eq!(serial.buffer_config, transport_config.buffer_config);

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
        use crate::{camera::CameraConfig, profiles::PtzOpticsG2, transport::SerialStartup};

        let plain = CameraConfig::<PtzOpticsG2>::serial("/dev/fake-visca", 9_600)
            .serial_config("/dev/fake-visca", 9_600)
            .expect("valid serial config");
        assert_eq!(plain.startup, SerialStartup::default());
        assert!(!plain.startup.address_set && !plain.startup.interface_clear);

        let selected = SerialStartup::default()
            .with_address_set(true)
            .with_interface_clear(true);
        let configured = CameraConfig::<PtzOpticsG2>::serial("/dev/fake-visca", 9_600)
            .with_serial_startup(selected)
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
                .with_transport_config(TransportConfig {
                    buffer_config,
                    ..TransportConfig::default()
                })
                .serial_config("/dev/not-opened", 9_600)
                .expect_err("invalid buffer bounds must fail before serial-device open");
            assert!(matches!(
                error,
                Error::InvalidRequest { reason: actual } if actual.as_ref() == message
            ));
        }
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_transport_options_serialization() {
        use super::TransportOptions;
        for (transport, tag) in [
            (TransportOptions::tcp("camera.local:5678"), "tcp"),
            (TransportOptions::udp("camera.local:1259"), "udp"),
            (TransportOptions::serial("/dev/ttyUSB0", 9600), "serial"),
            (TransportOptions::Custom, "custom"),
        ] {
            let json = serde_json::to_value(&transport).expect("transport serialization");
            assert_eq!(json["type"], tag);
            assert_eq!(
                serde_json::from_value::<TransportOptions>(json.clone())
                    .expect("transport round trip"),
                transport
            );
            let mut unknown = json;
            unknown["unexpected"] = true.into();
            assert!(serde_json::from_value::<TransportOptions>(unknown).is_err());
        }
    }
    #[test]
    fn typed_configs_clone_without_requiring_a_cloneable_profile_marker() {
        use crate::profiles::{CompileTimeProfile, GenericVisca, ProfileId, ProfileSpec};
        struct Community;
        impl CompileTimeProfile for Community {
            const TYPED_SUPPORT: crate::capabilities::TypedSupportSet = GenericVisca::TYPED_SUPPORT;
            const ID: Option<ProfileId> = GenericVisca::ID;
            fn spec() -> crate::Result<ProfileSpec> {
                GenericVisca::spec()
            }
        }
        impl crate::capabilities::SupportsTcp for Community {}
        let config = super::CameraConfig::<Community>::tcp("camera.local")
            .with_camera_id(crate::CameraId::CAMERA_2);
        let cloned = config.clone();
        assert_eq!(cloned.address(), Some("camera.local"));
        assert_eq!(cloned.camera_id(), Some(crate::CameraId::CAMERA_2));
        assert!(!format!("{cloned:?}").is_empty());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn typed_and_runtime_configs_share_one_checked_serialized_shape() {
        use crate::profiles::{PtzOpticsG2, SonyFr7};
        let typed = super::CameraConfig::<PtzOpticsG2>::tcp("camera.local")
            .with_camera_id(crate::CameraId::CAMERA_2)
            .with_strict_unconfirmed_poison(true);
        let json = serde_json::to_value(&typed).expect("typed config serialization");
        assert_eq!(json["profile"], "ptzoptics_g2");
        assert_eq!(json["transport"]["type"], "tcp");
        assert_eq!(json["camera_id"], 2);
        let runtime: super::CameraConfig =
            serde_json::from_value(json.clone()).expect("runtime shape");
        assert_eq!(
            serde_json::to_value(&runtime).expect("runtime serialization"),
            json
        );
        let restored = runtime
            .into_typed::<PtzOpticsG2>()
            .expect("same complete spec");
        assert_eq!(
            serde_json::to_value(restored.into_any().expect("runtime selection"))
                .expect("any serialization"),
            json
        );
        assert!(serde_json::from_value::<super::CameraConfig<SonyFr7>>(json.clone()).is_err());
        let mut unknown = json.clone();
        unknown["unexpected"] = true.into();
        assert!(serde_json::from_value::<super::CameraConfig>(unknown).is_err());
        let mut unknown_policy = json;
        unknown_policy["policy"]["unexpected"] = true.into();
        assert!(serde_json::from_value::<super::CameraConfig>(unknown_policy).is_err());
    }
}
