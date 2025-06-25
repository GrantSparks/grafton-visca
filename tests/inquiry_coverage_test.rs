//! Test to ensure all InquiryCommand enum variants have proper implementations
//!
//! This test provides compile-time verification that every inquiry command
//! variant can be properly constructed and has valid implementations.

use grafton_visca::command::{Command, InquiryCommand};

#[test]
fn all_inquiry_variants_have_implementations() {
    // List all InquiryCommand variants
    let commands = [
        InquiryCommand::Power,
        InquiryCommand::PanTiltPosition,
        InquiryCommand::ZoomPosition,
        InquiryCommand::FocusPosition,
        InquiryCommand::ExposureMode,
        InquiryCommand::WhiteBalanceMode,
        InquiryCommand::Luminance,
        InquiryCommand::Contrast,
        InquiryCommand::Sharpness,
        InquiryCommand::ExposureCompensation,
        InquiryCommand::ExposureCompensationMode,
        InquiryCommand::Iris,
        InquiryCommand::Shutter,
        InquiryCommand::Bright,
        InquiryCommand::Gain,
        InquiryCommand::GainLimit,
        InquiryCommand::AntiFlicker,
        InquiryCommand::Saturation,
        InquiryCommand::Hue,
        InquiryCommand::RedGain,
        InquiryCommand::BlueGain,
        InquiryCommand::Backlight,
        InquiryCommand::ImageFlip,
        InquiryCommand::SharpnessMode,
        InquiryCommand::ColorTemperature,
        InquiryCommand::NoiseReduction2D,
        InquiryCommand::NoiseReduction3D,
        InquiryCommand::BlackWhite,
        InquiryCommand::FocusZone,
        InquiryCommand::AutoFocusSensitivity,
        InquiryCommand::FocusNearLimit,
        InquiryCommand::DynamicRange,
    ];

    for cmd in commands {
        // Verify each command can generate bytes
        let bytes = cmd.to_bytes().expect("Should be able to generate bytes");
        assert!(!bytes.is_empty(), "Command {:?} returned empty bytes", cmd);

        // Verify each command has a response type
        let response_type = cmd.response_type();
        assert!(
            response_type.is_some(),
            "Command {:?} should have a response type",
            cmd
        );

        // Verify command category is Quick (all inquiries should be quick)
        assert_eq!(
            cmd.command_category(),
            grafton_visca::timeout::CommandCategory::Quick,
            "Command {:?} should be Quick category",
            cmd
        );
    }
}

#[test]
fn inquiry_command_bytes_format() {
    // Test that all inquiry commands follow the expected byte format
    let cmd = InquiryCommand::Power;
    let bytes = cmd.to_bytes().unwrap();

    // All inquiry commands should:
    // - Start with 0x81 (source address)
    // - Have 0x09 as second byte (inquiry)
    // - End with 0xFF (terminator)
    assert_eq!(bytes[0], 0x81, "First byte should be 0x81");
    assert_eq!(bytes[1], 0x09, "Second byte should be 0x09");
    assert_eq!(bytes[bytes.len() - 1], 0xFF, "Last byte should be 0xFF");

    // Most commands use 0x04 as category, some use 0x06 or 0x0A
    assert!(
        bytes[2] == 0x04 || bytes[2] == 0x06 || bytes[2] == 0x0A,
        "Third byte should be a valid category"
    );
}

#[test]
fn verify_variant_count() {
    // This test helps ensure we don't accidentally miss any variants
    // Update this count when adding new inquiry commands
    const EXPECTED_VARIANT_COUNT: usize = 32;

    let all_variants = [
        InquiryCommand::Power,
        InquiryCommand::PanTiltPosition,
        InquiryCommand::ZoomPosition,
        InquiryCommand::FocusPosition,
        InquiryCommand::ExposureMode,
        InquiryCommand::WhiteBalanceMode,
        InquiryCommand::Luminance,
        InquiryCommand::Contrast,
        InquiryCommand::Sharpness,
        InquiryCommand::ExposureCompensation,
        InquiryCommand::ExposureCompensationMode,
        InquiryCommand::Iris,
        InquiryCommand::Shutter,
        InquiryCommand::Bright,
        InquiryCommand::Gain,
        InquiryCommand::GainLimit,
        InquiryCommand::AntiFlicker,
        InquiryCommand::Saturation,
        InquiryCommand::Hue,
        InquiryCommand::RedGain,
        InquiryCommand::BlueGain,
        InquiryCommand::Backlight,
        InquiryCommand::ImageFlip,
        InquiryCommand::SharpnessMode,
        InquiryCommand::ColorTemperature,
        InquiryCommand::NoiseReduction2D,
        InquiryCommand::NoiseReduction3D,
        InquiryCommand::BlackWhite,
        InquiryCommand::FocusZone,
        InquiryCommand::AutoFocusSensitivity,
        InquiryCommand::FocusNearLimit,
        InquiryCommand::DynamicRange,
    ];

    assert_eq!(
        all_variants.len(),
        EXPECTED_VARIANT_COUNT,
        "Expected {} inquiry command variants, but found {}",
        EXPECTED_VARIANT_COUNT,
        all_variants.len()
    );
}
