//! Internal inquiry command structs using derive macro.
//!
//! This module uses the InquiryCommand derive macro to generate
//! Command implementations for all inquiry types, eliminating
//! code duplication with the manual enum implementation.

use crate::command::InquiryCommand as InquiryCommandEnum;
use grafton_visca_macros::InquiryCommand;

// Power and System Inquiries

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x00, response = "Power", inquiry_variant = "Power")]
pub(crate) struct PowerInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x02, sub_command = 0x00, response = "Version", inquiry_variant = "Version")]
pub(crate) struct VersionInquiry;

// Position Inquiries

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x12, sub_command = 0x06, response = "PanTiltPosition", inquiry_variant = "PanTiltPosition")]
pub(crate) struct PanTiltPositionInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x47, response = "ZoomPosition", inquiry_variant = "ZoomPosition")]
pub(crate) struct ZoomPositionInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x48, response = "FocusPosition", inquiry_variant = "FocusPosition")]
pub(crate) struct FocusPositionInquiry;

// Exposure Inquiries

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x39, response = "ExposureMode", inquiry_variant = "ExposureMode")]
pub(crate) struct ExposureModeInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x4E, response = "ExposureCompensation", inquiry_variant = "ExposureCompensation")]
pub(crate) struct ExposureCompensationInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x3E, response = "ExposureCompensationMode", inquiry_variant = "ExposureCompensationMode")]
pub(crate) struct ExposureCompensationModeInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x4B, response = "Iris", inquiry_variant = "Iris")]
pub(crate) struct IrisInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x4A, response = "Shutter", inquiry_variant = "Shutter")]
pub(crate) struct ShutterInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x4D, response = "Bright", inquiry_variant = "Bright")]
pub(crate) struct BrightInquiry;

// White Balance and Color Inquiries

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x35, response = "WhiteBalanceMode", inquiry_variant = "WhiteBalanceMode")]
pub(crate) struct WhiteBalanceModeInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x20, response = "ColorTemperature", inquiry_variant = "ColorTemperature")]
pub(crate) struct ColorTemperatureInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x12, sub_command = 0x0A, response = "RedGain", inquiry_variant = "RedGain")]
pub(crate) struct RedGainInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x13, sub_command = 0x0A, response = "BlueGain", inquiry_variant = "BlueGain")]
pub(crate) struct BlueGainInquiry;

// Image Adjustment Inquiries

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0xA1, response = "Luminance", inquiry_variant = "Luminance")]
pub(crate) struct LuminanceInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0xA2, response = "Contrast", inquiry_variant = "Contrast")]
pub(crate) struct ContrastInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x42, response = "Sharpness", inquiry_variant = "Sharpness")]
pub(crate) struct SharpnessInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x05, response = "SharpnessMode", inquiry_variant = "SharpnessMode")]
pub(crate) struct SharpnessModeInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x49, response = "Saturation", inquiry_variant = "Saturation")]
pub(crate) struct SaturationInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x4F, response = "Hue", inquiry_variant = "Hue")]
pub(crate) struct HueInquiry;

// Gain Inquiries

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x4C, response = "Gain", inquiry_variant = "Gain")]
pub(crate) struct GainInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x2C, response = "GainLimit", inquiry_variant = "GainLimit")]
pub(crate) struct GainLimitInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x23, response = "AntiFlicker", inquiry_variant = "AntiFlicker")]
pub(crate) struct AntiFlickerInquiry;

// Image Processing Inquiries

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x33, response = "Backlight", inquiry_variant = "Backlight")]
pub(crate) struct BacklightInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x61, response = "ImageFlip", inquiry_variant = "ImageFlip")]
pub(crate) struct ImageFlipInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x01, response = "BlackWhite", inquiry_variant = "BlackWhite")]
pub(crate) struct BlackWhiteInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x53, response = "NoiseReduction2D", inquiry_variant = "NoiseReduction2D")]
pub(crate) struct NoiseReduction2DInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x54, response = "NoiseReduction3D", inquiry_variant = "NoiseReduction3D")]
pub(crate) struct NoiseReduction3DInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x25, response = "DynamicRange", inquiry_variant = "DynamicRange")]
pub(crate) struct DynamicRangeInquiry;

// Focus Inquiries

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x3C, response = "FocusZone", inquiry_variant = "FocusZone")]
pub(crate) struct FocusZoneInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x58, response = "AutoFocusSensitivity", inquiry_variant = "AutoFocusSensitivity")]
pub(crate) struct AutoFocusSensitivityInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x28, response = "FocusNearLimit", inquiry_variant = "FocusNearLimit")]
pub(crate) struct FocusNearLimitInquiry;

/// Factory function to create inquiry command instances based on the enum variant.
/// This provides a bridge between the enum-based API and the struct implementations.
pub(crate) fn create_inquiry_command(command: InquiryCommandEnum) -> Box<dyn crate::command::Command> {
    match command {
        InquiryCommandEnum::Power => Box::new(PowerInquiry),
        InquiryCommandEnum::PanTiltPosition => Box::new(PanTiltPositionInquiry),
        InquiryCommandEnum::ZoomPosition => Box::new(ZoomPositionInquiry),
        InquiryCommandEnum::FocusPosition => Box::new(FocusPositionInquiry),
        InquiryCommandEnum::ExposureMode => Box::new(ExposureModeInquiry),
        InquiryCommandEnum::WhiteBalanceMode => Box::new(WhiteBalanceModeInquiry),
        InquiryCommandEnum::Luminance => Box::new(LuminanceInquiry),
        InquiryCommandEnum::Contrast => Box::new(ContrastInquiry),
        InquiryCommandEnum::Sharpness => Box::new(SharpnessInquiry),
        InquiryCommandEnum::ExposureCompensation => Box::new(ExposureCompensationInquiry),
        InquiryCommandEnum::ExposureCompensationMode => Box::new(ExposureCompensationModeInquiry),
        InquiryCommandEnum::Iris => Box::new(IrisInquiry),
        InquiryCommandEnum::Shutter => Box::new(ShutterInquiry),
        InquiryCommandEnum::Bright => Box::new(BrightInquiry),
        InquiryCommandEnum::Gain => Box::new(GainInquiry),
        InquiryCommandEnum::GainLimit => Box::new(GainLimitInquiry),
        InquiryCommandEnum::AntiFlicker => Box::new(AntiFlickerInquiry),
        InquiryCommandEnum::Saturation => Box::new(SaturationInquiry),
        InquiryCommandEnum::Hue => Box::new(HueInquiry),
        InquiryCommandEnum::RedGain => Box::new(RedGainInquiry),
        InquiryCommandEnum::BlueGain => Box::new(BlueGainInquiry),
        InquiryCommandEnum::Backlight => Box::new(BacklightInquiry),
        InquiryCommandEnum::ImageFlip => Box::new(ImageFlipInquiry),
        InquiryCommandEnum::SharpnessMode => Box::new(SharpnessModeInquiry),
        InquiryCommandEnum::ColorTemperature => Box::new(ColorTemperatureInquiry),
        InquiryCommandEnum::NoiseReduction2D => Box::new(NoiseReduction2DInquiry),
        InquiryCommandEnum::NoiseReduction3D => Box::new(NoiseReduction3DInquiry),
        InquiryCommandEnum::BlackWhite => Box::new(BlackWhiteInquiry),
        InquiryCommandEnum::FocusZone => Box::new(FocusZoneInquiry),
        InquiryCommandEnum::AutoFocusSensitivity => Box::new(AutoFocusSensitivityInquiry),
        InquiryCommandEnum::FocusNearLimit => Box::new(FocusNearLimitInquiry),
        InquiryCommandEnum::DynamicRange => Box::new(DynamicRangeInquiry),
        InquiryCommandEnum::Version => Box::new(VersionInquiry),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ResponseType;

    #[test]
    fn test_all_inquiries_match_enum() {
        // Test each inquiry variant creates the correct command bytes
        let test_cases = vec![
            (InquiryCommandEnum::Power, vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
            (InquiryCommandEnum::PanTiltPosition, vec![0x81, 0x09, 0x06, 0x12, 0xFF]),
            (InquiryCommandEnum::ZoomPosition, vec![0x81, 0x09, 0x04, 0x47, 0xFF]),
            (InquiryCommandEnum::FocusPosition, vec![0x81, 0x09, 0x04, 0x48, 0xFF]),
            (InquiryCommandEnum::ExposureMode, vec![0x81, 0x09, 0x04, 0x39, 0xFF]),
            (InquiryCommandEnum::WhiteBalanceMode, vec![0x81, 0x09, 0x04, 0x35, 0xFF]),
            (InquiryCommandEnum::Luminance, vec![0x81, 0x09, 0x04, 0xA1, 0xFF]),
            (InquiryCommandEnum::Contrast, vec![0x81, 0x09, 0x04, 0xA2, 0xFF]),
            (InquiryCommandEnum::Sharpness, vec![0x81, 0x09, 0x04, 0x42, 0xFF]),
            (InquiryCommandEnum::ExposureCompensation, vec![0x81, 0x09, 0x04, 0x4E, 0xFF]),
            (InquiryCommandEnum::ExposureCompensationMode, vec![0x81, 0x09, 0x04, 0x3E, 0xFF]),
            (InquiryCommandEnum::Iris, vec![0x81, 0x09, 0x04, 0x4B, 0xFF]),
            (InquiryCommandEnum::Shutter, vec![0x81, 0x09, 0x04, 0x4A, 0xFF]),
            (InquiryCommandEnum::Bright, vec![0x81, 0x09, 0x04, 0x4D, 0xFF]),
            (InquiryCommandEnum::Gain, vec![0x81, 0x09, 0x04, 0x4C, 0xFF]),
            (InquiryCommandEnum::GainLimit, vec![0x81, 0x09, 0x04, 0x2C, 0xFF]),
            (InquiryCommandEnum::AntiFlicker, vec![0x81, 0x09, 0x04, 0x23, 0xFF]),
            (InquiryCommandEnum::Saturation, vec![0x81, 0x09, 0x04, 0x49, 0xFF]),
            (InquiryCommandEnum::Hue, vec![0x81, 0x09, 0x04, 0x4F, 0xFF]),
            (InquiryCommandEnum::RedGain, vec![0x81, 0x09, 0x0A, 0x12, 0xFF]),
            (InquiryCommandEnum::BlueGain, vec![0x81, 0x09, 0x0A, 0x13, 0xFF]),
            (InquiryCommandEnum::Backlight, vec![0x81, 0x09, 0x04, 0x33, 0xFF]),
            (InquiryCommandEnum::ImageFlip, vec![0x81, 0x09, 0x04, 0x61, 0xFF]),
            (InquiryCommandEnum::SharpnessMode, vec![0x81, 0x09, 0x04, 0x05, 0xFF]),
            (InquiryCommandEnum::ColorTemperature, vec![0x81, 0x09, 0x04, 0x20, 0xFF]),
            (InquiryCommandEnum::NoiseReduction2D, vec![0x81, 0x09, 0x04, 0x53, 0xFF]),
            (InquiryCommandEnum::NoiseReduction3D, vec![0x81, 0x09, 0x04, 0x54, 0xFF]),
            (InquiryCommandEnum::BlackWhite, vec![0x81, 0x09, 0x04, 0x01, 0xFF]),
            (InquiryCommandEnum::FocusZone, vec![0x81, 0x09, 0x04, 0x3C, 0xFF]),
            (InquiryCommandEnum::AutoFocusSensitivity, vec![0x81, 0x09, 0x04, 0x58, 0xFF]),
            (InquiryCommandEnum::FocusNearLimit, vec![0x81, 0x09, 0x04, 0x28, 0xFF]),
            (InquiryCommandEnum::DynamicRange, vec![0x81, 0x09, 0x04, 0x25, 0xFF]),
            (InquiryCommandEnum::Version, vec![0x81, 0x09, 0x00, 0x02, 0xFF]),
        ];

        for (variant, expected_bytes) in test_cases {
            let cmd = create_inquiry_command(variant);
            let bytes = cmd.to_bytes().expect("Failed to get bytes");
            assert_eq!(bytes, expected_bytes, "Mismatch for {:?}", variant);
        }
    }

    #[test]
    fn test_response_types_match() {
        // Test that each inquiry returns the correct response type
        let test_cases = vec![
            (InquiryCommandEnum::Power, ResponseType::Power),
            (InquiryCommandEnum::PanTiltPosition, ResponseType::PanTiltPosition),
            (InquiryCommandEnum::ZoomPosition, ResponseType::ZoomPosition),
            (InquiryCommandEnum::FocusPosition, ResponseType::FocusPosition),
            (InquiryCommandEnum::ExposureMode, ResponseType::ExposureMode),
            (InquiryCommandEnum::WhiteBalanceMode, ResponseType::WhiteBalanceMode),
            (InquiryCommandEnum::Luminance, ResponseType::Luminance),
            (InquiryCommandEnum::Contrast, ResponseType::Contrast),
            (InquiryCommandEnum::Sharpness, ResponseType::Sharpness),
            (InquiryCommandEnum::ExposureCompensation, ResponseType::ExposureCompensation),
            (InquiryCommandEnum::ExposureCompensationMode, ResponseType::ExposureCompensationMode),
            (InquiryCommandEnum::Iris, ResponseType::Iris),
            (InquiryCommandEnum::Shutter, ResponseType::Shutter),
            (InquiryCommandEnum::Bright, ResponseType::Bright),
            (InquiryCommandEnum::Gain, ResponseType::Gain),
            (InquiryCommandEnum::GainLimit, ResponseType::GainLimit),
            (InquiryCommandEnum::AntiFlicker, ResponseType::AntiFlicker),
            (InquiryCommandEnum::Saturation, ResponseType::Saturation),
            (InquiryCommandEnum::Hue, ResponseType::Hue),
            (InquiryCommandEnum::RedGain, ResponseType::RedGain),
            (InquiryCommandEnum::BlueGain, ResponseType::BlueGain),
            (InquiryCommandEnum::Backlight, ResponseType::Backlight),
            (InquiryCommandEnum::ImageFlip, ResponseType::ImageFlip),
            (InquiryCommandEnum::SharpnessMode, ResponseType::SharpnessMode),
            (InquiryCommandEnum::ColorTemperature, ResponseType::ColorTemperature),
            (InquiryCommandEnum::NoiseReduction2D, ResponseType::NoiseReduction2D),
            (InquiryCommandEnum::NoiseReduction3D, ResponseType::NoiseReduction3D),
            (InquiryCommandEnum::BlackWhite, ResponseType::BlackWhite),
            (InquiryCommandEnum::FocusZone, ResponseType::FocusZone),
            (InquiryCommandEnum::AutoFocusSensitivity, ResponseType::AutoFocusSensitivity),
            (InquiryCommandEnum::FocusNearLimit, ResponseType::FocusNearLimit),
            (InquiryCommandEnum::DynamicRange, ResponseType::DynamicRange),
            (InquiryCommandEnum::Version, ResponseType::Version),
        ];

        for (variant, expected_response) in test_cases {
            let cmd = create_inquiry_command(variant);
            let response_type = cmd.response_type();
            assert_eq!(response_type, Some(expected_response), "Mismatch for {:?}", variant);
        }
    }
}