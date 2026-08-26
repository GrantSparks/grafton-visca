//! No-default coverage guard for the pure protocol surface.
//!
//! The owner facades are intentionally absent when no feature is selected.
//! Keep a small executable probe here so the no-default matrix proves that
//! request/profile behavior still runs, while the two facade imports remain
//! compile-fail contracts.

#![cfg(not(any(feature = "blocking", feature = "async")))]

#[path = "common/compile_fail.rs"]
mod compile_fail;

use std::{fs, path::PathBuf};

use grafton_visca::{command::PowerOn, profiles::GenericVisca, CameraId, ProfileSpec, Request};

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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for relative in [
        "src/runtime/engine/tests.rs",
        "tests/issue_551_request_contract.rs",
    ] {
        let source = fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("read pure test inventory {relative}: {error}"));
        assert!(
            source.contains("#[test]"),
            "pure test inventory {relative} contains no executable tests"
        );
    }

    let features = compile_fail::active_grafton_visca_features();
    compile_fail::assert_compile_fail_fixtures(
        &["tests/api_contract/fail_no_canonical"],
        &features,
    );
}
