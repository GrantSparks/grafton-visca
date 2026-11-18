//! Read-only, target-local projections of write-only camera state.
//!
//! A target owns one inline slot per [`crate::state_cache::StateKey`]. Exact applied effects store
//! [`crate::state_cache::KnownState`] values; a write that may have taken effect without confirmation
//! makes its key unknown. Cloned views share the same fixed table.

#[cfg(any(feature = "blocking", feature = "async"))]
use crate::target::{PerTarget, TargetSlot};
#[cfg(any(feature = "blocking", feature = "async"))]
use std::{
    ops::{Index, IndexMut},
    sync::{Arc, Mutex},
};

// One row declares the key, its value type and its noun's typed accessor.
// Enum values travel directly from the request to the cache; there is no
// intermediate integer encoding to maintain or decode.
macro_rules! state_key_ledger {
    ($(#[doc = $doc:literal] $key:ident($value:ty) => $view:ident::$getter:ident),+ $(,)?) => {
        /// Closed keys for write-only state maintained by a session owner.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(rename_all = "snake_case"))]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
        #[non_exhaustive]
        pub enum StateKey { $(#[doc = $doc] $key),+ }

        impl StateKey {
            #[cfg(all(test, any(feature = "blocking", feature = "async")))]
            pub(crate) const fn noun_name(self) -> &'static str {
                match self { $(Self::$key => <$view as CacheNoun>::NAME),+ }
            }

            #[cfg(all(test, any(feature = "blocking", feature = "async")))]
            pub(crate) const fn property_name(self) -> &'static str {
                match self { $(Self::$key => stringify!($getter)),+ }
            }

            #[cfg(all(test, any(feature = "blocking", feature = "async")))]
            pub(crate) const fn value_type_name(self) -> &'static str {
                match self { $(Self::$key => stringify!($value)),+ }
            }

            /// Every supported write-only key in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$key),+];
            #[cfg(any(feature = "blocking", feature = "async"))]
            const COUNT: usize = Self::ALL.len();
        }

        /// An exact applied write-only value, with its key implied by its variant.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(rename_all = "snake_case"))]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
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
                    #[cfg(all(test, any(feature = "blocking", feature = "async")))]
                    const KEY: StateKey = StateKey::$key;
                    type Value = $value;

                    fn known(value: Self::Value) -> KnownState {
                        KnownState::$key(value)
                    }
                }
            )+
        }

        $(
            #[cfg(any(feature = "blocking", feature = "async"))]
            impl $view {
                #[doc = $doc]
                #[must_use]
                pub fn $getter(&self) -> Option<$value> {
                    match self.cache.value(StateKey::$key) {
                        StateEntry::Set(KnownState::$key(value)) => Some(value),
                        _ => None,
                    }
                }
            }
        )+
    };
}

// This list names views only. Keys, values and getters are declared together
// in state_key_ledger, so a view cannot invent a second projection for a key.
#[cfg(all(test, any(feature = "blocking", feature = "async")))]
trait CacheNoun {
    const NAME: &'static str;
}

macro_rules! state_cache_nouns {
    ($(#[doc = $doc:literal] $view:ident => $noun:ident),+ $(,)?) => {
        $(
            #[cfg(any(feature = "blocking", feature = "async"))]
            #[doc = $doc]
            #[derive(Debug, Clone)]
            pub struct $view {
                cache: StateCache,
            }

            #[cfg(all(test, any(feature = "blocking", feature = "async")))]
            impl CacheNoun for $view {
                const NAME: &'static str = stringify!($noun);
            }

            #[cfg(any(feature = "blocking", feature = "async"))]
            impl StateCache {
                #[doc = $doc]
                #[must_use]
                pub fn $noun(&self) -> $view {
                    $view { cache: self.clone() }
                }
            }
        )+
    };
}

state_cache_nouns! {
    /// Read-only pan/tilt state for this target.
    PanTiltState => pan_tilt,
    /// Read-only preset state for this target.
    PresetsState => presets,
    /// Read-only focus state for this target.
    FocusState => focus,
    /// Read-only exposure state for this target.
    ExposureState => exposure,
    /// Read-only ND-filter state for this target.
    NdFilterState => nd_filter,
    /// Read-only image state for this target.
    ImageState => image,
    /// Read-only zoom state for this target.
    ZoomState => zoom,
    /// Read-only streaming state for this target.
    StreamingState => streaming,
    /// Read-only tally state for this target.
    TallyState => tally,
    /// Read-only synchronized-motion state for this target.
    MotionSyncState => motion_sync,
}

state_key_ledger! {
    /// Returns the most recent corner-local limit update, including known clears.
    PanTiltLimit(PanTiltLimitUpdate) => PanTiltState::limit,
    /// Returns the last applied preset recall speed.
    PresetsRecallSpeed(crate::types::PresetSpeed) => PresetsState::recall_speed,
    /// Returns whether focus lock was last applied as enabled.
    FocusLock(bool) => FocusState::lock,
    /// Returns whether spotlight compensation was last applied as enabled.
    ExposureSpotlight(bool) => ExposureState::spotlight,
    /// Returns whether auto slow shutter was last applied as enabled.
    ExposureAutoSlowShutter(bool) => ExposureState::auto_slow_shutter,
    /// Returns the last applied ND-filter selection mode.
    NdFilterMode(crate::types::NdFilterMode) => NdFilterState::mode,
    /// Returns whether automatic ND control was last applied as enabled.
    NdFilterAuto(bool) => NdFilterState::auto,
    /// Returns whether image freeze was last applied as enabled.
    ImageFreeze(bool) => ImageState::freeze,
    /// Returns whether digital zoom was last applied as enabled.
    ZoomDigitalZoom(bool) => ZoomState::digital_zoom,
    /// Returns whether multicast streaming was last applied as enabled.
    StreamingMulticast(bool) => StreamingState::multicast,
    /// Returns the last applied NDI streaming quality.
    StreamingNdiQuality(crate::types::NdiQuality) => StreamingState::ndi_quality,
    /// Returns the last applied tally brightness.
    TallyBrightness(crate::types::TallyBrightness) => TallyState::brightness,
    /// Returns the last applied pan/tilt speed mode.
    PanTiltSpeedMode(crate::types::PanTiltSpeedMode) => PanTiltState::speed_mode,
    /// Returns the last applied tally mode, including flashing.
    TallyMode(crate::types::TallyMode) => TallyState::mode,
    /// Returns the last applied synchronized-motion speed.
    MotionSyncSpeed(crate::types::MotionSyncSpeed) => MotionSyncState::speed,
    /// Returns the last applied wide-dynamic-range level.
    ExposureWideDynamicRange(crate::types::WideDynamicRangeLevel) => ExposureState::wide_dynamic_range,
    /// Returns whether synchronized preset movement was last applied as enabled.
    MotionSyncEnabled(bool) => MotionSyncState::enabled,
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
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
    pub const fn target(&self) -> crate::IndividualCameraId {
        self.target.individual_camera()
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
/// The owner's state cache keeps one entry per [`crate::state_cache::StateKey`], so this describes
/// the last corner that was written, not the full limit rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct PanTiltLimitUpdate {
    corner: crate::command::PanTiltLimitCorner,
    position: Option<crate::types::PanTiltPosition>,
}

impl PanTiltLimitUpdate {
    /// Records an exact applied limit position for one corner.
    #[must_use]
    pub const fn set(
        corner: crate::command::PanTiltLimitCorner,
        position: crate::types::PanTiltPosition,
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

    /// Returns the limit position in applied degrees, or `None` for a clear.
    #[must_use]
    pub const fn position(self) -> Option<crate::types::PanTiltPosition> {
        self.position
    }

    /// Returns whether this update cleared the corner.
    #[must_use]
    pub const fn is_cleared(self) -> bool {
        self.position.is_none()
    }
}

#[cfg(test)]
#[path = "state_cache/tests.rs"]
mod tests;

#[cfg(all(test, feature = "serde"))]
mod portable_data_tests {
    use super::*;
    use crate::types::{PanTiltLimitCorner, PanTiltPosition};

    #[test]
    fn cached_updates_distinguish_a_position_from_an_explicit_clear(
    ) -> Result<(), Box<dyn std::error::Error>> {
        for (update, position) in [
            (
                PanTiltLimitUpdate::set(
                    PanTiltLimitCorner::UpRight,
                    PanTiltPosition::from_degrees(-12.25, 34.5)?,
                ),
                serde_json::json!({"pan": -12.25, "tilt": 34.5}),
            ),
            (
                PanTiltLimitUpdate::clear(PanTiltLimitCorner::UpRight),
                serde_json::Value::Null,
            ),
        ] {
            let entry = StateEntry::Set(KnownState::PanTiltLimit(update));
            let json = serde_json::json!({
                "set": {"pan_tilt_limit": {"corner": "up_right", "position": position}}
            });
            assert_eq!(serde_json::to_value(entry)?, json);
            assert_eq!(serde_json::from_value::<StateEntry>(json)?, entry);
        }
        assert_eq!(
            serde_json::to_value(StateKey::PanTiltLimit)?,
            "pan_tilt_limit"
        );
        assert_eq!(serde_json::to_value(StateEntry::Unknown)?, "unknown");
        assert!(serde_json::from_value::<KnownState>(serde_json::json!({
            "presets_recall_speed": 0
        }))
        .is_err());
        assert!(
            serde_json::from_value::<PanTiltLimitUpdate>(serde_json::json!({
                "corner": "up_right", "position": null, "applied": true
            }))
            .is_err()
        );
        Ok(())
    }

    #[test]
    #[cfg(all(feature = "schemars", feature = "ts-rs"))]
    fn snapshot_schema_and_typescript_keep_typed_values() {
        let schema = schemars::schema_for!(PanTiltLimitUpdate);
        assert_eq!(schema.as_value()["additionalProperties"], false);
        assert!(schema.as_value()["$defs"]["PanTiltPosition"].is_object());
        let config = ts_rs::Config::default();
        assert_eq!(
            <PanTiltLimitUpdate as ts_rs::TS>::decl(&config),
            "type PanTiltLimitUpdate = { corner: PanTiltLimitCorner, position: PanTiltPosition | null, };"
        );
        let declaration = <KnownState as ts_rs::TS>::decl(&config);
        assert!(declaration.contains("\"tally_mode\": TallyMode"));
        assert!(declaration.contains("\"pan_tilt_limit\": PanTiltLimitUpdate"));
    }
}
