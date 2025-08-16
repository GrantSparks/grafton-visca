//! Integration test demonstrating compile-time capability checking with the generic API.

#![allow(clippy::expect_used)]
#![cfg(not(feature = "async"))]

#[cfg(feature = "test-utils")]
use grafton_visca::prelude::blocking::{GenericViscaCam, PTZOpticsG2Cam, SonyFR7Cam};
#[cfg(feature = "test-utils")]
use grafton_visca::{
    camera::methods::{
        focus::FocusOpsBlocking, pan_tilt::PanTiltOpsBlocking, power::PowerOpsBlocking,
        presets::PresetsOpsBlocking, zoom::ZoomOpsBlocking,
    },
    capabilities::*,
    Error, PresetNumber,
};

#[cfg(feature = "test-utils")]
use grafton_visca::testing::testkit::{helpers, ScriptedBlockingTransport};

#[cfg(feature = "test-utils")]
#[test]
fn test_ptzoptics_g2_capabilities() {
    // Create scripted transport with enough responses for all commands (5 commands total)
    let transport = ScriptedBlockingTransport::new(vec![
        helpers::auto_respond_step(), // power_on
        helpers::auto_respond_step(), // pan_tilt_home
        helpers::auto_respond_step(), // zoom_stop
        helpers::auto_respond_step(), // focus_auto
        helpers::auto_respond_step(), // preset_recall
    ]);

    let camera = PTZOpticsG2Cam::from_transport(transport);

    // These methods exist for PTZOpticsG2 - checked at compile time
    assert!(camera.power_on().is_ok());
    assert!(camera.pan_tilt_home().is_ok());
    assert!(camera.zoom_stop().is_ok());
    assert!(camera.focus_auto().is_ok());
    assert!(camera.preset_recall(PresetNumber::new(1).unwrap()).is_ok());

    // This would NOT compile - G2 doesn't implement NDFilter trait!
    // camera.set_nd_filter_mode(NDFilterMode::Clear).unwrap(); // COMPILE ERROR!
}

#[cfg(feature = "test-utils")]
#[test]
fn test_sony_fr7_has_nd_filter() {
    // Create scripted transport with Sony envelope responses (3 commands total)
    let transport = ScriptedBlockingTransport::new(vec![
        helpers::sony_auto_respond_step(), // power_on
        helpers::sony_auto_respond_step(), // pan_tilt_home
        helpers::sony_auto_respond_step(), // zoom_stop
    ]);

    let camera = SonyFR7Cam::from_transport(transport);

    // FR7 has all standard features
    assert!(camera.power_on().is_ok());
    assert!(camera.pan_tilt_home().is_ok());
    assert!(camera.zoom_stop().is_ok());

    // PLUS ND filter support! This compiles because SonyFR7 implements NDFilter
    // Note: The command API would be available here for ND filter control
}

// This test demonstrates compile-time capability checking
#[cfg(feature = "test-utils")]
#[test]
fn test_compile_time_capability_checking() {
    // This function can only be called with cameras that have ND filter support
    fn adjust_nd_filter<P, T>(
        _camera: &grafton_visca::camera::Camera<grafton_visca::camera::BlockingMode, P, T>,
    ) -> Result<(), Error>
    where
        P: Profile + NDFilter,
        T: grafton_visca::transport::BlockingTransport + Send + Sync + 'static,
    {
        // ND filter methods would be available here
        Ok(())
    }

    let fr7_transport = ScriptedBlockingTransport::new(vec![helpers::auto_respond_step()]);
    let fr7 = SonyFR7Cam::from_transport(fr7_transport);

    let _g2_transport = ScriptedBlockingTransport::new(vec![helpers::auto_respond_step()]);
    let _g2 = PTZOpticsG2Cam::from_transport(_g2_transport);

    // This compiles - FR7 has NDFilter
    assert!(adjust_nd_filter(&fr7).is_ok());

    // This would NOT compile - G2 doesn't have NDFilter
    // adjust_nd_filter(&g2); // COMPILE ERROR!

    // The compiler prevents calling unsupported methods at compile time
}

#[cfg(feature = "test-utils")]
#[test]
fn test_generic_functions_with_trait_bounds() {
    // Function that works with any camera
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
        // Use blocking operations directly on the camera
        camera.power_on()?;
        camera.zoom_stop()?;
        Ok(())
    }

    // Function that requires motion sync capability
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
        // Motion sync methods would be available here
        Ok(())
    }

    let g2_transport = ScriptedBlockingTransport::new(vec![
        helpers::auto_respond_step(), // power_on
        helpers::auto_respond_step(), // zoom_stop
        helpers::auto_respond_step(), // for motion_sync_control
    ]);
    let g2 = PTZOpticsG2Cam::from_transport(g2_transport);

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

    // Test each camera individually to isolate the issue
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

    // Only G2 can use motion_sync_control (it implements MotionSync)
    assert!(motion_sync_control(&g2).is_ok());
    // Note: FR7 doesn't have MotionSync in current implementation

    // This would NOT compile - GenericVisca doesn't implement MotionSync
    // motion_sync_control(&generic); // COMPILE ERROR!
}
