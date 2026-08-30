//! Compile-time coverage for the canonical async noun surface.

#![cfg(feature = "async")]

use std::future::Future;

use grafton_visca::{
    capabilities::{HasFocusZoneInquiry, HasImageFlip, HasImageProcessing, HasUsbAudio},
    completion::{AppliedOnly, Targeted},
    profiles::{PtzOptics30X, PtzOpticsG2, SonyFR7},
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
    plain(camera.system().save_settings());
    plain(
        camera
            .exposure()
            .set_mode(grafton_visca::ExposureMode::Auto),
    );
    plain(camera.white_balance().auto());
    plain(camera.menu().display(true));
    plain(camera.advanced().multicast_on());
    inquiry(camera.exposure().flicker_mode());
    targeted(camera.pan_tilt().home());
    applied(camera.pan_tilt().stop());
    applied(camera.zoom().stop());
    applied(camera.focus().stop());
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
    let _: fn(&Camera<PtzOpticsG2>) = image_flip_surface::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG2>) = focus_zone_inquiry_gate::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOptics30X>) = focus_zone_inquiry_gate::<PtzOptics30X>;
    let _: fn(&Camera<PtzOpticsG2>) = usb_audio_gate::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOptics30X>) = usb_audio_gate::<PtzOptics30X>;
}

#[allow(dead_code)]
fn _error_type_is_public(_: Error) {}
