//! Closed operation-completion markers.

#[cfg(any(feature = "blocking", feature = "async"))]
use std::time::Duration;

#[cfg(any(feature = "blocking", feature = "async"))]
use crate::{AffectedAxes, OperationalTuning, Result};

/// A closed operation completion kind. Settlement lowering remains private
/// to the owner implementation.
pub trait Kind: private::Sealed + Send + Sync + 'static {}

#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) fn lower_settlement<K: Kind>(
    registration: &crate::TargetRegistration,
    tuning: OperationalTuning,
    axes: AffectedAxes,
    default_budget: Duration,
) -> Result<Settlement<K>> {
    <K as private::Sealed>::lower(registration, tuning, axes, default_budget)
}

/// An operation with a target state and a profile-selected protocol settlement
/// plan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Targeted;

/// An operation whose terminal protocol application is its only typed wait.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct AppliedOnly;

impl Kind for Targeted {}
impl Kind for AppliedOnly {}

mod private {
    /// The plan association lives on the private seal, so it adds no public
    /// implementation obligation or supported API surface.
    pub trait Sealed {
        #[cfg(any(feature = "blocking", feature = "async"))]
        type Plan: std::fmt::Debug + Send + Sync + Unpin;

        #[cfg(any(feature = "blocking", feature = "async"))]
        fn lower(
            registration: &crate::TargetRegistration,
            tuning: crate::OperationalTuning,
            axes: crate::AffectedAxes,
            default_budget: std::time::Duration,
        ) -> crate::Result<super::Settlement<Self>>
        where
            Self: super::Kind + Sized;
    }

    impl Sealed for super::Targeted {
        #[cfg(any(feature = "blocking", feature = "async"))]
        type Plan = super::lowering::TargetedSettlementPlan;

        #[cfg(any(feature = "blocking", feature = "async"))]
        fn lower(
            registration: &crate::TargetRegistration,
            tuning: crate::OperationalTuning,
            axes: crate::AffectedAxes,
            default_budget: std::time::Duration,
        ) -> crate::Result<super::Settlement<Self>> {
            super::lowering::targeted(registration, tuning, axes, default_budget)
        }
    }

    impl Sealed for super::AppliedOnly {
        #[cfg(any(feature = "blocking", feature = "async"))]
        type Plan = ();

        #[cfg(any(feature = "blocking", feature = "async"))]
        fn lower(
            registration: &crate::TargetRegistration,
            tuning: crate::OperationalTuning,
            axes: crate::AffectedAxes,
            default_budget: std::time::Duration,
        ) -> crate::Result<super::Settlement<Self>> {
            super::lowering::applied_only(registration, tuning, axes, default_budget)
        }
    }
}

#[cfg(any(feature = "blocking", feature = "async"))]
pub use lowering::Settlement;

/// Settlement lowering into the owner-private plan; it exists only where an
/// owner does.
#[cfg(any(feature = "blocking", feature = "async"))]
mod lowering {
    use std::time::Duration;

    use super::{AppliedOnly, Kind, Targeted};
    use crate::{AffectedAxes, OperationalTuning, Result};

    /// Opaque settlement lowering returned by [`Kind`].
    ///
    /// Each kind stores exactly its own plan: targeted operations retain their
    /// prepared settlement plan; applied-only operations retain no plan.
    #[doc(hidden)]
    #[derive(Debug)]
    pub struct Settlement<K>
    where
        K: Kind,
    {
        plan: <K as super::private::Sealed>::Plan,
    }

    impl<K: Kind> Settlement<K> {
        fn new(plan: <K as super::private::Sealed>::Plan) -> Self {
            Self { plan }
        }
    }

    impl Settlement<Targeted> {
        pub(crate) fn default_budget(&self) -> Duration {
            self.plan.inner.default_budget()
        }

        /// The prepared settlement plan. Borrowed, so an abandoned wait can
        /// restart on the same handle without reconstructing its plan (#777).
        pub(crate) const fn plan(&self) -> &crate::prepared::SettlementPlan {
            &self.plan.inner
        }
    }

    // Public only within this private module for the private sealed
    // association. The representation and its fields remain owner-private.
    #[doc(hidden)]
    #[derive(Debug)]
    pub struct TargetedSettlementPlan {
        inner: crate::prepared::SettlementPlan,
    }

    pub(super) fn applied_only(
        registration: &crate::TargetRegistration,
        tuning: OperationalTuning,
        axes: AffectedAxes,
        default_budget: Duration,
    ) -> Result<Settlement<AppliedOnly>> {
        crate::prepared::lower_applied_only_settlement(registration, tuning, axes, default_budget)?;
        Ok(Settlement::new(()))
    }

    pub(super) fn targeted(
        registration: &crate::TargetRegistration,
        tuning: OperationalTuning,
        axes: AffectedAxes,
        default_budget: Duration,
    ) -> Result<Settlement<Targeted>> {
        crate::prepared::lower_targeted_settlement(registration, tuning, axes, default_budget)
            .map(|inner| Settlement::new(TargetedSettlementPlan { inner }))
    }
}
