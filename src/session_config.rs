//! Shared profile/target configuration for the owner-backed session facades.
//!
//! [`SessionConfig`] is intentionally independent of either execution mode.
//! It is therefore available in builds that select only the blocking facade,
//! only the async facade, or neither facade.  A configuration may be edited
//! while it is being built; once passed to a session, the resulting registry
//! is immutable for the lifetime of that owner.

use std::{num::NonZeroUsize, sync::Arc};

use crate::{
    profile::{CompileTimeProfile, OperationalTuning, ProfileSpec},
    CameraId, Error, Result,
};

#[cfg(any(feature = "async", feature = "blocking"))]
use crate::camera::TransportKind;

/// Maximum number of individually addressable cameras in one session.
pub(crate) const MAX_REGISTERED_TARGETS: usize = 7;

/// Default number of requests a session may admit before fail-fast rejection.
#[allow(clippy::unwrap_used)]
pub const DEFAULT_ADMISSION_CAPACITY: NonZeroUsize = NonZeroUsize::new(64).unwrap();

/// The construction-time session policy both [`SessionConfig`] and
/// [`CameraConfig`](crate::camera::CameraConfig) carry, stored once and
/// validated once, when a session opens (#805).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SessionPolicy {
    pub(crate) tuning: OperationalTuning,
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
        pub const fn with_tuning(mut self, tuning: $crate::OperationalTuning) -> Self {
            self.policy.tuning = tuning;
            self
        }

        /// Returns the operational overrides this configuration carries.
        ///
        /// This is the *construction* value. A session opened from it may
        /// since have been reconfigured at runtime; read the session's own
        /// `tuning` for the value requests are actually prepared under.
        #[must_use]
        pub const fn tuning(&self) -> $crate::OperationalTuning {
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
pub struct SessionConfig {
    targets: [Option<Arc<ProfileSpec>>; MAX_REGISTERED_TARGETS],
    policy: SessionPolicy,
}

impl SessionConfig {
    /// Creates a single-target configuration for camera 1.
    #[must_use]
    pub fn new(profile: ProfileSpec) -> Self {
        // Camera 1 is always a valid individual target, so this constructor
        // is infallible.
        Self::with_profile(CameraId::CAMERA_1, profile)
    }

    /// Creates a single-target configuration for an explicit camera.
    pub fn for_target(target: CameraId, profile: ProfileSpec) -> Result<Self> {
        Self::validate_target(target)?;
        Ok(Self::with_profile(target, profile))
    }

    /// Creates a camera-1 configuration from a compile-time profile.
    pub fn from_compile_time<P>() -> Result<Self>
    where
        P: CompileTimeProfile,
    {
        Ok(Self::new(ProfileSpec::from_compile_time::<P>()?))
    }

    /// Adds one target/profile registration to this not-yet-open config.
    ///
    /// Registration is bounded to seven individual camera IDs.  Broadcast,
    /// duplicate IDs, and registrations beyond capacity are rejected before
    /// mutating the configuration. The session policy, including tuning, is
    /// checked against every registered profile when the session opens.
    pub fn register_target(&mut self, target: CameraId, profile: ProfileSpec) -> Result<&mut Self> {
        Self::validate_target(target)?;
        let slot = Self::slot(target);
        if self.targets[slot].is_some() {
            return Err(Error::InvalidRequest(
                "session target is already registered".into(),
            ));
        }
        if self.target_count() >= MAX_REGISTERED_TARGETS {
            return Err(Error::InvalidRequest(
                "session target registry is full".into(),
            ));
        }
        self.targets[slot] = Some(Arc::new(profile));
        Ok(self)
    }

    /// Builder-style equivalent of [`Self::register_target`].
    pub fn with_target(mut self, target: CameraId, profile: ProfileSpec) -> Result<Self> {
        self.register_target(target, profile)?;
        Ok(self)
    }

    /// Returns the registered profile for one individual target.
    #[must_use]
    pub fn profile(&self, target: CameraId) -> Option<&ProfileSpec> {
        Self::slot_checked(target).and_then(|slot| self.targets[slot].as_deref())
    }

    /// Returns the registered individual target IDs in camera-number order.
    ///
    /// Empty slots are represented by `None`; the fixed-size result preserves
    /// the registry's bounded shape without allocating.
    #[must_use]
    pub fn targets(&self) -> [Option<CameraId>; MAX_REGISTERED_TARGETS] {
        std::array::from_fn(|index| {
            self.targets[index]
                .as_ref()
                .and_then(|_| CameraId::new((index + 1) as u8).ok())
        })
    }

    /// Returns the sole registered target, or `None` for an empty or
    /// multi-target registry.
    #[must_use]
    pub fn sole_target(&self) -> Option<CameraId> {
        if self.target_count() != 1 {
            return None;
        }
        self.targets().into_iter().flatten().next()
    }

    /// Returns the number of registered individual targets.
    #[must_use]
    pub fn target_count(&self) -> usize {
        self.targets.iter().flatten().count()
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
    pub(crate) fn validate_tuning(&self, tuning: OperationalTuning) -> Result<()> {
        for profile in self.targets.iter().flatten() {
            profile.validate_tuning(tuning)?;
        }
        Ok(())
    }

    /// Validates this configuration's session policy against its registered
    /// profiles: the one check every session open runs, before any transport
    /// I/O.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn validated(self) -> Result<ValidatedSessionConfig> {
        if self.target_count() == 0 {
            return Err(Error::InvalidRequest(
                "session requires at least one registered target".into(),
            ));
        }
        // The async admission boundary is a flume channel. Its receive path
        // temporarily accounts for one additional pending sender, so the
        // representable maximum would overflow that channel's capacity
        // arithmetic. Reject it before adapter or owner construction; every
        // smaller non-zero value remains a valid caller-selected bound.
        if self.policy.admission_capacity.get() == usize::MAX {
            return Err(Error::InvalidRequest(
                "session admission capacity must be less than usize::MAX".into(),
            ));
        }
        if self.policy.sony_sequence_reset_on_connect
            && self.targets.iter().flatten().any(|profile| {
                profile.envelope() != crate::profile::ProfileEnvelope::SonyEncapsulated
            })
        {
            return Err(Error::InvalidRequest(
                "Sony sequence reset on connect requires Sony encapsulated profiles".into(),
            ));
        }
        self.validate_tuning(self.policy.tuning)?;
        Ok(ValidatedSessionConfig(self))
    }

    /// Borrow all registrations in the shape consumed by the owner adapter.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn profile_registry(&self) -> Vec<(CameraId, &ProfileSpec)> {
        self.targets
            .iter()
            .enumerate()
            .filter_map(|(index, profile)| {
                profile.as_deref().and_then(|profile| {
                    CameraId::new((index + 1) as u8)
                        .ok()
                        .map(|target| (target, profile))
                })
            })
            .collect()
    }

    /// The registered profile of `target`, shared with an owned camera view.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn registered_profile(&self, target: CameraId) -> Result<Arc<ProfileSpec>> {
        Self::slot_checked(target)
            .and_then(|slot| self.targets[slot].clone())
            .ok_or_else(|| Error::InvalidRequest("session target is not registered".into()))
    }

    /// [`Self::registered_profile`], only when its capabilities, coordinate
    /// codec, and envelope match `P`.
    ///
    /// This is the shared pure projection boundary used by both execution
    /// facades. It performs no owner, transport, or protocol work.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn registered_profile_for<P>(&self, target: CameraId) -> Result<Arc<ProfileSpec>>
    where
        P: CompileTimeProfile,
    {
        let profile = self.registered_profile(target)?;
        profile.ensure_compile_time::<P>()?;
        Ok(profile)
    }

    fn with_profile(target: CameraId, profile: ProfileSpec) -> Self {
        let mut config = Self::default();
        config.targets[Self::slot(target)] = Some(Arc::new(profile));
        config
    }

    fn validate_target(target: CameraId) -> Result<()> {
        if target.is_broadcast() {
            return Err(Error::InvalidRequest(
                "a session target must be an individual camera".into(),
            ));
        }
        Ok(())
    }

    fn slot(target: CameraId) -> usize {
        usize::from(target.id() - 1)
    }

    fn slot_checked(target: CameraId) -> Option<usize> {
        (!target.is_broadcast()).then(|| Self::slot(target))
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            targets: std::array::from_fn(|_| None),
            policy: SessionPolicy::DEFAULT,
        }
    }
}

/// A [`SessionConfig`] whose session policy was validated against its
/// registered profiles ([`SessionConfig::validated`]). Opening a session
/// consumes one, so the policy is validated exactly once per open.
#[cfg(any(feature = "async", feature = "blocking"))]
#[derive(Debug, Clone)]
pub(crate) struct ValidatedSessionConfig(SessionConfig);

#[cfg(any(feature = "async", feature = "blocking"))]
impl ValidatedSessionConfig {
    /// Validates every registered profile against a standard transport kind.
    ///
    /// This check intentionally does not inspect transport configuration.  A
    /// caller can therefore reject an unsupported standard profile before any
    /// adapter/owner setup or transport I/O boundary is entered.
    pub(crate) fn validate_for_transport(&self, kind: Option<TransportKind>) -> Result<()> {
        for profile in self.0.targets.iter().flatten() {
            crate::runtime::owner::validate_profile_transport(profile, kind)?;
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
        self.validate_for_transport(transport.standard_transport_kind())?;
        let Some(bus) = transport.addressed_bus() else {
            return Ok(());
        };
        let addressed = bus.cameras;
        match self
            .0
            .targets()
            .into_iter()
            .flatten()
            .find(|target| target.id() > addressed)
        {
            None => Ok(()),
            Some(missing) => Err(Error::connection_failed(
                bus.port.clone(),
                Arc::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "camera {} was not addressed by Address Set (chain reported {addressed})",
                        missing.id()
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
        self.0.sole_target().ok_or_else(|| {
            Error::InvalidState(
                format!("session has multiple registered targets; select one with {targeted}")
                    .into(),
            )
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::profiles::{GenericVisca, SonyFR7};

    #[test]
    fn sony_sequence_reset_is_opt_in_and_requires_the_sony_envelope() {
        let raw = SessionConfig::from_compile_time::<GenericVisca>()
            .unwrap()
            .with_sony_sequence_reset_on_connect(true);
        assert!(matches!(
            raw.validated(),
            Err(Error::InvalidRequest(message))
                if message == "Sony sequence reset on connect requires Sony encapsulated profiles"
        ));

        let sony = SessionConfig::from_compile_time::<SonyFR7>()
            .unwrap()
            .with_sony_sequence_reset_on_connect(true);
        assert!(sony.sony_sequence_reset_on_connect());
        sony.validated()
            .unwrap()
            .validate_for_transport(None)
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
