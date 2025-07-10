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
    #[allow(dead_code)]
    struct MockTransport;

    // These type declarations compile
    #[allow(dead_code)]
    type G2Camera = Camera<PTZOpticsG2, MockTransport>;
    #[allow(dead_code)]
    type FR7Camera = Camera<SonyFR7, MockTransport>;

    // The test validates that these types compile correctly
    // No assertion needed - the test passes if compilation succeeds
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
