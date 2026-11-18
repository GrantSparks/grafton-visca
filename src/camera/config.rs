//! Pure configuration for camera connection and behavior.
//!
//! This module provides a pure-data configuration struct that holds all the settings
//! needed to establish a camera connection. The configuration is separate from the
//! actual connection process, allowing for easy cloning, reuse, and modification.

use crate::{
    camera_id::IndividualCameraId,
    capabilities::{SupportsSerial, SupportsTcp, SupportsUdp},
    profiles::{
        AnyProfile, CameraProfile, CompileTimeProfile, ProfileSelection, TypedProfileSelection,
    },
    transport::{IoBounds, SerialConfig, TcpConnect, UdpConnect},
    MountPosition,
};

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
    /// TCP endpoint and its connection and I/O settings.
    #[non_exhaustive]
    Tcp {
        /// Host with an optional port, such as "192.168.0.110:5678".
        address: String,
        /// Settings applied while establishing the connection.
        connect: TcpConnect,
        /// Bounds used by the owner after connection.
        io_bounds: IoBounds,
    },
    /// UDP endpoint and its connection and I/O settings.
    #[non_exhaustive]
    Udp {
        /// Host with an optional port, such as "192.168.0.110:1259".
        address: String,
        /// Settings applied while establishing the connection.
        connect: UdpConnect,
        /// Bounds used by the owner after connection.
        io_bounds: IoBounds,
    },
    /// Serial device and its I/O settings.
    #[non_exhaustive]
    Serial {
        /// Device, baud rate and startup settings.
        config: SerialConfig,
        /// Bounds used for startup and subsequent I/O.
        io_bounds: IoBounds,
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

    /// The selected standard descriptor, or a caller-owned transport selection.
    pub(crate) const fn identity(&self) -> Option<crate::transport::TransportIdentity<'static>> {
        match self {
            Self::Tcp { .. } => Some(crate::transport::TransportIdentity::Tcp),
            Self::Udp { .. } => Some(crate::transport::TransportIdentity::Udp),
            Self::Serial { .. } => Some(crate::transport::TransportIdentity::Serial { bus: None }),
            Self::Custom => None,
        }
    }

    /// Creates TCP transport options with the default connection and I/O settings.
    pub fn tcp(address: impl Into<String>) -> Self {
        Self::tcp_configured(address, TcpConnect::default(), IoBounds::TCP_DEFAULT)
    }

    /// Creates TCP transport options with explicit connection and I/O settings.
    pub fn tcp_configured(
        address: impl Into<String>,
        connect: TcpConnect,
        io_bounds: IoBounds,
    ) -> Self {
        Self::Tcp {
            address: address.into(),
            connect,
            io_bounds,
        }
    }

    /// Creates UDP transport options with the default connection and I/O settings.
    pub fn udp(address: impl Into<String>) -> Self {
        Self::udp_configured(address, UdpConnect::default(), IoBounds::UDP_DEFAULT)
    }

    /// Creates UDP transport options with explicit connection and I/O settings.
    pub fn udp_configured(
        address: impl Into<String>,
        connect: UdpConnect,
        io_bounds: IoBounds,
    ) -> Self {
        Self::Udp {
            address: address.into(),
            connect,
            io_bounds,
        }
    }

    /// Creates serial transport options with default I/O settings and no startup writes.
    pub fn serial(port: impl Into<String>, baud_rate: u32) -> Self {
        Self::serial_configured(
            SerialConfig::new(port).with_baud_rate(baud_rate),
            IoBounds::SERIAL_DEFAULT,
        )
    }

    /// Creates serial transport options with explicit device and I/O settings.
    pub fn serial_configured(config: SerialConfig, io_bounds: IoBounds) -> Self {
        Self::Serial { config, io_bounds }
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
    camera_id: Option<IndividualCameraId>,
    mount: Option<MountPosition>,
}

impl<P: CameraProfile> Clone for CameraConfig<P> {
    fn clone(&self) -> Self {
        Self {
            profile: self.profile.clone(),
            transport: self.transport.clone(),
            policy: self.policy,
            camera_id: self.camera_id,
            mount: self.mount,
        }
    }
}
impl<P: CameraProfile> std::fmt::Debug for CameraConfig<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraConfig")
            .field("profile", &self.profile)
            .field("transport", &self.transport)
            .field("policy", &self.policy)
            .field("camera_id", &self.camera_id)
            .field("mount", &self.mount)
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
        Self::tcp_configured(address, TcpConnect::default(), IoBounds::TCP_DEFAULT)
    }
    /// Selects UDP using the profile's default port for a host-only address.
    pub fn udp(address: impl Into<String>) -> Self
    where
        P: SupportsUdp,
    {
        Self::udp_configured(address, UdpConnect::default(), IoBounds::UDP_DEFAULT)
    }
    /// Selects an individual camera on a serial VISCA bus.
    pub fn serial(port: impl Into<String>, baud_rate: u32) -> Self
    where
        P: SupportsSerial,
    {
        Self::serial_configured(
            SerialConfig::new(port).with_baud_rate(baud_rate),
            IoBounds::SERIAL_DEFAULT,
        )
    }
    /// Selects TCP with explicit connection and I/O settings.
    pub fn tcp_configured(
        address: impl Into<String>,
        connect: TcpConnect,
        io_bounds: IoBounds,
    ) -> Self
    where
        P: SupportsTcp,
    {
        const {
            assert!(
                P::TRANSPORTS.contains(crate::transport::StandardTransport::Tcp),
                "profile marker `SupportsTcp` requires its transport in TRANSPORTS",
            );
        }
        Self::typed(TransportOptions::tcp_configured(
            address, connect, io_bounds,
        ))
    }

    /// Selects UDP with explicit connection and I/O settings.
    pub fn udp_configured(
        address: impl Into<String>,
        connect: UdpConnect,
        io_bounds: IoBounds,
    ) -> Self
    where
        P: SupportsUdp,
    {
        const {
            assert!(
                P::TRANSPORTS.contains(crate::transport::StandardTransport::Udp),
                "profile marker `SupportsUdp` requires its transport in TRANSPORTS",
            );
        }
        Self::typed(TransportOptions::udp_configured(
            address, connect, io_bounds,
        ))
    }

    /// Selects an individual camera on a serial bus with explicit settings.
    pub fn serial_configured(config: SerialConfig, io_bounds: IoBounds) -> Self
    where
        P: SupportsSerial,
    {
        const {
            assert!(
                P::TRANSPORTS.contains(crate::transport::StandardTransport::Serial),
                "profile marker `SupportsSerial` requires its transport in TRANSPORTS",
            );
        }
        Self::typed(TransportOptions::serial_configured(config, io_bounds))
    }
}
impl<P: CameraProfile> CameraConfig<P> {
    fn from_selection(profile: P::Selection, transport: TransportOptions) -> Self {
        Self {
            profile,
            transport,
            policy: crate::session_config::SessionPolicy::DEFAULT,
            camera_id: None,
            mount: None,
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
            TransportOptions::Tcp { address, .. } | TransportOptions::Udp { address, .. } => {
                Some(address)
            }
            _ => None,
        }
    }
    /// Returns the explicit VISCA address override, or `None` for the profile default.
    pub const fn camera_id(&self) -> Option<IndividualCameraId> {
        self.camera_id
    }
    /// Selects an explicit individual VISCA target, whose type validates addresses 1–7.
    #[must_use]
    pub const fn with_camera_id(mut self, id: IndividualCameraId) -> Self {
        self.camera_id = Some(id);
        self
    }
    /// Returns the explicit installation mount, or `None` for the profile default.
    ///
    /// Mount-dependent profiles default to [`MountPosition::Desktop`]; fixed-range
    /// profiles have no mount selection.
    pub const fn mount(&self) -> Option<MountPosition> {
        self.mount
    }
    /// Selects the mounting position already configured on the camera.
    ///
    /// Validation refuses this override for profiles with one fixed tilt range,
    /// before endpoint parsing or connection I/O.
    #[must_use]
    pub const fn with_mount(mut self, mount: MountPosition) -> Self {
        self.mount = Some(mount);
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
            camera_id: self.camera_id,
            mount: self.mount,
        }
    }
    /// Resolves the sole registration once and builds its reusable session data.
    #[cfg(any(feature = "async", feature = "blocking"))]
    fn session_config_data(
        &self,
    ) -> crate::Result<(
        crate::session_config::SessionConfig,
        std::sync::Arc<crate::TargetRegistration>,
    )> {
        let profile = P::selected_spec(&self.profile)?;
        let id = self
            .camera_id
            .unwrap_or(profile.capabilities().default_camera_id);
        let mut registration = crate::TargetRegistration::new(id, profile);
        if let Some(mount) = self.mount {
            registration = registration.with_mount(mount)?;
        }
        let registration = std::sync::Arc::new(registration);
        let config = crate::session_config::SessionConfig::from_registration(registration.clone())
            .with_policy(self.policy)
            .with_transport(self.transport.clone());
        Ok((config, registration))
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
        self.validated_camera_config().map(|(config, _)| config)
    }
    /// Keeps configured-transport validation for caller-owned `open`, too.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn validated_camera_config(
        &self,
    ) -> crate::Result<(
        crate::session_config::ValidatedSessionConfig,
        std::sync::Arc<crate::TargetRegistration>,
    )> {
        let (config, registration) = self.session_config_data()?;
        let config = config.validated()?;
        validate_selected_transport(&config, &self.transport)?;
        Ok((config, registration))
    }
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn connection_plan(&self) -> crate::Result<CameraConnectionPlan> {
        let (config, registration) = self.session_config_data()?;
        Ok(CameraConnectionPlan {
            connection: config.connection_plan()?,
            registration,
        })
    }
}

pub(crate) fn selection_for_spec(spec: crate::profiles::ProfileSpec) -> ProfileSelection {
    match spec.capabilities().profile_id {
        Some(id) => ProfileSelection::Builtin(id),
        None => ProfileSelection::Custom(Box::new(spec)),
    }
}

#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) fn validate_selected_transport(
    config: &crate::session_config::ValidatedSessionConfig,
    selected: &TransportOptions,
) -> crate::Result<()> {
    match selected.identity() {
        Some(identity) => config.validate_for_transport(identity).map(|_| ()),
        None => Ok(()),
    }
}

/// One validated construction plan, shared by both execution facades.
#[derive(Debug, Clone)]
#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) struct ConnectionPlan {
    pub(crate) transport: ConnectionTransport,
    pub(crate) session_config: crate::session_config::ValidatedSessionConfig,
}

/// Only network variants carry endpoints and network transport settings.
#[derive(Debug, Clone)]
#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) enum ConnectionTransport {
    Network(NetworkConnection),
    Serial {
        config: SerialConfig,
        io_bounds: IoBounds,
    },
}

#[cfg(any(feature = "async", feature = "blocking"))]
impl ConnectionTransport {
    /// The already-resolved endpoint or serial device path.
    #[cfg(all(test, feature = "serde", feature = "blocking"))]
    pub(crate) fn endpoint(&self) -> String {
        match self {
            Self::Network(network) => network.endpoint.to_string(),
            Self::Serial { config, .. } => config.port().to_owned(),
        }
    }

    #[cfg(test)]
    #[allow(clippy::panic)]
    pub(crate) fn into_serial(self) -> SerialConfig {
        match self {
            Self::Serial { config, .. } => config,
            Self::Network(_) => panic!("test requires a serial plan"),
        }
    }

    #[cfg(test)]
    #[allow(clippy::panic)]
    pub(crate) fn into_network(self) -> NetworkConnection {
        match self {
            Self::Network(network) => network,
            Self::Serial { .. } => panic!("test requires a network plan"),
        }
    }
}

#[derive(Debug, Clone)]
#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) struct NetworkConnection {
    pub(crate) connector: NetworkConnector,
    pub(crate) endpoint: crate::transport::NetworkEndpoint,
    pub(crate) io_bounds: IoBounds,
}

#[derive(Debug, Clone, Copy)]
#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) enum NetworkConnector {
    Tcp(TcpConnect),
    Udp(UdpConnect),
}

#[cfg(any(feature = "async", feature = "blocking"))]
impl NetworkConnector {
    pub(crate) const fn kind(self) -> crate::transport::NetworkTransport {
        match self {
            Self::Tcp(_) => crate::transport::NetworkTransport::Tcp,
            Self::Udp(_) => crate::transport::NetworkTransport::Udp,
        }
    }
}

/// A single-camera plan retains the registration already proved by its config.
#[derive(Debug, Clone)]
#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) struct CameraConnectionPlan {
    pub(crate) connection: ConnectionPlan,
    pub(crate) registration: std::sync::Arc<crate::TargetRegistration>,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    #[cfg(any(feature = "blocking", feature = "async"))]
    use super::TransportOptions;
    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn every_connection_plan_shares_its_single_proven_registration() {
        use crate::{camera::CameraConfig, profiles::PtzOpticsG2, IndividualCameraId};
        for config in [
            CameraConfig::<PtzOpticsG2>::tcp("camera.invalid"),
            CameraConfig::<PtzOpticsG2>::udp("camera.invalid"),
            CameraConfig::<PtzOpticsG2>::serial("/dev/not-opened", 9600),
        ] {
            let plan = config
                .with_camera_id(IndividualCameraId::CAMERA_3)
                .connection_plan()
                .expect("pure connection plan");
            assert_eq!(plan.registration.target(), IndividualCameraId::CAMERA_3);
            let stored = plan
                .connection
                .session_config
                .registered_target(plan.registration.slot())
                .expect("same registered camera");
            assert!(std::sync::Arc::ptr_eq(&plan.registration, &stored));
            if matches!(
                plan.connection.transport,
                super::ConnectionTransport::Serial { .. }
            ) {
                assert_eq!(
                    plan.connection.transport.into_serial().port(),
                    "/dev/not-opened"
                );
            }
        }
        let config = CameraConfig::<crate::profiles::SonyFr7>::udp("camera.invalid")
            .with_camera_id(IndividualCameraId::CAMERA_3)
            .with_mount(crate::MountPosition::Ceiling);
        let plan = config.connection_plan().expect("ceiling connection plan");
        let stored = plan
            .connection
            .session_config
            .registered_target(plan.registration.slot())
            .expect("registered ceiling camera");
        assert!(std::sync::Arc::ptr_eq(&plan.registration, &stored));
        assert_eq!(
            plan.registration.mount(),
            Some(crate::MountPosition::Ceiling)
        );
        assert_eq!(plan.registration.tilt_range_degrees(), Some(-210.0..=15.0));
        let session = config.session_config().expect("ceiling session data");
        assert_eq!(
            session
                .registration(IndividualCameraId::CAMERA_3)
                .expect("target")
                .mount(),
            Some(crate::MountPosition::Ceiling)
        );
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn fixed_range_mount_is_refused_before_endpoint_parsing() {
        let config = super::CameraConfig::<crate::profiles::PtzOpticsG2>::tcp("/never-parsed")
            .with_mount(crate::MountPosition::Ceiling);
        for error in [
            config.validate().expect_err("fixed-range mount"),
            config
                .connection_plan()
                .expect_err("mount before endpoint parsing"),
        ] {
            assert!(matches!(
                error,
                crate::Error::FeatureNotSupported {
                    feature: "mount-dependent tilt range",
                }
            ));
        }
    }

    #[cfg(feature = "blocking")]
    #[test]
    fn blocking_camera_open_preserves_configured_transport_validation() {
        use crate::{
            blocking::{CameraSession, Session},
            camera::CameraConfig,
            profiles::{ProfileId, SonyFr7},
            transport::{blocking::Udp, StandardTransport},
            Error, SessionConfig,
        };
        let camera = CameraConfig::new(ProfileId::SonyFr7, TransportOptions::tcp("unused.invalid"))
            .with_camera_id(crate::types::IndividualCameraId::CAMERA_3);
        let udp = || {
            Udp::connect_with_config(
                "127.0.0.1:9",
                crate::transport::UdpConnect::default(),
                crate::transport::IoBounds::default(),
            )
            .expect("local UDP transport")
        };
        assert!(matches!(
            CameraSession::open(udp(), &camera),
            Err(Error::UnsupportedTransport {
                transport: StandardTransport::Tcp,
                ..
            })
        ));
        let config = SessionConfig::from_compile_time::<SonyFr7>()
            .expect("profile")
            .with_transport(TransportOptions::tcp("unused.invalid"));
        Session::open(udp(), &config)
            .expect("session checks actual UDP transport")
            .close()
            .expect("close");
        let single = CameraSession::open(udp(), &camera.with_transport(TransportOptions::Custom))
            .expect("custom selection uses opened UDP facts");
        assert_eq!(single.target(), crate::types::IndividualCameraId::CAMERA_3);
        single.close().expect("close");
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn unsupported_transport_names_built_in_and_custom_profiles() {
        use crate::{
            capabilities::Capabilities,
            config::CameraConfig,
            error::{ErrorKind, Recovery},
            profiles::{ProfileEnvelope, ProfileId, ProfileSpec, TransportCompatibility},
            transport::StandardTransport,
            types::CommandSocketCount,
            Error,
        };
        let built_in = ProfileId::SonyEviH100.spec().expect("built-in profile");
        let custom = ProfileSpec::builder(
            Capabilities::runtime_baseline("Custom serial camera", 1).expect("custom capabilities"),
        )
        .transports(TransportCompatibility::new().with_serial())
        .envelope(ProfileEnvelope::RawVisca)
        .timing(built_in.timing())
        .maximum_command_sockets(CommandSocketCount::ONE)
        .supports_operation_complete(false)
        .supports_command_cancel(false)
        .position_inquiries(None)
        .build()
        .expect("custom serial-only profile");
        for (profile, expected_identity) in
            [(built_in, Some(ProfileId::SonyEviH100)), (custom, None)]
        {
            let name = profile.name().to_owned();
            let config = CameraConfig::new(profile, TransportOptions::tcp("unresolved.invalid"));
            let error = config.validate().expect_err("serial-only profile");
            assert_eq!(error.kind(), ErrorKind::Unsupported);
            assert_eq!(error.recovery(), Recovery::Abandon);
            assert_eq!(
                error.to_string(),
                format!("Profile {name} does not support TCP transport")
            );
            assert!(matches!(
                error,
                Error::UnsupportedTransport {
                    model,
                    profile,
                    transport: StandardTransport::Tcp,
                } if model == name && profile == expected_identity
            ));
        }
    }

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
                    const DOMAINS: crate::capabilities::DomainSet = Base::DOMAINS;
                    const TRANSPORTS: crate::capabilities::TransportSet =
                        ($transports).transport_set();
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
                            .position_inquiries(
                                crate::AffectedAxes::ZOOM | crate::AffectedAxes::FOCUS,
                            )
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
        let plan = config.connection_plan().expect("default target plan");
        assert_eq!(RESOLUTIONS.load(Ordering::SeqCst), 1);
        assert_eq!(
            plan.registration.target(),
            crate::IndividualCameraId::CAMERA_2
        );
        assert_eq!(
            config
                .with_camera_id(crate::IndividualCameraId::CAMERA_3)
                .connection_plan()
                .expect("address override")
                .registration
                .target(),
            crate::IndividualCameraId::CAMERA_3
        );

        let endpoint = |config: CameraConfig<Downstream>| {
            config
                .connection_plan()
                .map(|plan| plan.connection.transport.into_network())
                .expect("downstream plan")
                .endpoint
                .to_string()
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

        // Explicit transport selection remains runtime checked for typed profiles.
        impl SupportsUdp for UdpOnly {}
        for transport in [
            TransportOptions::tcp("/never-resolved"),
            TransportOptions::serial("/never-opened", 9600),
        ] {
            let config =
                CameraConfig::<UdpOnly>::typed(TransportOptions::Custom).with_transport(transport);
            let preflight = config.validate().expect_err("unsupported custom transport");
            let session = config.session_config().expect_err("same session preflight");
            assert_eq!(preflight.to_string(), session.to_string());
            assert!(matches!(
                preflight,
                crate::Error::UnsupportedTransport { profile: None, .. }
            ));
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
            .connection_plan()
            .map(|plan| plan.connection.transport.into_network())
            .is_err());
        assert_eq!(
            CameraConfig::<UdpOnly>::udp("camera.local")
                .connection_plan()
                .map(|plan| plan.connection.transport.into_network())
                .expect("UDP plan")
                .endpoint
                .to_string(),
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
            const DOMAINS: crate::capabilities::DomainSet = crate::capabilities::DomainSet::empty();
            const TRANSPORTS: crate::capabilities::TransportSet =
                crate::profiles::TransportCompatibility::new()
                    .with_tcp(5678)
                    .transport_set();
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
                .connection_plan()
                .map(|plan| plan.connection.transport.into_network())
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
            camera::{CameraConfig, TransportOptions},
            profiles::{PtzOpticsG2, SonyFr7},
            transport::{BufferConfig, IoBounds, TcpConnect, TcpKeepaliveConfig, UdpConnect},
        };

        let io_bounds = IoBounds::new(
            Duration::from_millis(19),
            Duration::from_millis(23),
            BufferConfig::default()
                .with_recv_buffer_size(31)
                .with_max_buffer_size(37),
        )
        .expect("valid I/O bounds");
        let tcp_options = TcpConnect::default()
            .with_connect_timeout(Duration::from_millis(17))
            .expect("valid connect timeout")
            .with_nodelay(Some(false))
            .with_keepalive(Some(TcpKeepaliveConfig::new(Duration::from_secs(4))))
            .expect("valid keepalive")
            .with_ttl(Some(41))
            .expect("valid TTL");
        let udp_options = UdpConnect::default()
            .with_connect_timeout(Duration::from_millis(17))
            .expect("valid connect timeout")
            .with_ttl(Some(41))
            .expect("valid TTL");

        let tcp =
            CameraConfig::<PtzOpticsG2>::tcp_configured("camera.local", tcp_options, io_bounds)
                .connection_plan()
                .map(|plan| plan.connection.transport.into_network())
                .expect("TCP plan");
        assert_eq!(tcp.endpoint.to_string(), "camera.local:5678");
        assert_eq!(tcp.io_bounds, io_bounds);
        assert!(
            matches!(tcp.connector, super::NetworkConnector::Tcp(found) if found == tcp_options)
        );

        let udp =
            CameraConfig::<PtzOpticsG2>::udp_configured("camera.local", udp_options, io_bounds)
                .connection_plan()
                .map(|plan| plan.connection.transport.into_network())
                .expect("UDP plan");
        assert_eq!(udp.endpoint.to_string(), "camera.local:1259");
        assert_eq!(udp.io_bounds, io_bounds);
        assert!(
            matches!(udp.connector, super::NetworkConnector::Udp(found) if found == udp_options)
        );

        let sony = CameraConfig::<SonyFr7>::udp("camera.local")
            .connection_plan()
            .map(|plan| plan.connection.transport.into_network())
            .expect("Sony UDP plan");
        assert_eq!(sony.endpoint.to_string(), "camera.local:52381");

        let mut connector_calls = 0;
        let unsupported = CameraConfig::<SonyFr7>::typed(TransportOptions::Custom)
            .with_transport(TransportOptions::tcp("does-not-resolve.invalid"))
            .connection_plan()
            .map(|plan| plan.connection.transport.into_network());
        if unsupported.is_ok() {
            connector_calls += 1;
        }
        assert!(unsupported.is_err());
        assert_eq!(connector_calls, 0);

        let malformed = CameraConfig::<PtzOpticsG2>::udp("invalid:address:format")
            .connection_plan()
            .map(|plan| plan.connection.transport.into_network());
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
            transport::{BufferConfig, IoBounds, SerialConfig},
        };

        let bounds = IoBounds::new(
            Duration::from_millis(37),
            Duration::from_millis(41),
            BufferConfig::default()
                .with_recv_buffer_size(43)
                .with_max_buffer_size(53),
        )
        .expect("valid serial bounds");
        let device = SerialConfig::new("/dev/fake-visca").with_baud_rate(38_400);
        let plan = CameraConfig::<PtzOpticsG2>::serial_configured(device.clone(), bounds)
            .connection_plan()
            .expect("valid serial config");
        assert!(matches!(
            plan.connection.transport,
            super::ConnectionTransport::Serial { config, io_bounds }
                if config == device && io_bounds == bounds
        ));
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
        use crate::{
            camera::CameraConfig,
            profiles::PtzOpticsG2,
            transport::{IoBounds, SerialConfig, SerialStartup},
        };

        let plain = CameraConfig::<PtzOpticsG2>::serial("/dev/fake-visca", 9_600)
            .connection_plan()
            .map(|plan| plan.connection.transport.into_serial())
            .expect("valid serial config");
        assert_eq!(plain.startup, SerialStartup::default());
        assert!(!plain.startup.address_set && !plain.startup.interface_clear);

        let selected = SerialStartup::default()
            .with_address_set(true)
            .with_interface_clear(true);
        let configured = CameraConfig::<PtzOpticsG2>::serial_configured(
            SerialConfig::new("/dev/fake-visca").with_startup(selected),
            IoBounds::default(),
        )
        .connection_plan()
        .map(|plan| plan.connection.transport.into_serial())
        .expect("valid serial config");
        assert_eq!(configured.startup, selected);
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
            const DOMAINS: crate::capabilities::DomainSet = GenericVisca::DOMAINS;
            const TRANSPORTS: crate::capabilities::TransportSet = GenericVisca::TRANSPORTS;
            const TYPED_SUPPORT: crate::capabilities::TypedSupportSet = GenericVisca::TYPED_SUPPORT;
            const ID: Option<ProfileId> = GenericVisca::ID;
            fn spec() -> crate::Result<ProfileSpec> {
                GenericVisca::spec()
            }
        }
        impl crate::capabilities::SupportsTcp for Community {}
        let config = super::CameraConfig::<Community>::tcp("camera.local")
            .with_camera_id(crate::IndividualCameraId::CAMERA_2)
            .with_mount(crate::MountPosition::Ceiling);
        let cloned = config.clone();
        assert_eq!(cloned.address(), Some("camera.local"));
        assert_eq!(
            cloned.camera_id(),
            Some(crate::IndividualCameraId::CAMERA_2)
        );
        assert_eq!(cloned.mount(), Some(crate::MountPosition::Ceiling));
        assert!(!format!("{cloned:?}").is_empty());
    }

    #[cfg(all(feature = "schemars", feature = "ts-rs"))]
    #[test]
    fn portable_camera_config_keeps_mount_optional() {
        let schema = schemars::schema_for!(super::CameraConfig);
        let mount = schema
            .get("properties")
            .expect("config properties")
            .get("mount")
            .expect("mount property");
        assert!(mount.to_string().contains("MountPosition"));
        assert!(!schema
            .get("required")
            .expect("required config fields")
            .as_array()
            .expect("required fields array")
            .iter()
            .any(|field| field == "mount"));
        let declaration = <super::CameraConfig as ts_rs::TS>::decl(&ts_rs::Config::default());
        assert!(
            declaration.contains("mount: MountPosition | null"),
            "{declaration}"
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn typed_and_runtime_configs_share_one_checked_serialized_shape() {
        use crate::profiles::{PtzOpticsG2, SonyFr7};
        let typed = super::CameraConfig::<PtzOpticsG2>::tcp("camera.local")
            .with_camera_id(crate::IndividualCameraId::CAMERA_2)
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
        let mut broadcast = json.clone();
        broadcast["camera_id"] = 8.into();
        let error = serde_json::from_value::<super::CameraConfig>(broadcast)
            .expect_err("broadcast is not an individual target");
        assert!(error.to_string().contains("7"), "{error}");
        let mut unknown_policy = json;
        unknown_policy["policy"]["unexpected"] = true.into();
        assert!(serde_json::from_value::<super::CameraConfig>(unknown_policy).is_err());

        let ceiling = super::CameraConfig::<SonyFr7>::udp("camera.local")
            .with_mount(crate::MountPosition::Ceiling);
        let ceiling_json = serde_json::to_value(&ceiling).expect("ceiling serialization");
        assert_eq!(ceiling_json["mount"], "ceiling");
        let runtime: super::CameraConfig =
            serde_json::from_value(ceiling_json.clone()).expect("runtime ceiling config");
        assert_eq!(runtime.mount(), Some(crate::MountPosition::Ceiling));
        let typed = runtime
            .into_typed::<SonyFr7>()
            .expect("typed FR7 selection");
        assert_eq!(typed.mount(), Some(crate::MountPosition::Ceiling));
        assert_eq!(
            serde_json::to_value(typed.into_any().expect("runtime FR7 selection"))
                .expect("runtime ceiling serialization"),
            ceiling_json
        );
        for mount in [None, Some(serde_json::Value::Null)] {
            let mut json = ceiling_json.clone();
            if let Some(value) = mount {
                json["mount"] = value;
            } else {
                json.as_object_mut().expect("config object").remove("mount");
            }
            let restored: super::CameraConfig<SonyFr7> =
                serde_json::from_value(json).expect("default mount config");
            assert_eq!(restored.mount(), None);
            #[cfg(any(feature = "blocking", feature = "async"))]
            assert_eq!(
                restored
                    .validated_camera_config()
                    .expect("valid FR7 config")
                    .1
                    .mount(),
                Some(crate::MountPosition::Desktop)
            );
        }
        #[cfg(any(feature = "blocking", feature = "async"))]
        {
            let mut json =
                serde_json::to_value(super::CameraConfig::<PtzOpticsG2>::tcp("camera.local"))
                    .expect("G2 config");
            json["mount"] = "ceiling".into();
            let restored: super::CameraConfig = serde_json::from_value(json).expect("mount data");
            assert!(matches!(
                restored.validate(),
                Err(crate::Error::FeatureNotSupported {
                    feature: "mount-dependent tilt range",
                })
            ));
        }
    }
}
