//! Closed operation-completion markers.

#[cfg(any(feature = "blocking", feature = "async"))]
use std::time::Duration;

#[cfg(any(feature = "blocking", feature = "async"))]
use crate::{AffectedAxes, CameraId, OperationalTuning, ProfileSpec, Result};

/// A closed operation completion kind.
///
/// Settlement lowering is an owner implementation detail. Its hidden default
/// keeps downstream attempts to implement this sealed trait focused on the
/// sealing diagnostic instead of requiring internal lowering items.
pub trait Kind: private::Sealed + Send + Sync + 'static {
    /// Lower this closed completion marker into the owner-private settlement
    /// representation. The default is the applied-only contract; targeted
    /// completion overrides it inside the crate. Keeping this item optional
    /// for implementors preserves the useful sealed-trait diagnostic without
    /// exposing lowering details as an external implementation obligation.
    #[cfg(any(feature = "blocking", feature = "async"))]
    #[doc(hidden)]
    fn lower_settlement(
        target: CameraId,
        profile: &ProfileSpec,
        tuning: OperationalTuning,
        axes: AffectedAxes,
        default_budget: Duration,
    ) -> Result<Settlement<Self>>
    where
        Self: Sized,
    {
        lowering::applied_only(target, profile, tuning, axes, default_budget)
    }
}

/// An operation with a target state and a profile-selected protocol settlement
/// plan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Targeted;

/// An operation whose terminal protocol application is its only typed wait.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct AppliedOnly;

impl Kind for Targeted {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn lower_settlement(
        target: CameraId,
        profile: &ProfileSpec,
        tuning: OperationalTuning,
        axes: AffectedAxes,
        default_budget: Duration,
    ) -> Result<Settlement<Self>> {
        lowering::targeted(target, profile, tuning, axes, default_budget)
    }
}

impl Kind for AppliedOnly {}

mod private {
    pub trait Sealed {}

    impl Sealed for super::Targeted {}
    impl Sealed for super::AppliedOnly {}
}

#[cfg(any(feature = "blocking", feature = "async"))]
pub use lowering::Settlement;

/// Settlement lowering into the owner-private plan; it exists only where an
/// owner does.
#[cfg(any(feature = "blocking", feature = "async"))]
mod lowering {
    use std::{marker::PhantomData, time::Duration};

    use super::{Kind, Targeted};
    use crate::{AffectedAxes, CameraId, OperationalTuning, ProfileSpec, Result};

    /// Opaque settlement lowering returned by [`Kind`].
    ///
    /// The concrete plan remains owner-private. This wrapper keeps the public
    /// completion contract closed without exposing operation metadata or engine
    /// state in public handles.
    #[doc(hidden)]
    #[derive(Debug)]
    pub struct Settlement<K>
    where
        K: Kind,
    {
        plan: LoweredPlan,
        marker: PhantomData<fn() -> K>,
    }

    // Boxing this variant would add an allocation to every targeted operation's
    // settlement plan. The owner deliberately keeps the plan inert and inline.
    #[allow(clippy::large_enum_variant)]
    #[derive(Debug)]
    enum LoweredPlan {
        Targeted(TargetedSettlementPlan),
        AppliedOnly(AppliedOnlySettlementPlan),
    }

    impl<K> Settlement<K>
    where
        K: Kind,
    {
        fn new(plan: LoweredPlan) -> Self {
            Self {
                plan,
                marker: PhantomData,
            }
        }
    }

    impl Settlement<Targeted> {
        pub(crate) fn default_budget(&self) -> Option<Duration> {
            match &self.plan {
                LoweredPlan::Targeted(plan) => plan.inner.default_budget(),
                LoweredPlan::AppliedOnly(_) => None,
            }
        }

        /// The prepared settlement plan. Borrowed, so a settlement wait that is
        /// abandoned or times out can be restarted on the same handle (#777).
        pub(crate) fn plan(&self) -> Result<&crate::prepared::SettlementPlan> {
            match &self.plan {
                LoweredPlan::Targeted(plan) => Ok(&plan.inner),
                LoweredPlan::AppliedOnly(_) => Err(crate::Error::InvalidState(
                    "targeted settlement was lowered as applied-only".into(),
                )),
            }
        }
    }

    pub(crate) struct AppliedOnlySettlementPlan;

    impl core::fmt::Debug for AppliedOnlySettlementPlan {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter.write_str("AppliedOnlySettlementPlan")
        }
    }

    pub(crate) struct TargetedSettlementPlan {
        pub(crate) inner: crate::prepared::SettlementPlan,
    }

    impl core::fmt::Debug for TargetedSettlementPlan {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            self.inner.fmt(formatter)
        }
    }

    impl TargetedSettlementPlan {
        pub(crate) fn new(inner: crate::prepared::SettlementPlan) -> Self {
            Self { inner }
        }
    }

    impl Default for AppliedOnlySettlementPlan {
        fn default() -> Self {
            Self
        }
    }

    impl Clone for AppliedOnlySettlementPlan {
        fn clone(&self) -> Self {
            *self
        }
    }

    impl Copy for AppliedOnlySettlementPlan {}

    impl PartialEq for AppliedOnlySettlementPlan {
        fn eq(&self, _other: &Self) -> bool {
            true
        }
    }

    impl Eq for AppliedOnlySettlementPlan {}

    impl core::hash::Hash for AppliedOnlySettlementPlan {
        fn hash<H: core::hash::Hasher>(&self, _state: &mut H) {}
    }

    /// Lowers the applied-only contract after the shared profile checks.
    pub(super) fn applied_only<K: Kind>(
        target: CameraId,
        profile: &ProfileSpec,
        tuning: OperationalTuning,
        axes: AffectedAxes,
        default_budget: Duration,
    ) -> Result<Settlement<K>> {
        crate::prepared::lower_applied_only_settlement(
            target,
            profile,
            tuning,
            axes,
            default_budget,
        )?;
        Ok(Settlement::new(LoweredPlan::AppliedOnly(
            AppliedOnlySettlementPlan,
        )))
    }

    /// Lowers a targeted operation into its profile-selected settlement plan.
    pub(super) fn targeted(
        target: CameraId,
        profile: &ProfileSpec,
        tuning: OperationalTuning,
        axes: AffectedAxes,
        default_budget: Duration,
    ) -> Result<Settlement<Targeted>> {
        crate::prepared::lower_targeted_settlement(target, profile, tuning, axes, default_budget)
            .map(TargetedSettlementPlan::new)
            .map(LoweredPlan::Targeted)
            .map(Settlement::new)
    }
}
