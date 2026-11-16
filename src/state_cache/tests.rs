use super::*;

/// This exhaustive walk is independent of the generated ALL slice. A new key
/// requires an explicit successor here before the inventory test can compile.
const fn successor(key: StateKey) -> Option<StateKey> {
    match key {
        StateKey::PanTiltLimit => Some(StateKey::PresetsRecallSpeed),
        StateKey::PresetsRecallSpeed => Some(StateKey::FocusLock),
        StateKey::FocusLock => Some(StateKey::ExposureSpotlight),
        StateKey::ExposureSpotlight => Some(StateKey::ExposureAutoSlowShutter),
        StateKey::ExposureAutoSlowShutter => Some(StateKey::NdFilterMode),
        StateKey::NdFilterMode => Some(StateKey::NdFilterAuto),
        StateKey::NdFilterAuto => Some(StateKey::ImageFreeze),
        StateKey::ImageFreeze => Some(StateKey::ZoomDigitalZoom),
        StateKey::ZoomDigitalZoom => Some(StateKey::StreamingMulticast),
        StateKey::StreamingMulticast => Some(StateKey::StreamingNdiQuality),
        StateKey::StreamingNdiQuality => Some(StateKey::TallyBrightness),
        StateKey::TallyBrightness => Some(StateKey::PanTiltSpeedMode),
        StateKey::PanTiltSpeedMode => Some(StateKey::TallyMode),
        StateKey::TallyMode => Some(StateKey::ImageFlipMode),
        StateKey::ImageFlipMode => Some(StateKey::MotionSyncSpeed),
        StateKey::MotionSyncSpeed => None,
    }
}

#[test]
fn state_key_inventory_is_exact_and_unique() {
    let mut declared = Vec::new();
    let mut key = Some(StateKey::PanTiltLimit);
    while let Some(current) = key {
        assert!(!declared.contains(&current), "walk revisits {current:?}");
        declared.push(current);
        key = successor(current);
    }
    assert_eq!(StateKey::ALL, declared.as_slice());
    let unique = StateKey::ALL
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), StateKey::ALL.len());
}

#[test]
fn typed_effects_preserve_the_full_setter_value_including_flashing() -> crate::Result<()> {
    use crate::{request::builtin_contract::StateValue, types};
    for mode in [
        types::TallyMode::Off,
        types::TallyMode::On,
        types::TallyMode::Flash,
    ] {
        let known = state_values::TallyMode::known(mode);
        assert_eq!(known, KnownState::TallyMode(mode));
        assert_eq!(known.key(), StateKey::TallyMode);
    }
    for brightness in [types::TallyBrightness::Low, types::TallyBrightness::High] {
        assert_eq!(
            state_values::TallyBrightness::known(brightness),
            KnownState::TallyBrightness(brightness)
        );
    }
    // The union-domain speed remains a checked value instead of a raw byte.
    let recall = types::PresetSpeed::new(50)?;
    assert_eq!(
        state_values::PresetsRecallSpeed::known(recall),
        KnownState::PresetsRecallSpeed(recall)
    );
    let speed = types::MotionSyncSpeed::FAST;
    let known = state_values::MotionSyncSpeed::known(speed);
    assert_eq!(known, KnownState::MotionSyncSpeed(speed));
    assert_eq!(known.key(), StateKey::MotionSyncSpeed);
    assert_eq!(
        state_values::ImageFlipMode::known(types::ImageFlipMode::Both),
        KnownState::ImageFlipMode(types::ImageFlipMode::Both)
    );
    Ok(())
}

#[cfg(any(feature = "blocking", feature = "async"))]
#[test]
fn noun_views_keep_their_target_and_outlive_the_temporary_cache() {
    use crate::types;
    let registry = Arc::new(PerTarget::<Mutex<crate::runtime::owner::TargetStateCache>>::default());
    let cache = StateCache::from_registry(Arc::clone(&registry), TargetSlot::CAMERA_1);
    assert_eq!(cache.target(), CameraId::CAMERA_1);

    let _: Option<PanTiltLimitUpdate> = cache.pan_tilt().limit();
    let _: Option<types::PanTiltSpeedMode> = cache.pan_tilt().speed_mode();
    let _: Option<types::PresetSpeed> = cache.presets().recall_speed();
    let _: Option<bool> = cache.focus().lock();
    let _: Option<bool> = cache.exposure().spotlight();
    let _: Option<bool> = cache.exposure().auto_slow_shutter();
    let _: Option<types::NdFilterMode> = cache.nd_filter().mode();
    let _: Option<bool> = cache.nd_filter().auto();
    let _: Option<bool> = cache.image().freeze();
    let _: Option<types::ImageFlipMode> = cache.image().flip_mode();
    let _: Option<bool> = cache.zoom().digital_zoom();
    let _: Option<bool> = cache.streaming().multicast();
    let _: Option<types::NdiQuality> = cache.streaming().ndi_quality();
    let _: Option<types::TallyBrightness> = cache.tally().brightness();
    let _: Option<types::TallyMode> = cache.tally().mode();
    let _: Option<types::MotionSyncSpeed> = cache.motion_sync().speed();

    let tally = StateCache::from_registry(Arc::clone(&registry), TargetSlot::CAMERA_2).tally();
    let cloned = tally.clone();
    drop(cache);
    assert_eq!(tally.cache.target(), CameraId::CAMERA_2);
    assert_eq!(cloned.cache.target(), CameraId::CAMERA_2);
    assert_eq!(tally.mode(), None);
    assert_eq!(cloned.brightness(), None);
}
