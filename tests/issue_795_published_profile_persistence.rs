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
    profiles::{
        GenericVisca, NearusBRC300, ProfileId, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    },
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
        assert!(
            error.contains("missing field") || error.contains("unknown field"),
            "{error}"
        );
        assert_regeneration_instruction(&error);
    }
}

#[test]
fn a_current_spec_missing_a_current_field_is_refused() {
    let current = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2");
    for (domain, field) in [("focus", "zones"), ("zoom", "optical_ratio")] {
        let mut value = serde_json::to_value(&current).expect("serialize");
        value["capabilities"][domain]
            .as_object_mut()
            .expect("capability domain object")
            .remove(field)
            .expect("required current field");
        let error = refusal(&value.to_string());
        assert!(
            error.contains(&format!("missing field `{field}`")),
            "{field}: {error}"
        );
        assert_regeneration_instruction(&error);
    }
}

fn all_current_specs() -> Vec<ProfileSpec> {
    let mut specs = vec![
        ProfileSpec::from_compile_time::<GenericVisca>().expect("GenericVisca"),
        ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PtzOpticsG2"),
        ProfileSpec::from_compile_time::<PtzOpticsG3>().expect("PtzOpticsG3"),
        ProfileSpec::from_compile_time::<PtzOptics30X>().expect("PtzOptics30X"),
        ProfileSpec::from_compile_time::<SonyEVIH100>().expect("SonyEVIH100"),
        ProfileSpec::from_compile_time::<SonyBRC300>().expect("SonyBRC300"),
        ProfileSpec::from_compile_time::<NearusBRC300>().expect("NearusBRC300"),
        ProfileSpec::from_compile_time::<SonyBRCH900>().expect("SonyBRCH900"),
        ProfileSpec::from_compile_time::<SonyFR7>().expect("SonyFR7"),
    ];
    let covered: std::collections::HashSet<_> = specs
        .iter()
        .map(|spec| spec.capabilities().profile_id.expect("builtin identity"))
        .collect();
    assert_eq!(covered.len(), ProfileId::all().len());
    assert!(ProfileId::all().iter().all(|id| covered.contains(id)));

    // Both custom fixtures retain the complete current persisted shape,
    // including explicit nulls, with independent Raw and Sony facts.
    for (index, name) in [(1, "Custom raw camera"), (8, "Custom Sony camera")] {
        let mut custom = serde_json::to_value(&specs[index]).expect("serialize fixture");
        custom["capabilities"]["profile_id"] = serde_json::Value::Null;
        custom["capabilities"]["model_name"] = name.into();
        specs.push(serde_json::from_value(custom).expect("current custom fixture"));
    }
    specs
}

/// Enumerate every object key, including nested structs inside arrays.
/// The parent is a JSON pointer; the key is kept unescaped for removal.
fn current_key_paths(value: &serde_json::Value, parent: &str, paths: &mut Vec<(String, String)>) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, child) in fields {
                paths.push((parent.into(), key.clone()));
                let escaped = key.replace('~', "~0").replace('/', "~1");
                current_key_paths(child, &format!("{parent}/{escaped}"), paths);
            }
        }
        serde_json::Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                current_key_paths(child, &format!("{parent}/{index}"), paths);
            }
        }
        _ => {}
    }
}

#[test]
fn removing_any_current_key_is_refused_for_every_builtin_and_custom_spec() {
    for spec in all_current_specs() {
        let original = serde_json::to_value(&spec).expect("serialize current spec");
        let mut paths = Vec::new();
        current_key_paths(&original, "", &mut paths);
        assert!(paths.len() > 100, "walk all nested current facts");
        for (parent, key) in paths {
            let mut shortened = original.clone();
            shortened
                .pointer_mut(&parent)
                .and_then(serde_json::Value::as_object_mut)
                .expect("enumerated object parent")
                .remove(&key)
                .expect("enumerated current key");
            let error = refusal(&shortened.to_string());
            // Externally tagged enums require their sole variant key rather
            // than reporting it as a missing struct field.
            let missing_fact = if parent == "/capabilities/pan_tilt/tilt_ranges" {
                error.contains("expected value")
            } else {
                error.contains("missing field")
            };
            assert!(missing_fact, "{} {parent}/{key}: {error}", spec.name());
            assert_regeneration_instruction(&error);
        }
    }
}

#[test]
fn explicit_null_optional_facts_still_round_trip() {
    for spec in all_current_specs() {
        let json = serde_json::to_string(&spec).expect("serialize current spec");
        assert!(
            json.contains("null"),
            "fixture must exercise declared absence"
        );
        let restored: ProfileSpec = serde_json::from_str(&json).expect("explicit null is present");
        assert_eq!(restored, spec, "{}", spec.name());
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
