//! Compile-time contracts for the canonical blocking noun facade.

#![cfg(feature = "blocking")]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use grafton_visca::{
    blocking::{Camera, Operation},
    capabilities::{
        HasAutoFocusSensitivity, HasAutoWhiteBalanceSensitivity, HasBacklightCompensation,
        HasBrightnessControl, HasColorTemperature, HasContrastControl, HasExposure,
        HasExposureCompensation, HasExposureMode, HasFocus, HasFocusNearLimitInquiry, HasFocusZone,
        HasFocusZoneInquiry, HasGammaControl, HasHueControl, HasImageFlip, HasImageProcessing,
        HasIrisControl, HasIrisControlInquiry, HasLuminanceControl, HasMenuControl, HasMotionSync,
        HasNdFilter, HasNoiseReduction2D, HasNoiseReduction3D, HasPanTilt, HasPictureEffect,
        HasPower, HasPresets, HasPtzOpticsAntiFlicker, HasPtzOpticsMulticastStreaming,
        HasPtzOpticsNdiQuality, HasPtzOpticsPresetRecallSpeed, HasPtzOpticsSettingsSave,
        HasRgbGain, HasRgbTuning, HasSaturationControl, HasSharpnessControl,
        HasSonyAutoSlowShutter, HasSonySpotlight, HasTally, HasUsbAudio, HasVersionInquiry,
        HasWhiteBalance, HasWideDynamicRange, HasZoom,
    },
    command::MotionSyncMode,
    completion::{AppliedOnly, Targeted},
    profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    },
    CompileTimeProfile, Result,
};

fn plain(_: Result<()>) {}

fn applied(_: Result<Operation<AppliedOnly>>) {}

fn targeted(_: Result<Operation<Targeted>>) {}

fn baseline<P>(camera: &Camera<P>)
where
    P: CompileTimeProfile
        + HasPower
        + HasZoom
        + HasPanTilt
        + HasFocus
        + HasExposure
        + HasWhiteBalance
        + HasPresets
        + HasMenuControl,
{
    plain(camera.power().on());
    plain(camera.power().off());
    plain(camera.white_balance().auto());
    plain(camera.menu().display(true));

    targeted(camera.pan_tilt().home());
    applied(camera.pan_tilt().stop());
    applied(camera.zoom().stop());
    applied(camera.focus().stop());

    // The safety/observation surface is intentionally a separate noun.
    let _ = camera.motion();
}

fn exposure_mode_surface<P: CompileTimeProfile + HasExposureMode>(camera: &Camera<P>) {
    plain(
        camera
            .exposure()
            .set_mode(grafton_visca::ExposureMode::Auto),
    );
    let _ = camera.exposure().mode();
}

/// These rows used to be included in `baseline` because
/// `HasImageProcessing` was blanket-implemented from image metadata. Generic
/// VISCA's all-empty metadata deliberately does not imply the typed image
/// noun, so the explicit base marker now owns this surface.
fn base_image_surface<P: CompileTimeProfile + HasImageProcessing>(camera: &Camera<P>) {
    let _ = camera.image();
}

fn backlight_surface<P>(camera: &Camera<P>)
where
    P: CompileTimeProfile + HasImageProcessing + HasBacklightCompensation,
{
    let _ = camera.image().backlight();
    plain(camera.image().set_backlight(true));
}

fn all_base_inquiries<P: CompileTimeProfile>(camera: &Camera<P>) {
    let _ = camera.power().state();
    let _ = camera.zoom().position();
    let _ = camera.pan_tilt().position();
    let _ = camera.focus().position();
    let _ = camera.focus().mode();
    let _ = camera.focus().range();
    let _ = camera.exposure().shutter();
    let _ = camera.exposure().gain();
    let _ = camera.exposure().gain_limit();
    let _ = camera.white_balance().mode();
    let _ = camera.menu().status();
    let _ = camera.advanced().night_day_mode();
    let _ = camera.advanced().standby_enabled();
    let _ = camera.advanced().digital_ptz_enabled();
    let _ = camera.advanced().auto_trace_enabled();
    let _ = camera.advanced().focus_unlock();
    let _ = camera.advanced().broadcast_domain();
    let _ = camera.advanced().two_tone_mode_enabled();
    let _ = camera.advanced().digital_mode_enabled();
}

#[allow(dead_code)]
fn noise_2d_gate<P: CompileTimeProfile + HasImageProcessing + HasNoiseReduction2D>(
    camera: &Camera<P>,
) {
    let _ = camera.image().noise_reduction_2d_mode();
}

#[allow(dead_code)]
fn all_optional_inquiries<P>(camera: &Camera<P>)
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
        + HasFocusZoneInquiry
        + HasGammaControl
        + HasHueControl
        + HasImageFlip
        + HasImageProcessing
        + HasIrisControl
        + HasIrisControlInquiry
        + HasLuminanceControl
        + HasMotionSync
        + HasNdFilter
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
    let _ = camera.image().noise_reduction_2d_mode();
    let _ = camera.image().noise_reduction_3d();
    let _ = camera.image().flip();
    let _ = camera.image().picture_effect();
    let _ = camera.image().flip_mode();
    let _ = camera.white_balance().sensitivity();
    let _ = camera.white_balance().color_temperature();
    let _ = camera.white_balance().red_gain();
    let _ = camera.white_balance().blue_gain();
    let _ = camera.white_balance().red_tuning();
    let _ = camera.white_balance().blue_tuning();
    let _ = camera.tally().red_status();
    let _ = camera.tally().green_status();
    let _ = camera.nd_filter().position();
    let _ = camera.nd_filter().preset();
    let _ = camera.motion_sync().mode();
    let _ = camera.motion_sync().preset();
}

fn sony_optional(camera: &Camera<SonyFR7>) {
    plain(camera.tally().red_on());
    plain(camera.tally().green_off());
    targeted(camera.nd_filter().set_value(1));
    targeted(camera.nd_filter().step_up());
    applied(camera.zoom().tele());
}

#[allow(dead_code)]
fn awb_gate<P: CompileTimeProfile + HasAutoWhiteBalanceSensitivity>(camera: &Camera<P>) {
    plain(camera.white_balance().sensitivity().map(|_| ()));
}

#[allow(dead_code)]
fn motion_gate<P: CompileTimeProfile + HasMotionSync>(camera: &Camera<P>) {
    plain(camera.motion_sync().set_mode(MotionSyncMode::On));
    plain(camera.motion_sync().set_preset(1));
}

#[allow(dead_code)]
fn focus_zone_command_gate<P: CompileTimeProfile + HasFocusZone>(camera: &Camera<P>) {
    plain(
        camera
            .focus()
            .set_zone(grafton_visca::command::FocusZone::Center),
    );
}

#[allow(dead_code)]
fn focus_zone_inquiry_gate<P: CompileTimeProfile + HasFocusZoneInquiry>(camera: &Camera<P>) {
    let _ = camera.focus().zone();
}

/// `system().version()` decodes the Sony 7-byte `CAM_VersionInq` layout, so it
/// is reachable only on profiles whose replies have that layout. PTZOptics G2
/// replies with an unsourced 2-byte payload and does not implement the marker.
#[allow(dead_code)]
fn version_inquiry_gate<P: CompileTimeProfile + HasVersionInquiry>(camera: &Camera<P>) {
    let _ = camera.system().version();
}

#[allow(dead_code)]
fn usb_audio_gate<P: CompileTimeProfile + HasUsbAudio>(camera: &Camera<P>) {
    let _ = camera.advanced().usb_audio_enabled();
    plain(camera.advanced().usb_audio_on());
    plain(camera.advanced().usb_audio_off());
}

#[allow(dead_code)]
fn ptzoptics_vendor_command_gates<P>(camera: &Camera<P>)
where
    P: CompileTimeProfile
        + HasPtzOpticsAntiFlicker
        + HasPtzOpticsSettingsSave
        + HasPtzOpticsPresetRecallSpeed
        + HasPtzOpticsMulticastStreaming
        + HasPtzOpticsNdiQuality,
{
    plain(
        camera
            .exposure()
            .set_anti_flicker(grafton_visca::command::AntiFlickerMode::Hz50),
    );
    let _ = camera.exposure().flicker_mode();
    plain(camera.system().save_settings());
    plain(camera.presets().set_recall_speed(
        grafton_visca::command::PresetRecallSpeed::new(12).expect("valid recall speed"),
    ));
    plain(camera.advanced().multicast_on());
    plain(camera.advanced().multicast_off());
    plain(
        camera
            .advanced()
            .set_ndi_quality(grafton_visca::types::NdiQuality::High),
    );
}

#[allow(dead_code)]
fn sony_spotlight_command_gates<P>(camera: &Camera<P>)
where
    P: CompileTimeProfile + HasSonySpotlight,
{
    plain(camera.exposure().spotlight_on());
    plain(camera.exposure().spotlight_off());
}

#[allow(dead_code)]
fn sony_auto_slow_shutter_command_gates<P>(camera: &Camera<P>)
where
    P: CompileTimeProfile + HasSonyAutoSlowShutter,
{
    plain(camera.exposure().auto_slow_shutter_on());
    plain(camera.exposure().auto_slow_shutter_off());
}

fn non_default(camera: &Camera<profile_fixtures::NonDefaultCompileTimeProfile>) {
    baseline(camera);
}

#[test]
fn static_camera_surface_is_profile_typed_and_non_default() {
    let _: fn(&Camera<PtzOpticsG2>) = baseline::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG2>) = exposure_mode_surface::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG2>) = base_image_surface::<PtzOpticsG2>;
    let _: fn(&Camera<SonyBRC300>) = backlight_surface::<SonyBRC300>;
    let _: fn(&Camera<PtzOpticsG2>) = all_base_inquiries::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG2>) = usb_audio_gate::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOptics30X>) = usb_audio_gate::<PtzOptics30X>;
    let _: fn(&Camera<PtzOpticsG2>) = ptzoptics_vendor_command_gates::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG3>) = ptzoptics_vendor_command_gates::<PtzOpticsG3>;
    let _: fn(&Camera<PtzOptics30X>) = ptzoptics_vendor_command_gates::<PtzOptics30X>;
    let _: fn(&Camera<SonyFR7>) = sony_spotlight_command_gates::<SonyFR7>;
    let _: fn(&Camera<SonyBRCH900>) = sony_spotlight_command_gates::<SonyBRCH900>;
    let _: fn(&Camera<SonyEVIH100>) = sony_auto_slow_shutter_command_gates::<SonyEVIH100>;
    let _: fn(&Camera<SonyBRC300>) = sony_auto_slow_shutter_command_gates::<SonyBRC300>;
    let _: fn(&Camera<PtzOpticsG2>) = focus_zone_inquiry_gate::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOptics30X>) = focus_zone_inquiry_gate::<PtzOptics30X>;
    let _: fn(&Camera<PtzOpticsG3>) = focus_zone_command_gate::<PtzOpticsG3>;
    let _: fn(&Camera<SonyFR7>) = version_inquiry_gate::<SonyFR7>;
    let _: fn(&Camera<SonyBRCH900>) = version_inquiry_gate::<SonyBRCH900>;
    let _: fn(&Camera<SonyEVIH100>) = version_inquiry_gate::<SonyEVIH100>;
    let _: fn(&Camera<SonyBRC300>) = version_inquiry_gate::<SonyBRC300>;
    let _: fn(&Camera<NearusBRC300>) = version_inquiry_gate::<NearusBRC300>;
    let _: fn(&Camera<GenericVisca>) = version_inquiry_gate::<GenericVisca>;
    let _: fn(&Camera<SonyFR7>) = sony_optional;
    let _: fn(&Camera<profile_fixtures::NonDefaultCompileTimeProfile>) = non_default;
}
