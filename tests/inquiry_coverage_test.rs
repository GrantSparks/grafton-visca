//! Test to ensure all inquiry command structs have proper implementations
//!
//! This test provides compile-time verification that every inquiry command
//! struct can be properly constructed and has valid implementations.

use grafton_visca::command::{Command, *};

#[test]
fn all_inquiry_structs_have_implementations() {
    // Test PowerInquiry
    let cmd = PowerInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "PowerInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "PowerInquiry should have response type"
    );

    // Test PanTiltPositionInquiry
    let cmd = PanTiltPositionInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(
        !bytes.is_empty(),
        "PanTiltPositionInquiry returned empty bytes"
    );
    assert!(
        cmd.response_type().is_some(),
        "PanTiltPositionInquiry should have response type"
    );

    // Test ZoomPositionInquiry
    let cmd = ZoomPositionInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(
        !bytes.is_empty(),
        "ZoomPositionInquiry returned empty bytes"
    );
    assert!(
        cmd.response_type().is_some(),
        "ZoomPositionInquiry should have response type"
    );

    // Test FocusPositionInquiry
    let cmd = FocusPositionInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(
        !bytes.is_empty(),
        "FocusPositionInquiry returned empty bytes"
    );
    assert!(
        cmd.response_type().is_some(),
        "FocusPositionInquiry should have response type"
    );

    // Test ExposureModeInquiry
    let cmd = ExposureModeInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(
        !bytes.is_empty(),
        "ExposureModeInquiry returned empty bytes"
    );
    assert!(
        cmd.response_type().is_some(),
        "ExposureModeInquiry should have response type"
    );

    // Test WhiteBalanceModeInquiry
    let cmd = WhiteBalanceModeInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(
        !bytes.is_empty(),
        "WhiteBalanceModeInquiry returned empty bytes"
    );
    assert!(
        cmd.response_type().is_some(),
        "WhiteBalanceModeInquiry should have response type"
    );

    // Test LuminanceInquiry
    let cmd = LuminanceInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "LuminanceInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "LuminanceInquiry should have response type"
    );

    // Test ContrastInquiry
    let cmd = ContrastInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "ContrastInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "ContrastInquiry should have response type"
    );

    // Test SharpnessInquiry
    let cmd = SharpnessInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "SharpnessInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "SharpnessInquiry should have response type"
    );

    // Test ExposureCompensationInquiry
    let cmd = ExposureCompensationInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(
        !bytes.is_empty(),
        "ExposureCompensationInquiry returned empty bytes"
    );
    assert!(
        cmd.response_type().is_some(),
        "ExposureCompensationInquiry should have response type"
    );

    // Test ExposureCompensationModeInquiry
    let cmd = ExposureCompensationModeInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(
        !bytes.is_empty(),
        "ExposureCompensationModeInquiry returned empty bytes"
    );
    assert!(
        cmd.response_type().is_some(),
        "ExposureCompensationModeInquiry should have response type"
    );

    // Test IrisInquiry
    let cmd = IrisInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "IrisInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "IrisInquiry should have response type"
    );

    // Test ShutterInquiry
    let cmd = ShutterInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "ShutterInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "ShutterInquiry should have response type"
    );

    // Test BrightInquiry
    let cmd = BrightInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "BrightInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "BrightInquiry should have response type"
    );

    // Test GainInquiry
    let cmd = GainInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "GainInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "GainInquiry should have response type"
    );

    // Test GainLimitInquiry
    let cmd = GainLimitInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "GainLimitInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "GainLimitInquiry should have response type"
    );

    // Test AntiFlickerInquiry
    let cmd = AntiFlickerInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "AntiFlickerInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "AntiFlickerInquiry should have response type"
    );

    // Test SaturationInquiry
    let cmd = SaturationInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "SaturationInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "SaturationInquiry should have response type"
    );

    // Test HueInquiry
    let cmd = HueInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "HueInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "HueInquiry should have response type"
    );

    // Test RedGainInquiry
    let cmd = RedGainInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "RedGainInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "RedGainInquiry should have response type"
    );

    // Test BlueGainInquiry
    let cmd = BlueGainInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "BlueGainInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "BlueGainInquiry should have response type"
    );

    // Test BacklightInquiry
    let cmd = BacklightInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "BacklightInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "BacklightInquiry should have response type"
    );

    // Test ImageFlipInquiry
    let cmd = ImageFlipInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "ImageFlipInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "ImageFlipInquiry should have response type"
    );

    // Test SharpnessModeInquiry
    let cmd = SharpnessModeInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(
        !bytes.is_empty(),
        "SharpnessModeInquiry returned empty bytes"
    );
    assert!(
        cmd.response_type().is_some(),
        "SharpnessModeInquiry should have response type"
    );

    // Test ColorTemperatureInquiry
    let cmd = ColorTemperatureInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(
        !bytes.is_empty(),
        "ColorTemperatureInquiry returned empty bytes"
    );
    assert!(
        cmd.response_type().is_some(),
        "ColorTemperatureInquiry should have response type"
    );

    // Test NoiseReduction2DInquiry
    let cmd = NoiseReduction2DInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(
        !bytes.is_empty(),
        "NoiseReduction2DInquiry returned empty bytes"
    );
    assert!(
        cmd.response_type().is_some(),
        "NoiseReduction2DInquiry should have response type"
    );

    // Test NoiseReduction3DInquiry
    let cmd = NoiseReduction3DInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(
        !bytes.is_empty(),
        "NoiseReduction3DInquiry returned empty bytes"
    );
    assert!(
        cmd.response_type().is_some(),
        "NoiseReduction3DInquiry should have response type"
    );

    // Test BlackWhiteInquiry
    let cmd = BlackWhiteInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "BlackWhiteInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "BlackWhiteInquiry should have response type"
    );

    // Test FocusZoneInquiry
    let cmd = FocusZoneInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "FocusZoneInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "FocusZoneInquiry should have response type"
    );

    // Test AutoFocusSensitivityInquiry
    let cmd = AutoFocusSensitivityInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(
        !bytes.is_empty(),
        "AutoFocusSensitivityInquiry returned empty bytes"
    );
    assert!(
        cmd.response_type().is_some(),
        "AutoFocusSensitivityInquiry should have response type"
    );

    // Test FocusNearLimitInquiry
    let cmd = FocusNearLimitInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(
        !bytes.is_empty(),
        "FocusNearLimitInquiry returned empty bytes"
    );
    assert!(
        cmd.response_type().is_some(),
        "FocusNearLimitInquiry should have response type"
    );

    // Test DynamicRangeInquiry
    let cmd = DynamicRangeInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(
        !bytes.is_empty(),
        "DynamicRangeInquiry returned empty bytes"
    );
    assert!(
        cmd.response_type().is_some(),
        "DynamicRangeInquiry should have response type"
    );

    // Test VersionInquiry
    let cmd = VersionInquiry;
    let bytes = cmd.try_into_vec().expect("Should be able to generate bytes");
    assert!(!bytes.is_empty(), "VersionInquiry returned empty bytes");
    assert!(
        cmd.response_type().is_some(),
        "VersionInquiry should have response type"
    );
}

#[test]
fn test_inquiry_byte_commands() {
    // Test that specific inquiry structs generate the expected bytes
    assert_eq!(
        PowerInquiry.try_into_vec().unwrap(),
        vec![0x81, 0x09, 0x04, 0x00, 0xFF]
    );
    assert_eq!(
        PanTiltPositionInquiry.try_into_vec().unwrap(),
        vec![0x81, 0x09, 0x06, 0x12, 0xFF]
    );
    assert_eq!(
        ZoomPositionInquiry.try_into_vec().unwrap(),
        vec![0x81, 0x09, 0x04, 0x47, 0xFF]
    );
    assert_eq!(
        FocusPositionInquiry.try_into_vec().unwrap(),
        vec![0x81, 0x09, 0x04, 0x48, 0xFF]
    );
    assert_eq!(
        ExposureModeInquiry.try_into_vec().unwrap(),
        vec![0x81, 0x09, 0x04, 0x39, 0xFF]
    );
}
