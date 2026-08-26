//! Owner-backed, read-only state projections for one camera target.
//!
//! The registry is allocated once with the owner and shared by cheap camera
//! views. It has exactly seven usable target slots, at most the owner's fixed
//! 64 key projections per target, and a bounded inline value of at most four
//! `i64`s per key. Values change only when the owner applies an exact
//! `AppliedStateEffect`; preparing or merely writing a command does not change
//! this view. A view contains only one target index and an `Arc` clone, so it
//! adds no per-view state map or unbounded queue.

use std::sync::{Arc, Mutex};

use crate::CameraId;

const MAX_VALUES: usize = 4;

/// A target-local state key for a property without a reliable inquiry.
///
/// A value for one of these keys becomes known only after the corresponding
/// typed request reaches the owner's `Applied` state.  The set is deliberately
/// separate from the private command-semantics ledger: callers can inspect a
/// cache without naming an implementation module or semantic classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum StateKey {
    /// Pan/tilt limit corners.
    PanTiltLimits,
    /// Preset recall speed.
    PresetRecallSpeed,
    /// Focus-lock mode.
    FocusLockMode,
    /// Spotlight mode.
    Spotlight,
    /// Auto slow-shutter mode.
    AutoSlowShutter,
    /// Neutral-density filter selection mode.
    NdFilterMode,
    /// Automatic neutral-density filter mode.
    AutoNdFilter,
    /// Image-freeze mode.
    ImageFreeze,
    /// Digital-zoom enablement.
    DigitalZoomMode,
    /// Multicast streaming enablement.
    MulticastStreaming,
    /// NDI quality selection.
    NdiQuality,
    /// Tally brightness selection.
    TallyBrightness,
    /// Pan/tilt variable-speed mode.
    VariableSpeedMode,
    /// Vendor tally mode where the command has no corresponding inquiry.
    TallyMode,
}

impl StateKey {
    /// Every state key represented by the built-in applied-state ledger.
    pub const ALL: &[Self] = &[
        Self::PanTiltLimits,
        Self::PresetRecallSpeed,
        Self::FocusLockMode,
        Self::Spotlight,
        Self::AutoSlowShutter,
        Self::NdFilterMode,
        Self::AutoNdFilter,
        Self::ImageFreeze,
        Self::DigitalZoomMode,
        Self::MulticastStreaming,
        Self::NdiQuality,
        Self::TallyBrightness,
        Self::VariableSpeedMode,
        Self::TallyMode,
    ];

    /// Returns every currently supported state key in stable order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        Self::ALL
    }
}

/// Bounded inline values carried by one known state entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StateValue {
    values: [i64; MAX_VALUES],
    value_count: u8,
}

impl StateValue {
    /// Returns the number of scalar values in this entry.
    #[must_use]
    pub const fn len(self) -> usize {
        self.value_count as usize
    }

    /// Returns the inline values without allocating.
    #[must_use]
    pub fn as_slice(&self) -> &[i64] {
        &self.values[..self.value_count as usize]
    }

    /// Returns one scalar value, if present.
    #[must_use]
    pub fn get(self, index: usize) -> Option<i64> {
        self.as_slice().get(index).copied()
    }

    /// Returns whether this entry carries no discriminator values.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.value_count == 0
    }

    pub(crate) fn from_owner(value: crate::runtime::engine::AppliedStateValue) -> Self {
        Self {
            values: value.values,
            value_count: value.value_count,
        }
    }
}

/// Knowledge for one write-only key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StateEntry {
    /// No exact applied effect has established a value in this session.
    Unknown,
    /// The key has a known replacement value.
    Set(StateValue),
    /// The key is known to be absent; optional values retain a bounded clear
    /// discriminator such as a pan/tilt corner.
    Clear(StateValue),
}

/// A cheap, target-local, read-only view of the owner's state cache.
#[derive(Debug, Clone)]
pub struct StateCache {
    pub(crate) registry: Arc<[Mutex<crate::runtime::owner::TargetStateCache>; 9]>,
    target: CameraId,
}

impl StateCache {
    pub(crate) fn from_registry(
        registry: Arc<[Mutex<crate::runtime::owner::TargetStateCache>; 9]>,
        target: CameraId,
    ) -> Self {
        Self { registry, target }
    }

    /// Returns the fixed target represented by this view.
    #[must_use]
    pub const fn target(&self) -> CameraId {
        self.target
    }

    /// Reads one state key without allocating or mutating owner state.
    ///
    /// A poisoned internal lock is treated as unknown rather than exposing a
    /// mutable recovery path through the public API.
    #[must_use]
    pub fn value(&self, key: StateKey) -> StateEntry {
        let Some(slot) = self.registry.get(usize::from(self.target.id())) else {
            return StateEntry::Unknown;
        };
        let Ok(cache) = slot.lock() else {
            return StateEntry::Unknown;
        };
        cache.entry(key)
    }
}

#[cfg(test)]
mod tests {
    use super::StateKey;

    #[test]
    fn state_key_inventory_is_exact_and_unique() {
        let unique = StateKey::ALL
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(StateKey::ALL.len(), 14);
        assert_eq!(unique.len(), StateKey::ALL.len());
    }
}
