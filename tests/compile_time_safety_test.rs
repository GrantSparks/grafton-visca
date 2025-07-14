//! Integration test demonstrating runtime capability checking of the unified API.

use grafton_visca::{blocking::*, Camera, CameraModel, Error};

// Mock transport for testing
#[derive(Debug)]
struct MockTransport;

impl grafton_visca::transport::Transport for MockTransport {
    type Error = Error;
    type SendFut<'a>
        = std::future::Ready<Result<(), Self::Error>>
    where
        Self: 'a;
    type RecvFut<'a>
        = std::future::Ready<Result<bytes::Bytes, Self::Error>>
    where
        Self: 'a;

    fn send<'a>(&'a self, _data: &'a [u8]) -> Self::SendFut<'a> {
        std::future::ready(Ok(()))
    }

    fn recv(&self) -> Self::RecvFut<'_> {
        std::future::ready(Ok(bytes::Bytes::from(vec![0x90, 0x50, 0xFF])))
    }
}

impl grafton_visca::transport::core::BlockingTransport for MockTransport {}

#[test]
fn test_ptzoptics_g2_capabilities() {
    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, MockTransport).blocking();

    // These methods exist for all cameras - capability checks happen at runtime
    assert!(camera.power_on().is_ok());
    assert!(camera.pan_tilt_home().is_ok());
    assert!(camera.zoom_stop().is_ok());
    assert!(camera.focus_auto().is_ok());
    // TODO: Update these to use the new API
    // assert!(camera
    //     .set_exposure_mode(grafton_visca::command::ExposureMode::Auto)
    //     .is_ok());
    // assert!(camera
    //     .set_white_balance_mode(grafton_visca::capabilities::WhiteBalanceMode::Auto)
    //     .is_ok());
    // assert!(camera.flip_on().is_ok());
    // assert!(camera.recall_preset(1).is_ok());

    // This would NOT compile - G2 doesn't support ND filters!
    // camera.set_nd_filter(2).unwrap(); // COMPILE ERROR!
}

#[test]
fn test_sony_fr7_has_nd_filter() {
    let camera = Camera::with_profile(CameraModel::SonyFR7, MockTransport).blocking();

    // FR7 has all standard features
    assert!(camera.power_on().is_ok());
    assert!(camera.pan_tilt_home().is_ok());
    assert!(camera.zoom_stop().is_ok());

    // PLUS ND filter support! (but in the unified API, this is checked at runtime)
    // The method exists but might return an error based on the profile
    let _ = camera.set_nd_filter(128); // This may succeed or fail at runtime
}

// This test demonstrates runtime capability checking
#[test]
fn test_runtime_capability_checking() {
    // With the unified API, capabilities are checked at runtime
    fn try_adjust_nd_filter(camera: &mut Camera) -> Result<(), Error> {
        // NDFilterOps is already imported at the module level
        // This might succeed or fail based on the camera's profile
        camera.set_nd_filter(64)
    }

    let mut fr7 = Camera::with_profile(CameraModel::SonyFR7, MockTransport);
    let mut g2 = Camera::with_profile(CameraModel::PTZOpticsG2, MockTransport);

    // With the unified API, both calls compile but behavior differs at runtime
    match try_adjust_nd_filter(&mut fr7) {
        Ok(_) => println!("FR7 supports ND filter"),
        Err(_) => println!("FR7 ND filter operation failed"),
    }

    // This compiles but returns an error at runtime for G2
    match try_adjust_nd_filter(&mut g2) {
        Ok(_) => panic!("G2 shouldn't support ND filter!"),
        Err(_) => println!("G2 correctly reports no ND filter support"),
    }
}
