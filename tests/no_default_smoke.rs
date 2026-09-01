//! No-default coverage guard for the pure protocol surface.
//!
//! The owner facades are intentionally absent when no feature is selected.
//! Keep a small executable probe here so the no-default matrix proves that
//! request/profile behavior still runs, while the two facade imports remain
//! compile-fail contracts.

#![cfg(not(any(feature = "blocking", feature = "async")))]

#[path = "common/compile_fail.rs"]
mod compile_fail;

use grafton_visca::{command::PowerOn, profiles::GenericVisca, CameraId, ProfileSpec, Request};

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
    let request = PowerOn::new();
    request
        .validate_for_profile(&profile)
        .expect("power request is valid for GenericVisca");

    let mut bytes = [0_u8; 16];
    let written = request
        .write_into(CameraId::CAMERA_1, &mut bytes)
        .expect("power request encodes without a facade");
    assert_eq!(&bytes[..written], &[0x81, 0x01, 0x04, 0x00, 0x02, 0xff]);

    // The engine, request, and semantic/profile integration test modules are
    // deliberately feature-neutral. Their test declarations must remain
    // present in the pure matrix rather than being hidden behind a facade.
    //
    // The sources are `include_str!`d rather than read at run time: a runtime
    // read checks whatever is on disk when the binary happens to run, which can
    // be a different tree from the one the binary was built from, and does not
    // rebuild this test when those files change. The floors are deliberately
    // well below the current counts — they exist to catch a module being gutted
    // or feature-gated away, not to be a second inventory that has to be
    // updated whenever a test is added or removed.
    for (label, source, floor) in [
        (
            "src/runtime/engine/tests.rs",
            include_str!("../src/runtime/engine/tests.rs"),
            40_usize,
        ),
        (
            "tests/issue_551_request_contract.rs",
            include_str!("issue_551_request_contract.rs"),
            3,
        ),
    ] {
        let declared = executable_test_declarations(source);
        assert!(
            declared >= floor,
            "pure test inventory {label} declares {declared} executable tests, expected at least {floor}"
        );
    }

    let features = compile_fail::active_grafton_visca_features();
    // The base directory and no-canonical directory are defined for every
    // pure surface. The test-utils absence contract belongs only to pure
    // builds that do not expose `grafton_visca::testing`. The no-default CI
    // leg runs this smoke target rather than `api_stability_test`, so keep it
    // from silently skipping the shared 50-fixture public-contract set.
    let mut fail_dirs = vec![
        "tests/api_contract/fail",
        "tests/api_contract/fail_no_canonical",
    ];
    if !cfg!(feature = "test-utils") {
        fail_dirs.push("tests/api_contract/fail_no_test_utils");
    }
    compile_fail::assert_compile_fail_fixtures(&fail_dirs, &features);
}
