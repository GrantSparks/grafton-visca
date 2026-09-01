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
    tuning: OperationalTuning,
    admission_capacity: NonZeroUsize,
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
    /// mutating the configuration.
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
        profile.validate_tuning(self.tuning)?;
        self.targets[slot] = Some(Arc::new(profile));
        Ok(self)
    }

    /// Builder-style equivalent of [`Self::register_target`].
    pub fn with_target(mut self, target: CameraId, profile: ProfileSpec) -> Result<Self> {
        self.register_target(target, profile)?;
        Ok(self)
    }

    /// Sets immutable operational tuning for this session.
    ///
    /// Existing registrations are all checked before the tuning value is
    /// changed, so a failed update leaves the configuration untouched.
    pub fn with_tuning(mut self, tuning: OperationalTuning) -> Result<Self> {
        for profile in self.targets.iter().flatten() {
            profile.validate_tuning(tuning)?;
        }
        self.tuning = tuning;
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

    /// Returns the session tuning this configuration was built with.
    ///
    /// This is the *construction* value. A session opened from it may since
    /// have been reconfigured at runtime; read the session's own `tuning` for
    /// the value requests are actually prepared under.
    #[must_use]
    pub const fn tuning(&self) -> OperationalTuning {
        self.tuning
    }

    /// Returns the immutable request-admission capacity for this session.
    ///
    /// Admission is fail-fast: once this many requests are pending or active,
    /// a new submission returns [`Error::RuntimeQueueFull`](crate::Error::RuntimeQueueFull)
    /// rather than waiting for another request to finish. The capacity is
    /// fixed when the session opens and is independent of transport buffers
    /// and per-camera VISCA socket capacity.
    #[must_use]
    pub const fn admission_capacity(&self) -> NonZeroUsize {
        self.admission_capacity
    }

    /// Sets the immutable request-admission capacity for this session.
    ///
    /// The value is consumed when the session opens; changing a cloned
    /// configuration does not affect an already-open session.
    #[must_use]
    pub const fn with_admission_capacity(mut self, capacity: NonZeroUsize) -> Self {
        self.admission_capacity = capacity;
        self
    }

    /// Checks operational tuning against every registered profile.
    ///
    /// This is the same check [`Self::with_tuning`] performs, exposed for the
    /// runtime reconfiguration path (#631) so an update is rejected on exactly
    /// the grounds that would have rejected it at construction.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn validate_tuning(&self, tuning: OperationalTuning) -> Result<()> {
        for profile in self.targets.iter().flatten() {
            profile.validate_tuning(tuning)?;
        }
        Ok(())
    }

    /// Validate all registered profiles against a standard transport kind.
    ///
    /// This check intentionally does not inspect transport configuration.  A
    /// caller can therefore reject an unsupported standard profile before any
    /// adapter/owner setup or transport I/O boundary is entered.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn validate_for_transport(&self, kind: Option<TransportKind>) -> Result<()> {
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
        if self.admission_capacity.get() == usize::MAX {
            return Err(Error::InvalidRequest(
                "session admission capacity must be less than usize::MAX".into(),
            ));
        }
        for profile in self.targets.iter().flatten() {
            crate::runtime::owner::validate_profile_transport(profile, kind)?;
            profile.validate_tuning(self.tuning)?;
        }
        Ok(())
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

    /// Clone one profile reference for an owned camera view.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn profile_arc(&self, target: CameraId) -> Option<Arc<ProfileSpec>> {
        Self::slot_checked(target).and_then(|slot| self.targets[slot].clone())
    }

    /// Returns one registered profile only when it exactly matches `P`.
    ///
    /// This is the shared pure projection boundary used by both execution
    /// facades. It performs no owner, transport, or protocol work.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn profile_for_compile_time<P>(&self, target: CameraId) -> Result<&ProfileSpec>
    where
        P: CompileTimeProfile,
    {
        let profile = self
            .profile(target)
            .ok_or_else(|| Error::InvalidRequest("session target is not registered".into()))?;
        profile.ensure_compile_time::<P>()?;
        Ok(profile)
    }

    /// Owned counterpart of [`Self::profile_for_compile_time`] for async
    /// camera views.
    #[cfg(feature = "async")]
    pub(crate) fn profile_arc_for_compile_time<P>(
        &self,
        target: CameraId,
    ) -> Result<Arc<ProfileSpec>>
    where
        P: CompileTimeProfile,
    {
        self.profile_for_compile_time::<P>(target)?;
        self.profile_arc(target)
            .ok_or_else(|| Error::InvalidRequest("session target is not registered".into()))
    }

    fn with_profile(target: CameraId, profile: ProfileSpec) -> Self {
        let mut targets = std::array::from_fn(|_| None);
        targets[Self::slot(target)] = Some(Arc::new(profile));
        Self {
            targets,
            tuning: OperationalTuning::new(),
            admission_capacity: DEFAULT_ADMISSION_CAPACITY,
        }
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
            tuning: OperationalTuning::new(),
            admission_capacity: DEFAULT_ADMISSION_CAPACITY,
        }
    }
}
