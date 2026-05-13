//! Integration tests for the camera-first API introduced in issue #328.
//!
//! This test file verifies that:
//! - Direct transport access is completely removed
//! - Camera-first APIs are the only way to create cameras
//! - The new connect helpers function properly

#[cfg(not(feature = "mode-async"))]
#[path = "common/compile_fail.rs"]
mod compile_fail;

#[cfg(not(feature = "mode-async"))]
use grafton_visca::{profiles::GenericVisca, BlockingCamera, CameraBuilder};

#[cfg(not(feature = "mode-async"))]
#[test]
fn test_blocking_camera_tcp_connect() {
    // Test that BlockingCamera::connect_tcp exists and returns appropriate errors
    // for invalid addresses (since we can't connect to real cameras in tests)
    let result = BlockingCamera::<GenericVisca, _>::open_tcp("invalid:address");
    assert!(result.is_err(), "Invalid address should fail");

    // Test with a proper address format (will fail to connect but proves API exists)
    let result = BlockingCamera::<GenericVisca, _>::open_tcp("192.168.1.100");
    assert!(
        result.is_err(),
        "Connection to non-existent camera should fail"
    );
}

#[cfg(not(feature = "mode-async"))]
#[test]
fn test_blocking_camera_udp_connect() {
    // Test that BlockingCamera::connect_udp exists and returns appropriate errors
    let result = BlockingCamera::<GenericVisca, _>::open_udp("invalid:address");
    assert!(result.is_err(), "Invalid address should fail");

    // Note: UDP "connects" don't fail for non-existent hosts since UDP is connectionless
    // Just verify the API exists and can create a camera instance
    let result = BlockingCamera::<GenericVisca, _>::open_udp("192.168.1.100");
    assert!(
        result.is_ok(),
        "UDP should succeed even for non-existent camera (connectionless)"
    );
}

#[test]
#[cfg(not(feature = "mode-async"))]
fn test_camera_builder_tcp() {
    // Test that CameraBuilder::tcp exists and the builder pattern works
    let result = CameraBuilder::tcp("192.168.1.100")
        .profile::<GenericVisca>()
        .open();

    assert!(
        result.is_err(),
        "Connection to non-existent camera should fail"
    );
}

#[test]
#[cfg(not(feature = "mode-async"))]
fn test_camera_builder_udp() {
    // Test that CameraBuilder::udp exists and the builder pattern works
    let result = CameraBuilder::udp("192.168.1.100")
        .profile::<GenericVisca>()
        .open();

    // UDP is connectionless so it will succeed even for non-existent addresses
    assert!(
        result.is_ok(),
        "UDP should succeed even for non-existent camera (connectionless)"
    );
}

#[test]
#[cfg(not(feature = "mode-async"))]
fn test_no_direct_transport_access() {
    let features = compile_fail::active_grafton_visca_features();
    compile_fail::assert_compile_fail_fixtures(&["tests/camera_first_contract/fail"], &features);
}
