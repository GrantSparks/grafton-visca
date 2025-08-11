//! Compile-time tests for capability trait bounds.
//!
//! These tests verify that camera methods are only available
//! when the appropriate capability traits are implemented.

#[test]
fn capability_compile_tests() {
    let t = trybuild::TestCases::new();

    // Test that direct menu control is only available for cameras with HasDirectMenuControl
    t.compile_fail("tests/ui/menu_direct_control_capability.rs");
}
