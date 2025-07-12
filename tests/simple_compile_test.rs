//! Simple test to verify compilation succeeds with unified Camera implementation.

// This test simply verifies that the code compiles.
// The unified Camera API performs runtime capability checks instead of compile-time checks.

use grafton_visca::{Camera, ProfileId};

#[test]
fn test_compilation_succeeds() {
    // The unified Camera API doesn't use generics for profiles
    // Instead, it uses runtime profile selection

    // These would be created with actual transports in real usage:
    // let g2_camera = Camera::with_profile(ProfileId::PTZOpticsG2, transport);
    // let fr7_camera = Camera::with_profile(ProfileId::SonyFR7, transport);

    // The test validates that the Camera type compiles correctly
    // No assertion needed - the test passes if compilation succeeds
}

// Runtime capability checking example:
/*
fn runtime_capability_check() {
    // With the unified API, unsupported features return errors at runtime
    let camera = Camera::with_profile(ProfileId::PTZOpticsG2, transport);

    // This compiles but would return an error if PTZOpticsG2 doesn't support ND filter
    match camera.set_nd_filter_blocking(2) {
        Ok(_) => println!("ND filter set"),
        Err(e) => println!("ND filter not supported: {}", e),
    }
}
*/
