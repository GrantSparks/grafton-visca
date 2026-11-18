//! Closed operation-completion markers.

#[cfg(any(feature = "blocking", feature = "async"))]
use std::time::Duration;

#[cfg(any(feature = "blocking", feature = "async"))]
use crate::session_config::ValidatedTuning;
#[cfg(any(feature = "blocking", feature = "async"))]
use crate::{AffectedAxes, Result};

/// A closed operation completion kind. SettlementPlan lowering remains private
/// to the owner implementation.
pub trait Kind: private::Sealed + Send + Sync + 'static {}

#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) fn lower_settlement<K: Kind>(
    registration: &crate::TargetRegistration,
    tuning: ValidatedTuning,
    axes: AffectedAxes,
    default_budget: impl FnOnce() -> Duration,
) -> Result<SettlementPlan<K>> {
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

#[allow(private_interfaces)]
mod private {
    /// The plan association lives on the private seal, so it adds no public
    /// implementation obligation or supported API surface.
    pub trait Sealed {
        #[cfg(any(feature = "blocking", feature = "async"))]
        #[allow(private_bounds)] // Only the owner selects the admission reply.
        type Plan: crate::runtime::owner::SettlementAdmission;

        #[cfg(any(feature = "blocking", feature = "async"))]
        fn lower(
            registration: &crate::TargetRegistration,
            tuning: crate::session_config::ValidatedTuning,
            axes: crate::AffectedAxes,
            default_budget: impl FnOnce() -> std::time::Duration,
        ) -> crate::Result<super::SettlementPlan<Self>>
        where
            Self: super::Kind + Sized;
    }

    impl Sealed for super::Targeted {
        #[cfg(any(feature = "blocking", feature = "async"))]
        type Plan = super::lowering::TargetedSettlementPlan;

        #[cfg(any(feature = "blocking", feature = "async"))]
        fn lower(
            registration: &crate::TargetRegistration,
            tuning: crate::session_config::ValidatedTuning,
            axes: crate::AffectedAxes,
            default_budget: impl FnOnce() -> std::time::Duration,
        ) -> crate::Result<super::SettlementPlan<Self>> {
            super::lowering::targeted(registration, tuning, axes, default_budget)
        }
    }

    impl Sealed for super::AppliedOnly {
        #[cfg(any(feature = "blocking", feature = "async"))]
        type Plan = ();

        #[cfg(any(feature = "blocking", feature = "async"))]
        fn lower(
            registration: &crate::TargetRegistration,
            _tuning: crate::session_config::ValidatedTuning,
            axes: crate::AffectedAxes,
            _default_budget: impl FnOnce() -> std::time::Duration,
        ) -> crate::Result<super::SettlementPlan<Self>> {
            super::lowering::applied_only(registration, axes)
        }
    }
}

#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) use lowering::{SettlementPlan, TargetedSettlementPlan};

#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) type KindPlan<K> = <K as private::Sealed>::Plan;

#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) type Observation<K> =
    <KindPlan<K> as crate::runtime::owner::SettlementAdmission>::Observation;

/// SettlementPlan lowering into the owner-private plan; it exists only where an
/// owner does.
#[cfg(any(feature = "blocking", feature = "async"))]
mod lowering {
    use std::time::Duration;

    use super::{AppliedOnly, Kind, Targeted};
    #[cfg(any(feature = "blocking", feature = "async"))]
    use crate::session_config::ValidatedTuning;
    use crate::{AffectedAxes, Result};

    /// Owner-private settlement proof returned by the sealed lowering.
    ///
    /// Each kind stores exactly its own plan: targeted operations retain their
    /// settlement inputs and lazily cached polling templates; applied-only
    /// operations retain no plan.
    #[doc(hidden)]
    #[derive(Debug)]
    pub struct SettlementPlan<K>
    where
        K: Kind,
    {
        plan: <K as super::private::Sealed>::Plan,
    }

    impl<K: Kind> SettlementPlan<K> {
        fn new(plan: <K as super::private::Sealed>::Plan) -> Self {
            Self { plan }
        }
    }

    impl SettlementPlan<Targeted> {
        pub(crate) fn default_budget(&self) -> Duration {
            self.plan.inner.default_budget()
        }

        /// The immutable settlement selection and lazily cached query templates.
        /// Borrowed, so an abandoned wait can restart on the same handle
        /// without reconstructing its initialized plan (#777, #975).
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
        axes: AffectedAxes,
    ) -> Result<SettlementPlan<AppliedOnly>> {
        crate::prepared::lower_applied_only_settlement(registration, axes)?;
        Ok(SettlementPlan::new(()))
    }

    pub(super) fn targeted(
        registration: &crate::TargetRegistration,
        tuning: ValidatedTuning,
        axes: AffectedAxes,
        default_budget: impl FnOnce() -> Duration,
    ) -> Result<SettlementPlan<Targeted>> {
        crate::prepared::lower_targeted_settlement(registration, tuning, axes, default_budget())
            .map(|inner| SettlementPlan::new(TargetedSettlementPlan { inner }))
    }
}

pub use crate::operation_id::OperationId;
pub use crate::outcome::{CancellationOutcome, HaltOutcome, HaltReport, Settlement};

#[cfg(all(test, any(feature = "blocking", feature = "async")))]
#[test]
fn applied_only_settlement_storage_is_zero_sized() {
    assert_eq!(
        size_of::<(SettlementPlan<AppliedOnly>, Observation<AppliedOnly>)>(),
        0
    );
}
