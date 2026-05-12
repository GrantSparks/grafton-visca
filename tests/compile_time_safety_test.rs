//! Integration test demonstrating compile-time capability checking with the generic API.

#![allow(clippy::expect_used)]
#![cfg(not(feature = "mode-async"))]

#[cfg(feature = "test-utils")]
use grafton_visca::{
    camera::controls::{power::PowerControl, zoom::ZoomControl},
    capabilities::*,
    prelude::blocking::{GenericVisca, PtzOpticsG2, PtzOpticsG2Cam, SonyFR7, SonyFR7Cam},
    testing::testkit::{helpers, ScriptedBlockingTransport},
    Error,
};

#[cfg(feature = "test-utils")]
#[test]
fn test_ptzoptics_g2_capabilities() -> Result<(), Error> {
    let transport = ScriptedBlockingTransport::new(vec![
        helpers::auto_respond_step(),
        helpers::auto_respond_step(),
        helpers::auto_respond_step(),
        helpers::auto_respond_step(),
        helpers::auto_respond_step(),
    ]);

    let camera = PtzOpticsG2Cam::new(transport)?;

    assert!(camera.power().on().is_ok());
    assert!(camera.pan_tilt().home().is_ok());
    assert!(camera.zoom().stop().is_ok());
    assert!(camera.focus().auto().is_ok());
    assert!(camera.presets().recall(1).is_ok());
    Ok(())
}

#[cfg(feature = "test-utils")]
#[test]
fn test_sony_fr7_has_nd_filter() -> Result<(), Error> {
    let transport = ScriptedBlockingTransport::new(vec![
        helpers::sony_auto_respond_step(),
        helpers::sony_auto_respond_step(),
        helpers::sony_auto_respond_step(),
    ]);

    let camera = SonyFR7Cam::new(transport)?;

    assert!(camera.power().on().is_ok());
    assert!(camera.pan_tilt().home().is_ok());
    assert!(camera.zoom().stop().is_ok());
    Ok(())
}

#[cfg(feature = "test-utils")]
#[test]
fn test_compile_time_capability_checking() {
    fn adjust_nd_filter<P, T>(_camera: &grafton_visca::BlockingCamera<P, T>) -> Result<(), Error>
    where
        P: Profile + NdFilter,
        T: grafton_visca::transport::BlockingTransport + Send + Sync + 'static,
    {
        Ok(())
    }

    let fr7_transport = ScriptedBlockingTransport::new(vec![helpers::auto_respond_step()]);
    let fr7 = SonyFR7Cam::new(fr7_transport).unwrap();

    let _g2_transport = ScriptedBlockingTransport::new(vec![helpers::auto_respond_step()]);
    let _g2 = PtzOpticsG2Cam::new(_g2_transport).unwrap();

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
        T: grafton_visca::transport::BlockingTransport,
        grafton_visca::camera::Camera<grafton_visca::mode::Blocking, P, T, ()>: PowerControl<Mode = grafton_visca::mode::Blocking>
            + ZoomControl<Mode = grafton_visca::mode::Blocking>,
    {
        use grafton_visca::mode::BlockingFutureExt;
        camera.power_on().block()?;
        camera.zoom_stop().block()?;
        Ok(())
    }

    fn motion_sync_control<P>(
        _camera: &grafton_visca::BlockingCamera<P, ScriptedBlockingTransport>,
    ) -> Result<(), Error>
    where
        P: Profile + MotionSync + Default,
    {
        Ok(())
    }

    let g2_transport = ScriptedBlockingTransport::new(vec![
        helpers::auto_respond_step(),
        helpers::auto_respond_step(),
    ]);
    let mut g2_camera = grafton_visca::camera::Camera::<
        grafton_visca::mode::Blocking,
        PtzOpticsG2,
        _,
        (),
    >::new_blocking(g2_transport)
    .unwrap();

    let g2_transport_wrapper = ScriptedBlockingTransport::new(vec![helpers::auto_respond_step()]);
    let g2_wrapper = PtzOpticsG2Cam::new(g2_transport_wrapper).unwrap();

    let fr7_transport = ScriptedBlockingTransport::new(vec![
        helpers::sony_auto_respond_step(),
        helpers::sony_auto_respond_step(),
    ]);
    let mut fr7_camera = grafton_visca::camera::Camera::<
        grafton_visca::mode::Blocking,
        SonyFR7,
        _,
        (),
    >::new_blocking(fr7_transport)
    .unwrap();

    let generic_transport = ScriptedBlockingTransport::new(vec![
        helpers::auto_respond_step(),
        helpers::auto_respond_step(),
    ]);
    let mut generic_camera = grafton_visca::camera::Camera::<
        grafton_visca::mode::Blocking,
        GenericVisca,
        _,
        (),
    >::new_blocking(generic_transport)
    .unwrap();

    assert!(basic_control(&mut g2_camera).is_ok());

    let fr7_result = basic_control(&mut fr7_camera);
    assert!(fr7_result.is_ok());

    assert!(basic_control(&mut generic_camera).is_ok());

    assert!(motion_sync_control(&g2_wrapper).is_ok());
}
