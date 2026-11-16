//! Persisted settlement facts must admit the public inquiry they select.

#![cfg(feature = "serde")]
#![allow(clippy::expect_used)]

use grafton_visca::{
    profiles::{ProfileSpec, PtzOpticsG2},
    types::AffectedAxes,
};

#[test]
fn persisted_iris_settlement_requires_the_iris_inquiry_grant() {
    let base = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
    let base = serde_json::to_value(base).expect("profile JSON");
    for recall in [false, true] {
        let mut json = base.clone();
        json["capabilities"]["profile_id"] = serde_json::Value::Null;
        json["supports_operation_complete"] = false.into();
        json["capabilities"]["typed_support"]
            .as_array_mut()
            .expect("typed-support list")
            .retain(|surface| surface != "iris");
        if recall {
            json["position_inquiries"] = serde_json::to_value(
                AffectedAxes::PAN_TILT | AffectedAxes::ZOOM | AffectedAxes::FOCUS,
            )
            .expect("query axes");
            json["capabilities"]["presets"]["recall_axes"] =
                serde_json::to_value(AffectedAxes::IRIS).expect("recall axes");
        }
        let error = serde_json::from_value::<ProfileSpec>(json.clone())
            .expect_err("settlement cannot poll an inquiry without its grant")
            .to_string();
        let field = if recall {
            "capabilities.presets.recall_axes"
        } else {
            "position_inquiries"
        };
        assert!(error.contains(field), "{error}");
        assert!(error.contains("capabilities.typed_support"), "{error}");

        json["capabilities"]["typed_support"] = base["capabilities"]["typed_support"].clone();
        json["position_inquiries"] = base["position_inquiries"].clone();
        serde_json::from_value::<ProfileSpec>(json)
            .expect("the Iris grant permits settlement polling");
    }
}
