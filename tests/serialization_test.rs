//! Tests for serialization support of grafton-visca types

#[cfg(all(feature = "serde", feature = "schemars"))]
#[test]
fn test_preset_number_serialization() {
    use grafton_visca::PresetNumber;

    let preset = PresetNumber::new(42).expect("Valid preset number");

    // Test serialization
    let json = serde_json::to_string(&preset).expect("Serialization failed");
    assert_eq!(json, "42");

    // Test deserialization
    let deserialized: PresetNumber = serde_json::from_str(&json).expect("Deserialization failed");
    assert_eq!(deserialized.value(), 42);
}

#[cfg(all(feature = "serde", feature = "schemars"))]
#[test]
fn test_normalized_serialization() {
    use grafton_visca::units::Normalized;

    let normalized = Normalized::new(0.75_f32);

    // Test serialization
    let json = serde_json::to_string(&normalized).expect("Serialization failed");
    assert_eq!(json, "0.75");

    // Test deserialization
    let deserialized: Normalized<f32> =
        serde_json::from_str(&json).expect("Deserialization failed");
    assert_eq!(deserialized.value(), &0.75_f32);
}

#[cfg(all(feature = "serde", feature = "schemars"))]
#[test]
fn test_degrees_serialization() {
    use grafton_visca::units::Degrees;

    let degrees = Degrees::new(45.5_f32);

    // Test serialization
    let json = serde_json::to_string(&degrees).expect("Serialization failed");
    assert_eq!(json, "45.5");

    // Test deserialization
    let deserialized: Degrees<f32> = serde_json::from_str(&json).expect("Deserialization failed");
    assert_eq!(deserialized.value(), &45.5_f32);
}

#[cfg(all(feature = "serde", feature = "schemars"))]
#[test]
fn test_schema_generation() {
    use grafton_visca::{
        units::{Degrees, Normalized},
        PresetNumber,
    };
    use schemars::schema_for;

    // Verify schema can be generated
    let _preset_schema = schema_for!(PresetNumber);
    let _normalized_schema = schema_for!(Normalized<f32>);
    let _degrees_schema = schema_for!(Degrees<f32>);
}
