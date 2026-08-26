//! Closed operation-completion markers.

use std::{marker::PhantomData, time::Duration};

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
}

/// Opaque settlement lowering returned by [`Kind`].
///
/// The concrete plan remains owner-private. This wrapper keeps the public
/// completion contract closed without exposing operation metadata or engine
/// state in public handles.
#[doc(hidden)]
#[derive(Debug)]
#[cfg_attr(
    not(any(feature = "async", feature = "blocking", test)),
    allow(dead_code)
)]
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
#[cfg_attr(
    not(any(feature = "async", feature = "blocking", test)),
    allow(dead_code)
)]
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
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn default_budget(&self) -> Option<Duration> {
        match &self.plan {
            LoweredPlan::Targeted(plan) => plan.inner.default_budget(),
            LoweredPlan::AppliedOnly(_) => None,
        }
    }

    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub(crate) fn into_plan(self) -> Result<TargetedSettlementPlan> {
        match self.plan {
            LoweredPlan::Targeted(plan) => Ok(plan),
            LoweredPlan::AppliedOnly(_) => Err(crate::Error::InvalidState(
                "targeted settlement was lowered as applied-only".into(),
            )),
        }
    }
}

/// An operation with a physical target and a settlement plan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Targeted;

/// An operation whose terminal protocol application is its only typed wait.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct AppliedOnly;

#[doc(hidden)]
pub struct AppliedOnlySettlementPlan;

impl core::fmt::Debug for AppliedOnlySettlementPlan {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("AppliedOnlySettlementPlan")
    }
}

#[doc(hidden)]
pub struct TargetedSettlementPlan {
    pub(crate) inner: crate::prepared::SettlementPlan,
}

impl core::fmt::Debug for TargetedSettlementPlan {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.inner.fmt(formatter)
    }
}

#[cfg_attr(
    not(any(feature = "async", feature = "blocking", test)),
    allow(dead_code)
)]
impl TargetedSettlementPlan {
    pub(crate) fn new(inner: crate::prepared::SettlementPlan) -> Self {
        Self { inner }
    }

    pub(crate) fn into_inner(self) -> crate::prepared::SettlementPlan {
        self.inner
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

impl Kind for Targeted {
    fn lower_settlement(
        target: CameraId,
        profile: &ProfileSpec,
        tuning: OperationalTuning,
        axes: AffectedAxes,
        default_budget: Duration,
    ) -> Result<Settlement<Self>> {
        crate::prepared::lower_targeted_settlement(target, profile, tuning, axes, default_budget)
            .map(TargetedSettlementPlan::new)
            .map(LoweredPlan::Targeted)
            .map(Settlement::new)
    }
}

impl Kind for AppliedOnly {}

mod private {
    pub trait Sealed {}

    impl Sealed for super::Targeted {}
    impl Sealed for super::AppliedOnly {}
}
