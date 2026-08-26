//! Compile-time contracts for the canonical blocking noun facade.

#![cfg(feature = "blocking")]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use grafton_visca::{
    blocking::{Camera, Operation},
    capabilities::{
        HasAutoFocusSensitivity, HasAutoWhiteBalanceSensitivity, HasBacklightCompensation,
        HasBrightnessControl, HasColorTemperature, HasContrastControl, HasExposure,
        HasExposureCompensation, HasFocus, HasFocusNearLimitInquiry, HasFocusZone, HasGammaControl,
        HasHueControl, HasImageFlip, HasImageProcessing, HasIrisControl, HasLuminanceControl,
        HasMenuControl, HasMotionSync, HasNdFilter, HasNoiseReduction, HasNoiseReduction2D,
        HasNoiseReduction3D, HasPanTilt, HasPictureEffect, HasPower, HasPresets, HasRgbGain,
        HasRgbTuning, HasSaturationControl, HasSharpnessControl, HasTally, HasWhiteBalance,
        HasWideDynamicRange, HasZoom,
    },
    command::MotionSyncMode,
    completion::{AppliedOnly, Targeted},
    profiles::{PtzOpticsG2, SonyFR7},
    CompileTimeProfile, Result,
};

fn plain(_: Result<()>) {}

fn applied<'session>(_: Result<Operation<'session, AppliedOnly>>) {}

fn targeted<'session>(_: Result<Operation<'session, Targeted>>) {}

fn baseline<'session, P>(camera: &Camera<'session, P>)
where
    P: CompileTimeProfile
        + HasPower
        + HasZoom
        + HasPanTilt
        + HasFocus
        + HasExposure
        + HasWhiteBalance
        + HasImageProcessing
        + HasPresets
        + HasMenuControl,
{
    plain(camera.power().on());
    plain(camera.power().off());
    plain(camera.system().save_settings());
    plain(
        camera
            .exposure()
            .set_mode(grafton_visca::ExposureMode::Auto),
    );
    plain(camera.white_balance().auto());
    plain(camera.image().freeze_on());
    plain(camera.menu().display(true));
    plain(camera.advanced().multicast_on());

    targeted(camera.pan_tilt().home());
    applied(camera.pan_tilt().stop());
    applied(camera.zoom().stop());
    applied(camera.focus().stop());

    // The safety/observation surface is intentionally a separate noun.
    let _ = camera.motion();
}

fn all_unconditional_inquiries<'session, P: CompileTimeProfile>(camera: &Camera<'session, P>) {
    let _ = camera.power().state();
    let _ = camera.zoom().position();
    let _ = camera.system().version();
    let _ = camera.pan_tilt().position();
    let _ = camera.focus().position();
    let _ = camera.focus().mode();
    let _ = camera.focus().range();
    let _ = camera.exposure().mode();
    let _ = camera.exposure().shutter();
    let _ = camera.exposure().gain();
    let _ = camera.exposure().gain_limit();
    let _ = camera.exposure().flicker_mode();
    let _ = camera.white_balance().mode();
    let _ = camera.image().resolution();
    let _ = camera.image().defog_level();
    let _ = camera.menu().status();
    let _ = camera.advanced().night_day_mode();
    let _ = camera.advanced().standby_enabled();
    let _ = camera.advanced().digital_ptz_enabled();
    let _ = camera.advanced().auto_trace_enabled();
    let _ = camera.advanced().focus_unlock();
    let _ = camera.advanced().broadcast_domain();
    let _ = camera.advanced().usb_audio_enabled();
    let _ = camera.advanced().two_tone_mode_enabled();
    let _ = camera.advanced().digital_mode_enabled();
}

#[allow(dead_code)]
fn noise_gate<'session, P: CompileTimeProfile + HasNoiseReduction>(camera: &Camera<'session, P>) {
    let _ = camera.image().noise_reduction_level();
    let _ = camera.image().noise_reduction_mode();
}

#[allow(dead_code)]
fn all_optional_inquiries<'session, P>(camera: &Camera<'session, P>)
where
    P: CompileTimeProfile
        + HasAutoFocusSensitivity
        + HasAutoWhiteBalanceSensitivity
        + HasBacklightCompensation
        + HasBrightnessControl
        + HasColorTemperature
        + HasContrastControl
        + HasExposureCompensation
        + HasFocusNearLimitInquiry
        + HasFocusZone
        + HasGammaControl
        + HasHueControl
        + HasImageFlip
        + HasIrisControl
        + HasLuminanceControl
        + HasMotionSync
        + HasNdFilter
        + HasNoiseReduction
        + HasNoiseReduction2D
        + HasNoiseReduction3D
        + HasPictureEffect
        + HasRgbGain
        + HasRgbTuning
        + HasSaturationControl
        + HasSharpnessControl
        + HasTally
        + HasWideDynamicRange,
{
    let _ = camera.focus().near_limit();
    let _ = camera.focus().zone();
    let _ = camera.focus().sensitivity();
    let _ = camera.exposure().compensation();
    let _ = camera.exposure().compensation_enabled();
    let _ = camera.exposure().compensation_position();
    let _ = camera.exposure().dynamic_range();
    let _ = camera.exposure().iris_control();
    let _ = camera.exposure().iris();
    let _ = camera.exposure().brightness();
    let _ = camera.image().saturation();
    let _ = camera.image().hue();
    let _ = camera.image().luminance();
    let _ = camera.image().contrast();
    let _ = camera.image().gamma();
    let _ = camera.image().sharpness_mode();
    let _ = camera.image().sharpness_level();
    let _ = camera.image().backlight();
    let _ = camera.image().noise_reduction_2d();
    let _ = camera.image().noise_reduction_3d();
    let _ = camera.image().flip();
    let _ = camera.image().black_white();
    let _ = camera.image().black_white_mode();
    let _ = camera.image().picture_effect();
    let _ = camera.image().flip_mode();
    let _ = camera.white_balance().sensitivity();
    let _ = camera.white_balance().color_temperature();
    let _ = camera.white_balance().red_gain();
    let _ = camera.white_balance().blue_gain();
    let _ = camera.white_balance().red_tuning();
    let _ = camera.white_balance().blue_tuning();
    let _ = camera.tally().status();
    let _ = camera.tally().red_status();
    let _ = camera.tally().green_status();
    let _ = camera.tally().auto_adjust_enabled();
    let _ = camera.nd_filter().position();
    let _ = camera.nd_filter().preset();
    let _ = camera.motion_sync().mode();
    let _ = camera.motion_sync().preset();
}

fn sony_optional<'session>(camera: &Camera<'session, SonyFR7>) {
    plain(camera.tally().red_on());
    plain(camera.tally().off());
    targeted(camera.nd_filter().set_value(1));
    targeted(camera.nd_filter().step_up());
    applied(camera.zoom().tele());
    plain(camera.exposure().flicker_mode().map(|_| ())); // typed inquiry is blocking
}

#[allow(dead_code)]
fn awb_gate<'session, P: CompileTimeProfile + HasAutoWhiteBalanceSensitivity>(
    camera: &Camera<'session, P>,
) {
    plain(camera.white_balance().sensitivity().map(|_| ()));
}

#[allow(dead_code)]
fn motion_gate<'session, P: CompileTimeProfile + HasMotionSync>(camera: &Camera<'session, P>) {
    plain(camera.motion_sync().set_mode(MotionSyncMode::On));
    plain(camera.motion_sync().set_preset(1));
}

fn non_default<'session>(
    camera: &Camera<'session, profile_fixtures::NonDefaultCompileTimeProfile>,
) {
    baseline(camera);
}

#[test]
fn static_camera_surface_is_profile_typed_and_non_default() {
    let _: for<'session> fn(&'session Camera<'session, PtzOpticsG2>) = baseline::<PtzOpticsG2>;
    let _: for<'session> fn(&'session Camera<'session, PtzOpticsG2>) =
        all_unconditional_inquiries::<PtzOpticsG2>;
    let _: for<'session> fn(&'session Camera<'session, SonyFR7>) = sony_optional;
    let _: for<'session> fn(
        &'session Camera<'session, profile_fixtures::NonDefaultCompileTimeProfile>,
    ) = non_default;
}
