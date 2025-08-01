//! Compile-fail tests for the CameraBuilder API to ensure type safety.
//!
//! These tests ensure that the builder API prevents misuse at compile time:
//! - Cannot call `.build()` before `.profile()`
//! - Cannot call `.profile()` twice
//! - Cannot use types that don't implement `Profile`

#[test]
fn ui() {
    let t = trybuild::TestCases::new();

    // Test that build() cannot be called before profile()
    t.compile_fail("tests/ui/builder_build_without_profile.rs");

    // Test that profile() cannot be called twice
    t.compile_fail("tests/ui/builder_double_profile.rs");

    // Test that non-Profile types cannot be used
    t.compile_fail("tests/ui/builder_non_profile_type.rs");
}
