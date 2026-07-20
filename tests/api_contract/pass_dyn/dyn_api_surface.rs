#![allow(dead_code)]

use std::{marker::PhantomData, time::Duration};

use grafton_visca::{
    capabilities::Capabilities,
    command::{PanTiltDirection, PanTiltLimitCorner, PresetNumber},
    dynapi::{
        DynCameraControl, DynFocusControl, DynMotionControl, DynPanTiltControl, DynPresetsControl,
        DynZoomControl, InFlightDyn, IntoDynCamera, OperationCategory,
    },
    mode::BoxFuture,
    types::{
        FocusPosition, PanPosition, PanSpeed, SpeedLevel, TiltPosition, TiltSpeed, ZoomPosition,
        ZoomSpeed,
    },
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
    let _: BoxFuture<'_, Result<(), Error>> = zoom.zoom_tele(
        Some(ZoomSpeed::new(1).unwrap()),
        Some(Duration::from_secs(1)),
    );
    let _: BoxFuture<'_, Result<(), Error>> = zoom.zoom_wide(None, None);
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

fn assert_dyn_pan_tilt_surface(
    pan_tilt: &dyn DynPanTiltControl,
    pan: PanPosition,
    tilt: TiltPosition,
) {
    let timeout = Some(Duration::from_secs(1));
    let _: BoxFuture<'_, Result<(), Error>> = pan_tilt.pan_tilt_home(timeout);
    let _: BoxFuture<'_, Result<InFlightDyn, Error>> = pan_tilt.pan_tilt_home_op();
    let _: BoxFuture<'_, Result<(), Error>> =
        pan_tilt.pan_tilt_absolute(0.0, 0.0, SpeedLevel::Medium, timeout);
    let _: BoxFuture<'_, Result<InFlightDyn, Error>> =
        pan_tilt.pan_tilt_absolute_op(0.0, 0.0, SpeedLevel::Medium);
    let _: BoxFuture<'_, Result<(), Error>> =
        pan_tilt.pan_tilt_relative(0.0, 0.0, SpeedLevel::Medium, timeout);
    let _: BoxFuture<'_, Result<InFlightDyn, Error>> =
        pan_tilt.pan_tilt_relative_op(0.0, 0.0, SpeedLevel::Medium);
    let _: BoxFuture<'_, Result<(), Error>> = pan_tilt.pan_tilt_reset(timeout);
    let _: BoxFuture<'_, Result<InFlightDyn, Error>> = pan_tilt.pan_tilt_reset_op();
    let _: BoxFuture<'_, Result<(), Error>> = pan_tilt.pan_tilt_move(
        PanTiltDirection::Stop,
        PanSpeed::new(1).unwrap(),
        TiltSpeed::new(1).unwrap(),
    );
    let _: BoxFuture<'_, Result<(), Error>> =
        pan_tilt.pan_tilt_limit_set(PanTiltLimitCorner::UpRight, pan, tilt);
}

fn assert_dyn_focus_surface(focus: &dyn DynFocusControl, position: FocusPosition) {
    let _: BoxFuture<'_, Result<(), Error>> =
        focus.set_focus(position, Some(Duration::from_secs(1)));
    let _: BoxFuture<'_, Result<InFlightDyn, Error>> = focus.set_focus_op(position);
}

fn assert_dyn_preset_surface(presets: &dyn DynPresetsControl, preset: PresetNumber) {
    let _: BoxFuture<'_, Result<(), Error>> =
        presets.preset_recall(preset, Some(Duration::from_secs(1)));
    let _: BoxFuture<'_, Result<InFlightDyn, Error>> = presets.preset_recall_op(preset);
}

fn assert_dyn_handle_surface(handle: InFlightDyn) {
    let _: grafton_visca::camera::CommandId = handle.id();
    let _: OperationCategory = handle.category();
    let _: BoxFuture<'_, Result<(), Error>> = handle.cancel();
    let _: BoxFuture<'_, Result<(), Error>> = handle.await_applied(Duration::from_secs(1));
    let _: BoxFuture<'_, Result<(), Error>> = handle.await_settled(Duration::from_secs(1));
    let _: BoxFuture<'_, Result<(), Error>> = handle.await_completion(Duration::from_secs(1));
    handle.detach();
}

fn assert_into_dyn<T: IntoDynCamera>() {}

fn main() {
    let _: PhantomData<Box<dyn DynCameraControl + Send + Sync>> = PhantomData;
    let _: PhantomData<fn() -> InFlightDyn> = PhantomData;
    let _: PhantomData<fn(&dyn DynCameraControl)> = PhantomData;
    let _ = assert_dyn_camera_surface;
    let _ = assert_dyn_motion_surface;
    let _ = assert_dyn_pan_tilt_surface;
    let _ = assert_dyn_zoom_surface;
    let _ = assert_dyn_focus_surface;
    let _ = assert_dyn_preset_surface;
    let _ = assert_dyn_handle_surface;
}
