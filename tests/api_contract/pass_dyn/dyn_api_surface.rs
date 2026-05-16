#![allow(dead_code)]

use std::{marker::PhantomData, time::Duration};

use grafton_visca::{
    capabilities::Capabilities,
    dynapi::{
        DynCameraControl, DynFocusControl, DynMotionControl, DynPanTiltControl, DynPresetsControl,
        DynZoomControl, InFlightDyn, IntoDynCamera,
    },
    mode::BoxFuture,
    types::ZoomPosition,
    Error, StateCache, UnitInterval, ZoomDomain,
};

fn assert_dyn_camera_surface(camera: &dyn DynCameraControl) {
    let _: &Capabilities = camera.capabilities();
    let _: &dyn DynPanTiltControl = camera.pan_tilt();
    let _: &dyn DynZoomControl = camera.zoom();
    let _: &dyn DynFocusControl = camera.focus();
    let _: &dyn DynPresetsControl = camera.presets();
    let _: &dyn DynMotionControl = camera.motion();
    let _: &StateCache = camera.state_cache();
}

fn assert_dyn_motion_surface(motion: &dyn DynMotionControl) {
    let _: BoxFuture<'_, Result<(), Error>> = motion.stop_all_motion();
    let _: BoxFuture<'_, Result<(), Error>> = motion.await_idle(Duration::from_secs(1));
    let _: BoxFuture<'_, Result<(), Error>> = motion.await_pan_tilt_idle(Duration::from_secs(1));
    let _: BoxFuture<'_, Result<(), Error>> = motion.await_zoom_idle(Duration::from_secs(1));
    let _: BoxFuture<'_, Result<(), Error>> = motion.await_focus_idle(Duration::from_secs(1));
}

fn assert_dyn_zoom_surface(zoom: &dyn DynZoomControl) {
    let _: BoxFuture<'_, Result<(), Error>> =
        zoom.set_zoom(ZoomPosition::MIN, Some(Duration::from_secs(1)));
    let _: BoxFuture<'_, Result<InFlightDyn, Error>> = zoom.set_zoom_op(ZoomPosition::MIN);
    let _: BoxFuture<'_, Result<(), Error>> =
        zoom.set_zoom_normalized(UnitInterval::ZERO, Some(Duration::from_secs(1)));
    let _: BoxFuture<'_, Result<(), Error>> = zoom.set_zoom_normalized_in_domain(
        UnitInterval::ONE,
        ZoomDomain::OpticalPlusDigital,
        Some(Duration::from_secs(1)),
    );
}

fn assert_into_dyn<T: IntoDynCamera>() {}

fn main() {
    let _: PhantomData<Box<dyn DynCameraControl + Send + Sync>> = PhantomData;
    let _: PhantomData<fn() -> InFlightDyn> = PhantomData;
    let _: PhantomData<fn(&dyn DynCameraControl)> = PhantomData;
    let _ = assert_dyn_camera_surface;
    let _ = assert_dyn_motion_surface;
    let _ = assert_dyn_zoom_surface;
}
