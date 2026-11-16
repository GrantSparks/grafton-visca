//! Shared profile/target configuration for the owner-backed session facades.
//!
//! [`crate::config::SessionConfig`] is intentionally independent of either execution mode.
//! It is therefore available in builds that select only the blocking facade,
//! only the async facade, or neither facade.  A configuration may be edited
//! while it is being built; once passed to a session, the resulting registry
//! is immutable for the lifetime of that owner.

use std::{num::NonZeroUsize, sync::Arc};

use crate::{
    config::OperationalTuning,
    profiles::{CompileTimeProfile, ProfileSelection, ProfileSpec},
    target::{PerTarget, TargetSlot},
    CameraId, Error, Result,
};

/// Operational overrides checked against every profile in one session.
#[cfg(any(feature = "async", feature = "blocking"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ValidatedTuning(OperationalTuning);

#[cfg(any(feature = "async", feature = "blocking"))]
impl ValidatedTuning {
    pub(crate) fn validate<'a>(
        tuning: OperationalTuning,
        profiles: impl IntoIterator<Item = &'a ProfileSpec>,
    ) -> Result<Self> {
        let mut profiles = profiles.into_iter().peekable();
        if profiles.peek().is_none() {
            return Err(Error::InvalidRequest {
                reason: "tuning validation requires a registered profile".into(),
            });
        }
        for profile in profiles {
            profile.validate_tuning(tuning)?;
        }
        Ok(Self(tuning))
    }

    pub(crate) const fn get(self) -> OperationalTuning {
        self.0
    }

    #[cfg(test)]
    pub(crate) fn for_test(tuning: OperationalTuning) -> Result<Self> {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()?;
        Self::validate(tuning, [&profile])
    }
}

/// Default number of requests a session may admit before fail-fast rejection.
#[allow(clippy::unwrap_used)]
pub const DEFAULT_ADMISSION_CAPACITY: NonZeroUsize = NonZeroUsize::new(64).unwrap();

/// The construction-time session policy both [`crate::config::SessionConfig`] and
/// [`crate::config::CameraConfig`](crate::config::CameraConfig) carry, stored once and
/// validated once, when a session opens (#805).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(
    feature = "schemars",
    derive(schemars::JsonSchema),
    schemars(deny_unknown_fields)
)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub(crate) struct SessionPolicy {
    pub(crate) tuning: OperationalTuning,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub(crate) admission_capacity: NonZeroUsize,
    pub(crate) sony_sequence_reset_on_connect: bool,
    pub(crate) strict_unconfirmed_poison: bool,
}

impl SessionPolicy {
    pub(crate) const DEFAULT: Self = Self {
        tuning: OperationalTuning::new(),
        admission_capacity: DEFAULT_ADMISSION_CAPACITY,
        sony_sequence_reset_on_connect: false,
        strict_unconfirmed_poison: false,
    };
}

/// The session-policy builder and getter pairs, expanded on every
/// configuration type that carries a [`SessionPolicy`] in a `policy` field, so
/// their names, contracts and docs cannot drift apart. Nothing here validates:
/// a session validates its policy once, against every registered profile,
/// when it opens.
macro_rules! session_policy_accessors {
    () => {
        /// Sets the operational overrides prepared requests use.
        ///
        /// Nothing is validated here: opening a session checks the tuning
        /// against every registered profile, before any transport I/O, and
        /// rejects values that weaken pacing minima, raise a socket limit,
        /// undercut a deadline, or specify incoherent retry timing.
        #[must_use]
        pub const fn with_tuning(mut self, tuning: $crate::config::OperationalTuning) -> Self {
            self.policy.tuning = tuning;
            self
        }

        /// Returns the operational overrides this configuration carries.
        ///
        /// This is the *construction* value. A session opened from it may
        /// since have been reconfigured at runtime; read the session's own
        /// `tuning` for the value requests are actually prepared under.
        #[must_use]
        pub const fn tuning(&self) -> $crate::config::OperationalTuning {
            self.policy.tuning
        }

        /// Sets the immutable request-admission capacity for sessions opened
        /// from this configuration.
        ///
        /// Admission is fail-fast: once this many ordinary requests are
        /// pending or active, a new submission returns
        /// [`Error::RuntimeQueueFull`](crate::Error::RuntimeQueueFull) rather
        /// than waiting for another request to finish.
        ///
        /// Each registered camera also has a control reserve: one admission
        /// slot per typed STOP its profile supports (pan/tilt, zoom, focus; at
        /// most three), which only an urgent typed STOP may use. A STOP
        /// therefore still gets through when ordinary work fills this
        /// capacity. At most `capacity` plus the sum of the reserves can be
        /// pending or active at once (D26, #778).
        ///
        /// The capacity is fixed when the session opens and is independent of
        /// transport buffers and per-camera VISCA socket capacity; changing a
        /// cloned configuration does not affect an already-open session.
        /// Opening rejects capacities whose control reserves, channel
        /// bookkeeping, or raw-stream correlation bounds cannot be represented
        /// by `usize`, before any transport I/O.
        #[must_use]
        pub const fn with_admission_capacity(mut self, capacity: std::num::NonZeroUsize) -> Self {
            self.policy.admission_capacity = capacity;
            self
        }

        /// Returns the request-admission capacity this configuration carries;
        /// see [`Self::with_admission_capacity`].
        #[must_use]
        pub const fn admission_capacity(&self) -> std::num::NonZeroUsize {
            self.policy.admission_capacity
        }

        /// Opts in to sending Sony's sequence-number RESET control command
        /// before the owner starts accepting requests.
        ///
        /// This is disabled by default because opening a session otherwise
        /// performs no protocol write. (A serial transport's optional Address
        /// Set and I/F Clear belong to the transport and run before the
        /// session opens; see `transport::serial::Startup`, which also writes
        /// nothing by default.) Enabling it requires every registered profile
        /// to use the Sony encapsulated envelope; an incompatible
        /// configuration fails to open before transport I/O.
        #[must_use]
        pub const fn with_sony_sequence_reset_on_connect(mut self, enabled: bool) -> Self {
            self.policy.sony_sequence_reset_on_connect = enabled;
            self
        }

        /// Returns whether session startup sends Sony's sequence-number
        /// RESET.
        #[must_use]
        pub const fn sony_sequence_reset_on_connect(&self) -> bool {
            self.policy.sony_sequence_reset_on_connect
        }

        /// Selects strict whole-session poisoning for unconfirmable raw
        /// commands.
        ///
        /// The default (`false`) fails only the affected command with
        /// [`Error::UnsequencedCommandUnconfirmed`](crate::Error::UnsequencedCommandUnconfirmed)
        /// and quarantines its raw-VISCA correlation while the session and
        /// unrelated requests remain usable. Setting `true` instead makes that
        /// uncertainty terminal and reports
        /// [`Error::StreamPoisoned`](crate::Error::StreamPoisoned). The policy
        /// is fixed when the session opens, unlike the runtime-replaceable
        /// tuning, and has no effect on Sony's sequence-correlated envelope.
        #[must_use]
        pub const fn with_strict_unconfirmed_poison(mut self, enabled: bool) -> Self {
            self.policy.strict_unconfirmed_poison = enabled;
            self
        }

        /// Returns whether unconfirmable raw commands poison the whole
        /// session.
        #[must_use]
        pub const fn strict_unconfirmed_poison(&self) -> bool {
            self.policy.strict_unconfirmed_poison
        }
    };
}
pub(crate) use session_policy_accessors;

/// Immutable target/profile registration and session-wide operational tuning.
///
/// Targets are VISCA camera IDs 1 through 7.  Broadcast is deliberately not a
/// session target: a session owns response routing and every response must be
/// attributable to one registered camera.  Profile values are kept behind
/// [`Arc`] so cloning a configuration or producing async camera views does not
/// copy the validated profile inventory.
#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "schemars",
    derive(schemars::JsonSchema),
    schemars(deny_unknown_fields)
)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct SessionConfig {
    #[cfg_attr(feature = "schemars", schemars(with = "Vec<SessionTargetData>"))]
    #[cfg_attr(feature = "ts-rs", ts(as = "Vec<SessionTargetData>"))]
    targets: PerTarget<Option<Arc<crate::TargetRegistration>>>,
    policy: SessionPolicy,
    transport: crate::camera::config::TransportOptions,
    transport_config: Option<crate::transport::TransportConfig>,
    serial_startup: crate::transport::SerialStartup,
}

impl SessionConfig {
    /// Creates a single-target configuration for the profile's individual default.
    #[must_use]
    pub fn new(profile: ProfileSpec) -> Self {
        let target = TargetSlot::from(profile.capabilities().default_camera_id);
        Self::with_profile(target, profile)
    }

    /// Creates a single-target configuration for an explicit camera.
    pub fn for_target(target: CameraId, profile: ProfileSpec) -> Result<Self> {
        Ok(Self::with_profile(TargetSlot::try_from(target)?, profile))
    }

    /// Creates a configuration using the compile-time profile's default target.
    pub fn from_compile_time<P>() -> Result<Self>
    where
        P: CompileTimeProfile,
    {
        Ok(Self::new(ProfileSpec::from_compile_time::<P>()?))
    }

    /// Adds one individual target using a built-in ID or a custom specification.
    /// Duplicate and broadcast IDs are rejected before registration changes.
    pub fn with_target(
        mut self,
        target: CameraId,
        profile: impl Into<ProfileSelection>,
    ) -> Result<Self> {
        let slot = TargetSlot::try_from(target)?;
        if self.targets[slot].is_some() {
            return Err(Error::InvalidRequest {
                reason: "session target is already registered".into(),
            });
        }
        let profile = profile.into().spec()?;
        self.targets[slot] = Some(Arc::new(crate::TargetRegistration::from_profile(
            slot, profile,
        )));
        Ok(self)
    }

    /// Returns the registered profile for one individual target.
    #[must_use]
    pub fn profile(&self, target: CameraId) -> Option<&ProfileSpec> {
        TargetSlot::try_from(target)
            .ok()
            .and_then(|slot| self.targets[slot].as_deref())
            .map(crate::TargetRegistration::profile)
    }

    /// Selects the mounting position already configured on one target.
    ///
    /// The canonical profile remains unchanged. Profiles without source-backed
    /// mount-dependent limits refuse this setting before the config changes.
    pub fn with_mount(mut self, target: CameraId, mount: crate::MountPosition) -> Result<Self> {
        let slot = TargetSlot::try_from(target)?;
        let registration = self.targets[slot]
            .as_deref()
            .ok_or_else(|| Error::InvalidRequest {
                reason: "session target is not registered".into(),
            })?;
        let selected = registration.clone().with_mount(mount)?;
        self.targets[slot] = Some(Arc::new(selected));
        Ok(self)
    }

    /// Returns the target's profile and installation facts.
    #[must_use]
    pub fn registration(&self, target: CameraId) -> Option<&crate::TargetRegistration> {
        TargetSlot::try_from(target)
            .ok()
            .and_then(|slot| self.targets[slot].as_deref())
    }

    /// Returns registered target IDs in camera-number order without allocating.
    pub fn targets(&self) -> impl Iterator<Item = CameraId> + '_ {
        self.targets
            .iter()
            .filter_map(|(target, registration)| registration.as_ref().map(|_| target.camera()))
    }
    /// Returns the only target, or `None` when there are zero or several.
    pub fn sole_target(&self) -> Option<CameraId> {
        let mut targets = self.targets();
        let first = targets.next()?;
        targets.next().is_none().then_some(first)
    }
    /// Returns the transport selected for standard session connection.
    pub const fn transport(&self) -> &crate::camera::config::TransportOptions {
        &self.transport
    }
    /// Selects a standard transport; the default is a caller-supplied custom transport.
    #[must_use]
    pub fn with_transport(mut self, transport: crate::camera::config::TransportOptions) -> Self {
        self.transport = transport;
        self
    }
    /// Returns explicit transport settings, or `None` for the transport defaults.
    pub const fn transport_config(&self) -> Option<crate::transport::TransportConfig> {
        self.transport_config
    }
    /// Selects explicit transport timeout and buffer settings.
    #[must_use]
    pub const fn with_transport_config(
        mut self,
        config: crate::transport::TransportConfig,
    ) -> Self {
        self.transport_config = Some(config);
        self
    }
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) const fn with_optional_transport_config(
        mut self,
        config: Option<crate::transport::TransportConfig>,
    ) -> Self {
        self.transport_config = config;
        self
    }
    /// Returns the optional serial startup writes; the default writes nothing.
    pub const fn serial_startup(&self) -> crate::transport::SerialStartup {
        self.serial_startup
    }
    /// Selects optional serial Address Set and I/F Clear startup writes.
    #[must_use]
    pub const fn with_serial_startup(mut self, startup: crate::transport::SerialStartup) -> Self {
        self.serial_startup = startup;
        self
    }
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn serial_config(
        &self,
        port: &str,
        baud_rate: u32,
    ) -> Result<crate::transport::SerialConfig> {
        crate::camera::config::serial_device_config(
            port,
            baud_rate,
            self.transport_config,
            self.serial_startup,
        )
    }

    session_policy_accessors!();

    /// The session policy this configuration carries.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) const fn with_policy(mut self, policy: SessionPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Checks operational tuning against every registered profile.
    ///
    /// Opening a session runs this on its configured tuning, and the runtime
    /// reconfiguration path (#631) on each update, so an update is rejected on
    /// exactly the grounds that would have rejected it at construction.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn validate_tuning(&self, tuning: OperationalTuning) -> Result<ValidatedTuning> {
        ValidatedTuning::validate(
            tuning,
            self.targets.iter().filter_map(|(_, registration)| {
                registration
                    .as_deref()
                    .map(crate::TargetRegistration::profile)
            }),
        )
    }

    /// Validates this configuration's session policy against its registered
    /// profiles: the one check every session open runs, before any transport
    /// I/O.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn validated(self) -> Result<ValidatedSessionConfig> {
        if self.targets().next().is_none() {
            return Err(Error::InvalidRequest {
                reason: "session requires at least one registered target".into(),
            });
        }
        let capacity = self.policy.admission_capacity.get();
        let invalid_capacity = || Error::InvalidRequest {
            reason: "session admission capacity overflows control-reserve or correlation bounds"
                .into(),
        };
        let mut total_capacity = capacity;
        for profile in self.targets.iter().filter_map(|(_, registration)| {
            registration
                .as_deref()
                .map(crate::TargetRegistration::profile)
        }) {
            let reserve = usize::from(profile.typed_stop_paths());
            total_capacity = total_capacity
                .checked_add(reserve)
                .ok_or_else(invalid_capacity)?;
            let admissible = capacity.checked_add(reserve).ok_or_else(invalid_capacity)?;
            crate::runtime::engine::checked_stream_ledger_bound(admissible)
                .ok_or_else(invalid_capacity)?;
        }
        // Flume's receive path temporarily accounts for one additional
        // pending sender on top of ordinary and control-reserve capacity.
        if total_capacity.checked_add(1).is_none() {
            return Err(Error::InvalidRequest {
                reason: "session admission capacity leaves no room for channel bookkeeping".into(),
            });
        }
        if self.policy.sony_sequence_reset_on_connect
            && self
                .targets
                .iter()
                .filter_map(|(_, registration)| {
                    registration
                        .as_deref()
                        .map(crate::TargetRegistration::profile)
                })
                .any(|profile| {
                    profile.envelope() != crate::profiles::ProfileEnvelope::SonyEncapsulated
                })
        {
            return Err(Error::InvalidRequest {
                reason: "Sony sequence reset on connect requires Sony encapsulated profiles".into(),
            });
        }
        let tuning = self.validate_tuning(self.policy.tuning)?;
        Ok(ValidatedSessionConfig(self, tuning))
    }

    /// Borrow all registrations in the shape consumed by the owner adapter.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn profile_registry(&self) -> Vec<(TargetSlot, &ProfileSpec)> {
        self.targets
            .iter()
            .filter_map(|(target, profile)| {
                profile
                    .as_deref()
                    .map(|registration| (target, registration.profile()))
            })
            .collect()
    }

    /// The registered profile of `target`, shared with an owned camera view.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn registered_target(
        &self,
        target: TargetSlot,
    ) -> Result<Arc<crate::TargetRegistration>> {
        self.targets[target]
            .clone()
            .ok_or_else(|| Error::InvalidRequest {
                reason: "session target is not registered".into(),
            })
    }

    fn with_profile(target: TargetSlot, profile: ProfileSpec) -> Self {
        let mut config = Self::default();
        config.targets[target] = Some(Arc::new(crate::TargetRegistration::from_profile(
            target, profile,
        )));
        config
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            targets: PerTarget::default(),
            policy: SessionPolicy::DEFAULT,
            transport: crate::camera::config::TransportOptions::Custom,
            transport_config: None,
            serial_startup: crate::transport::SerialStartup::default(),
        }
    }
}

#[cfg(any(feature = "async", feature = "blocking"))]
impl SessionConfig {
    pub(crate) fn standard_connection_plan(
        &self,
    ) -> Result<crate::camera::config::SessionConnectionPlan> {
        use crate::camera::config::{SessionConnectionPlan, TransportKind, TransportOptions};
        use crate::transport::TransportConfig;
        let session_config = self.clone().validated()?;
        crate::camera::config::validate_selected_transport(&session_config, &self.transport)?;
        let (kind, address, configured_default) = match &self.transport {
            TransportOptions::Tcp { address } => (
                TransportKind::Tcp,
                address.as_str(),
                TransportConfig::for_tcp(),
            ),
            TransportOptions::Udp { address } => (
                TransportKind::Udp,
                address.as_str(),
                TransportConfig::for_udp(),
            ),
            TransportOptions::Serial { .. } => {
                return Err(Error::InvalidState {
                    reason: "serial transport requires its serial construction path".into(),
                })
            }
            TransportOptions::Custom => {
                return Err(Error::InvalidState {
                    reason: "custom transport requires Session::open".into(),
                })
            }
        };
        let port = |profile: &ProfileSpec| match kind {
            TransportKind::Tcp => profile.transports().tcp_port(),
            TransportKind::Udp => profile.transports().udp_port(),
            _ => None,
        };
        let mut ports = self.targets.iter().filter_map(|(_, registration)| {
            registration.as_deref().map(|target| port(target.profile()))
        });
        let first = ports.next().flatten();
        // A mixed registry can supply an explicit endpoint port. A host-only
        // endpoint needs a single default agreed by every registered profile.
        let default_port = if ports.all(|candidate| candidate == first) {
            first
        } else {
            None
        };
        let transport_config = self.transport_config.unwrap_or(configured_default);
        transport_config.validate()?;
        let endpoint = crate::transport::address::canonicalize_endpoint(address, default_port)?;
        Ok(SessionConnectionPlan {
            kind,
            endpoint,
            transport_config,
            session_config,
        })
    }
}

#[cfg(any(feature = "serde", feature = "schemars", feature = "ts-rs"))]
#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(
    feature = "schemars",
    derive(schemars::JsonSchema),
    schemars(deny_unknown_fields)
)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
struct SessionTargetData {
    camera_id: CameraId,
    profile: ProfileSelection,
    mount: Option<crate::MountPosition>,
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionConfigData {
    targets: Vec<SessionTargetData>,
    policy: SessionPolicy,
    transport: crate::camera::config::TransportOptions,
    transport_config: Option<crate::transport::TransportConfig>,
    serial_startup: crate::transport::SerialStartup,
}

#[cfg(feature = "serde")]
impl serde::Serialize for SessionConfig {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let targets = self
            .targets
            .iter()
            .filter_map(|(_, registration)| registration.as_deref())
            .map(|target| SessionTargetData {
                camera_id: target.target(),
                profile: crate::camera::config::selection_for_spec(target.profile().clone()),
                mount: target.mount(),
            })
            .collect();
        serde::Serialize::serialize(
            &SessionConfigData {
                targets,
                policy: self.policy,
                transport: self.transport.clone(),
                transport_config: self.transport_config,
                serial_startup: self.serial_startup,
            },
            serializer,
        )
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for SessionConfig {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let data = <SessionConfigData as serde::Deserialize>::deserialize(deserializer)?;
        let mut config = SessionConfig {
            targets: PerTarget::default(),
            policy: data.policy,
            transport: data.transport,
            transport_config: data.transport_config,
            serial_startup: data.serial_startup,
        };
        for target in data.targets {
            config = config
                .with_target(target.camera_id, target.profile)
                .map_err(serde::de::Error::custom)?;
            if let Some(mount) = target.mount {
                config = config
                    .with_mount(target.camera_id, mount)
                    .map_err(serde::de::Error::custom)?;
            }
        }
        Ok(config)
    }
}

/// A [`crate::config::SessionConfig`] whose session policy was validated against its
/// registered profiles ([`crate::config::SessionConfig::validated`]). Opening a session
/// consumes one, so the policy is validated exactly once per open.
#[cfg(any(feature = "async", feature = "blocking"))]
#[derive(Debug, Clone)]
pub(crate) struct ValidatedSessionConfig(SessionConfig, ValidatedTuning);

#[cfg(any(feature = "async", feature = "blocking"))]
impl ValidatedSessionConfig {
    pub(crate) fn target_count(&self) -> usize {
        self.targets.iter().count()
    }

    pub(crate) const fn validated_tuning(&self) -> ValidatedTuning {
        self.1
    }

    /// Validates every registered profile against a standard transport kind.
    ///
    /// This check intentionally does not inspect transport configuration.  A
    /// caller can therefore reject an unsupported standard profile before any
    /// adapter/owner setup or transport I/O boundary is entered.
    pub(crate) fn validate_for_transport(
        &self,
        identity: crate::transport::TransportIdentity<'_>,
    ) -> Result<()> {
        for profile in self.0.targets.iter().filter_map(|(_, registration)| {
            registration
                .as_deref()
                .map(crate::TargetRegistration::profile)
        }) {
            crate::runtime::owner::validate_profile_transport(profile, identity)?;
        }
        Ok(())
    }

    /// Validates the registry against an opened transport before any protocol
    /// I/O: its standard kind ([`Self::validate_for_transport`]) and, when a
    /// serial bus startup ran Address Set, the cameras it addressed.
    ///
    /// Address Set assigns `1..=n`; a registered target above `n` is not on
    /// the bus, so the open fails with [`Error::ConnectionFailed`] naming the
    /// port, whose `NotFound` source names the camera. Cameras beyond the
    /// registered ones are allowed.
    pub(crate) fn validate_opened_transport<T>(&self, transport: &T) -> Result<()>
    where
        T: crate::transport::HasTransportConfig + ?Sized,
    {
        let identity = transport.identity();
        self.validate_for_transport(identity)?;
        let Some(bus) = identity.addressed_bus() else {
            return Ok(());
        };
        let addressed = bus.cameras;
        match self.0.targets().find(|target| target.value() > addressed) {
            None => Ok(()),
            Some(missing) => Err(Error::connection_failed(
                bus.port.clone(),
                Arc::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "camera {} was not addressed by Address Set (chain reported {addressed})",
                        missing.value()
                    ),
                )),
            )),
        }
    }

    pub(crate) fn into_config(self) -> SessionConfig {
        self.0
    }
    /// The sole registered target, for a selector that names no target.
    /// `targeted` is the selector a caller with several targets uses instead;
    /// a validated configuration always has at least one target.
    pub(crate) fn sole_target_for(&self, targeted: &'static str) -> Result<CameraId> {
        self.0.sole_target().ok_or_else(|| Error::InvalidState {
            reason: format!("session has multiple registered targets; select one with {targeted}")
                .into(),
        })
    }
}

/// A validated configuration reads as the configuration it validated.
#[cfg(any(feature = "async", feature = "blocking"))]
impl std::ops::Deref for ValidatedSessionConfig {
    type Target = SessionConfig;

    fn deref(&self) -> &SessionConfig {
        &self.0
    }
}

#[cfg(all(test, feature = "blocking"))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn tuning_proof_requires_successful_validation_for_the_registry() {
        use super::*;
        let profile = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().unwrap();
        assert!(ValidatedTuning::validate(OperationalTuning::new(), []).is_err());
        assert!(ValidatedTuning::validate(
            OperationalTuning::new().with_ack_timeout(std::time::Duration::ZERO),
            [&profile],
        )
        .is_err());
        let tuning = OperationalTuning::new();
        assert_eq!(
            ValidatedTuning::validate(tuning, [&profile]).unwrap().get(),
            tuning
        );
    }

    use super::*;
    use crate::profiles::{GenericVisca, SonyFr7};

    #[test]
    fn sony_sequence_reset_is_opt_in_and_requires_the_sony_envelope() {
        let raw = SessionConfig::from_compile_time::<GenericVisca>()
            .unwrap()
            .with_sony_sequence_reset_on_connect(true);
        assert!(matches!(
            raw.validated(),
            Err(Error::InvalidRequest { reason: message })
                if message == "Sony sequence reset on connect requires Sony encapsulated profiles"
        ));

        let sony = SessionConfig::from_compile_time::<SonyFr7>()
            .unwrap()
            .with_sony_sequence_reset_on_connect(true);
        assert!(sony.sony_sequence_reset_on_connect());
        sony.validated()
            .unwrap()
            .validate_for_transport(crate::transport::TransportIdentity::Custom {
                semantics: crate::transport::SendSemantics::Stream,
                addressing: crate::transport::AddressingMode::Ip,
            })
            .unwrap();
    }

    #[test]
    fn strict_unconfirmed_poison_is_explicit_immutable_session_policy() {
        let default = SessionConfig::from_compile_time::<GenericVisca>().unwrap();
        assert!(!default.strict_unconfirmed_poison());

        let strict = default.with_strict_unconfirmed_poison(true);
        assert!(strict.strict_unconfirmed_poison());
        assert_eq!(strict.tuning(), OperationalTuning::new());
        strict.validated().unwrap();
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod config_value_tests {
    use super::*;
    use crate::profiles::ProfileId;

    #[test]
    fn consuming_target_updates_keep_individual_ids_sorted_and_reject_duplicates() -> Result<()> {
        let config = SessionConfig::default()
            .with_target(CameraId::CAMERA_2, ProfileId::PtzOpticsG2)?
            .with_target(CameraId::CAMERA_1, ProfileId::GenericVisca)?;
        assert_eq!(
            config.targets().collect::<Vec<_>>(),
            [CameraId::CAMERA_1, CameraId::CAMERA_2]
        );
        assert_eq!(config.sole_target(), None);
        assert!(config
            .clone()
            .with_target(CameraId::CAMERA_2, ProfileId::PtzOpticsG2)
            .is_err());
        assert!(config
            .with_target(CameraId::BROADCAST, ProfileId::PtzOpticsG2)
            .is_err());
        Ok(())
    }

    #[cfg(feature = "serde")]
    #[test]
    fn session_config_round_trips_transport_targets_mount_and_closed_policy() -> Result<()> {
        let config = SessionConfig::default()
            .with_target(CameraId::CAMERA_2, ProfileId::SonyFr7)?
            .with_mount(CameraId::CAMERA_2, crate::MountPosition::Ceiling)?
            .with_transport(crate::camera::config::TransportOptions::udp("camera.local"))
            .with_strict_unconfirmed_poison(true);
        let json = serde_json::to_value(&config).expect("session config serialization");
        assert_eq!(json["targets"][0]["profile"], "sony_fr7");
        let restored: SessionConfig =
            serde_json::from_value(json.clone()).expect("session config round trip");
        assert_eq!(restored.targets().collect::<Vec<_>>(), [CameraId::CAMERA_2]);
        assert_eq!(
            restored
                .registration(CameraId::CAMERA_2)
                .expect("target")
                .mount(),
            Some(crate::MountPosition::Ceiling)
        );
        assert_eq!(restored.transport(), config.transport());
        assert!(restored.strict_unconfirmed_poison());
        assert_eq!(
            serde_json::to_value(restored).expect("session reserialization"),
            json
        );
        let mut unknown = json.clone();
        unknown["unexpected"] = true.into();
        assert!(serde_json::from_value::<SessionConfig>(unknown).is_err());
        let mut duplicate = json.clone();
        let row = duplicate["targets"][0].clone();
        duplicate["targets"]
            .as_array_mut()
            .expect("targets array")
            .push(row);
        assert!(serde_json::from_value::<SessionConfig>(duplicate).is_err());
        let mut unknown_target = json;
        unknown_target["targets"][0]["unexpected"] = true.into();
        assert!(serde_json::from_value::<SessionConfig>(unknown_target).is_err());
        Ok(())
    }
}
