//! Read-only, target-local projections of write-only camera state.
//!
//! A target owns one inline slot per [`StateKey`]. Exact applied effects store
//! [`KnownState`] values; a write that may have taken effect without confirmation
//! makes its key unknown. Cloned views share the same fixed table.

#[cfg(any(feature = "blocking", feature = "async"))]
use crate::{
    target::{PerTarget, TargetSlot},
    CameraId,
};
#[cfg(any(feature = "blocking", feature = "async"))]
use std::{
    ops::{Index, IndexMut},
    sync::{Arc, Mutex},
};

// One row declares the key, its value type and its public typed accessor.
// Enum values travel directly from the request to the cache; there is no
// intermediate integer encoding to maintain or decode.
macro_rules! state_key_ledger {
    ($(#[doc = $doc:literal] $key:ident($value:ty) => $getter:ident),+ $(,)?) => {
        /// Closed keys for write-only state maintained by a session owner.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[non_exhaustive]
        pub enum StateKey { $(#[doc = $doc] $key),+ }

        impl StateKey {
            /// Every supported write-only key in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$key),+];
            #[cfg(any(feature = "blocking", feature = "async"))]
            const COUNT: usize = Self::ALL.len();
            /// Returns every supported write-only key.
            #[must_use]
            pub const fn all() -> &'static [Self] { Self::ALL }
        }

        /// An exact applied write-only value, with its key implied by its variant.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[non_exhaustive]
        pub enum KnownState { $(#[doc = $doc] $key($value)),+ }

        impl KnownState {
            /// Returns the key whose value this variant establishes.
            #[must_use]
            pub const fn key(self) -> StateKey {
                match self { $(Self::$key(_) => StateKey::$key),+ }
            }
        }

        /// Type-level state keys used by the built-in semantic ledger.
        pub(crate) mod state_values {
            use super::*;

            $(
                pub(crate) struct $key;

                impl crate::request::builtin_contract::StateValue for $key {
                    const KEY: StateKey = StateKey::$key;
                    type Value = $value;

                    fn known(value: Self::Value) -> KnownState {
                        KnownState::$key(value)
                    }
                }
            )+
        }

        #[cfg(any(feature = "blocking", feature = "async"))]
        impl StateCache {
            $(#[doc = $doc]
            #[must_use]
            pub fn $getter(&self) -> Option<$value> {
                match self.value(StateKey::$key) {
                    StateEntry::Set(KnownState::$key(value)) => Some(value),
                    _ => None,
                }
            })+
        }
    };
}

state_key_ledger! {
    /// Returns the most recent corner-local limit update, including known clears.
    PanTiltLimits(PanTiltLimitUpdate) => pan_tilt_limits,
    /// Returns the last applied preset recall speed in raw VISCA units.
    PresetRecallSpeed(u8) => preset_recall_speed,
    /// Returns whether focus lock was last applied as enabled.
    FocusLockMode(bool) => focus_lock,
    /// Returns whether spotlight compensation was last applied as enabled.
    Spotlight(bool) => spotlight,
    /// Returns whether auto slow shutter was last applied as enabled.
    AutoSlowShutter(bool) => auto_slow_shutter,
    /// Returns the last applied ND-filter selection mode.
    NdFilterMode(crate::command::NdFilterMode) => nd_filter_mode,
    /// Returns whether automatic ND control was last applied as enabled.
    AutoNdFilter(bool) => auto_nd_filter,
    /// Returns whether image freeze was last applied as enabled.
    ImageFreeze(bool) => image_freeze,
    /// Returns whether digital zoom was last applied as enabled.
    DigitalZoomMode(bool) => digital_zoom,
    /// Returns whether multicast streaming was last applied as enabled.
    MulticastStreaming(bool) => multicast_streaming,
    /// Returns the last applied NDI streaming quality.
    NdiQuality(crate::types::NdiQuality) => ndi_quality,
    /// Returns whether tally brightness was last applied as high.
    TallyBrightness(bool) => tally_brightness_is_high,
    /// Returns the last applied pan/tilt variable-speed mode.
    PanTiltSpeedMode(crate::command::PanTiltSpeedMode) => variable_speed_mode,
    /// Returns the last applied steady tally mode; a flash leaves it unknown.
    TallyMode(bool) => tally_mode,
    /// Returns the last applied combined flip state; a single-axis change leaves it unknown.
    Flip(crate::command::ImageFlipMode) => flip_state,
}

/// A fixed table with exactly one inline slot per declared state key.
#[cfg(any(feature = "blocking", feature = "async"))]
#[derive(Debug)]
pub(crate) struct PerStateKey<T>([T; StateKey::COUNT]);

#[cfg(any(feature = "blocking", feature = "async"))]
impl<T: Default> Default for PerStateKey<T> {
    fn default() -> Self {
        Self(std::array::from_fn(|_| T::default()))
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl<T> Index<StateKey> for PerStateKey<T> {
    type Output = T;
    fn index(&self, key: StateKey) -> &Self::Output {
        &self.0[key as usize]
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl<T> IndexMut<StateKey> for PerStateKey<T> {
    fn index_mut(&mut self, key: StateKey) -> &mut Self::Output {
        &mut self.0[key as usize]
    }
}

/// The owner's current knowledge of one write-only key.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StateEntry {
    /// No exact applied value is known, or a later uncertain write invalidated it.
    #[default]
    Unknown,
    /// An exact applied typed value, including a corner-local limit clear.
    Set(KnownState),
}

/// Cheap, read-only view into the bounded state table for one target.
#[cfg(any(feature = "blocking", feature = "async"))]
#[derive(Clone)]
pub struct StateCache {
    registry: Arc<PerTarget<Mutex<crate::runtime::owner::TargetStateCache>>>,
    target: TargetSlot,
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl std::fmt::Debug for StateCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateCache")
            .field("target", &self.target.camera())
            .finish_non_exhaustive()
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl StateCache {
    pub(crate) fn from_registry(
        registry: Arc<PerTarget<Mutex<crate::runtime::owner::TargetStateCache>>>,
        target: TargetSlot,
    ) -> Self {
        Self { registry, target }
    }
    /// Returns the individual target addressed by this view.
    #[must_use]
    pub const fn target(&self) -> CameraId {
        self.target.camera()
    }
    /// Returns the current typed entry for a write-only key.
    #[must_use]
    pub fn value(&self, key: StateKey) -> StateEntry {
        match self.registry[self.target].lock() {
            Ok(cache) => cache.entry(key),
            Err(_) => StateEntry::Unknown,
        }
    }
}

/// The most recent pan/tilt movement-limit update recorded for one target.
///
/// The owner's state cache keeps one entry per [`StateKey`], so this describes
/// the last corner that was written, not the full limit rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanTiltLimitUpdate {
    corner: crate::command::PanTiltLimitCorner,
    position: Option<crate::camera::PanTiltUnits>,
}

impl PanTiltLimitUpdate {
    /// Records an exact applied limit position for one corner.
    #[must_use]
    pub const fn set(
        corner: crate::command::PanTiltLimitCorner,
        position: crate::camera::PanTiltUnits,
    ) -> Self {
        Self {
            corner,
            position: Some(position),
        }
    }

    /// Records an exact applied removal of one corner's limit.
    #[must_use]
    pub const fn clear(corner: crate::command::PanTiltLimitCorner) -> Self {
        Self {
            corner,
            position: None,
        }
    }

    /// Returns the corner this update applied to.
    #[must_use]
    pub const fn corner(self) -> crate::command::PanTiltLimitCorner {
        self.corner
    }

    /// Returns the raw limit position, or `None` when the corner was cleared.
    #[must_use]
    pub const fn position(self) -> Option<crate::camera::PanTiltUnits> {
        self.position
    }

    /// Returns whether this update cleared the corner.
    #[must_use]
    pub const fn is_cleared(self) -> bool {
        self.position.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::StateKey;

    /// The declared order of [`StateKey`], expressed as an exhaustive match.
    ///
    /// `#[non_exhaustive]` closes the enum to *downstream* crates only: inside
    /// the defining crate this match must still be total, so adding a variant
    /// to [`StateKey`] stops this module compiling until the variant is given
    /// a successor here. That is the compiler-enforced half of the ledger; the
    /// test below is the half that then requires the same variant to appear in
    /// [`StateKey::ALL`], which a bare `ALL.len() == 15` could never see.
    const fn successor(key: StateKey) -> Option<StateKey> {
        match key {
            StateKey::PanTiltLimits => Some(StateKey::PresetRecallSpeed),
            StateKey::PresetRecallSpeed => Some(StateKey::FocusLockMode),
            StateKey::FocusLockMode => Some(StateKey::Spotlight),
            StateKey::Spotlight => Some(StateKey::AutoSlowShutter),
            StateKey::AutoSlowShutter => Some(StateKey::NdFilterMode),
            StateKey::NdFilterMode => Some(StateKey::AutoNdFilter),
            StateKey::AutoNdFilter => Some(StateKey::ImageFreeze),
            StateKey::ImageFreeze => Some(StateKey::DigitalZoomMode),
            StateKey::DigitalZoomMode => Some(StateKey::MulticastStreaming),
            StateKey::MulticastStreaming => Some(StateKey::NdiQuality),
            StateKey::NdiQuality => Some(StateKey::TallyBrightness),
            StateKey::TallyBrightness => Some(StateKey::PanTiltSpeedMode),
            StateKey::PanTiltSpeedMode => Some(StateKey::TallyMode),
            StateKey::TallyMode => Some(StateKey::Flip),
            StateKey::Flip => None,
        }
    }

    #[test]
    fn state_key_inventory_is_exact_and_unique() {
        let mut declared = Vec::new();
        let mut key = Some(StateKey::PanTiltLimits);
        while let Some(current) = key {
            assert!(
                !declared.contains(&current),
                "the declaration walk revisits {current:?}"
            );
            declared.push(current);
            key = successor(current);
        }

        assert_eq!(
            StateKey::ALL,
            declared.as_slice(),
            "StateKey::ALL drifted from the declared variants"
        );
        let unique = StateKey::ALL
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), StateKey::ALL.len());
        assert_eq!(StateKey::all(), StateKey::ALL);
    }
}
