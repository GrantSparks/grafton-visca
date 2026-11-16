//! Compile-time contracts for the canonical blocking noun facade.

#![cfg(feature = "blocking")]

use grafton_visca_test_support::profile_fixtures;

use grafton_visca::{
    blocking::{Camera, Operation},
    capabilities::{
        HasAntiFlicker, HasAutoFocusSensitivity, HasAutoSlowShutter,
        HasAutoWhiteBalanceSensitivity, HasBacklightCompensation, HasBrightness,
        HasColorTemperature, HasColorTemperatureInquiry, HasContrast, HasExposure,
        HasExposureCompensation, HasExposureMode, HasFocus, HasFocusNearLimit, HasFocusZone,
        HasFocusZoneInquiry, HasGamma, HasHue, HasImage, HasImageFlip, HasIris, HasLuminance,
        HasMotionSync, HasMulticastStreaming, HasNdFilter, HasNdiQuality, HasNoiseReduction2D,
        HasNoiseReduction2DMode, HasNoiseReduction3D, HasPanTilt, HasPictureEffect, HasPower,
        HasPresetRecallSpeed, HasPresets, HasRgbGain, HasRgbTuning, HasSaturation, HasSettingsSave,
        HasSharpness, HasSharpnessMode, HasSpotlight, HasTally, HasUsbAudio, HasVersionInquiry,
        HasWhiteBalance, HasWideDynamicRange, HasZoom,
    },
    operation::{AppliedOnly, Targeted},
    profiles::{
        CompileTimeProfile, GenericVisca, NearusBrc300, PtzOptics30x, PtzOpticsG2, PtzOpticsG3,
        SonyBrc300, SonyBrcH900, SonyEviH100, SonyFr7,
    },
    types::{MotionSyncSpeed, NdFilterDensity},
    Result,
};

fn plain(_: Result<()>) {}

fn source_refinements<P>(camera: &Camera<P>)
where
    P: CompileTimeProfile
        + grafton_visca::capabilities::HasPower
        + grafton_visca::capabilities::HasFocus
        + grafton_visca::capabilities::HasPresets
        + grafton_visca::capabilities::HasExposure
        + grafton_visca::capabilities::HasWhiteBalance
        + grafton_visca::capabilities::HasMenu
        + grafton_visca::capabilities::HasPowerStandby
        + grafton_visca::capabilities::HasFocusInfinity
        + grafton_visca::capabilities::HasPresetMemory
        + grafton_visca::capabilities::HasShutter
        + grafton_visca::capabilities::HasDirectShutter
        + grafton_visca::capabilities::HasGain
        + grafton_visca::capabilities::HasGainLimit
        + grafton_visca::capabilities::HasMenuDisplay
        + grafton_visca::capabilities::HasMenuEnterReturn
        + grafton_visca::capabilities::HasMenuDisplayInquiry,
{
    plain(camera.power().standby());
    targeted(camera.focus().infinity());
    plain(
        camera
            .presets()
            .store(grafton_visca::types::PresetNumber::MIN),
    );
    plain(
        camera
            .presets()
            .clear(grafton_visca::types::PresetNumber::MIN),
    );
    plain(camera.exposure().shutter_reset());
    plain(camera.exposure().shutter_up());
    plain(camera.exposure().shutter_down());
    plain(
        camera
            .exposure()
            .set_shutter(grafton_visca::types::ShutterSpeed::new(0).expect("valid value")),
    );
    let _ = camera.exposure().shutter();
    plain(camera.exposure().gain_reset());
    plain(camera.exposure().gain_up());
    plain(camera.exposure().gain_down());
    plain(
        camera
            .exposure()
            .set_gain(grafton_visca::types::GainLevel::MIN),
    );
    let _ = camera.exposure().gain();
    plain(
        camera
            .exposure()
            .set_gain_limit(grafton_visca::types::GainLimit::MIN),
    );
    let _ = camera.exposure().gain_limit();
    plain(
        camera
            .white_balance()
            .set_mode(grafton_visca::types::WhiteBalanceMode::Auto),
    );
    plain(
        camera
            .white_balance()
            .set_mode(grafton_visca::types::WhiteBalanceMode::Indoor),
    );
    plain(
        camera
            .white_balance()
            .set_mode(grafton_visca::types::WhiteBalanceMode::Outdoor),
    );
    plain(camera.menu().set_visible(true));
    plain(camera.menu().enter());
    plain(camera.menu().back());
    let _ = camera.menu().visible();
}

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
        + HasPresets,
{
    plain(camera.power().on());

    targeted(camera.pan_tilt().home());
    applied(camera.pan_tilt().stop());
    applied(camera.zoom().stop());
    applied(camera.focus().stop());

    // The safety/observation surface is intentionally a separate noun.
    let _ = camera.motion();
}

fn exposure_mode_surface<
    P: CompileTimeProfile + grafton_visca::capabilities::HasExposure + HasExposureMode,
>(
    camera: &Camera<P>,
) {
    plain(
        camera
            .exposure()
            .set_mode(grafton_visca::types::ExposureMode::Auto),
    );
    let _ = camera.exposure().mode();
}

/// These rows used to be included in `baseline` because
/// `HasImage` was blanket-implemented from image metadata. Generic
/// VISCA's all-empty metadata deliberately does not imply the typed image
/// noun, so the explicit base marker now owns this surface.
fn base_image_surface<P: CompileTimeProfile + HasImage>(camera: &Camera<P>) {
    let _ = camera.image();
}

fn backlight_surface<P>(camera: &Camera<P>)
where
    P: CompileTimeProfile + grafton_visca::capabilities::HasExposure + HasBacklightCompensation,
{
    let _ = camera.exposure().backlight_compensation();
    plain(camera.exposure().set_backlight_compensation(true));
}

fn all_base_inquiries<
    P: CompileTimeProfile
        + grafton_visca::capabilities::HasPower
        + grafton_visca::capabilities::HasZoom
        + grafton_visca::capabilities::HasFocus
        + grafton_visca::capabilities::HasPanTilt
        + grafton_visca::capabilities::HasExposure
        + grafton_visca::capabilities::HasWhiteBalance
        + grafton_visca::capabilities::HasMenu
        + grafton_visca::capabilities::HasShutter
        + grafton_visca::capabilities::HasGain
        + grafton_visca::capabilities::HasGainLimit
        + grafton_visca::capabilities::HasMenuDisplayInquiry,
>(
    camera: &Camera<P>,
) {
    let _ = camera.power().status();
    let _ = camera.zoom().position();
    let _ = camera.pan_tilt().position();
    let _ = camera.focus().position();
    let _ = camera.focus().mode();
    let _ = camera.exposure().shutter();
    let _ = camera.exposure().gain();
    let _ = camera.exposure().gain_limit();
    let _ = camera.white_balance().mode();
    let _ = camera.menu().visible();
}

fn noise_2d_gate<P: CompileTimeProfile + HasImage + HasNoiseReduction2DMode>(camera: &Camera<P>) {
    let _ = camera.image().noise_reduction_2d_mode();
}

fn combined_flip_inquiry(camera: &Camera<PtzOpticsG2>) {
    let _ = camera.image().flip_mode();
}

fn all_optional_inquiries<P>(camera: &Camera<P>)
where
    P: CompileTimeProfile
        + grafton_visca::capabilities::HasFocus
        + grafton_visca::capabilities::HasExposure
        + grafton_visca::capabilities::HasWhiteBalance
        + HasAutoFocusSensitivity
        + HasAutoWhiteBalanceSensitivity
        + HasBacklightCompensation
        + HasBrightness
        + HasColorTemperature
        + HasColorTemperatureInquiry
        + HasContrast
        + HasExposureCompensation
        + HasFocusNearLimit
        + HasFocusZoneInquiry
        + HasGamma
        + HasHue
        + HasImageFlip
        + HasImage
        + HasIris
        + HasLuminance
        + HasMotionSync
        + HasNdFilter
        + HasNoiseReduction2D
        + HasNoiseReduction2DMode
        + HasNoiseReduction3D
        + HasPictureEffect
        + HasRgbGain
        + HasRgbTuning
        + HasSaturation
        + HasSharpness
        + HasSharpnessMode
        + HasTally
        + HasWideDynamicRange,
{
    let _ = camera.focus().near_limit();
    let _ = camera.focus().zone();
    let _ = camera.focus().sensitivity();
    let _ = camera.exposure().compensation();
    let _ = camera.exposure().compensation_enabled();
    let _ = camera.exposure().compensation();
    let _ = camera.exposure().iris();
    let _ = camera.exposure().brightness();
    let _ = camera.image().saturation();
    let _ = camera.image().hue();
    let _ = camera.image().luminance();
    let _ = camera.image().contrast();
    let _ = camera.image().gamma();
    let _ = camera.image().sharpness_mode();
    let _ = camera.image().sharpness();
    let _ = camera.exposure().backlight_compensation();
    let _ = camera.image().noise_reduction_2d();
    let _ = camera.image().noise_reduction_2d_mode();
    let _ = camera.image().noise_reduction_3d();
    let _ = camera.image().flip();
    let _ = camera.image().picture_effect();
    let _ = camera.white_balance().sensitivity();
    let _ = camera.white_balance().color_temperature();
    let _ = camera.white_balance().red_gain();
    let _ = camera.white_balance().blue_gain();
    let _ = camera.white_balance().red_tuning();
    let _ = camera.white_balance().blue_tuning();
    let _ = camera.tally().red();
    let _ = camera.tally().green();
    let _ = camera.nd_filter().density();
    let _ = camera.nd_filter().preset();
}

fn sony_optional(camera: &Camera<SonyFr7>) {
    plain(camera.tally().set_red(true));
    plain(camera.tally().set_green(false));
    targeted(
        camera
            .nd_filter()
            .set_density(NdFilterDensity::new(1).expect("ND value")),
    );
    targeted(camera.nd_filter().density_up());
    applied(camera.zoom().tele());
}

fn awb_gate<
    P: CompileTimeProfile
        + grafton_visca::capabilities::HasWhiteBalance
        + HasAutoWhiteBalanceSensitivity,
>(
    camera: &Camera<P>,
) {
    plain(camera.white_balance().sensitivity().map(|_| ()));
}

fn motion_gate<P: CompileTimeProfile + HasMotionSync>(camera: &Camera<P>) {
    plain(camera.motion_sync().set_enabled(true));
    plain(camera.motion_sync().set_speed(MotionSyncSpeed::SLOW));
}

fn focus_zone_command_gate<
    P: CompileTimeProfile + grafton_visca::capabilities::HasFocus + HasFocusZone,
>(
    camera: &Camera<P>,
) {
    plain(
        camera
            .focus()
            .set_zone(grafton_visca::types::FocusZone::Center),
    );
}

fn focus_zone_inquiry_gate<
    P: CompileTimeProfile + grafton_visca::capabilities::HasFocus + HasFocusZoneInquiry,
>(
    camera: &Camera<P>,
) {
    let _ = camera.focus().zone();
}

/// `system().version()` decodes the Sony 7-byte `CAM_VersionInq` layout, so it
/// is reachable only on profiles whose replies have that layout. PTZOptics G2
/// replies with an unsourced 2-byte payload and does not implement the marker.
fn version_inquiry_gate<P: CompileTimeProfile + HasVersionInquiry>(camera: &Camera<P>) {
    let _ = camera.system().version();
}

fn usb_audio_gate<P: CompileTimeProfile + HasUsbAudio>(camera: &Camera<P>) {
    let _ = camera.streaming().usb_audio();
    plain(camera.streaming().set_usb_audio(true));
    plain(camera.streaming().set_usb_audio(false));
}

fn ptzoptics_vendor_command_gates<P>(camera: &Camera<P>)
where
    P: CompileTimeProfile
        + grafton_visca::capabilities::HasPresets
        + grafton_visca::capabilities::HasExposure
        + HasAntiFlicker
        + HasSettingsSave
        + HasPresetRecallSpeed
        + HasMulticastStreaming
        + HasNdiQuality,
{
    plain(
        camera
            .exposure()
            .set_anti_flicker(grafton_visca::types::AntiFlickerMode::Hz50),
    );
    let _ = camera.exposure().anti_flicker();
    plain(camera.system().save_settings());
    plain(
        camera.presets().set_recall_speed(
            grafton_visca::types::PresetSpeed::new(12).expect("valid recall speed"),
        ),
    );
    plain(camera.streaming().set_multicast(true));
    plain(camera.streaming().set_multicast(false));
    plain(
        camera
            .streaming()
            .set_ndi_quality(grafton_visca::types::NdiQuality::High),
    );
}

fn sony_spotlight_command_gates<P>(camera: &Camera<P>)
where
    P: CompileTimeProfile + grafton_visca::capabilities::HasExposure + HasSpotlight,
{
    plain(camera.exposure().set_spotlight(true));
    plain(camera.exposure().set_spotlight(false));
}

fn sony_auto_slow_shutter_command_gates<P>(camera: &Camera<P>)
where
    P: CompileTimeProfile + grafton_visca::capabilities::HasExposure + HasAutoSlowShutter,
{
    plain(camera.exposure().set_auto_slow_shutter(true));
    plain(camera.exposure().set_auto_slow_shutter(false));
}

fn non_default(camera: &Camera<profile_fixtures::NonDefaultCompileTimeProfile>) {
    baseline(camera);
}

#[test]
fn static_camera_surface_is_profile_typed_and_non_default() {
    let _: fn(&Camera<profile_fixtures::OptionalSurfacesTypedSupport>) =
        source_refinements::<profile_fixtures::OptionalSurfacesTypedSupport>;
    let _: fn(&Camera<PtzOpticsG2>) = baseline::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG2>) = combined_flip_inquiry;
    let _: fn(&Camera<PtzOpticsG2>) = noise_2d_gate::<PtzOpticsG2>;
    let _: fn(&Camera<profile_fixtures::OptionalSurfacesTypedSupport>) =
        all_optional_inquiries::<profile_fixtures::OptionalSurfacesTypedSupport>;
    let _: fn(&Camera<PtzOpticsG2>) = awb_gate::<PtzOpticsG2>;
    let _: fn(&Camera<profile_fixtures::MotionSyncTypedSupport>) =
        motion_gate::<profile_fixtures::MotionSyncTypedSupport>;
    let _: fn(&Camera<PtzOpticsG2>) = exposure_mode_surface::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG2>) = base_image_surface::<PtzOpticsG2>;
    let _: fn(&Camera<SonyBrc300>) = backlight_surface::<SonyBrc300>;
    let _: fn(&Camera<PtzOpticsG2>) = all_base_inquiries::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG2>) = usb_audio_gate::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOptics30x>) = usb_audio_gate::<PtzOptics30x>;
    let _: fn(&Camera<PtzOpticsG2>) = ptzoptics_vendor_command_gates::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG3>) = ptzoptics_vendor_command_gates::<PtzOpticsG3>;
    let _: fn(&Camera<PtzOptics30x>) = ptzoptics_vendor_command_gates::<PtzOptics30x>;
    let _: fn(&Camera<SonyFr7>) = sony_spotlight_command_gates::<SonyFr7>;
    let _: fn(&Camera<SonyEviH100>) = sony_auto_slow_shutter_command_gates::<SonyEviH100>;
    let _: fn(&Camera<SonyBrc300>) = sony_auto_slow_shutter_command_gates::<SonyBrc300>;
    let _: fn(&Camera<PtzOpticsG2>) = focus_zone_inquiry_gate::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOptics30x>) = focus_zone_inquiry_gate::<PtzOptics30x>;
    let _: fn(&Camera<PtzOpticsG3>) = focus_zone_command_gate::<PtzOpticsG3>;
    let _: fn(&Camera<SonyFr7>) = version_inquiry_gate::<SonyFr7>;
    let _: fn(&Camera<SonyBrcH900>) = version_inquiry_gate::<SonyBrcH900>;
    let _: fn(&Camera<SonyEviH100>) = version_inquiry_gate::<SonyEviH100>;
    let _: fn(&Camera<SonyBrc300>) = version_inquiry_gate::<SonyBrc300>;
    let _: fn(&Camera<NearusBrc300>) = version_inquiry_gate::<NearusBrc300>;
    let _: fn(&Camera<GenericVisca>) = version_inquiry_gate::<GenericVisca>;
    let _: fn(&Camera<SonyFr7>) = sony_optional;
    let _: fn(&Camera<profile_fixtures::NonDefaultCompileTimeProfile>) = non_default;
}
