//! Integration tests for the camera-first API introduced in issue #328.
//!
//! This test file verifies that:
//! - Direct transport access is completely removed
//! - `Connect` is the primary simple camera construction API
//! - Accessor calls are the primary control API

#[cfg(not(feature = "mode-async"))]
#[path = "common/compile_fail.rs"]
mod compile_fail;

#[cfg(not(feature = "mode-async"))]
use grafton_visca::{camera::Connect, profiles::GenericVisca};

#[cfg(not(feature = "mode-async"))]
#[test]
fn test_blocking_camera_tcp_connect() {
    // Test that Connect TCP returns appropriate errors
    // for invalid addresses (since we can't connect to real cameras in tests)
    let result = Connect::open_tcp_blocking::<GenericVisca>("invalid:address");
    assert!(result.is_err(), "Invalid address should fail");

    // Test with a proper address format (will fail to connect but proves API exists)
    let result = Connect::open_tcp_blocking::<GenericVisca>("192.168.1.100");
    assert!(
        result.is_err(),
        "Connection to non-existent camera should fail"
    );
}

#[cfg(not(feature = "mode-async"))]
#[test]
fn test_blocking_camera_udp_connect() {
    // Test that Connect UDP returns appropriate errors.
    let result = Connect::open_udp_blocking::<GenericVisca>("invalid:address");
    assert!(result.is_err(), "Invalid address should fail");

    // Note: UDP "connects" don't fail for non-existent hosts since UDP is connectionless
    // Just verify the API exists and can create a camera instance
    let result = Connect::open_udp_blocking::<GenericVisca>("192.168.1.100");
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
