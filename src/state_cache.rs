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
    /// Combined horizontal/vertical image-flip state.
    Flip,
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
        Self::Flip,
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

    /// Reads one key that stores a single boolean discriminator as `1`/`0`.
    fn boolean(&self, key: StateKey) -> Option<bool> {
        match self.value(key) {
            StateEntry::Set(value) => value.get(0).map(|flag| flag != 0),
            StateEntry::Unknown | StateEntry::Clear(_) => None,
        }
    }

    /// Reads one key that stores a single scalar discriminator.
    fn scalar(&self, key: StateKey) -> Option<i64> {
        match self.value(key) {
            StateEntry::Set(value) => value.get(0),
            StateEntry::Unknown | StateEntry::Clear(_) => None,
        }
    }

    /// Returns the last known auto slow-shutter setting.
    ///
    /// [`StateKey::AutoSlowShutter`] stores `1` when the last applied
    /// `exposure().auto_slow_shutter_on()` succeeded and `0` for the `_off`
    /// twin. `None` means no such command has reached `Applied` on this target
    /// in this session.
    #[must_use]
    pub fn auto_slow_shutter(&self) -> Option<bool> {
        self.boolean(StateKey::AutoSlowShutter)
    }

    /// Returns the last known spotlight-compensation setting.
    ///
    /// [`StateKey::Spotlight`] stores `1` for `spotlight_on` and `0` for
    /// `spotlight_off`.
    #[must_use]
    pub fn spotlight(&self) -> Option<bool> {
        self.boolean(StateKey::Spotlight)
    }

    /// Returns the last known focus-lock setting.
    ///
    /// [`StateKey::FocusLockMode`] stores `1` for a locked focus and `0` for an
    /// unlocked one.
    #[must_use]
    pub fn focus_lock(&self) -> Option<bool> {
        self.boolean(StateKey::FocusLockMode)
    }

    /// Returns the last known image-freeze setting.
    ///
    /// [`StateKey::ImageFreeze`] stores `1` for a frozen frame and `0` for live
    /// output.
    #[must_use]
    pub fn image_freeze(&self) -> Option<bool> {
        self.boolean(StateKey::ImageFreeze)
    }

    /// Returns the last known digital-zoom enablement.
    ///
    /// [`StateKey::DigitalZoomMode`] stores `1` when digital zoom was enabled
    /// and `0` when it was disabled.
    #[must_use]
    pub fn digital_zoom(&self) -> Option<bool> {
        self.boolean(StateKey::DigitalZoomMode)
    }

    /// Returns the last known automatic ND-filter setting.
    ///
    /// [`StateKey::AutoNdFilter`] stores `1` for `auto_on` and `0` for
    /// `auto_off`.
    #[must_use]
    pub fn auto_nd_filter(&self) -> Option<bool> {
        self.boolean(StateKey::AutoNdFilter)
    }

    /// Returns the last known multicast-streaming enablement.
    ///
    /// [`StateKey::MulticastStreaming`] stores `1` for `multicast_on` and `0`
    /// for `multicast_off`.
    #[must_use]
    pub fn multicast_streaming(&self) -> Option<bool> {
        self.boolean(StateKey::MulticastStreaming)
    }

    /// Returns the last known vendor tally mode.
    ///
    /// [`StateKey::TallyMode`] stores `1` for tally on and `0` for tally off.
    /// A tally *flash* command invalidates the key rather than setting it,
    /// because the resulting steady-state mode is not determined by the
    /// request, so this reports `None` after a flash.
    #[must_use]
    pub fn tally_mode(&self) -> Option<bool> {
        self.boolean(StateKey::TallyMode)
    }

    /// Returns the last known preset recall speed in raw VISCA units.
    ///
    /// [`StateKey::PresetRecallSpeed`] stores the profile-validated speed
    /// exactly as it was sent.
    #[must_use]
    pub fn preset_recall_speed(&self) -> Option<u8> {
        u8::try_from(self.scalar(StateKey::PresetRecallSpeed)?).ok()
    }

    /// Returns the last known ND-filter selection mode.
    ///
    /// [`StateKey::NdFilterMode`] stores `0` for
    /// [`NdFilterMode::Preset`](crate::command::NdFilterMode::Preset) and `1`
    /// for [`NdFilterMode::Variable`](crate::command::NdFilterMode::Variable).
    #[must_use]
    pub fn nd_filter_mode(&self) -> Option<crate::command::NdFilterMode> {
        match self.scalar(StateKey::NdFilterMode)? {
            0 => Some(crate::command::NdFilterMode::Preset),
            1 => Some(crate::command::NdFilterMode::Variable),
            _ => None,
        }
    }

    /// Returns the last known NDI streaming quality.
    ///
    /// [`StateKey::NdiQuality`] stores `1` for `High`, `2` for `Medium`, `3`
    /// for `Low`, and `4` for `Off`.
    #[must_use]
    pub fn ndi_quality(&self) -> Option<crate::types::NdiQuality> {
        match self.scalar(StateKey::NdiQuality)? {
            1 => Some(crate::types::NdiQuality::High),
            2 => Some(crate::types::NdiQuality::Medium),
            3 => Some(crate::types::NdiQuality::Low),
            4 => Some(crate::types::NdiQuality::Off),
            _ => None,
        }
    }

    /// Returns the last known pan/tilt variable-speed mode.
    ///
    /// [`StateKey::VariableSpeedMode`] stores `1` for
    /// [`VariableSpeedMode::Standard24`](crate::command::VariableSpeedMode::Standard24)
    /// and `2` for
    /// [`VariableSpeedMode::Fine50`](crate::command::VariableSpeedMode::Fine50).
    #[must_use]
    pub fn variable_speed_mode(&self) -> Option<crate::command::VariableSpeedMode> {
        match self.scalar(StateKey::VariableSpeedMode)? {
            1 => Some(crate::command::VariableSpeedMode::Standard24),
            2 => Some(crate::command::VariableSpeedMode::Fine50),
            _ => None,
        }
    }

    /// Returns whether the tally light was last set to its high brightness.
    ///
    /// [`StateKey::TallyBrightness`] stores `0` for the low setting and `1`
    /// for the high one.
    #[must_use]
    pub fn tally_brightness_is_high(&self) -> Option<bool> {
        match self.scalar(StateKey::TallyBrightness)? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }

    /// Returns the last known combined image-flip state.
    ///
    /// [`StateKey::Flip`] stores the horizontal axis in slot 0 and the vertical
    /// axis in slot 1, both as `1`/`0`. Only the combined-flip opcode
    /// (`image().set_flip_mode()` and `image().set_flip_both()`) establishes
    /// both axes at once, so only those commands set the key. The single-axis
    /// opcodes (`enable_flip`, `disable_flip`, `enable_horizontal_flip`,
    /// `disable_horizontal_flip`) move one axis without saying anything about
    /// the other and therefore invalidate the key, which reads back as `None`.
    #[must_use]
    pub fn flip_state(&self) -> Option<crate::command::FlipState> {
        let StateEntry::Set(value) = self.value(StateKey::Flip) else {
            return None;
        };
        Some(crate::command::FlipState {
            horizontal: value.get(0)? != 0,
            vertical: value.get(1)? != 0,
        })
    }

    /// Returns the last recorded pan/tilt movement-limit update.
    ///
    /// [`StateKey::PanTiltLimits`] stores the corner discriminator in slot 0
    /// (`0x00` for [`PanTiltLimitCorner::DownLeft`], `0x03` for
    /// [`PanTiltLimitCorner::UpRight`]). A `limit_set` additionally stores the
    /// converted raw pan and tilt in slots 1 and 2; a `limit_clear` stores only
    /// the corner.
    ///
    /// The owner keeps exactly one entry per key, so this reports the most
    /// recent corner update rather than a merged two-corner rectangle: writing
    /// the other corner replaces this value.
    ///
    /// [`PanTiltLimitCorner::DownLeft`]: crate::command::PanTiltLimitCorner::DownLeft
    /// [`PanTiltLimitCorner::UpRight`]: crate::command::PanTiltLimitCorner::UpRight
    #[must_use]
    pub fn pan_tilt_limits(&self) -> Option<PanTiltLimitUpdate> {
        let (value, cleared) = match self.value(StateKey::PanTiltLimits) {
            StateEntry::Set(value) => (value, false),
            StateEntry::Clear(value) => (value, true),
            StateEntry::Unknown => return None,
        };
        let corner = match value.get(0)? {
            0x00 => crate::command::PanTiltLimitCorner::DownLeft,
            0x03 => crate::command::PanTiltLimitCorner::UpRight,
            _ => return None,
        };
        if cleared {
            return Some(PanTiltLimitUpdate {
                corner,
                position: None,
            });
        }
        let pan = i16::try_from(value.get(1)?).ok()?;
        let tilt = i16::try_from(value.get(2)?).ok()?;
        Some(PanTiltLimitUpdate {
            corner,
            position: Some(crate::camera::PanTiltPosition::new(pan, tilt)),
        })
    }
}

/// The most recent pan/tilt movement-limit update recorded for one target.
///
/// The owner's state cache keeps one entry per [`StateKey`], so this describes
/// the last corner that was written, not the full limit rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanTiltLimitUpdate {
    corner: crate::command::PanTiltLimitCorner,
    position: Option<crate::camera::PanTiltPosition>,
}

impl PanTiltLimitUpdate {
    /// Returns the corner this update applied to.
    #[must_use]
    pub const fn corner(self) -> crate::command::PanTiltLimitCorner {
        self.corner
    }

    /// Returns the raw limit position, or `None` when the corner was cleared.
    #[must_use]
    pub const fn position(self) -> Option<crate::camera::PanTiltPosition> {
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
            StateKey::TallyBrightness => Some(StateKey::VariableSpeedMode),
            StateKey::VariableSpeedMode => Some(StateKey::TallyMode),
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
