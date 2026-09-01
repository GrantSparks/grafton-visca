//! Serde invariants for constrained value wrappers.

#[cfg(feature = "serde")]
mod serde_validation {
    use grafton_visca::{
        capabilities::CapabilityRange,
        command::{FocusSpeed as CommandFocusSpeed, PresetRecallSpeed},
        types::{
            ExposureCompensationLevel, GainLevel, NoiseReduction2DLevel, PanPosition, ShutterSpeed,
            ZoomPosition,
        },
        PresetNumber, ViscaValue,
    };

    grafton_visca::visca_range_type! {
        /// Test-only downstream range wrapper.
        MacroRange: u8 {
            min: 10,
            max: 12
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "serde", serde(try_from = "u8", into = "u8"))]
    #[visca_value(min = "1", max = "4")]
    struct RangeValue(u8);

    #[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "serde", serde(try_from = "u8", into = "u8"))]
    #[visca_value(valid_values = "[0x01, 0x03]")]
    struct SparseValue(u8);

    #[cfg(feature = "schemars")]
    #[allow(dead_code)]
    #[derive(schemars::JsonSchema)]
    struct BuiltinRangeSchemaContainer {
        preset: PresetNumber,
        recall: PresetRecallSpeed,
        focus: CommandFocusSpeed,
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
        let gain_max: GainLevel = serde_json::from_str("7").expect("maximum gain");
        assert_eq!(gain_min.value(), 0);
        assert_eq!(gain_max.value(), 7);
        assert!(serde_json::from_str::<GainLevel>("8").is_err());

        let zoom_min: ZoomPosition = serde_json::from_str("0").expect("minimum zoom");
        let zoom_max: ZoomPosition = serde_json::from_str("32767").expect("maximum zoom");
        assert_eq!(zoom_min.value(), 0);
        assert_eq!(zoom_max.value(), 0x7fff);
        assert!(serde_json::from_str::<ZoomPosition>("32768").is_err());

        let shutter_min: ShutterSpeed = serde_json::from_str("1").expect("minimum shutter");
        let shutter_max: ShutterSpeed = serde_json::from_str("17").expect("maximum shutter");
        assert_eq!(shutter_min.value(), 1);
        assert_eq!(shutter_max.value(), 0x11);
        assert!(serde_json::from_str::<ShutterSpeed>("0").is_err());

        let pan_min: PanPosition = serde_json::from_str("-2448").expect("minimum pan");
        let pan_max: PanPosition = serde_json::from_str("2448").expect("maximum pan");
        assert_eq!(pan_min.value(), -2448);
        assert_eq!(pan_max.value(), 2448);
        assert!(serde_json::from_str::<PanPosition>("2449").is_err());
    }

    #[test]
    fn exposure_compensation_deserialization_rejects_invalid_and_overflowing_values() {
        let minimum: ExposureCompensationLevel =
            serde_json::from_str("-7").expect("minimum compensation");
        let maximum: ExposureCompensationLevel =
            serde_json::from_str("7").expect("maximum compensation");

        assert_eq!(i8::from(minimum), -7);
        assert_eq!(i8::from(maximum), 7);
        assert_eq!(minimum.to_protocol_value(), 0);
        assert_eq!(maximum.to_protocol_value(), 14);
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
        let recall_min: PresetRecallSpeed =
            serde_json::from_str("1").expect("minimum preset recall speed");
        let recall_max: PresetRecallSpeed =
            serde_json::from_str("24").expect("maximum preset recall speed");
        assert_eq!(recall_min.value(), 1);
        assert_eq!(recall_max.value(), 24);
        assert!(serde_json::from_str::<PresetRecallSpeed>("0").is_err());
        assert!(serde_json::from_str::<PresetRecallSpeed>("25").is_err());

        let focus_min: CommandFocusSpeed = serde_json::from_str("0").expect("minimum focus speed");
        let focus_max: CommandFocusSpeed = serde_json::from_str("7").expect("maximum focus speed");
        assert_eq!(focus_min.value(), 0);
        assert_eq!(focus_max.value(), 7);
        assert!(serde_json::from_str::<CommandFocusSpeed>("8").is_err());

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
        assert_integer_schema::<PresetRecallSpeed>();
        assert_integer_schema::<CommandFocusSpeed>();
    }

    #[cfg(feature = "schemars")]
    #[test]
    fn range_macro_types_keep_real_newtype_schema_metadata_and_references() {
        use schemars::JsonSchema;

        assert!(!PresetNumber::inline_schema());
        assert!(!PresetRecallSpeed::inline_schema());
        assert!(!CommandFocusSpeed::inline_schema());
        assert_eq!(PresetNumber::schema_name(), "PresetNumber");
        assert_eq!(PresetRecallSpeed::schema_name(), "PresetRecallSpeed");
        assert_eq!(CommandFocusSpeed::schema_name(), "FocusSpeed");

        let schema = schemars::schema_for!(BuiltinRangeSchemaContainer);
        let json = serde_json::to_value(schema).expect("serialize range container schema");
        for (field, definition) in [
            ("preset", "PresetNumber"),
            ("recall", "PresetRecallSpeed"),
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
        }

        assert_eq!(
            json.pointer("/$defs/PresetNumber/description")
                .and_then(serde_json::Value::as_str)
                .map(|docs| docs.lines().next()),
            Some(Some("Preset number with validation."))
        );
        assert_eq!(
            json.pointer("/$defs/PresetRecallSpeed/description")
                .and_then(serde_json::Value::as_str)
                .map(|docs| docs.lines().next()),
            Some(Some("Preset recall speed."))
        );
        assert_eq!(
            json.pointer("/$defs/FocusSpeed/description")
                .and_then(serde_json::Value::as_str)
                .map(|docs| docs.lines().next()),
            Some(Some("Variable focus speed."))
        );

        // These are the u8 schema's bounds, not the macro's validation bounds.
        assert_eq!(
            json.pointer("/$defs/PresetRecallSpeed/minimum"),
            Some(&serde_json::json!(0))
        );
        assert_eq!(
            json.pointer("/$defs/PresetRecallSpeed/maximum"),
            Some(&serde_json::json!(255))
        );
    }

    #[cfg(feature = "ts-rs")]
    #[test]
    fn remaining_constrained_types_keep_scalar_typescript_bindings() {
        use ts_rs::TS;

        let cfg = ts_rs::Config::default();
        let exposure = ExposureCompensationLevel::export_to_string(&cfg)
            .expect("export ExposureCompensationLevel");
        let recall = PresetRecallSpeed::export_to_string(&cfg).expect("export PresetRecallSpeed");
        let focus = CommandFocusSpeed::export_to_string(&cfg).expect("export FocusSpeed");

        assert!(exposure.contains("export type ExposureCompensationLevel = number;"));
        assert!(recall.contains("export type PresetRecallSpeed = number;"));
        assert!(focus.contains("export type FocusSpeed = number;"));
    }
}
