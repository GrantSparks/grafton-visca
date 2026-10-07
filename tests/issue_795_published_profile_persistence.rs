//! Persisted built-in `ProfileSpec` JSON from the last published release,
//! 2.0.0-rc.2.
//!
//! The fixtures in `tests/fixtures/issue_795_rc2_profile_specs/` are the exact
//! `serde_json` output of `ProfileSpec::from_compile_time::<P>()` at tag
//! `v2.0.0-rc.2`, pretty-printed. That shape predates #795, #807, #808, #818,
//! #822 and #828, so it differs from the current one:
//!
//! - `capabilities.focus_zones`, `optical_zoom_ratio`, `highest_preset` and
//!   `motion_sync_speed_range` are absent;
//! - it carries the removed `max_presets`, `max_motion_sync_speed`,
//!   `max_motion_sync_speed_profile`, `default_tcp_port`, `default_udp_port`,
//!   `zoom_magnification_to_units`, `exposure_comp_profile_range`,
//!   `supports_hue` and `has_*` capability flags;
//! - shutter entries carry a `label` string instead of an `exposure`
//!   fraction, and the iris and brightness ranges are `{start, end}` instead
//!   of `{min, max, gaps}` domains;
//! - `capabilities.typed_support` lacks the surfaces added since: Sony FR7 and
//!   Generic VISCA have no `"version-inquiry"` tag, and PTZOptics G2 has no
//!   `"color-temperature-inquiry"` or `"noise-reduction2-d-mode"` tag.
//!
//! 2.0 keeps no compatibility path for older shapes. Every such spec is
//! refused with an error that tells the caller to regenerate it from the
//! current registry, and a regenerated spec round-trips.

#![cfg(feature = "serde")]
#![allow(clippy::expect_used)]

use grafton_visca::{
    capabilities::TypedSupportSurface,
    profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
    Error, ProfileSpec,
};

const RC2_SONY_FR7: &str = include_str!("fixtures/issue_795_rc2_profile_specs/sony_fr7.json");
const RC2_GENERIC_VISCA: &str =
    include_str!("fixtures/issue_795_rc2_profile_specs/generic_visca.json");
const RC2_PTZ_OPTICS_G2: &str =
    include_str!("fixtures/issue_795_rc2_profile_specs/ptz_optics_g2.json");

/// Pins the fixture to the 2.0.0-rc.2 shape: current required fields are
/// absent, removed fields are present, and `missing_tag` is a typed-support
/// surface the current registry grants that rc.2 did not.
fn assert_rc2_shape(fixture: &str, missing_tag: &str) {
    let value: serde_json::Value = serde_json::from_str(fixture).expect("fixture is JSON");
    let capabilities = &value["capabilities"];
    for field in [
        "focus_zones",
        "optical_zoom_ratio",
        "highest_preset",
        "motion_sync_speed_range",
    ] {
        assert!(capabilities.get(field).is_none(), "{field}");
    }
    for field in ["max_presets", "default_udp_port", "has_focus_zone"] {
        assert!(capabilities.get(field).is_some(), "{field}");
    }
    assert!(capabilities["shutter_speeds"][0].get("label").is_some());
    assert!(capabilities["shutter_speeds"][0].get("exposure").is_none());
    let typed_support = capabilities["typed_support"]
        .as_array()
        .expect("typed_support list");
    assert!(!typed_support.iter().any(|tag| tag == missing_tag));
}

fn refusal(json: &str) -> String {
    serde_json::from_str::<ProfileSpec>(json)
        .expect_err("a spec in another release's shape must not load")
        .to_string()
}

fn assert_regeneration_instruction(error: &str) {
    assert!(
        error.contains(
            "the spec was saved by another release, so regenerate it with \
             `ProfileSpec::from_compile_time::<P>()`"
        ),
        "{error}"
    );
}

#[test]
fn every_rc2_built_in_spec_is_refused_with_a_regeneration_instruction() {
    for (fixture, missing_tag) in [
        (RC2_SONY_FR7, "version-inquiry"),
        (RC2_GENERIC_VISCA, "version-inquiry"),
        (RC2_PTZ_OPTICS_G2, "color-temperature-inquiry"),
    ] {
        assert_rc2_shape(fixture, missing_tag);
        let error = refusal(fixture);
        assert!(error.contains("missing field"), "{error}");
        assert_regeneration_instruction(&error);
    }
}

#[test]
fn a_current_spec_missing_a_current_field_is_refused() {
    let current = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2");
    for field in ["focus_zones", "optical_zoom_ratio"] {
        let mut value = serde_json::to_value(&current).expect("serialize");
        value["capabilities"]
            .as_object_mut()
            .expect("capabilities object")
            .remove(field);
        let error = refusal(&value.to_string());
        assert!(
            error.contains(&format!("missing field `{field}`")),
            "{field}: {error}"
        );
        assert_regeneration_instruction(&error);
    }
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
fn regenerated_specs_round_trip() {
    for (spec, version_inquiry) in [
        (
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7"),
            true,
        ),
        (
            ProfileSpec::from_compile_time::<GenericVisca>().expect("Generic VISCA"),
            true,
        ),
        (
            ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2"),
            false,
        ),
    ] {
        let json = serde_json::to_string(&spec).expect("serialize");
        let restored: ProfileSpec = serde_json::from_str(&json).expect("current shape loads");
        assert_eq!(restored, spec);
        assert_eq!(
            restored
                .capabilities()
                .supports_typed(TypedSupportSurface::VersionInquiry),
            version_inquiry
        );
    }
}
