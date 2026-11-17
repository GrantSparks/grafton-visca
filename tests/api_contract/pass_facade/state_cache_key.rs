// The canonical root cache is an owner-backed target view, not a standalone
// mutable value, so it exists only with a facade. Construction is exercised
// through Session and Camera facade tests; this fixture keeps the root type
// public.
use grafton_visca::{
    state_cache::KnownState, state_cache::StateCache, state_cache::StateEntry,
    state_cache::StateKey,
};

fn inspect_cache(cache: &StateCache) {
    let _: StateEntry = cache.value(StateKey::ImageFreeze);
    let known = KnownState::ImageFreeze(true);
    let _: StateKey = known.key();
    let _: StateEntry = StateEntry::Set(known);
    let _: &'static [StateKey] = StateKey::ALL;
    let _: Option<bool> = cache.focus().lock();
    let _: Option<bool> = cache.exposure().spotlight();
    let _: Option<bool> = cache.exposure().auto_slow_shutter();
    let _: Option<bool> = cache.image().freeze();
    let _: Option<bool> = cache.zoom().digital_zoom();
    let _: Option<bool> = cache.streaming().multicast();
    let _: Option<grafton_visca::types::PresetSpeed> = cache.presets().recall_speed();
    let _: Option<grafton_visca::types::NdFilterMode> = cache.nd_filter().mode();
    let _: Option<grafton_visca::types::TallyMode> = cache.tally().mode();
    let _: Option<grafton_visca::types::TallyBrightness> = cache.tally().brightness();
    let _: Option<grafton_visca::types::NdiQuality> = cache.streaming().ndi_quality();
    let _: Option<grafton_visca::types::PanTiltSpeedMode> = cache.pan_tilt().speed_mode();
    let _: Option<grafton_visca::types::ImageFlipMode> = cache.image().flip_mode();
    let _: Option<grafton_visca::types::MotionSyncSpeed> = cache.motion_sync().speed();
    let _: Option<grafton_visca::types::WideDynamicRangeLevel> = cache.exposure().wide_dynamic_range();
    let _: Option<bool> = cache.motion_sync().enabled();
    let _: Option<grafton_visca::state_cache::PanTiltLimitUpdate> = cache.pan_tilt().limit();
}

fn main() {
    let _ = inspect_cache;
}
