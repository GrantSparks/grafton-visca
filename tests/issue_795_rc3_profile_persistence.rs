//! Persisted built-in `ProfileSpec` JSON from before #795.
//!
//! The fixtures in `tests/fixtures/issue_795_rc3_profile_specs/` are the exact
//! `serde_json` output of `ProfileSpec::from_compile_time::<P>()` at the
//! 2.0.0-rc.3 release commit (`5a3e81d1`), pretty-printed. That shape predates
//! #795: `capabilities.typed_support` has no `"version-inquiry"` tag and
//! `capabilities.focus_zones` does not exist.
//!
//! A built-in identity must match the current registry exactly, so a stale
//! Sony or Generic VISCA spec is refused with an error that names the profile,
//! the differing surfaces, and the constructor that regenerates it. A stale
//! PTZOptics spec, whose typed-support set did not change, still loads and
//! equals the current registry profile.

#![cfg(feature = "serde")]
#![allow(clippy::expect_used)]

use grafton_visca::{
    capabilities::TypedSupportSurface,
    command::FocusZone,
    profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
    Error, ProfileSpec,
};

const RC3_SONY_FR7: &str = include_str!("fixtures/issue_795_rc3_profile_specs/sony_fr7.json");
const RC3_GENERIC_VISCA: &str =
    include_str!("fixtures/issue_795_rc3_profile_specs/generic_visca.json");
const RC3_PTZ_OPTICS_G2: &str =
    include_str!("fixtures/issue_795_rc3_profile_specs/ptz_optics_g2.json");

fn assert_pre_795_shape(fixture: &str) {
    let value: serde_json::Value = serde_json::from_str(fixture).expect("fixture is JSON");
    let capabilities = &value["capabilities"];
    assert!(
        capabilities.get("focus_zones").is_none(),
        "the rc.3 shape has no focus_zones field"
    );
    let typed_support = capabilities["typed_support"]
        .as_array()
        .expect("typed_support list");
    assert!(
        !typed_support.iter().any(|tag| tag == "version-inquiry"),
        "the rc.3 shape has no version-inquiry tag"
    );
}

fn stale_builtin_error(fixture: &str) -> String {
    assert_pre_795_shape(fixture);
    serde_json::from_str::<ProfileSpec>(fixture)
        .expect_err("a stale built-in identity must not load")
        .to_string()
}

#[test]
fn rc3_sony_fr7_spec_is_refused_with_a_regeneration_instruction() {
    assert_eq!(
        stale_builtin_error(RC3_SONY_FR7),
        "Invalid request: profile fields `capabilities.profile_id`, \
         `capabilities.typed_support`: built-in profile `SonyFR7`: the stored \
         typed-support set differs from the current built-in registry (missing: \
         VersionInquiry; not in registry: none); the spec was saved by another \
         release, so regenerate it with \
         `ProfileSpec::from_compile_time::<grafton_visca::profiles::SonyFR7>()` \
         and persist the result"
    );
}

#[test]
fn rc3_generic_visca_spec_is_refused_with_a_regeneration_instruction() {
    assert_eq!(
        stale_builtin_error(RC3_GENERIC_VISCA),
        "Invalid request: profile fields `capabilities.profile_id`, \
         `capabilities.typed_support`: built-in profile `GenericVisca`: the stored \
         typed-support set differs from the current built-in registry (missing: \
         VersionInquiry; not in registry: none); the spec was saved by another \
         release, so regenerate it with \
         `ProfileSpec::from_compile_time::<grafton_visca::profiles::GenericVisca>()` \
         and persist the result"
    );
}

/// Building (not only deserializing) a spec whose built-in identity carries
/// the pre-#795 typed-support set fails with the same `InvalidRequest` variant
/// the identity check has always used.
#[test]
fn stale_builtin_typed_support_keeps_the_invalid_request_variant() {
    let current = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7");
    let mut capabilities = current.capabilities().clone();
    capabilities.typed_support = capabilities
        .typed_support
        .without(TypedSupportSurface::VersionInquiry);
    let error = ProfileSpec::builder(capabilities)
        .transports(current.transports())
        .envelope(current.envelope())
        .timing(current.timing())
        .maximum_command_sockets(current.maximum_command_sockets())
        .supports_operation_complete(current.supports_operation_complete())
        .supports_command_cancel(current.supports_command_cancel())
        .preset_recall_axes(current.preset_recall_axes())
        .position_inquiries(current.position_inquiries())
        .build()
        .expect_err("a stale built-in identity must not build");
    assert!(
        matches!(&error, Error::InvalidRequest(message)
            if message.contains("built-in profile `SonyFR7`")
                && message.contains("missing: VersionInquiry")
                && message.contains("ProfileSpec::from_compile_time::<grafton_visca::profiles::SonyFR7>()")),
        "unexpected error: {error:?}"
    );
}

#[test]
fn regenerated_sony_and_generic_specs_round_trip() {
    for spec in [
        ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7"),
        ProfileSpec::from_compile_time::<GenericVisca>().expect("Generic VISCA"),
    ] {
        let json = serde_json::to_string(&spec).expect("serialize");
        let restored: ProfileSpec = serde_json::from_str(&json).expect("current shape loads");
        assert_eq!(restored, spec);
        assert!(restored
            .capabilities()
            .supports_typed(TypedSupportSurface::VersionInquiry));
    }
}

#[test]
fn rc3_ptz_optics_g2_spec_still_loads_and_matches_the_current_registry() {
    assert_pre_795_shape(RC3_PTZ_OPTICS_G2);
    let restored: ProfileSpec =
        serde_json::from_str(RC3_PTZ_OPTICS_G2).expect("rc.3 G2 spec still loads");
    let current = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2");
    assert_eq!(restored.capabilities(), current.capabilities());
    assert_eq!(restored, current);
    assert_eq!(
        restored.capabilities().focus_zones,
        [
            FocusZone::Top,
            FocusZone::Center,
            FocusZone::Bottom,
            FocusZone::Zone03
        ],
        "the built-in identity restores its registry focus zones"
    );
    assert!(!restored
        .capabilities()
        .supports_typed(TypedSupportSurface::VersionInquiry));
}
