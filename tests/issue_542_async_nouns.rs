//! Compile-time coverage for the canonical async noun surface.

#![cfg(feature = "async")]

use std::future::Future;

use grafton_visca::{
    capabilities::{
        HasAntiFlicker, HasAutoSlowShutter, HasBacklightCompensation, HasExposureMode,
        HasFocusZoneInquiry, HasImage, HasImageFlip, HasMulticastStreaming, HasNdiQuality,
        HasPresetRecallSpeed, HasSettingsSave, HasSpotlight, HasUsbAudio,
    },
    operation::{AppliedOnly, Targeted},
    profiles::{
        CompileTimeProfile, PtzOptics30x, PtzOpticsG2, PtzOpticsG3, SonyBrc300, SonyEviH100,
        SonyFr7,
    },
    r#async::{Camera, Operation},
    Error, Result,
};

use grafton_visca_test_support::profile_fixtures;

fn plain<F>(_: F)
where
    F: Future<Output = Result<()>>,
{
}

fn inquiry<T, F>(_: F)
where
    F: Future<Output = Result<T>>,
{
}

fn targeted<F>(_: F)
where
    F: Future<Output = Result<Operation<Targeted>>>,
{
}

fn applied<F>(_: F)
where
    F: Future<Output = Result<Operation<AppliedOnly>>>,
{
}

fn baseline_surface<
    P: CompileTimeProfile
        + grafton_visca::capabilities::HasPower
        + grafton_visca::capabilities::HasZoom
        + grafton_visca::capabilities::HasFocus
        + grafton_visca::capabilities::HasPanTilt,
>(
    camera: &Camera<P>,
) {
    plain(camera.power().on());
    targeted(camera.pan_tilt().home());
    applied(camera.pan_tilt().stop());
    applied(camera.zoom().stop());
    applied(camera.focus().stop());
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
    inquiry(camera.exposure().mode());
}

/// The image rows were originally included in `baseline_surface` while
/// `HasImage` was blanket-implemented for all profile metadata.
/// Keep the generic baseline useful for the metadata-only fixture, but make
/// the base image contract explicit now that Generic VISCA intentionally has
/// no typed image noun.
fn base_image_surface<P: CompileTimeProfile + HasImage>(camera: &Camera<P>) {
    let _ = camera.image();
}

fn backlight_surface<P>(camera: &Camera<P>)
where
    P: CompileTimeProfile + grafton_visca::capabilities::HasExposure + HasBacklightCompensation,
{
    inquiry(camera.exposure().backlight_compensation());
    plain(camera.exposure().set_backlight_compensation(true));
}

fn sony_optional_surface(camera: &Camera<SonyFr7>) {
    plain(camera.tally().set_red(true));
    plain(camera.nd_filter().set_auto(true));
    applied(camera.zoom().tele());
    targeted(
        camera
            .zoom()
            .set_position(grafton_visca::types::ZoomPosition::new(0).unwrap()),
    );
}

fn ptzoptics_optional_surface(camera: &Camera<PtzOpticsG2>) {
    inquiry(camera.white_balance().sensitivity());
    inquiry(camera.image().flip_mode());
}

fn image_flip_surface<
    P: CompileTimeProfile
        + HasImage
        + HasImageFlip
        + grafton_visca::capabilities::HasCombinedImageFlip,
>(
    camera: &Camera<P>,
) {
    inquiry(camera.image().flip_mode());
}

fn focus_zone_inquiry_gate<
    P: CompileTimeProfile + grafton_visca::capabilities::HasFocus + HasFocusZoneInquiry,
>(
    camera: &Camera<P>,
) {
    inquiry(camera.focus().zone());
}

fn usb_audio_gate<P: CompileTimeProfile + HasUsbAudio>(camera: &Camera<P>) {
    inquiry(camera.streaming().usb_audio());
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
    inquiry(camera.exposure().anti_flicker());
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

fn non_default_profile_surface(camera: &Camera<profile_fixtures::NonDefaultCompileTimeProfile>) {
    baseline_surface(camera);
}

#[test]
fn noun_views_are_borrowed_profile_typed_handles() {
    fn assert_camera_is_profile_typed<P: CompileTimeProfile>(_: &Camera<P>) {}

    let _ = assert_camera_is_profile_typed::<PtzOpticsG2>;
    let _ = assert_camera_is_profile_typed::<SonyFr7>;
    let _: fn(&Camera<profile_fixtures::NonDefaultCompileTimeProfile>) =
        assert_camera_is_profile_typed::<profile_fixtures::NonDefaultCompileTimeProfile>;
    let _: fn(&Camera<PtzOpticsG2>) = base_image_surface::<PtzOpticsG2>;
    let _: fn(&Camera<SonyFr7>) = sony_optional_surface;
    let _: fn(&Camera<PtzOpticsG2>) = ptzoptics_optional_surface;
    let _: fn(&Camera<profile_fixtures::NonDefaultCompileTimeProfile>) =
        non_default_profile_surface;
    let _: fn(&Camera<SonyBrc300>) = backlight_surface::<SonyBrc300>;
    let _: fn(&Camera<PtzOpticsG2>) = exposure_mode_surface::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG2>) = image_flip_surface::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG2>) = focus_zone_inquiry_gate::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOptics30x>) = focus_zone_inquiry_gate::<PtzOptics30x>;
    let _: fn(&Camera<PtzOpticsG2>) = usb_audio_gate::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOptics30x>) = usb_audio_gate::<PtzOptics30x>;
    let _: fn(&Camera<PtzOpticsG2>) = ptzoptics_vendor_command_gates::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG3>) = ptzoptics_vendor_command_gates::<PtzOpticsG3>;
    let _: fn(&Camera<PtzOptics30x>) = ptzoptics_vendor_command_gates::<PtzOptics30x>;
    let _: fn(&Camera<SonyFr7>) = sony_spotlight_command_gates::<SonyFr7>;
    let _: fn(&Camera<SonyEviH100>) = sony_auto_slow_shutter_command_gates::<SonyEviH100>;
    let _: fn(&Camera<SonyBrc300>) = sony_auto_slow_shutter_command_gates::<SonyBrc300>;
}

fn _error_type_is_public(_: Error) {}
