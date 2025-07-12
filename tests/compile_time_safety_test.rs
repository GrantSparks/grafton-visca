//! Integration test demonstrating compile-time safety of the new API.

use grafton_visca::{
    camera::methods::*,
    profiles::{PTZOpticsG2, SonyFR7},
    CameraBlocking,
    Error,
};

// Mock transport for testing
#[derive(Debug)]
struct MockTransport;

impl grafton_visca::transport::Transport for MockTransport {
    type Error = Error;
    type SendFut<'a> = std::future::Ready<Result<(), Self::Error>> where Self: 'a;
    type RecvFut<'a> = std::future::Ready<Result<bytes::Bytes, Self::Error>> where Self: 'a;

    fn send<'a>(&'a self, _data: &'a [u8]) -> Self::SendFut<'a> {
        std::future::ready(Ok(()))
    }

    fn recv<'a>(&'a self) -> Self::RecvFut<'a> {
        std::future::ready(Ok(bytes::Bytes::from(vec![0x90, 0x50, 0xFF])))
    }
}

impl grafton_visca::transport::core::BlockingTransport for MockTransport {}

#[test]
fn test_ptzoptics_g2_capabilities() {
    let mut camera: CameraBlocking<PTZOpticsG2, _> = CameraBlocking::new(MockTransport);

    // These methods exist - G2 supports these capabilities
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
    let mut camera: CameraBlocking<SonyFR7, _> = CameraBlocking::new(MockTransport);

    // FR7 has all standard features
    assert!(camera.power_on().is_ok());
    assert!(camera.pan_tilt_home().is_ok());
    assert!(camera.zoom_stop().is_ok());

    // PLUS ND filter support!
    assert!(camera.set_nd_filter(128).is_ok());
}

// This test demonstrates that methods literally don't exist for unsupported features
#[test]
fn test_compile_time_method_availability() {
    // This function will only accept cameras with ND filter support
    fn adjust_nd_filter<P, T>(camera: &mut CameraBlocking<P, T>) -> Result<(), Error>
    where
        P: grafton_visca::capabilities::ProfileMetadata
            + grafton_visca::capabilities::NDFilter
            + Default,
        T: grafton_visca::transport::core::BlockingTransport,
        CameraBlocking<P, T>: NDFilterBlockingExt<P, T>,
    {
        camera.set_nd_filter(64)
    }

    let mut fr7: CameraBlocking<SonyFR7, _> = CameraBlocking::new(MockTransport);
    let mut _g2: CameraBlocking<PTZOpticsG2, _> = CameraBlocking::new(MockTransport);

    // This compiles - FR7 has ND filter
    assert!(adjust_nd_filter(&mut fr7).is_ok());

    // This would NOT compile - G2 doesn't have ND filter
    // adjust_nd_filter(&mut g2); // COMPILE ERROR!
}
