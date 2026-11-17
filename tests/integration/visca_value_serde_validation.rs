//! Serde invariants for constrained value wrappers.

#[cfg(feature = "serde")]
mod serde_validation {
    use grafton_visca::wire::WireValue;
    use grafton_visca::{
        capabilities::CapabilityRange,
        types::{
            ExposureCompensationLevel, FocusSpeed, GainLevel, NoiseReduction2DLevel, PresetNumber,
            PresetSpeed, ShutterSpeed, ZoomPosition,
        },
        value,
    };

    #[grafton_visca::value(min = 10, max = 12)]
    /// Test-only downstream range wrapper.
    pub struct MacroRange(u8);

    #[value(min = 1, max = 4)]
    struct RangeValue(u8);

    #[value(valid_values = [0x01, 0x03])]
    struct SparseValue(u8);

    #[cfg(feature = "schemars")]
    #[derive(serde::Serialize, schemars::JsonSchema)]
    struct BuiltinRangeSchemaContainer {
        preset: PresetNumber,
        recall: PresetSpeed,
        focus: FocusSpeed,
    }

    #[cfg(feature = "schemars")]
    #[derive(serde::Serialize, schemars::JsonSchema)]
    struct ScalarDomainSchemaContainer {
        gain: GainLevel,
        compensation: ExposureCompensationLevel,
        kelvin: grafton_visca::types::Kelvin,
        unknown_effect: grafton_visca::types::UnknownPictureEffect,
    }

    #[test]
    fn deserialization_preserves_range_bounds_and_scalar_representation() {
        let minimum: RangeValue = serde_json::from_str("1").expect("minimum is valid");
        let maximum: RangeValue = serde_json::from_str("4").expect("maximum is valid");

        assert_eq!(minimum.value(), 1);
        assert_eq!(maximum.value(), 4);
        assert_eq!(
            serde_json::to_string(&maximum).expect("serialize maximum"),
            "4"
        );
        assert!(serde_json::from_str::<RangeValue>("0").is_err());
        assert!(serde_json::from_str::<RangeValue>("5").is_err());
    }

    #[test]
    fn deserialization_rejects_sparse_values_outside_the_declared_set() {
        let first: SparseValue = serde_json::from_str("1").expect("first sparse value is valid");
        let last: SparseValue = serde_json::from_str("3").expect("last sparse value is valid");

        assert_eq!(first.value(), 1);
        assert_eq!(last.value(), 3);
        assert_eq!(
            serde_json::to_string(&last).expect("serialize sparse value"),
            "3"
        );
        assert!(serde_json::from_str::<SparseValue>("0").is_err());
        assert!(serde_json::from_str::<SparseValue>("2").is_err());
    }

    #[test]
    fn noise_reduction_2d_255_cannot_bypass_validation() {
        assert!(serde_json::from_str::<NoiseReduction2DLevel>("255").is_err());
    }

    #[test]
    fn public_visca_value_representatives_revalidate_deserialized_scalars() {
        let gain_min: GainLevel = serde_json::from_str("0").expect("minimum gain");
        let gain_max: GainLevel = serde_json::from_str("25").expect("maximum gain");
        assert_eq!(gain_min.value(), 0);
        assert_eq!(gain_max.value(), 25);
        assert!(serde_json::from_str::<GainLevel>("26").is_err());

        let zoom_min: ZoomPosition = serde_json::from_str("0").expect("minimum zoom");
        let zoom_max: ZoomPosition = serde_json::from_str("32767").expect("maximum zoom");
        assert_eq!(zoom_min.value(), 0);
        assert_eq!(zoom_max.value(), 0x7fff);
        assert!(serde_json::from_str::<ZoomPosition>("32768").is_err());

        // The `0p 0q` shutter field (#819).
        let shutter_min: ShutterSpeed = serde_json::from_str("0").expect("minimum shutter");
        let shutter_max: ShutterSpeed = serde_json::from_str("255").expect("maximum shutter");
        assert_eq!(shutter_min.value(), 0);
        assert_eq!(shutter_max.value(), 0xFF);
        assert!(serde_json::from_str::<ShutterSpeed>("256").is_err());
    }

    #[test]
    fn exposure_compensation_deserialization_rejects_invalid_and_overflowing_values() {
        let minimum: ExposureCompensationLevel =
            serde_json::from_str("-7").expect("minimum compensation");
        let maximum: ExposureCompensationLevel =
            serde_json::from_str("7").expect("maximum compensation");

        assert_eq!(i8::from(minimum), -7);
        assert_eq!(i8::from(maximum), 7);
        assert_eq!(minimum.to_wire(), 0);
        assert_eq!(maximum.to_wire(), 14);
        assert_eq!(serde_json::to_string(&minimum).expect("scalar JSON"), "-7");
        assert!(serde_json::from_str::<ExposureCompensationLevel>("127").is_err());
        assert!(serde_json::from_str::<ExposureCompensationLevel>("128").is_err());
    }

    #[test]
    fn range_macro_deserialization_uses_the_validated_constructor() {
        let minimum: MacroRange = serde_json::from_str("10").expect("macro minimum");
        let maximum: MacroRange = serde_json::from_str("12").expect("macro maximum");
        assert_eq!(minimum.value(), 10);
        assert_eq!(maximum.value(), 12);
        assert_eq!(serde_json::to_string(&maximum).expect("scalar JSON"), "12");
        assert!(serde_json::from_str::<MacroRange>("9").is_err());
        assert!(serde_json::from_str::<MacroRange>("13").is_err());
    }

    #[test]
    fn in_tree_range_types_reject_out_of_range_deserialization() {
        let recall_min: PresetSpeed =
            serde_json::from_str("1").expect("minimum preset recall speed");
        let recall_max: PresetSpeed =
            serde_json::from_str("50").expect("maximum union preset speed");
        assert_eq!(recall_min.value(), 1);
        assert_eq!(recall_max.value(), 50);
        assert!(serde_json::from_str::<PresetSpeed>("0").is_err());
        assert!(serde_json::from_str::<PresetSpeed>("51").is_err());

        let focus_min: FocusSpeed = serde_json::from_str("0").expect("minimum focus speed");
        let focus_max: FocusSpeed = serde_json::from_str("7").expect("maximum focus speed");
        assert_eq!(focus_min.value(), 0);
        assert_eq!(focus_max.value(), 7);
        assert!(serde_json::from_str::<FocusSpeed>("8").is_err());

        let full_domain: PresetNumber =
            serde_json::from_str("255").expect("full-domain preset remains valid");
        assert_eq!(full_domain.value(), u8::MAX);
    }

    #[test]
    fn capability_range_deserialization_preserves_shape_and_rejects_inversion_without_panicking() {
        let range: CapabilityRange<i16> =
            serde_json::from_str(r#"{"min":-20,"max":30}"#).expect("ordered range");
        assert_eq!(range.min(), -20);
        assert_eq!(range.max(), 30);
        assert_eq!(
            serde_json::to_value(range).expect("serialize capability range"),
            serde_json::json!({"min": -20, "max": 30})
        );

        let inverted = std::panic::catch_unwind(|| {
            serde_json::from_str::<CapabilityRange<i16>>(r#"{"min":30,"max":-20}"#)
        });
        assert!(
            inverted
                .expect("deserialization must return instead of panicking")
                .is_err(),
            "inverted endpoints must be rejected"
        );
    }

    #[cfg(feature = "schemars")]
    #[test]
    fn validation_preserves_scalar_json_schema_representation() {
        let schema = schemars::schema_for!(NoiseReduction2DLevel);
        let json = serde_json::to_value(schema).expect("serialize generated schema");

        assert_eq!(
            json.pointer("/type").and_then(|value| value.as_str()),
            Some("integer")
        );
    }

    #[cfg(feature = "schemars")]
    #[test]
    fn remaining_constrained_types_keep_scalar_json_schemas() {
        fn assert_integer_schema<T: schemars::JsonSchema>() {
            let schema = schemars::schema_for!(T);
            let json = serde_json::to_value(schema).expect("serialize generated schema");
            assert_eq!(
                json.pointer("/type").and_then(|value| value.as_str()),
                Some("integer"),
                "schema was not scalar: {json}"
            );
        }

        assert_integer_schema::<ExposureCompensationLevel>();
        assert_integer_schema::<PresetSpeed>();
        assert_integer_schema::<FocusSpeed>();
    }

    #[cfg(feature = "schemars")]
    #[test]
    fn range_macro_types_keep_real_newtype_schema_metadata_and_references() {
        use schemars::JsonSchema;

        assert!(!PresetNumber::inline_schema());
        assert!(!PresetSpeed::inline_schema());
        assert!(!FocusSpeed::inline_schema());
        assert_eq!(PresetNumber::schema_name(), "PresetNumber");
        assert_eq!(PresetSpeed::schema_name(), "PresetSpeed");
        assert_eq!(FocusSpeed::schema_name(), "FocusSpeed");

        let schema = schemars::schema_for!(BuiltinRangeSchemaContainer);
        let json = serde_json::to_value(schema).expect("serialize range container schema");
        let instance = serde_json::to_value(BuiltinRangeSchemaContainer {
            preset: PresetNumber::new(1).expect("valid preset"),
            recall: PresetSpeed::new(12).expect("valid recall speed"),
            focus: FocusSpeed::new(1).expect("valid focus speed"),
        })
        .expect("serialize range container");
        for (field, definition) in [
            ("preset", "PresetNumber"),
            ("recall", "PresetSpeed"),
            ("focus", "FocusSpeed"),
        ] {
            assert_eq!(
                json.pointer(&format!("/properties/{field}/$ref")),
                Some(&serde_json::json!(format!("#/$defs/{definition}"))),
                "missing newtype reference for {field}: {json}"
            );
            assert_eq!(
                json.pointer(&format!("/$defs/{definition}/type")),
                Some(&serde_json::json!("integer"))
            );
            assert!(
                instance[field].is_u64(),
                "{field} must serialize as the integer its schema describes: {instance}"
            );
        }

        assert_eq!(
            json.pointer("/$defs/PresetNumber/description")
                .and_then(serde_json::Value::as_str)
                .map(|docs| docs.lines().next()),
            Some(Some(
                "Zero-based preset number over the union of supported profile domains."
            ))
        );
        assert_eq!(
            json.pointer("/$defs/PresetSpeed/description")
                .and_then(serde_json::Value::as_str)
                .map(|docs| docs.lines().next()),
            Some(Some(
                "An FR7 preset recall speed, from 1 through 50 (R7 p. 19)."
            ))
        );
        assert_eq!(
            json.pointer("/$defs/FocusSpeed/description")
                .and_then(serde_json::Value::as_str)
                .map(|docs| docs.lines().next()),
            Some(Some(
                "Variable focus speed, from zero (slowest movement) through seven (fastest)."
            ))
        );

        // Named references retain the declared domain rather than u8 storage bounds.
        assert_eq!(
            json.pointer("/$defs/PresetSpeed/minimum"),
            Some(&serde_json::json!(1))
        );
        assert_eq!(
            json.pointer("/$defs/PresetSpeed/maximum"),
            Some(&serde_json::json!(50))
        );
    }

    #[cfg(feature = "schemars")]
    fn assert_schema_integer_domain<T: serde::de::DeserializeOwned>(
        schema: &serde_json::Value,
        inputs: impl IntoIterator<Item = i64>,
        constructor_accepts: impl Fn(i64) -> bool,
    ) {
        assert_eq!(schema["type"], "integer");
        let minimum = schema["minimum"].as_i64().expect("integer minimum");
        let maximum = schema["maximum"].as_i64().expect("integer maximum");
        let members = schema.get("enum").map(|values| {
            values
                .as_array()
                .expect("sparse schema membership is an array")
        });

        for input in inputs {
            let json = serde_json::json!(input);
            let admitted = input >= minimum
                && input <= maximum
                && members.is_none_or(|values| values.contains(&json));
            let expected = constructor_accepts(input);
            assert_eq!(admitted, expected, "schema disagrees at {input}: {schema}");
            assert_eq!(
                serde_json::from_value::<T>(json).is_ok(),
                expected,
                "checked serde disagrees at {input}"
            );
        }
    }

    #[cfg(feature = "schemars")]
    #[test]
    fn scalar_root_and_nested_schema_domains_agree_with_checked_input() {
        use grafton_visca::types::{Kelvin, UnknownPictureEffect};

        assert!(
            UnknownPictureEffect::new(255).is_err(),
            "FF is the frame terminator"
        );
        assert!(serde_json::from_str::<UnknownPictureEffect>("255").is_err());
        let nested = serde_json::to_value(schemars::schema_for!(ScalarDomainSchemaContainer))
            .expect("nested scalar schemas");
        let instance = serde_json::to_value(ScalarDomainSchemaContainer {
            gain: GainLevel::new(25).expect("maximum gain"),
            compensation: ExposureCompensationLevel::new(-7).expect("minimum compensation"),
            kelvin: Kelvin::new(2000).expect("minimum Kelvin"),
            unknown_effect: UnknownPictureEffect::new(254).expect("extension effect"),
        })
        .expect("scalar container JSON");
        assert_eq!(
            instance,
            serde_json::json!({
                "gain": 25, "compensation": -7, "kelvin": 2000, "unknown_effect": 254
            })
        );
        for (field, definition) in [
            ("gain", "GainLevel"),
            ("compensation", "ExposureCompensationLevel"),
            ("kelvin", "Kelvin"),
            ("unknown_effect", "UnknownPictureEffect"),
        ] {
            assert_eq!(
                nested.pointer(&format!("/properties/{field}/$ref")),
                Some(&serde_json::json!(format!("#/$defs/{definition}")))
            );
        }

        for schema in [
            serde_json::to_value(schemars::schema_for!(GainLevel)).expect("gain schema"),
            nested["$defs"]["GainLevel"].clone(),
        ] {
            assert_eq!(schema["minimum"], 0);
            assert_eq!(schema["maximum"], 25);
            assert_schema_integer_domain::<GainLevel>(&schema, -1..=256, |input| {
                u8::try_from(input).is_ok_and(|value| GainLevel::new(value).is_ok())
            });
        }
        for schema in [
            serde_json::to_value(schemars::schema_for!(ExposureCompensationLevel))
                .expect("compensation schema"),
            nested["$defs"]["ExposureCompensationLevel"].clone(),
        ] {
            assert_eq!(schema["minimum"], -7);
            assert_eq!(schema["maximum"], 7);
            assert_schema_integer_domain::<ExposureCompensationLevel>(
                &schema,
                -129..=128,
                |input| {
                    i8::try_from(input)
                        .is_ok_and(|value| ExposureCompensationLevel::new(value).is_ok())
                },
            );
        }
        for schema in [
            serde_json::to_value(schemars::schema_for!(Kelvin)).expect("Kelvin schema"),
            nested["$defs"]["Kelvin"].clone(),
        ] {
            assert_eq!(schema["minimum"], 2000);
            assert_eq!(schema["maximum"], 15000);
            assert_schema_integer_domain::<Kelvin>(&schema, -1..=65536, |input| {
                u16::try_from(input).is_ok_and(|value| Kelvin::new(value).is_ok())
            });
        }
        for schema in [
            serde_json::to_value(schemars::schema_for!(UnknownPictureEffect))
                .expect("picture effect schema"),
            nested["$defs"]["UnknownPictureEffect"].clone(),
        ] {
            let members = schema["enum"].as_array().expect("exact sparse membership");
            assert_eq!(members.len(), 252);
            for excluded in [0, 2, 4, 255] {
                assert!(!members.contains(&serde_json::json!(excluded)));
            }
            assert_schema_integer_domain::<UnknownPictureEffect>(&schema, -1..=256, |input| {
                u8::try_from(input).is_ok_and(|value| UnknownPictureEffect::new(value).is_ok())
            });
        }
    }

    #[cfg(feature = "ts-rs")]
    #[test]
    fn remaining_constrained_types_keep_scalar_typescript_bindings() {
        use ts_rs::TS;

        let cfg = ts_rs::Config::default();
        let exposure = ExposureCompensationLevel::export_to_string(&cfg)
            .expect("export ExposureCompensationLevel");
        let recall = PresetSpeed::export_to_string(&cfg).expect("export PresetSpeed");
        let focus = FocusSpeed::export_to_string(&cfg).expect("export FocusSpeed");

        assert!(exposure.contains("export type ExposureCompensationLevel = number;"));
        assert!(recall.contains("export type PresetSpeed = number;"));
        assert!(focus.contains("export type FocusSpeed = number;"));
    }
}
