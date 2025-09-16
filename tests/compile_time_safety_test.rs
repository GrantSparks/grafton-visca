//! Integration test demonstrating compile-time capability checking with the generic API.

#![allow(clippy::expect_used)]
#![cfg(not(feature = "mode-async"))]

#[cfg(feature = "test-utils")]
use grafton_visca::{
    capabilities::*,
    mode::BlockingFutureExt,
    prelude::blocking::{GenericViscaCam, PtzOpticsG2Cam, SonyFR7Cam},
    testing::testkit::{helpers, ScriptedSyncTransport},
    Error, FocusControl, PanTiltControl, PowerControl, PresetNumber, PresetsControl, ZoomControl,
};

#[cfg(feature = "test-utils")]
#[test]
fn test_ptzoptics_g2_capabilities() -> Result<(), Error> {
    let transport = ScriptedSyncTransport::new(vec![
        helpers::auto_respond_step(),
        helpers::auto_respond_step(),
        helpers::auto_respond_step(),
        helpers::auto_respond_step(),
        helpers::auto_respond_step(),
    ]);

    let camera = PtzOpticsG2Cam::new_blocking(transport)?;

    assert!(camera.power_on().block().is_ok());
    assert!(camera.pan_tilt_home().block().is_ok());
    assert!(camera.zoom_stop().block().is_ok());
    assert!(camera.focus_auto().block().is_ok());
    assert!(camera
        .preset_recall(PresetNumber::new(1).unwrap())
        .block()
        .is_ok());
    Ok(())
}

#[cfg(feature = "test-utils")]
#[test]
fn test_sony_fr7_has_nd_filter() -> Result<(), Error> {
    let transport = ScriptedSyncTransport::new(vec![
        helpers::sony_auto_respond_step(),
        helpers::sony_auto_respond_step(),
        helpers::sony_auto_respond_step(),
    ]);

    let camera = SonyFR7Cam::new_blocking(transport)?;

    assert!(camera.power_on().block().is_ok());
    assert!(camera.pan_tilt_home().block().is_ok());
    assert!(camera.zoom_stop().block().is_ok());
    Ok(())
}

#[cfg(feature = "test-utils")]
#[test]
fn test_compile_time_capability_checking() {
    fn adjust_nd_filter<P, T>(_camera: &grafton_visca::BlockingCamera<P, T>) -> Result<(), Error>
    where
        P: Profile + NdFilter,
        T: grafton_visca::transport::SyncTransport + Send + Sync + 'static,
    {
        Ok(())
    }

    let fr7_transport = ScriptedSyncTransport::new(vec![helpers::auto_respond_step()]);
    let fr7_camera = SonyFR7Cam::new_blocking(fr7_transport).unwrap();
    let fr7 = grafton_visca::BlockingCamera::from(fr7_camera);

    let _g2_transport = ScriptedSyncTransport::new(vec![helpers::auto_respond_step()]);
    let _g2_camera = PtzOpticsG2Cam::new_blocking(_g2_transport).unwrap();
    let _g2 = grafton_visca::BlockingCamera::from(_g2_camera);

    assert!(adjust_nd_filter(&fr7).is_ok());
}

#[cfg(feature = "test-utils")]
#[test]
fn test_generic_functions_with_trait_bounds() {
    fn basic_control<P, T>(
        camera: &mut grafton_visca::camera::Camera<grafton_visca::mode::Blocking, P, T, ()>,
    ) -> Result<(), Error>
    where
        P: Profile + Default,
        T: grafton_visca::transport::SyncTransport,
        grafton_visca::camera::Camera<grafton_visca::mode::Blocking, P, T, ()>: PowerControl<Mode = grafton_visca::mode::Blocking>
            + ZoomControl<Mode = grafton_visca::mode::Blocking>,
    {
        use grafton_visca::mode::BlockingFutureExt;
        camera.power_on().block()?;
        camera.zoom_stop().block()?;
        Ok(())
    }

    fn motion_sync_control<P>(
        _camera: &grafton_visca::BlockingCamera<P, ScriptedSyncTransport>,
    ) -> Result<(), Error>
    where
        P: Profile + MotionSync + Default,
    {
        Ok(())
    }

    let g2_transport = ScriptedSyncTransport::new(vec![
        helpers::auto_respond_step(),
        helpers::auto_respond_step(),
    ]);
    let mut g2_camera = PtzOpticsG2Cam::new_blocking(g2_transport).unwrap();

    let g2_transport_wrapper = ScriptedSyncTransport::new(vec![helpers::auto_respond_step()]);
    let g2_camera_wrapper = PtzOpticsG2Cam::new_blocking(g2_transport_wrapper).unwrap();
    let g2_wrapper = grafton_visca::BlockingCamera::from(g2_camera_wrapper);

    let fr7_transport = ScriptedSyncTransport::new(vec![
        helpers::sony_auto_respond_step(),
        helpers::sony_auto_respond_step(),
    ]);
    let mut fr7_camera = SonyFR7Cam::new_blocking(fr7_transport).unwrap();

    let generic_transport = ScriptedSyncTransport::new(vec![
        helpers::auto_respond_step(),
        helpers::auto_respond_step(),
    ]);
    let mut generic_camera = GenericViscaCam::new_blocking(generic_transport).unwrap();

    println!("Testing G2 camera...");
    assert!(basic_control(&mut g2_camera).is_ok());

    println!("Testing FR7 camera...");
    let fr7_result = basic_control(&mut fr7_camera);
    if let Err(e) = &fr7_result {
        println!("FR7 error: {e:?}");
    }
    assert!(fr7_result.is_ok());

    println!("Testing generic camera...");
    assert!(basic_control(&mut generic_camera).is_ok());

    assert!(motion_sync_control(&g2_wrapper).is_ok());
}
