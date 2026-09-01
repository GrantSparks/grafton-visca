//! Compile-time coverage for the canonical async noun surface.

#![cfg(feature = "async")]

use std::future::Future;

use grafton_visca::{
    capabilities::{
        HasExposureMode, HasFocusZoneInquiry, HasImageFlip, HasImageProcessing,
        HasPtzOpticsAntiFlicker, HasPtzOpticsMulticastStreaming, HasPtzOpticsNdiQuality,
        HasPtzOpticsPresetRecallSpeed, HasPtzOpticsSettingsSave, HasSonyAutoSlowShutter,
        HasSonySpotlight, HasUsbAudio,
    },
    completion::{AppliedOnly, Targeted},
    profiles::{
        PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300, SonyBRCH900, SonyEVIH100, SonyFR7,
    },
    Camera, CompileTimeProfile, Error, Operation, Result,
};

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

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

fn baseline_surface<P: CompileTimeProfile>(camera: &Camera<P>) {
    plain(camera.power().on());
    plain(camera.power().off());
    plain(camera.white_balance().auto());
    plain(camera.menu().display(true));
    targeted(camera.pan_tilt().home());
    applied(camera.pan_tilt().stop());
    applied(camera.zoom().stop());
    applied(camera.focus().stop());
}

fn exposure_mode_surface<P: CompileTimeProfile + HasExposureMode>(camera: &Camera<P>) {
    plain(
        camera
            .exposure()
            .set_mode(grafton_visca::ExposureMode::Auto),
    );
    inquiry(camera.exposure().mode());
}

/// The image rows were originally included in `baseline_surface` while
/// `HasImageProcessing` was blanket-implemented for all profile metadata.
/// Keep the generic baseline useful for the metadata-only fixture, but make
/// the base image contract explicit now that Generic VISCA intentionally has
/// no typed image noun.
#[allow(dead_code)]
fn base_image_surface<P: CompileTimeProfile + HasImageProcessing>(camera: &Camera<P>) {
    plain(camera.image().freeze_on());
    plain(camera.image().freeze_off());
    inquiry(camera.image().defog_level());
}

#[allow(dead_code)]
fn sony_optional_surface(camera: &Camera<SonyFR7>) {
    plain(camera.tally().red_on());
    plain(camera.nd_filter().auto_on());
    applied(camera.zoom().tele());
    targeted(
        camera
            .zoom()
            .set_position(grafton_visca::types::ZoomPosition::new(0).unwrap()),
    );
}

#[allow(dead_code)]
fn ptzoptics_optional_surface(camera: &Camera<PtzOpticsG2>) {
    inquiry(camera.white_balance().sensitivity());
    inquiry(camera.image().flip_mode());
}

#[allow(dead_code)]
fn image_flip_surface<P: CompileTimeProfile + HasImageProcessing + HasImageFlip>(
    camera: &Camera<P>,
) {
    inquiry(camera.image().flip_mode());
}

#[allow(dead_code)]
fn focus_zone_inquiry_gate<P: CompileTimeProfile + HasFocusZoneInquiry>(camera: &Camera<P>) {
    inquiry(camera.focus().zone());
}

#[allow(dead_code)]
fn usb_audio_gate<P: CompileTimeProfile + HasUsbAudio>(camera: &Camera<P>) {
    inquiry(camera.advanced().usb_audio_enabled());
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
    inquiry(camera.exposure().flicker_mode());
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

#[allow(dead_code)]
fn non_default_profile_surface(camera: &Camera<profile_fixtures::NonDefaultCompileTimeProfile>) {
    baseline_surface(camera);
}

#[test]
fn noun_views_are_borrowed_profile_typed_handles() {
    fn assert_camera_is_profile_typed<P: CompileTimeProfile>(_: &Camera<P>) {}

    let _ = assert_camera_is_profile_typed::<PtzOpticsG2>;
    let _ = assert_camera_is_profile_typed::<SonyFR7>;
    let _: fn(&Camera<profile_fixtures::NonDefaultCompileTimeProfile>) =
        assert_camera_is_profile_typed::<profile_fixtures::NonDefaultCompileTimeProfile>;
    let _: fn(&Camera<PtzOpticsG2>) = base_image_surface::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG2>) = exposure_mode_surface::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG2>) = image_flip_surface::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG2>) = focus_zone_inquiry_gate::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOptics30X>) = focus_zone_inquiry_gate::<PtzOptics30X>;
    let _: fn(&Camera<PtzOpticsG2>) = usb_audio_gate::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOptics30X>) = usb_audio_gate::<PtzOptics30X>;
    let _: fn(&Camera<PtzOpticsG2>) = ptzoptics_vendor_command_gates::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG3>) = ptzoptics_vendor_command_gates::<PtzOpticsG3>;
    let _: fn(&Camera<PtzOptics30X>) = ptzoptics_vendor_command_gates::<PtzOptics30X>;
    let _: fn(&Camera<SonyFR7>) = sony_spotlight_command_gates::<SonyFR7>;
    let _: fn(&Camera<SonyBRCH900>) = sony_spotlight_command_gates::<SonyBRCH900>;
    let _: fn(&Camera<SonyEVIH100>) = sony_auto_slow_shutter_command_gates::<SonyEVIH100>;
    let _: fn(&Camera<SonyBRC300>) = sony_auto_slow_shutter_command_gates::<SonyBRC300>;
}

#[allow(dead_code)]
fn _error_type_is_public(_: Error) {}
