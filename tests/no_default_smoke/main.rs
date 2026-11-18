//! No-default coverage guard for the pure protocol surface.
//!
//! The owner facades are intentionally absent when no feature is selected.
//! Keep a small executable probe here so the no-default matrix proves that
//! request/profile behavior still runs, while the two facade imports remain
//! compile-fail contracts.

#![cfg(not(any(feature = "blocking", feature = "async")))]

use grafton_visca_test_support::{active_grafton_visca_features, compile_fail};

use grafton_visca::{
    profiles::{GenericVisca, ProfileSpec},
    request::Request,
    types::CameraId,
};

/// Counts executable test declarations in one source file.
///
/// A trimmed line equal to `#[test]` cannot be a commented-out one: any `//`
/// prefix makes the trimmed line something else. A bare `source.contains(...)`
/// was satisfied by a comment, which is what this replaces.
fn executable_test_declarations(source: &str) -> usize {
    source
        .lines()
        .map(str::trim)
        .filter(|line| *line == "#[test]")
        .count()
}

#[test]
fn no_default_pure_request_profile_smoke_and_facade_inventory() {
    let profile = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");
    let request = grafton_visca::command::power::On;
    request
        .validate_for_target(&grafton_visca::config::TargetRegistration::new(
            grafton_visca::types::IndividualCameraId::CAMERA_1,
            profile.clone(),
        ))
        .expect("power request is valid for GenericVisca");

    let mut bytes = [0_u8; 16];
    let written = request
        .write_into(CameraId::CAMERA_1, &mut bytes)
        .expect("power request encodes without a facade");
    assert_eq!(&bytes[..written], &[0x81, 0x01, 0x04, 0x00, 0x02, 0xff]);

    // Track the feature-neutral request contract inventory from the same tree
    // that built this test.
    let source = include_str!("../integration/issue_551_request_contract.rs");
    let declared = executable_test_declarations(source);
    assert!(
        declared >= 3,
        "pure request contract declares {declared} executable tests, expected at least 3"
    );

    let features = active_grafton_visca_features!();
    // The base directory and no-canonical directory are defined for every
    // pure surface. The test-utils absence contract belongs only to pure
    // builds that do not expose `grafton_visca::testing`. This target owns
    // the no-default compile contracts.
    let mut fail_dirs = vec![
        "tests/api_contract/fail",
        "tests/api_contract/fail_no_canonical",
    ];
    if !cfg!(feature = "test-utils") {
        fail_dirs.push("tests/api_contract/fail_no_test_utils");
    }
    compile_fail::assert_compile_fail_fixtures(&fail_dirs, &features);
}
