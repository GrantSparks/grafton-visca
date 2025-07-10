//! Simple test to verify compilation succeeds with Phase 4 implementation.

// This test simply verifies that the code compiles.
// The key point is that methods for unsupported features literally don't exist.

use grafton_visca::{
    camera::Camera,
    profiles::{PTZOpticsG2, SonyFR7},
};

#[test]
fn test_compilation_succeeds() {
    // Mock transport type (not instantiated)
    struct MockTransport;

    // These type declarations compile
    type G2Camera = Camera<PTZOpticsG2, MockTransport>;
    type FR7Camera = Camera<SonyFR7, MockTransport>;

    // The test passes if this compiles
    assert!(true);
}

// This would demonstrate compile-time errors if uncommented:
/*
fn invalid_nd_filter_usage() {
    struct MockTransport;
    let mut g2: Camera<PTZOpticsG2, MockTransport> = Camera::new(MockTransport);

    // This line would NOT compile because PTZOpticsG2 doesn't implement SupportsNDFilter
    g2.set_nd_filter(2); // COMPILE ERROR!
}
*/
