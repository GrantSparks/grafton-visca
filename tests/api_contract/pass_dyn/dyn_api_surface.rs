#![allow(dead_code)]

use std::marker::PhantomData;

use grafton_visca::{
    camera::{IdleWait, MotionQuery},
    capabilities::Capabilities,
    command::{
        AntiFlickerMode, AutoFocusSensitivity, AutoWhiteBalanceSensitivity, BlackWhiteMode,
        FocusLock, FocusMode, FocusRange, FocusZone, ImageFlipMode, MenuDirection, MotionSyncMode,
        MotionSyncPreset, NdFilterMode, PanTiltDirection, PanTiltLimitCorner, PictureEffectMode,
        PresetNumber, ResolutionMode, SharpnessMode, TallyStatusState, VariableSpeedMode,
        WhiteBalanceMode,
    },
    dynapi::{
        DynAppliedOperation, DynAppliedRequest, DynFuture, DynMotion, DynPower, DynSessionCamera,
        DynSessionCameraControl, DynSessionCameraNouns, DynTargetedOperation, DynTargetedRequest,
    },
    state_cache::StateCache,
    types::{
        BlueChannel, BlueTuning, BrightnessLevel, ColorTemp, ContrastLevel, DefogLevel,
        DynamicRangeLevel, ExposureCompensationLevel, ExposureCompensationPosition, FocusPosition,
        GainLevel, GainLimit, IrisLevel, LuminanceLevel, MotionSyncSpeed, NdFilterPreset,
        NoiseReduction2DLevel, NoiseReduction3DLevel, NoiseReductionLevel, PanSpeed, RedChannel,
        RedTuning, SaturationLevel, SharpnessLevel, ShutterSpeed, SpeedLevel, TiltSpeed,
        ZoomPosition, ZoomSpeed,
    },
    units::{Degrees, UnitInterval},
    AffectedAxes, Error, ZoomDomain,
};

fn camera_surface(camera: &dyn DynSessionCameraControl) {
    let _: &Capabilities = camera.capabilities();
    let cache = camera.state_cache();
    let _: &StateCache = &cache;
}

fn motion_surface(motion: &dyn DynMotion) {
    let _: DynFuture<'_, Result<(), Error>> = motion.stop_all_motion();
    let _: DynFuture<'_, Result<bool, Error>> = motion.is_moving();
    let _: DynFuture<'_, Result<bool, Error>> =
        motion.is_moving_axes(MotionQuery::new(AffectedAxes::ZOOM));
    let _: DynFuture<'_, Result<(), Error>> = motion.wait_until_idle(IdleWait::new(
        AffectedAxes::ZOOM,
        std::time::Duration::from_secs(1),
    ));
}

fn noun_surfaces(camera: &dyn DynSessionCameraNouns) {
    motion_surface(camera.motion());
    let power = camera.power();
    let _: DynFuture<'_, Result<bool, Error>> = power.state();
    let _: DynFuture<'_, Result<(), Error>> = power.on();
    let _: DynFuture<'_, Result<(), Error>> = power.off();

    let zoom = camera.zoom();
    let _: DynFuture<'_, Result<ZoomPosition, Error>> = zoom.position();
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> = zoom.tele();
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> = zoom.wide();
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> = zoom.stop();
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> =
        zoom.tele_variable(ZoomSpeed::new(1).unwrap());
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> =
        zoom.wide_variable(ZoomSpeed::new(1).unwrap());
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> =
        zoom.set_position(ZoomPosition::MIN);
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> =
        zoom.set_normalized(UnitInterval::ZERO);
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> =
        zoom.set_normalized_in_domain(UnitInterval::ONE, ZoomDomain::Optical);
    let _: DynFuture<'_, Result<(), Error>> = zoom.set_digital_zoom(false);

    let system = camera.system();
    let _: DynFuture<'_, Result<grafton_visca::command::VersionInfo, Error>> = system.version();
    let _: DynFuture<'_, Result<(), Error>> = system.save_settings();

    let pan_tilt = camera.pan_tilt();
    let _: DynFuture<'_, Result<grafton_visca::camera::PanTiltPosition, Error>> =
        pan_tilt.position();
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> = pan_tilt.home();
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> = pan_tilt.reset();
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> = pan_tilt.move_direction(
        PanTiltDirection::Stop,
        PanSpeed::new(1).unwrap(),
        TiltSpeed::new(1).unwrap(),
    );
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> =
        pan_tilt.up(PanSpeed::new(1).unwrap(), TiltSpeed::new(1).unwrap());
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> =
        pan_tilt.down(PanSpeed::new(1).unwrap(), TiltSpeed::new(1).unwrap());
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> =
        pan_tilt.left(PanSpeed::new(1).unwrap(), TiltSpeed::new(1).unwrap());
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> =
        pan_tilt.right(PanSpeed::new(1).unwrap(), TiltSpeed::new(1).unwrap());
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> = pan_tilt.stop();
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> =
        pan_tilt.absolute(Degrees::new(0.0), Degrees::new(0.0), SpeedLevel::Medium);
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> =
        pan_tilt.relative(Degrees::new(0.0), Degrees::new(0.0), SpeedLevel::Medium);
    let _: DynFuture<'_, Result<(), Error>> = pan_tilt.limit_set(
        PanTiltLimitCorner::UpRight,
        Degrees::new(0.0),
        Degrees::new(0.0),
    );
    let _: DynFuture<'_, Result<(), Error>> = pan_tilt.limit_clear(PanTiltLimitCorner::UpRight);

    let focus = camera.focus();
    let _: DynFuture<'_, Result<FocusPosition, Error>> = focus.position();
    let _: DynFuture<'_, Result<FocusMode, Error>> = focus.mode();
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> = focus.far();
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> = focus.near();
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> =
        focus.far_variable(grafton_visca::command::FocusSpeed::new(1).unwrap());
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> =
        focus.near_variable(grafton_visca::command::FocusSpeed::new(1).unwrap());
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> = focus.stop();
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> =
        focus.set_position(FocusPosition::new(0));
    let _: DynFuture<'_, Result<(), Error>> = focus.auto();
    let _: DynFuture<'_, Result<(), Error>> = focus.manual();
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> = focus.one_push();
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> = focus.infinity();
    let _: DynFuture<'_, Result<(), Error>> = focus.toggle();
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> = focus.snap();
    let _: DynFuture<'_, Result<(), Error>> = focus.set_zone(FocusZone::Center);
    let _: DynFuture<'_, Result<(), Error>> = focus.set_sensitivity(AutoFocusSensitivity::Normal);
    let _: DynFuture<'_, Result<(), Error>> = focus.set_near_limit(FocusPosition::new(0));
    let _: DynFuture<'_, Result<(), Error>> = focus.set_lock(FocusLock::On);
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> = focus.push_af_press();
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> = focus.push_af_release();
    let _: DynFuture<'_, Result<FocusPosition, Error>> = focus.near_limit();
    let _: DynFuture<'_, Result<FocusZone, Error>> = focus.zone();
    let _: DynFuture<'_, Result<AutoFocusSensitivity, Error>> = focus.sensitivity();
    let _: DynFuture<'_, Result<FocusRange, Error>> = focus.range();

    let presets = camera.presets();
    let preset = PresetNumber::new(1).unwrap();
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> = presets.recall(preset);
    let _: DynFuture<'_, Result<(), Error>> =
        presets.set_recall_speed(grafton_visca::command::PresetRecallSpeed::new(1).unwrap());
    let _: DynFuture<'_, Result<(), Error>> = presets.set(preset);
    let _: DynFuture<'_, Result<(), Error>> = presets.reset(preset);

    let exposure = camera.exposure();
    let _: DynFuture<'_, Result<grafton_visca::command::ExposureMode, Error>> = exposure.mode();
    let _: DynFuture<'_, Result<(), Error>> =
        exposure.set_mode(grafton_visca::command::ExposureMode::Auto);
    let _: DynFuture<'_, Result<ShutterSpeed, Error>> = exposure.shutter();
    let _: DynFuture<'_, Result<(), Error>> = exposure.shutter_reset();
    let _: DynFuture<'_, Result<(), Error>> = exposure.shutter_up();
    let _: DynFuture<'_, Result<(), Error>> = exposure.shutter_down();
    let _: DynFuture<'_, Result<(), Error>> = exposure.shutter_direct(ShutterSpeed::MIN);
    let _: DynFuture<'_, Result<ExposureCompensationLevel, Error>> = exposure.compensation();
    let _: DynFuture<'_, Result<bool, Error>> = exposure.compensation_enabled();
    let _: DynFuture<'_, Result<ExposureCompensationPosition, Error>> =
        exposure.compensation_position();
    let _: DynFuture<'_, Result<(), Error>> = exposure.compensation_on();
    let _: DynFuture<'_, Result<(), Error>> = exposure.compensation_off();
    let _: DynFuture<'_, Result<(), Error>> = exposure.compensation_reset();
    let _: DynFuture<'_, Result<(), Error>> = exposure.compensation_up();
    let _: DynFuture<'_, Result<(), Error>> = exposure.compensation_down();
    let _: DynFuture<'_, Result<(), Error>> =
        exposure.compensation_direct(ExposureCompensationLevel::new(0).unwrap());
    let _: DynFuture<'_, Result<DynamicRangeLevel, Error>> = exposure.dynamic_range();
    let _: DynFuture<'_, Result<(), Error>> = exposure.set_dynamic_range(DynamicRangeLevel::MIN);
    let _: DynFuture<'_, Result<bool, Error>> = exposure.iris_control();
    let _: DynFuture<'_, Result<IrisLevel, Error>> = exposure.iris();
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> = exposure.iris_reset();
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> = exposure.iris_up();
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> = exposure.iris_down();
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> =
        exposure.iris_direct(IrisLevel::MIN);
    let _: DynFuture<'_, Result<BrightnessLevel, Error>> = exposure.brightness();
    let _: DynFuture<'_, Result<(), Error>> = exposure.brightness_reset();
    let _: DynFuture<'_, Result<(), Error>> = exposure.brightness_up();
    let _: DynFuture<'_, Result<(), Error>> = exposure.brightness_down();
    let _: DynFuture<'_, Result<(), Error>> = exposure.brightness_set(BrightnessLevel::MIN);
    let _: DynFuture<'_, Result<(), Error>> = exposure.brightness_direct(BrightnessLevel::MIN);
    let _: DynFuture<'_, Result<GainLevel, Error>> = exposure.gain();
    let _: DynFuture<'_, Result<(), Error>> = exposure.gain_reset();
    let _: DynFuture<'_, Result<(), Error>> = exposure.gain_up();
    let _: DynFuture<'_, Result<(), Error>> = exposure.gain_down();
    let _: DynFuture<'_, Result<(), Error>> = exposure.gain_direct(GainLevel::MIN);
    let _: DynFuture<'_, Result<GainLimit, Error>> = exposure.gain_limit();
    let _: DynFuture<'_, Result<(), Error>> = exposure.set_gain_limit(GainLimit::MIN);
    let _: DynFuture<'_, Result<(), Error>> = exposure.set_anti_flicker(AntiFlickerMode::Off);
    let _: DynFuture<'_, Result<AntiFlickerMode, Error>> = exposure.flicker_mode();
    let _: DynFuture<'_, Result<(), Error>> = exposure.spotlight_on();
    let _: DynFuture<'_, Result<(), Error>> = exposure.spotlight_off();
    let _: DynFuture<'_, Result<(), Error>> = exposure.auto_slow_shutter_on();
    let _: DynFuture<'_, Result<(), Error>> = exposure.auto_slow_shutter_off();

    let white_balance = camera.white_balance();
    let _: DynFuture<'_, Result<WhiteBalanceMode, Error>> = white_balance.mode();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.auto();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.indoor();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.outdoor();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.one_push();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.atw();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.manual();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.color_temperature_mode();
    let _: DynFuture<'_, Result<(), Error>> =
        white_balance.set_sensitivity(AutoWhiteBalanceSensitivity::Normal);
    let _: DynFuture<'_, Result<AutoWhiteBalanceSensitivity, Error>> = white_balance.sensitivity();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.one_push_trigger();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.set_red_tuning(RedTuning::MIN);
    let _: DynFuture<'_, Result<(), Error>> = white_balance.set_blue_tuning(BlueTuning::MIN);
    let _: DynFuture<'_, Result<ColorTemp, Error>> = white_balance.color_temperature();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.reset_color_temperature();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.increase_color_temperature();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.decrease_color_temperature();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.set_color_temperature(ColorTemp::MIN);
    let _: DynFuture<'_, Result<RedChannel, Error>> = white_balance.red_gain();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.reset_red_gain();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.increase_red_gain();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.decrease_red_gain();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.set_red_gain(RedChannel::MIN);
    let _: DynFuture<'_, Result<BlueChannel, Error>> = white_balance.blue_gain();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.reset_blue_gain();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.increase_blue_gain();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.decrease_blue_gain();
    let _: DynFuture<'_, Result<(), Error>> = white_balance.set_blue_gain(BlueChannel::MIN);
    let _: DynFuture<'_, Result<RedTuning, Error>> = white_balance.red_tuning();
    let _: DynFuture<'_, Result<BlueTuning, Error>> = white_balance.blue_tuning();

    let image = camera.image();
    let _: DynFuture<'_, Result<ResolutionMode, Error>> = image.resolution();
    let _: DynFuture<'_, Result<SaturationLevel, Error>> = image.saturation();
    let _: DynFuture<'_, Result<(), Error>> = image.set_saturation(SaturationLevel::MIN);
    let _: DynFuture<'_, Result<grafton_visca::types::HueLevel, Error>> = image.hue();
    let _: DynFuture<'_, Result<(), Error>> = image.set_hue(grafton_visca::types::HueLevel::MIN);
    let _: DynFuture<'_, Result<LuminanceLevel, Error>> = image.luminance();
    let _: DynFuture<'_, Result<(), Error>> = image.set_luminance(LuminanceLevel::MIN);
    let _: DynFuture<'_, Result<ContrastLevel, Error>> = image.contrast();
    let _: DynFuture<'_, Result<(), Error>> = image.set_contrast(ContrastLevel::MIN);
    let _: DynFuture<'_, Result<grafton_visca::types::GammaLevel, Error>> = image.gamma();
    let _: DynFuture<'_, Result<(), Error>> =
        image.set_gamma(grafton_visca::types::GammaLevel::MIN);
    let _: DynFuture<'_, Result<SharpnessMode, Error>> = image.sharpness_mode();
    let _: DynFuture<'_, Result<SharpnessLevel, Error>> = image.sharpness_level();
    let _: DynFuture<'_, Result<(), Error>> = image.set_sharpness_mode(SharpnessMode::Auto);
    let _: DynFuture<'_, Result<(), Error>> = image.reset_sharpness();
    let _: DynFuture<'_, Result<(), Error>> = image.increase_sharpness();
    let _: DynFuture<'_, Result<(), Error>> = image.decrease_sharpness();
    let _: DynFuture<'_, Result<(), Error>> = image.set_sharpness(SharpnessLevel::MIN);
    let _: DynFuture<'_, Result<bool, Error>> = image.backlight();
    let _: DynFuture<'_, Result<(), Error>> = image.set_backlight(false);
    let _: DynFuture<'_, Result<NoiseReduction2DLevel, Error>> = image.noise_reduction_2d();
    let _: DynFuture<'_, Result<(), Error>> =
        image.set_noise_reduction_2d(NoiseReduction2DLevel::MIN);
    let _: DynFuture<'_, Result<NoiseReduction3DLevel, Error>> = image.noise_reduction_3d();
    let _: DynFuture<'_, Result<(), Error>> =
        image.set_noise_reduction_3d(NoiseReduction3DLevel::MIN);
    let _: DynFuture<'_, Result<NoiseReductionLevel, Error>> = image.noise_reduction_level();
    let _: DynFuture<'_, Result<grafton_visca::command::NoiseReductionMode, Error>> =
        image.noise_reduction_mode();
    let _: DynFuture<'_, Result<(), Error>> = image.disable_flip();
    let _: DynFuture<'_, Result<(), Error>> = image.enable_flip();
    let _: DynFuture<'_, Result<(), Error>> = image.enable_horizontal_flip();
    let _: DynFuture<'_, Result<(), Error>> = image.set_flip_both();
    let _: DynFuture<'_, Result<(), Error>> = image.set_flip_mode(ImageFlipMode::Both);
    let _: DynFuture<'_, Result<(), Error>> = image.freeze_on();
    let _: DynFuture<'_, Result<(), Error>> = image.freeze_off();
    let _: DynFuture<'_, Result<grafton_visca::command::FlipState, Error>> = image.flip();
    let _: DynFuture<'_, Result<grafton_visca::command::FlipState, Error>> = image.flip_mode();
    let _: DynFuture<'_, Result<bool, Error>> = image.black_white();
    let _: DynFuture<'_, Result<BlackWhiteMode, Error>> = image.black_white_mode();
    let _: DynFuture<'_, Result<PictureEffectMode, Error>> = image.picture_effect();
    let _: DynFuture<'_, Result<(), Error>> = image.set_picture_effect(PictureEffectMode::Off);
    let _: DynFuture<'_, Result<DefogLevel, Error>> = image.defog_level();

    let presets = camera.presets();
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> =
        presets.recall(PresetNumber::new(1).unwrap());

    let tally = camera.tally();
    let _: DynFuture<'_, Result<TallyStatusState, Error>> = tally.status();
    let _: DynFuture<'_, Result<(), Error>> = tally.red_on();
    let _: DynFuture<'_, Result<(), Error>> = tally.red_off();
    let _: DynFuture<'_, Result<(), Error>> = tally.bright_lo();
    let _: DynFuture<'_, Result<(), Error>> = tally.bright_hi();
    let _: DynFuture<'_, Result<(), Error>> = tally.green_on();
    let _: DynFuture<'_, Result<(), Error>> = tally.green_off();
    let _: DynFuture<'_, Result<(), Error>> = tally.flash();
    let _: DynFuture<'_, Result<(), Error>> = tally.on();
    let _: DynFuture<'_, Result<(), Error>> = tally.off();
    let _: DynFuture<'_, Result<bool, Error>> = tally.red_status();
    let _: DynFuture<'_, Result<bool, Error>> = tally.green_status();
    let _: DynFuture<'_, Result<bool, Error>> = tally.auto_adjust_enabled();

    let nd = camera.nd_filter();
    let _: DynFuture<'_, Result<grafton_visca::command::NdFilterPosition, Error>> = nd.position();
    let _: DynFuture<'_, Result<NdFilterPreset, Error>> = nd.preset();
    let _: DynFuture<'_, Result<(), Error>> = nd.set_mode(NdFilterMode::Preset);
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> = nd.set_value(1);
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> = nd.set_stops(2.0);
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> = nd.step_up();
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> = nd.step_down();
    let _: DynFuture<'_, Result<(), Error>> = nd.auto_on();
    let _: DynFuture<'_, Result<(), Error>> = nd.auto_off();

    let sync = camera.motion_sync();
    let _: DynFuture<'_, Result<MotionSyncMode, Error>> = sync.mode();
    let _: DynFuture<'_, Result<MotionSyncPreset, Error>> = sync.preset();
    let _: DynFuture<'_, Result<(), Error>> = sync.set_mode(MotionSyncMode::On);
    let _: DynFuture<'_, Result<(), Error>> = sync.set_preset(1);
    let _: DynFuture<'_, Result<(), Error>> = sync.set_speed(MotionSyncSpeed::new(1).unwrap());

    let menu = camera.menu();
    let _: DynFuture<'_, Result<bool, Error>> = menu.status();
    let _: DynFuture<'_, Result<(), Error>> = menu.display(true);
    let _: DynFuture<'_, Result<(), Error>> = menu.navigate(MenuDirection::Up);
    let _: DynFuture<'_, Result<(), Error>> = menu.select();
    let _: DynFuture<'_, Result<(), Error>> = menu.cancel();
    let _: DynFuture<'_, Result<(), Error>> = menu.direct(0, 0);
    let _: DynFuture<'_, Result<(), Error>> = menu.toggle_display();

    let advanced = camera.advanced();
    let _: DynFuture<'_, Result<bool, Error>> = advanced.night_day_mode();
    let _: DynFuture<'_, Result<bool, Error>> = advanced.standby_enabled();
    let _: DynFuture<'_, Result<bool, Error>> = advanced.digital_ptz_enabled();
    let _: DynFuture<'_, Result<bool, Error>> = advanced.auto_trace_enabled();
    let _: DynFuture<'_, Result<bool, Error>> = advanced.focus_unlock();
    let _: DynFuture<'_, Result<grafton_visca::types::BroadcastDomain, Error>> =
        advanced.broadcast_domain();
    let _: DynFuture<'_, Result<bool, Error>> = advanced.usb_audio_enabled();
    let _: DynFuture<'_, Result<bool, Error>> = advanced.two_tone_mode_enabled();
    let _: DynFuture<'_, Result<bool, Error>> = advanced.digital_mode_enabled();
    let _: DynFuture<'_, Result<(), Error>> = advanced.multicast_on();
    let _: DynFuture<'_, Result<(), Error>> = advanced.multicast_off();
    let _: DynFuture<'_, Result<(), Error>> =
        advanced.set_ndi_quality(grafton_visca::types::NdiQuality::High);
    let _: DynFuture<'_, Result<(), Error>> = advanced.usb_audio_on();
    let _: DynFuture<'_, Result<(), Error>> = advanced.usb_audio_off();
    let _: DynFuture<'_, Result<(), Error>> =
        advanced.set_variable_speed_mode(VariableSpeedMode::Standard24);
}

fn assert_custom_requests(
    camera: &DynSessionCamera,
    targeted: &dyn DynTargetedRequest,
    applied: &dyn DynAppliedRequest,
) {
    let _: DynFuture<'_, Result<DynTargetedOperation, Error>> =
        grafton_visca::dynapi::submit_targeted(camera, targeted);
    let _: DynFuture<'_, Result<DynAppliedOperation, Error>> =
        grafton_visca::dynapi::submit_applied(camera, applied);
    let _ = (targeted, applied);
}

fn assert_targeted_settled(targeted: DynTargetedOperation) {
    let _ = targeted.settled();
}

fn assert_targeted_applied(targeted: DynTargetedOperation) {
    let _ = targeted.applied();
}

fn assert_applied_wait(applied: DynAppliedOperation) {
    let _ = applied.applied();
}

fn assert_applied_detach(applied: DynAppliedOperation) {
    applied.detach();
}

fn assert_handle_shapes(targeted: DynTargetedOperation, applied: DynAppliedOperation) {
    let _: grafton_visca::OperationId = targeted.id();
    let _: grafton_visca::OperationId = applied.id();
    assert_targeted_settled(targeted);
    assert_applied_wait(applied);
}

fn main() {
    let _: PhantomData<Box<dyn DynSessionCameraControl + Send + Sync>> = PhantomData;
    let _: PhantomData<Box<dyn DynSessionCameraNouns + Send + Sync>> = PhantomData;
    let _: PhantomData<Box<dyn DynPower + Send + Sync>> = PhantomData;
    let _ = camera_surface;
    let _ = noun_surfaces;
    let _ = motion_surface;
    let _ = assert_custom_requests;
    let _ = assert_handle_shapes;
}
