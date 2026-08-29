//! Integration tests for ts-rs TypeScript type generation.
//!
//! This test module verifies that all types with `#[derive(ts_rs::TS)]` can
//! successfully generate TypeScript definitions. These tests are only compiled
//! when the `ts-rs` feature is enabled.

#![cfg(feature = "ts-rs")]

use ts_rs::{Config, TS};

use grafton_visca::{
    camera::{profiles::ProfileId, TransportOptions},
    types::{SpeedLevel, ZoomPosition},
    AffectedAxes, PanTiltDirection, PresetNumber, SubmissionClass,
};

#[test]
fn test_request_contract_exports() {
    let cfg = Config::default();
    let axes = AffectedAxes::export_to_string(&cfg).expect("Failed to export AffectedAxes");
    let qos = SubmissionClass::export_to_string(&cfg).expect("Failed to export SubmissionClass");
    assert!(!axes.is_empty());
    assert!(qos.contains("\"Background\""));
    assert!(qos.contains("\"Normal\""));
    assert!(qos.contains("\"User\""));
    assert!(!qos.contains("\"Urgent\""));
}

#[test]
fn test_transport_options_export() {
    let cfg = Config::default();
    let ts = TransportOptions::export_to_string(&cfg).expect("Failed to export TransportOptions");
    assert!(!ts.is_empty());
    // Verify it contains expected variant information
    assert!(ts.contains("type"), "Should have type tag from serde");
}

#[test]
fn test_profile_id_export() {
    let cfg = Config::default();
    let ts = ProfileId::export_to_string(&cfg).expect("Failed to export ProfileId");
    assert!(!ts.is_empty());
}

#[test]
fn test_pan_tilt_direction_export() {
    let cfg = Config::default();
    let ts = PanTiltDirection::export_to_string(&cfg).expect("Failed to export PanTiltDirection");
    assert!(!ts.is_empty());
}

#[test]
fn test_speed_level_export() {
    let cfg = Config::default();
    let ts = SpeedLevel::export_to_string(&cfg).expect("Failed to export SpeedLevel");
    assert!(!ts.is_empty());
}

#[test]
fn test_zoom_position_export() {
    let cfg = Config::default();
    let ts = ZoomPosition::export_to_string(&cfg).expect("Failed to export ZoomPosition");
    assert!(!ts.is_empty());
}

#[test]
fn test_preset_number_export() {
    let cfg = Config::default();
    let ts = PresetNumber::export_to_string(&cfg).expect("Failed to export PresetNumber");
    assert!(!ts.is_empty());
}

#[test]
fn test_profile_group_export() {
    use grafton_visca::camera::profiles::ProfileGroup;
    let cfg = Config::default();
    let ts = ProfileGroup::export_to_string(&cfg).expect("Failed to export ProfileGroup");
    assert!(!ts.is_empty());
}

#[test]
fn test_value_types_export() {
    use grafton_visca::types::{
        BrightnessLevel, ColorTemp, ContrastLevel, DefogLevel, FocusPosition, GainLevel, GainLimit,
        IrisLevel, MotionSyncSpeed, PanPosition, PanSpeed, TiltPosition, TiltSpeed,
    };

    let cfg = Config::default();

    // Test a selection of value types
    assert!(!GainLevel::export_to_string(&cfg)
        .expect("Failed to export GainLevel")
        .is_empty());
    assert!(!GainLimit::export_to_string(&cfg)
        .expect("Failed to export GainLimit")
        .is_empty());
    assert!(!IrisLevel::export_to_string(&cfg)
        .expect("Failed to export IrisLevel")
        .is_empty());
    assert!(!BrightnessLevel::export_to_string(&cfg)
        .expect("Failed to export BrightnessLevel")
        .is_empty());
    assert!(!ContrastLevel::export_to_string(&cfg)
        .expect("Failed to export ContrastLevel")
        .is_empty());
    assert!(!ColorTemp::export_to_string(&cfg)
        .expect("Failed to export ColorTemp")
        .is_empty());
    assert!(!FocusPosition::export_to_string(&cfg)
        .expect("Failed to export FocusPosition")
        .is_empty());
    assert!(!PanPosition::export_to_string(&cfg)
        .expect("Failed to export PanPosition")
        .is_empty());
    assert!(!TiltPosition::export_to_string(&cfg)
        .expect("Failed to export TiltPosition")
        .is_empty());
    assert!(!PanSpeed::export_to_string(&cfg)
        .expect("Failed to export PanSpeed")
        .is_empty());
    assert!(!TiltSpeed::export_to_string(&cfg)
        .expect("Failed to export TiltSpeed")
        .is_empty());
    assert!(!MotionSyncSpeed::export_to_string(&cfg)
        .expect("Failed to export MotionSyncSpeed")
        .is_empty());
    assert!(!DefogLevel::export_to_string(&cfg)
        .expect("Failed to export DefogLevel")
        .is_empty());
}

#[test]
fn test_enum_types_export() {
    use grafton_visca::types::{FStop, NdiQuality, NoiseReductionStrength};

    let cfg = Config::default();

    assert!(!FStop::export_to_string(&cfg)
        .expect("Failed to export FStop")
        .is_empty());
    assert!(!NdiQuality::export_to_string(&cfg)
        .expect("Failed to export NdiQuality")
        .is_empty());
    assert!(!NoiseReductionStrength::export_to_string(&cfg)
        .expect("Failed to export NoiseReductionStrength")
        .is_empty());
}
