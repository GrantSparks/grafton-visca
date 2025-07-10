//! Integration test demonstrating compile-time safety of the new API.

use grafton_visca::{
    camera::{Camera, methods::*},
    profiles::{PTZOpticsG2, SonyFR7},
    Error,
};

// Mock transport for testing
#[derive(Debug)]
struct MockTransport;

impl grafton_visca::transport::blocking::BlockingTransport for MockTransport {
    fn send(&mut self, _data: &[u8]) -> Result<(), Error> {
        Ok(())
    }
    
    fn receive(&mut self, _timeout: std::time::Duration) -> Result<Vec<u8>, Error> {
        Ok(vec![0x90, 0x50, 0xFF]) // Mock completion response
    }
    
    fn is_connected(&self) -> bool {
        true
    }
    
    fn description(&self) -> &str {
        "MockTransport"
    }
}

#[test]
fn test_ptzoptics_g2_capabilities() {
    let mut camera: Camera<PTZOpticsG2, _> = Camera::new(MockTransport);
    
    // These methods exist - G2 supports these capabilities
    assert!(camera.power_on().is_ok());
    assert!(camera.pan_tilt_home().is_ok());
    assert!(camera.zoom_stop().is_ok());
    assert!(camera.focus_auto().is_ok());
    assert!(camera.exposure_auto().is_ok());
    assert!(camera.white_balance_auto().is_ok());
    assert!(camera.enable_flip().is_ok());
    assert!(camera.preset_recall(1).is_ok());
    
    // This would NOT compile - G2 doesn't support ND filters!
    // camera.set_nd_filter(2).unwrap(); // COMPILE ERROR!
}

#[test]
fn test_sony_fr7_has_nd_filter() {
    let mut camera: Camera<SonyFR7, _> = Camera::new(MockTransport);
    
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
    fn adjust_nd_filter<P, T>(camera: &mut Camera<P, T>) -> Result<(), Error>
    where
        P: grafton_visca::capabilities::ProfileMetadata 
            + grafton_visca::capabilities::SupportsNDFilter
            + Default,
        T: grafton_visca::transport::blocking::BlockingTransport,
        Camera<P, T>: NDFilterMethods,
    {
        camera.set_nd_filter(64)
    }
    
    let mut fr7: Camera<SonyFR7, _> = Camera::new(MockTransport);
    let mut _g2: Camera<PTZOpticsG2, _> = Camera::new(MockTransport);
    
    // This compiles - FR7 has ND filter
    assert!(adjust_nd_filter(&mut fr7).is_ok());
    
    // This would NOT compile - G2 doesn't have ND filter
    // adjust_nd_filter(&mut g2); // COMPILE ERROR!
}