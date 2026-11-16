#![allow(dead_code)]

use grafton_visca::{
    operation::AppliedOnly, operation::CancellationOutcome, operation::Targeted,
    profiles::AnyProfile, r#async::Camera, r#async::Operation, r#async::Session,
    request::OperationCommand, state_cache::StateCache, types::*, Error,
};
use std::{future::Future, marker::PhantomData, time::Duration};

fn future<T>(_: impl Future<Output = Result<T, Error>>) {}

fn camera_surface(camera: &Camera<AnyProfile>) {
    let _: &grafton_visca::capabilities::Capabilities = camera.capabilities();
    let _: StateCache = camera.state_cache();
}

// Check each public noun's concrete native future output with a runtime profile.
// Constructing these futures performs no transport I/O.
fn noun_surfaces(camera: &Camera<AnyProfile>) {
    future::<PowerStatus>(camera.power().status());
    future::<()>(camera.power().on());
    future::<()>(camera.power().standby());
    future::<ZoomPosition>(camera.zoom().position());
    future::<Operation<AppliedOnly>>(camera.zoom().tele());
    future::<Operation<AppliedOnly>>(camera.zoom().wide());
    future::<Operation<AppliedOnly>>(camera.zoom().stop());
    future::<Operation<AppliedOnly>>(camera.zoom().tele_variable(SpeedLevel::Medium));
    future::<Operation<AppliedOnly>>(camera.zoom().wide_variable(SpeedLevel::Medium));
    future::<Operation<Targeted>>(camera.zoom().set_position(ZoomPosition::MIN));
    future::<Operation<Targeted>>(
        camera
            .zoom()
            .set_normalized(UnitInterval::MIN, ZoomDomain::Optical),
    );
    future::<()>(camera.zoom().set_digital_zoom(true));
    future::<Operation<AppliedOnly>>(
        camera
            .zoom()
            .tele_high_resolution(HighResolutionZoomSpeed::MIN),
    );
    future::<Operation<AppliedOnly>>(
        camera
            .zoom()
            .wide_high_resolution(HighResolutionZoomSpeed::MIN),
    );
    future::<Operation<AppliedOnly>>(camera.zoom().stop_high_resolution());
    future::<ClearImageZoomPosition>(camera.zoom().clear_image_position());
    future::<VersionInfo>(camera.system().version());
    future::<()>(camera.system().save_settings());
    future::<PanTiltPosition>(camera.pan_tilt().position());
    future::<Operation<Targeted>>(camera.pan_tilt().home());
    future::<Operation<Targeted>>(camera.pan_tilt().reset());
    future::<Operation<AppliedOnly>>(
        camera
            .pan_tilt()
            .drive(PanTiltDirection::Up, SpeedLevel::Medium),
    );
    future::<Operation<AppliedOnly>>(camera.pan_tilt().stop());
    future::<Operation<Targeted>>(camera.pan_tilt().absolute(
        PanTiltPosition {
            pan: Degrees::new(0.0).unwrap(),
            tilt: Degrees::new(0.0).unwrap(),
        },
        SpeedLevel::Medium,
    ));
    future::<Operation<Targeted>>(camera.pan_tilt().relative(
        PanTiltPosition {
            pan: Degrees::new(0.0).unwrap(),
            tilt: Degrees::new(0.0).unwrap(),
        },
        SpeedLevel::Medium,
    ));
    future::<()>(camera.pan_tilt().set_limit(
        PanTiltLimitCorner::UpRight,
        PanTiltPosition {
            pan: Degrees::new(0.0).unwrap(),
            tilt: Degrees::new(0.0).unwrap(),
        },
    ));
    future::<()>(camera.pan_tilt().clear_limit(PanTiltLimitCorner::UpRight));
    future::<FocusPosition>(camera.focus().position());
    future::<FocusMode>(camera.focus().mode());
    future::<Operation<AppliedOnly>>(camera.focus().far());
    future::<Operation<AppliedOnly>>(camera.focus().near());
    future::<Operation<AppliedOnly>>(camera.focus().far_variable(SpeedLevel::Medium));
    future::<Operation<AppliedOnly>>(camera.focus().near_variable(SpeedLevel::Medium));
    future::<Operation<AppliedOnly>>(camera.focus().stop());
    future::<Operation<Targeted>>(camera.focus().set_position(FocusPosition::MIN));
    future::<Operation<AppliedOnly>>(camera.focus().one_push());
    future::<Operation<Targeted>>(camera.focus().infinity());
    future::<()>(camera.focus().toggle_mode());
    future::<Operation<AppliedOnly>>(camera.focus().snap());
    future::<()>(camera.focus().set_zone(FocusZone::Center));
    future::<()>(camera.focus().set_sensitivity(AutoFocusSensitivity::Normal));
    future::<()>(camera.focus().set_near_limit(FocusPosition::MIN));
    future::<()>(camera.focus().set_lock(true));
    future::<Operation<AppliedOnly>>(camera.focus().push_auto_focus_press());
    future::<Operation<AppliedOnly>>(camera.focus().push_auto_focus_release());
    future::<FocusPosition>(camera.focus().near_limit());
    future::<FocusZone>(camera.focus().zone());
    future::<AutoFocusSensitivity>(camera.focus().sensitivity());
    future::<Operation<Targeted>>(camera.presets().recall(PresetNumber::MIN));
    future::<()>(camera.presets().set_recall_speed(PresetSpeed::MIN));
    future::<()>(camera.presets().store(PresetNumber::MIN));
    future::<()>(camera.presets().clear(PresetNumber::MIN));
    future::<()>(
        camera
            .presets()
            .set_speed_selection(PresetSpeedSelection::Common),
    );
    future::<PresetSpeedSelection>(camera.presets().speed_selection());
    future::<PresetSpeed>(camera.presets().recall_speed());
    future::<()>(
        camera
            .presets()
            .set_separate_mode(PresetNumber::MIN, PresetSeparateMode::Speed),
    );
    future::<PresetSeparateMode>(camera.presets().separate_mode(PresetNumber::MIN));
    future::<()>(
        camera
            .presets()
            .set_separate_speed(PresetNumber::MIN, PresetSpeed::MIN),
    );
    future::<PresetSpeed>(camera.presets().separate_speed(PresetNumber::MIN));
    future::<()>(
        camera
            .presets()
            .set_separate_duration(PresetNumber::MIN, Duration::from_millis(100)),
    );
    future::<std::time::Duration>(camera.presets().separate_duration(PresetNumber::MIN));
    future::<ExposureMode>(camera.exposure().mode());
    future::<()>(camera.exposure().set_mode(ExposureMode::Auto));
    future::<ShutterSpeed>(camera.exposure().shutter());
    future::<()>(camera.exposure().shutter_reset());
    future::<()>(camera.exposure().shutter_up());
    future::<()>(camera.exposure().shutter_down());
    future::<()>(camera.exposure().set_shutter(ShutterSpeed::MIN));
    future::<ExposureCompensationLevel>(camera.exposure().compensation());
    future::<bool>(camera.exposure().compensation_enabled());
    future::<()>(camera.exposure().set_compensation_enabled(true));
    future::<()>(camera.exposure().compensation_reset());
    future::<()>(camera.exposure().compensation_up());
    future::<()>(camera.exposure().compensation_down());
    future::<()>(
        camera
            .exposure()
            .set_compensation(ExposureCompensationLevel::MIN),
    );
    future::<()>(
        camera
            .exposure()
            .set_wide_dynamic_range(WideDynamicRangeLevel::MIN),
    );
    future::<IrisLevel>(camera.exposure().iris());
    future::<Operation<Targeted>>(camera.exposure().iris_reset());
    future::<Operation<Targeted>>(camera.exposure().iris_up());
    future::<Operation<Targeted>>(camera.exposure().iris_down());
    future::<Operation<Targeted>>(camera.exposure().set_iris(IrisLevel::MIN));
    future::<BrightnessLevel>(camera.exposure().brightness());
    future::<()>(camera.exposure().brightness_reset());
    future::<()>(camera.exposure().brightness_up());
    future::<()>(camera.exposure().brightness_down());
    future::<()>(camera.exposure().set_brightness(BrightnessLevel::MIN));
    future::<GainLevel>(camera.exposure().gain());
    future::<()>(camera.exposure().gain_reset());
    future::<()>(camera.exposure().gain_up());
    future::<()>(camera.exposure().gain_down());
    future::<()>(camera.exposure().set_gain(GainLevel::MIN));
    future::<GainLimit>(camera.exposure().gain_limit());
    future::<()>(camera.exposure().set_gain_limit(GainLimit::MIN));
    future::<()>(camera.exposure().set_anti_flicker(AntiFlickerMode::Off));
    future::<AntiFlickerMode>(camera.exposure().anti_flicker());
    future::<()>(camera.exposure().set_spotlight(true));
    future::<()>(camera.exposure().set_auto_slow_shutter(true));
    future::<()>(camera.exposure().ae_level_up());
    future::<()>(camera.exposure().ae_level_down());
    future::<AeLevel>(camera.exposure().ae_level());
    future::<()>(camera.exposure().set_auto_gain(true));
    future::<bool>(camera.exposure().auto_gain());
    future::<()>(camera.exposure().set_auto_shutter(true));
    future::<bool>(camera.exposure().auto_shutter());
    future::<()>(camera.exposure().set_auto_iris(true));
    future::<bool>(camera.exposure().auto_iris());
    future::<Operation<AppliedOnly>>(camera.exposure().iris_up_by(IrisStep::MIN));
    future::<Operation<AppliedOnly>>(camera.exposure().iris_down_by(IrisStep::MIN));
    future::<WhiteBalanceMode>(camera.white_balance().mode());
    future::<()>(
        camera
            .white_balance()
            .set_sensitivity(AutoWhiteBalanceSensitivity::Normal),
    );
    future::<AutoWhiteBalanceSensitivity>(camera.white_balance().sensitivity());
    future::<()>(camera.white_balance().one_push());
    future::<()>(camera.white_balance().set_red_tuning(RgbTuning::MIN));
    future::<()>(camera.white_balance().set_blue_tuning(RgbTuning::MIN));
    future::<Kelvin>(camera.white_balance().color_temperature());
    future::<()>(camera.white_balance().color_temperature_reset());
    future::<()>(camera.white_balance().color_temperature_up());
    future::<()>(camera.white_balance().color_temperature_down());
    future::<()>(camera.white_balance().set_color_temperature(Kelvin::MIN));
    future::<RgbGain>(camera.white_balance().red_gain());
    future::<()>(camera.white_balance().red_gain_reset());
    future::<()>(camera.white_balance().red_gain_up());
    future::<()>(camera.white_balance().red_gain_down());
    future::<()>(camera.white_balance().set_red_gain(RgbGain::MIN));
    future::<RgbGain>(camera.white_balance().blue_gain());
    future::<()>(camera.white_balance().blue_gain_reset());
    future::<()>(camera.white_balance().blue_gain_up());
    future::<()>(camera.white_balance().blue_gain_down());
    future::<()>(camera.white_balance().set_blue_gain(RgbGain::MIN));
    future::<RgbTuning>(camera.white_balance().red_tuning());
    future::<RgbTuning>(camera.white_balance().blue_tuning());
    future::<()>(
        camera
            .white_balance()
            .preset_white_up_by(WhiteBalanceStep::MIN),
    );
    future::<()>(
        camera
            .white_balance()
            .preset_white_down_by(WhiteBalanceStep::MIN),
    );
    future::<()>(camera.white_balance().set_preset_white(Kelvin::MIN));
    future::<Kelvin>(camera.white_balance().preset_white());
    future::<()>(
        camera
            .white_balance()
            .memory_color_temperature_up_by(MemoryColorTemperatureStep::MIN),
    );
    future::<()>(
        camera
            .white_balance()
            .memory_color_temperature_down_by(MemoryColorTemperatureStep::MIN),
    );
    future::<()>(
        camera
            .white_balance()
            .set_memory_color_temperature(Kelvin::MIN),
    );
    future::<Kelvin>(camera.white_balance().memory_color_temperature());
    future::<()>(
        camera
            .white_balance()
            .memory_red_gain_up_by(WhiteBalanceStep::MIN),
    );
    future::<()>(
        camera
            .white_balance()
            .memory_red_gain_down_by(WhiteBalanceStep::MIN),
    );
    future::<()>(
        camera
            .white_balance()
            .set_memory_red_gain(WhiteBalanceGain::MIN),
    );
    future::<WhiteBalanceGain>(camera.white_balance().memory_red_gain());
    future::<()>(
        camera
            .white_balance()
            .memory_blue_gain_up_by(WhiteBalanceStep::MIN),
    );
    future::<()>(
        camera
            .white_balance()
            .memory_blue_gain_down_by(WhiteBalanceStep::MIN),
    );
    future::<()>(
        camera
            .white_balance()
            .set_memory_blue_gain(WhiteBalanceGain::MIN),
    );
    future::<WhiteBalanceGain>(camera.white_balance().memory_blue_gain());
    future::<()>(
        camera
            .white_balance()
            .memory_tint_up_by(WhiteBalanceStep::MIN),
    );
    future::<()>(
        camera
            .white_balance()
            .memory_tint_down_by(WhiteBalanceStep::MIN),
    );
    future::<()>(
        camera
            .white_balance()
            .set_memory_tint(WhiteBalanceOffset::MIN),
    );
    future::<WhiteBalanceOffset>(camera.white_balance().memory_tint());
    future::<()>(
        camera
            .white_balance()
            .atw_color_temperature_offset_up_by(WhiteBalanceStep::MIN),
    );
    future::<()>(
        camera
            .white_balance()
            .atw_color_temperature_offset_down_by(WhiteBalanceStep::MIN),
    );
    future::<()>(
        camera
            .white_balance()
            .set_atw_color_temperature_offset(WhiteBalanceOffset::MIN),
    );
    future::<WhiteBalanceOffset>(camera.white_balance().atw_color_temperature_offset());
    future::<()>(
        camera
            .white_balance()
            .atw_tint_offset_up_by(WhiteBalanceStep::MIN),
    );
    future::<()>(
        camera
            .white_balance()
            .atw_tint_offset_down_by(WhiteBalanceStep::MIN),
    );
    future::<()>(
        camera
            .white_balance()
            .set_atw_tint_offset(WhiteBalanceOffset::MIN),
    );
    future::<WhiteBalanceOffset>(camera.white_balance().atw_tint_offset());
    future::<SaturationLevel>(camera.image().saturation());
    future::<()>(camera.image().set_saturation(SaturationLevel::MIN));
    future::<HueLevel>(camera.image().hue());
    future::<()>(camera.image().set_hue(HueLevel::MIN));
    future::<LuminanceLevel>(camera.image().luminance());
    future::<()>(camera.image().set_luminance(LuminanceLevel::MIN));
    future::<ContrastLevel>(camera.image().contrast());
    future::<()>(camera.image().set_contrast(ContrastLevel::MIN));
    future::<GammaLevel>(camera.image().gamma());
    future::<()>(camera.image().set_gamma(GammaLevel::MIN));
    future::<SharpnessMode>(camera.image().sharpness_mode());
    future::<SharpnessLevel>(camera.image().sharpness());
    future::<()>(camera.image().set_sharpness_mode(SharpnessMode::Manual));
    future::<()>(camera.image().sharpness_reset());
    future::<()>(camera.image().sharpness_up());
    future::<()>(camera.image().sharpness_down());
    future::<()>(camera.image().set_sharpness(SharpnessLevel::MIN));
    future::<NoiseReduction2D>(camera.image().noise_reduction_2d());
    future::<NoiseReduction2DMode>(camera.image().noise_reduction_2d_mode());
    future::<NoiseReduction3D>(camera.image().noise_reduction_3d());
    future::<()>(
        camera
            .image()
            .set_noise_reduction_2d_mode(NoiseReduction2DMode::Manual),
    );
    future::<()>(camera.image().set_noise_reduction_2d(NoiseReduction2D::Off));
    future::<()>(camera.image().set_noise_reduction_3d(NoiseReduction3D::Off));
    future::<()>(camera.image().set_flip(true));
    future::<()>(camera.image().set_mirror(true));
    future::<()>(camera.image().set_flip_mode(ImageFlipMode::Off));
    future::<()>(camera.image().set_freeze(true));
    future::<bool>(camera.image().flip());
    future::<ImageFlipMode>(camera.image().flip_mode());
    future::<PictureEffectMode>(camera.image().picture_effect());
    future::<()>(camera.image().set_picture_effect(PictureEffectMode::Off));
    future::<()>(camera.image().master_black_up_by(BlackStep::MIN));
    future::<()>(camera.image().master_black_down_by(BlackStep::MIN));
    future::<()>(camera.image().set_master_black(BlackLevel::MIN));
    future::<BlackLevel>(camera.image().master_black());
    future::<()>(camera.image().red_black_up_by(BlackStep::MIN));
    future::<()>(camera.image().red_black_down_by(BlackStep::MIN));
    future::<()>(camera.image().set_red_black(BlackLevel::MIN));
    future::<BlackLevel>(camera.image().red_black());
    future::<()>(camera.image().blue_black_up_by(BlackStep::MIN));
    future::<()>(camera.image().blue_black_down_by(BlackStep::MIN));
    future::<()>(camera.image().set_blue_black(BlackLevel::MIN));
    future::<BlackLevel>(camera.image().blue_black());
    future::<()>(camera.image().set_detail(true));
    future::<bool>(camera.image().detail());
    future::<()>(camera.image().set_knee(true));
    future::<bool>(camera.image().knee());
    future::<()>(camera.image().set_knee_mode(KneeMode::Auto));
    future::<KneeMode>(camera.image().knee_mode());
    future::<()>(camera.image().set_knee_slope(KneeSlope::MIN));
    future::<KneeSlope>(camera.image().knee_slope());
    future::<()>(camera.image().set_knee_point(KneePoint::MIN));
    future::<KneePoint>(camera.image().knee_point());
    future::<KneePermission>(camera.image().knee_permission(KneeParameter::Mode));
    future::<()>(camera.tally().set_red(true));
    future::<()>(camera.tally().set_brightness(TallyBrightness::Low));
    future::<()>(camera.tally().set_green(true));
    future::<()>(camera.tally().set_mode(TallyMode::Flash));
    future::<bool>(camera.tally().red());
    future::<bool>(camera.tally().green());
    future::<NdFilterDensity>(camera.nd_filter().density());
    future::<NdFilterPreset>(camera.nd_filter().preset());
    future::<()>(camera.nd_filter().set_mode(NdFilterMode::Variable));
    future::<Operation<Targeted>>(camera.nd_filter().set_density(NdFilterDensity::MIN));
    future::<Operation<Targeted>>(camera.nd_filter().density_up());
    future::<Operation<Targeted>>(camera.nd_filter().density_down());
    future::<()>(camera.nd_filter().set_auto(true));
    future::<bool>(camera.motion_sync().enabled());
    future::<()>(camera.motion_sync().set_enabled(true));
    future::<()>(camera.motion_sync().set_speed(MotionSyncSpeed::MIN));
    future::<bool>(camera.menu().visible());
    future::<()>(camera.menu().set_visible(true));
    future::<Operation<AppliedOnly>>(camera.menu().navigate(MenuDirection::Up));
    future::<()>(camera.menu().enter());
    future::<()>(camera.menu().back());
    future::<()>(
        camera
            .menu()
            .direct(DirectMenuTarget::NdFilter, MenuButton::Press),
    );
    future::<()>(camera.menu().toggle_visible());
    future::<bool>(camera.streaming().usb_audio());
    future::<()>(camera.streaming().set_multicast(true));
    future::<()>(camera.streaming().set_ndi_quality(NdiQuality::High));
    future::<()>(camera.streaming().set_usb_audio(true));
    future::<UnitInterval>(camera.zoom().normalized(ZoomDomain::Optical));
    future::<()>(
        camera
            .pan_tilt()
            .set_speed_mode(PanTiltSpeedMode::Standard24),
    );
    future::<()>(camera.focus().set_mode(FocusMode::Auto));
    future::<bool>(camera.exposure().backlight_compensation());
    future::<()>(camera.exposure().set_backlight_compensation(true));
    future::<()>(camera.white_balance().set_mode(WhiteBalanceMode::Auto));
    future::<grafton_visca::operation::HaltReport>(camera.motion().stop_all());
    future::<bool>(
        camera
            .motion()
            .is_moving(MotionQuery::new(AffectedAxes::ZOOM)),
    );
    future::<()>(
        camera
            .motion()
            .wait_until_idle(IdleWait::new(AffectedAxes::ZOOM, Duration::from_secs(1))),
    );
}

fn assert_custom_requests<T, A>(camera: &Camera, targeted: &T, applied: &A)
where
    T: OperationCommand<Targeted>,
    A: OperationCommand<AppliedOnly>,
{
    future::<Operation<Targeted>>(camera.submit(targeted));
    future::<Operation<AppliedOnly>>(camera.submit(applied));
}

fn assert_session_selectors(session: &Session) {
    let _: Result<Camera<AnyProfile>, Error> = session.camera::<AnyProfile>(CameraId::CAMERA_1);
    let _: Result<Camera<AnyProfile>, Error> = session.camera::<AnyProfile>(CameraId::CAMERA_2);
}

/// Waits borrow the handle (#777): application then settlement on one handle,
/// cached results, and idempotent cancellation.
async fn assert_targeted_lifecycle(
    mut targeted: Operation<Targeted>,
) -> Result<grafton_visca::operation::Settlement, Error> {
    targeted
        .applied_with_timeout(Duration::from_millis(1))
        .await?;
    targeted.applied().await?;
    targeted.settled().await?;
    targeted.settled_with_timeout(Duration::from_secs(1)).await
}

async fn assert_applied_lifecycle(
    mut applied: Operation<AppliedOnly>,
) -> Result<CancellationOutcome, Error> {
    applied.applied().await?;
    let _ = applied.cancel_with_timeout(Duration::from_secs(1)).await;
    let outcome = applied.cancel().await?;
    applied.detach();
    Ok(outcome)
}

fn assert_wait_futures_are_send(targeted: &mut Operation<Targeted>) {
    fn assert_send<T: Send>(_: &T) {}
    assert_send(&targeted.applied());
    assert_send(&targeted.settled());
    assert_send(&targeted.cancel());
}

fn assert_handle_shapes(targeted: Operation<Targeted>, applied: Operation<AppliedOnly>) {
    let _: grafton_visca::operation::OperationId = targeted.id();
    let _: grafton_visca::operation::OperationId = applied.id();
    let _ = (
        assert_targeted_lifecycle,
        assert_applied_lifecycle,
        assert_wait_futures_are_send,
    );
}

fn main() {
    let _: PhantomData<Camera<AnyProfile>> = PhantomData;
    let _: PhantomData<Operation<Targeted>> = PhantomData;
    let _: PhantomData<Operation<AppliedOnly>> = PhantomData;
    let _ = camera_surface;
    let _ = noun_surfaces;
    let _ = assert_session_selectors;
    let _ = assert_handle_shapes;
    let _ = assert_custom_requests::<
        grafton_visca::command::pan_tilt::Home,
        grafton_visca::command::zoom::Stop,
    >;
}
