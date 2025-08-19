//! Integration test demonstrating compile-time capability checking with the generic API.

#![allow(clippy::expect_used)]
#![cfg(not(feature = "async"))]

#[cfg(feature = "test-utils")]
use grafton_visca::{
    camera::methods::{
        focus::FocusControlBlocking, pan_tilt::PanTiltControlBlocking, power::PowerControlBlocking,
        presets::PresetsControlBlocking, zoom::ZoomControlBlocking,
    },
    capabilities::*,
    prelude::blocking::{GenericViscaCam, PtzOpticsG2Cam, SonyFR7Cam},
    testing::testkit::{helpers, ScriptedBlockingTransport},
    Error, PresetNumber,
};

#[cfg(feature = "test-utils")]
#[test]
fn test_ptzoptics_g2_capabilities() {
    let transport = ScriptedBlockingTransport::new(vec![
        helpers::auto_respond_step(), // power_on
        helpers::auto_respond_step(), // pan_tilt_home
        helpers::auto_respond_step(), // zoom_stop
        helpers::auto_respond_step(), // focus_auto
        helpers::auto_respond_step(), // preset_recall
    ]);

    let camera = PtzOpticsG2Cam::from_transport(transport);

    assert!(camera.power_on().is_ok());
    assert!(camera.pan_tilt_home().is_ok());
    assert!(camera.zoom_stop().is_ok());
    assert!(camera.focus_auto().is_ok());
    assert!(camera.preset_recall(PresetNumber::new(1).unwrap()).is_ok());
}

#[cfg(feature = "test-utils")]
#[test]
fn test_sony_fr7_has_nd_filter() {
    let transport = ScriptedBlockingTransport::new(vec![
        helpers::sony_auto_respond_step(), // power_on
        helpers::sony_auto_respond_step(), // pan_tilt_home
        helpers::sony_auto_respond_step(), // zoom_stop
    ]);

    let camera = SonyFR7Cam::from_transport(transport);

    assert!(camera.power_on().is_ok());
    assert!(camera.pan_tilt_home().is_ok());
    assert!(camera.zoom_stop().is_ok());
}

#[cfg(feature = "test-utils")]
#[test]
fn test_compile_time_capability_checking() {
    fn adjust_nd_filter<P, T>(
        _camera: &grafton_visca::camera::Camera<grafton_visca::camera::BlockingMode, P, T>,
    ) -> Result<(), Error>
    where
        P: Profile + NDFilter,
        T: grafton_visca::transport::BlockingTransport + Send + Sync + 'static,
    {
        Ok(())
    }

    let fr7_transport = ScriptedBlockingTransport::new(vec![helpers::auto_respond_step()]);
    let fr7 = SonyFR7Cam::from_transport(fr7_transport);

    let _g2_transport = ScriptedBlockingTransport::new(vec![helpers::auto_respond_step()]);
    let _g2 = PtzOpticsG2Cam::from_transport(_g2_transport);

    assert!(adjust_nd_filter(&fr7).is_ok());
}

#[cfg(feature = "test-utils")]
#[test]
fn test_generic_functions_with_trait_bounds() {
    fn basic_control<P>(
        camera: &grafton_visca::camera::Camera<
            grafton_visca::camera::BlockingMode,
            P,
            ScriptedBlockingTransport,
        >,
    ) -> Result<(), Error>
    where
        P: Profile,
    {
        camera.power_on()?;
        camera.zoom_stop()?;
        Ok(())
    }

    fn motion_sync_control<P>(
        _camera: &grafton_visca::camera::Camera<
            grafton_visca::camera::BlockingMode,
            P,
            ScriptedBlockingTransport,
        >,
    ) -> Result<(), Error>
    where
        P: Profile + MotionSync,
    {
        Ok(())
    }

    let g2_transport = ScriptedBlockingTransport::new(vec![
        helpers::auto_respond_step(), // power_on
        helpers::auto_respond_step(), // zoom_stop
        helpers::auto_respond_step(), // for motion_sync_control
    ]);
    let g2 = PtzOpticsG2Cam::from_transport(g2_transport);

    let fr7_transport = ScriptedBlockingTransport::new(vec![
        helpers::sony_auto_respond_step(), // power_on
        helpers::sony_auto_respond_step(), // zoom_stop
    ]);
    let fr7 = SonyFR7Cam::from_transport(fr7_transport);

    let generic_transport = ScriptedBlockingTransport::new(vec![
        helpers::auto_respond_step(), // power_on
        helpers::auto_respond_step(), // zoom_stop
    ]);
    let generic = GenericViscaCam::from_transport(generic_transport);

    println!("Testing G2 camera...");
    assert!(basic_control(&g2).is_ok());

    println!("Testing FR7 camera...");
    let fr7_result = basic_control(&fr7);
    if let Err(e) = &fr7_result {
        println!("FR7 error: {e:?}");
    }
    assert!(fr7_result.is_ok());

    println!("Testing generic camera...");
    assert!(basic_control(&generic).is_ok());

    assert!(motion_sync_control(&g2).is_ok());
}
