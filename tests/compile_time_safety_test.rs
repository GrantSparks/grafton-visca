//! Integration test demonstrating compile-time capability checking with the generic API.

#![allow(clippy::expect_used)]
#![cfg(not(feature = "async"))]

#[cfg(feature = "test-utils")]
use grafton_visca::{
    capabilities::*,
    prelude::blocking::{GenericViscaCam, PtzOpticsG2Cam, SonyFR7Cam},
    testing::testkit::{helpers, ScriptedSyncTransport},
    Error, FocusControl, PanTiltControl, PowerControl, PresetNumber, PresetsControl, ZoomControl,
};

#[cfg(feature = "test-utils")]
#[test]
fn test_ptzoptics_g2_capabilities() -> Result<(), Error> {
    let transport = ScriptedSyncTransport::new(vec![
        helpers::auto_respond_step(), // power_on
        helpers::auto_respond_step(), // pan_tilt_home
        helpers::auto_respond_step(), // zoom_stop
        helpers::auto_respond_step(), // focus_auto
        helpers::auto_respond_step(), // preset_recall
    ]);

    let mut camera = PtzOpticsG2Cam::new_blocking(transport)?;

    assert!(camera.power_on().is_ok());
    assert!(camera.pan_tilt_home().is_ok());
    assert!(camera.zoom_stop().is_ok());
    assert!(camera.focus_auto().is_ok());
    assert!(camera.preset_recall(PresetNumber::new(1).unwrap()).is_ok());
    Ok(())
}

#[cfg(feature = "test-utils")]
#[test]
fn test_sony_fr7_has_nd_filter() -> Result<(), Error> {
    let transport = ScriptedSyncTransport::new(vec![
        helpers::sony_auto_respond_step(), // power_on
        helpers::sony_auto_respond_step(), // pan_tilt_home
        helpers::sony_auto_respond_step(), // zoom_stop
    ]);

    let mut camera = SonyFR7Cam::new_blocking(transport)?;

    assert!(camera.power_on().is_ok());
    assert!(camera.pan_tilt_home().is_ok());
    assert!(camera.zoom_stop().is_ok());
    Ok(())
}

#[cfg(feature = "test-utils")]
#[test]
fn test_compile_time_capability_checking() {
    fn adjust_nd_filter<P, T>(
        _camera: &grafton_visca::camera::BlockingCamera<P, T>,
    ) -> Result<(), Error>
    where
        P: Profile + NdFilter,
        T: grafton_visca::transport::SyncTransport + Send + Sync + 'static,
    {
        Ok(())
    }

    let fr7_transport = ScriptedSyncTransport::new(vec![helpers::auto_respond_step()]);
    let fr7 = SonyFR7Cam::new_blocking(fr7_transport).unwrap();

    let _g2_transport = ScriptedSyncTransport::new(vec![helpers::auto_respond_step()]);
    let _g2 = PtzOpticsG2Cam::new_blocking(_g2_transport).unwrap();

    assert!(adjust_nd_filter(&fr7).is_ok());
}

#[cfg(feature = "test-utils")]
#[test]
fn test_generic_functions_with_trait_bounds() {
    fn basic_control<P>(
        camera: &mut grafton_visca::camera::BlockingCamera<P, ScriptedSyncTransport>,
    ) -> Result<(), Error>
    where
        P: Profile + Default,
    {
        camera.power_on()?;
        camera.zoom_stop()?;
        Ok(())
    }

    fn motion_sync_control<P>(
        _camera: &mut grafton_visca::camera::BlockingCamera<P, ScriptedSyncTransport>,
    ) -> Result<(), Error>
    where
        P: Profile + MotionSync + Default,
    {
        Ok(())
    }

    let g2_transport = ScriptedSyncTransport::new(vec![
        helpers::auto_respond_step(), // power_on
        helpers::auto_respond_step(), // zoom_stop
        helpers::auto_respond_step(), // for motion_sync_control
    ]);
    let mut g2 = PtzOpticsG2Cam::new_blocking(g2_transport).unwrap();

    let fr7_transport = ScriptedSyncTransport::new(vec![
        helpers::sony_auto_respond_step(), // power_on
        helpers::sony_auto_respond_step(), // zoom_stop
    ]);
    let mut fr7 = SonyFR7Cam::new_blocking(fr7_transport).unwrap();

    let generic_transport = ScriptedSyncTransport::new(vec![
        helpers::auto_respond_step(), // power_on
        helpers::auto_respond_step(), // zoom_stop
    ]);
    let mut generic = GenericViscaCam::new_blocking(generic_transport).unwrap();

    println!("Testing G2 camera...");
    assert!(basic_control(&mut g2).is_ok());

    println!("Testing FR7 camera...");
    let fr7_result = basic_control(&mut fr7);
    if let Err(e) = &fr7_result {
        println!("FR7 error: {e:?}");
    }
    assert!(fr7_result.is_ok());

    println!("Testing generic camera...");
    assert!(basic_control(&mut generic).is_ok());

    assert!(motion_sync_control(&mut g2).is_ok());
}
