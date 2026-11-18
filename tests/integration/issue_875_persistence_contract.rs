//! The persisted profile is a complete, closed fact graph.
#![cfg(feature = "serde")]
#![allow(clippy::expect_used)]

use grafton_visca::{
    config::OperationalTuning,
    profiles::{ProfileSpec, ProfileTiming, PtzOpticsG2},
};
use serde_json::{json, Value};
#[cfg(feature = "schemars")]
use std::collections::BTreeSet;

fn profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2")
}

fn object_paths(value: &Value, path: &str, paths: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            paths.push(path.into());
            for (key, child) in fields {
                let key = key.replace('~', "~0").replace('/', "~1");
                object_paths(child, &format!("{path}/{key}"), paths);
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                object_paths(child, &format!("{path}/{index}"), paths);
            }
        }
        _ => {}
    }
}

#[test]
fn every_nested_persisted_object_refuses_unknown_keys() {
    let original = serde_json::to_value(profile()).expect("serialize");
    let mut paths = Vec::new();
    object_paths(&original, "", &mut paths);
    assert!(
        paths.len() > 30,
        "exercise standard and profile-owned nested records"
    );
    for path in paths {
        let mut changed = original.clone();
        changed
            .pointer_mut(&path)
            .and_then(Value::as_object_mut)
            .expect("object")
            .insert("future_fact".into(), json!(true));
        let error = serde_json::from_value::<ProfileSpec>(changed)
            .expect_err("unknown facts must not be silently discarded")
            .to_string();
        // TiltRanges is externally tagged: an extra key violates the
        // single-variant map shape before field validation runs.
        let unknown_fact = if path == "/capabilities/pan_tilt/geometry/tilt_ranges" {
            error.contains("expected map with a single key")
        } else {
            error.contains("unknown field")
        };
        assert!(unknown_fact, "{path}: {error}");
        assert!(error.contains("regenerate it"), "{path}: {error}");
    }
}

#[test]
fn standalone_timing_runs_the_builder_validation() {
    let timing = profile().timing();
    let original = serde_json::to_value(timing).expect("serialize timing");
    assert_eq!(
        serde_json::from_value::<ProfileTiming>(original.clone()).expect("valid timing"),
        timing
    );
    for (field, value, reason) in [
        (
            "ack_timeout",
            json!({"secs":0,"nanos":0}),
            "protocol timeouts must be non-zero",
        ),
        (
            "raw_inquiry_reply_skew",
            json!({"secs":3600,"nanos":0}),
            "cannot exceed minimum inquiry spacing",
        ),
        (
            "inquiry_timeout",
            json!({"secs":u64::MAX,"nanos":0}),
            "representable by the monotonic clock",
        ),
    ] {
        let mut invalid = original.clone();
        invalid[field] = value;
        let error = serde_json::from_value::<ProfileTiming>(invalid)
            .expect_err("invalid timing")
            .to_string();
        assert!(error.contains(reason), "{field}: {error}");
    }
}

#[test]
fn tuning_refuses_misspellings_and_keeps_optional_overrides() {
    let error =
        serde_json::from_value::<OperationalTuning>(json!({"ack_timout":{"secs":5,"nanos":0}}))
            .expect_err("misspelled tuning must not silently select defaults")
            .to_string();
    assert!(error.contains("unknown field `ack_timout`"), "{error}");
    assert_eq!(
        serde_json::from_value::<OperationalTuning>(json!({})).expect("no overrides"),
        OperationalTuning::new()
    );
}

#[cfg(feature = "schemars")]
#[test]
fn schema_requires_every_profile_fact_and_closes_every_object() {
    fn check(value: &Value, path: &str, objects: &mut usize) {
        match value {
            Value::Object(fields) => {
                if let Some(properties) = fields.get("properties").and_then(Value::as_object) {
                    *objects += 1;
                    assert_eq!(
                        fields.get("additionalProperties"),
                        Some(&json!(false)),
                        "{path}"
                    );
                    if path == "/$defs/ProfileEncodings" {
                        // This is a sparse selection map: grants make entries
                        // mandatory during ProfileSpec validation. Its key set
                        // stays closed, while an empty grant set uses {}.
                        assert_eq!(
                            properties
                                .keys()
                                .map(String::as_str)
                                .collect::<BTreeSet<_>>(),
                            BTreeSet::from(["one_push_white_balance", "preset_recall_speed"]),
                            "{path}"
                        );
                        assert!(
                            fields.get("required").is_none_or(|required| {
                                required.as_array().is_some_and(Vec::is_empty)
                            }),
                            "{path}: standalone selections remain optional"
                        );
                    } else {
                        let actual: BTreeSet<_> = fields
                            .get("required")
                            .and_then(Value::as_array)
                            .unwrap_or_else(|| panic!("{path}: required properties"))
                            .iter()
                            .map(|v| v.as_str().expect("key"))
                            .collect();
                        assert_eq!(
                            actual,
                            properties.keys().map(String::as_str).collect(),
                            "{path}"
                        );
                    }
                }
                for (key, child) in fields {
                    check(child, &format!("{path}/{key}"), objects);
                }
            }
            Value::Array(values) => {
                for (index, child) in values.iter().enumerate() {
                    check(child, &format!("{path}/{index}"), objects);
                }
            }
            _ => {}
        }
    }
    let schema = serde_json::to_value(schemars::schema_for!(ProfileSpec)).expect("schema JSON");
    let mut objects = 0;
    check(&schema, "", &mut objects);
    assert!(
        objects > 10,
        "follow definitions, including Duration and RangeInclusive"
    );
    for path in [
        "/$defs/Capabilities/properties/profile_id",
        "/$defs/Capabilities/properties/zoom",
        "/$defs/Capabilities/properties/zoom/anyOf/0/properties/optical_ratio",
        "/$defs/TransportCompatibility/properties/tcp_port",
    ] {
        // Required optional domains are inlined beneath the null alternative.
        let property = schema
            .pointer(path)
            .expect("current capability schema path");
        let nullable = property["anyOf"]
            .as_array()
            .is_some_and(|variants| variants.iter().any(|v| v["type"] == "null"))
            || property["type"]
                .as_array()
                .is_some_and(|types| types.iter().any(|kind| kind == "null"));
        assert!(nullable, "{path}: {property}");
    }
}
