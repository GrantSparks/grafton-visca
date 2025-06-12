//! Inquiry commands for VISCA cameras.
//!
//! This module provides commands for querying the current state of various camera settings,
//! including power status, position, zoom, focus, and other camera parameters.

// Crate imports
use crate::{
    command::{Command, ResponseType},
    error::Error,
    timeout::CommandCategory,
};

/// Commands for querying the current state of camera settings.
///
/// These commands allow reading the current values of various camera parameters
/// without modifying them. Each command returns the specific type of inquiry response
/// appropriate for the requested parameter.
#[derive(Debug, Copy, Clone)]
pub enum InquiryCommand {
    /// Query the camera's power state (On/Off).
    Power,
    /// Query the current pan and tilt position.
    PanTiltPosition,
    /// Query the current zoom position.
    ZoomPosition,
    /// Query the current focus position.
    FocusPosition,
    /// Query the current exposure mode (Auto/Manual/Shutter/Iris/Bright).
    ExposureMode,
    /// Query the current white balance mode (Auto/Indoor/Outdoor/OnePush/Manual/ColorTemperature).
    WhiteBalanceMode,
    /// Query the current luminance level (0-14).
    Luminance,
    /// Query the current contrast level (0-14).
    Contrast,
    // New inquiry commands for features added in Sprint 1
    /// Query the current sharpness level (0-11).
    Sharpness,
    /// Query the current exposure compensation value (-7 to +7).
    ExposureCompensation,
    /// Query whether exposure compensation is enabled (On/Off).
    ExposureCompensationMode,
    /// Query the current iris setting (0x0=Close to 0xC=F1.8).
    Iris,
    /// Query the current shutter speed (0x01=1/30 to 0x11=1/10000).
    Shutter,
    /// Query the current brightness level (0-17).
    Bright,
    /// Query the current gain level (0-7).
    Gain,
    /// Query the current gain limit (0-15).
    GainLimit,
    /// Query the anti-flicker mode (Off/50Hz/60Hz).
    AntiFlicker,
    /// Query the current saturation level (60%-200%).
    Saturation,
    /// Query the current hue level (0-14).
    Hue,
    /// Query the red gain tuning value (-10 to +10).
    RedGain,
    /// Query the blue gain tuning value (-10 to +10).
    BlueGain,
    /// Query whether backlight compensation is enabled (On/Off).
    Backlight,
    /// Query the current image flip state.
    ImageFlip,
    // Additional inquiry commands for new features
    /// Query the sharpness mode (Auto/Manual).
    SharpnessMode,
    /// Query the current color temperature value.
    ColorTemperature,
    /// Query the 2D noise reduction setting.
    NoiseReduction2D,
    /// Query the 3D noise reduction setting.
    NoiseReduction3D,
    /// Query the black and white mode state.
    BlackWhite,
    /// Query the current focus zone setting.
    FocusZone,
    /// Query the auto-focus sensitivity setting.
    AutoFocusSensitivity,
    /// Query the focus near limit position.
    FocusNearLimit,
    /// Query the dynamic range control level (0-8).
    DynamicRange,
}

impl Command for InquiryCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let bytes = match self {
            Self::Power => vec![0x81, 0x09, 0x04, 0x00, 0xFF],
            Self::PanTiltPosition => vec![0x81, 0x09, 0x06, 0x12, 0xFF],
            Self::ZoomPosition => vec![0x81, 0x09, 0x04, 0x47, 0xFF],
            Self::FocusPosition => vec![0x81, 0x09, 0x04, 0x48, 0xFF],
            Self::ExposureMode => vec![0x81, 0x09, 0x04, 0x39, 0xFF],
            Self::WhiteBalanceMode => vec![0x81, 0x09, 0x04, 0x35, 0xFF],
            Self::Luminance => vec![0x81, 0x09, 0x04, 0xA1, 0xFF],
            Self::Contrast => vec![0x81, 0x09, 0x04, 0xA2, 0xFF],
            // New inquiry commands
            Self::Sharpness => vec![0x81, 0x09, 0x04, 0x42, 0xFF],
            Self::ExposureCompensation => vec![0x81, 0x09, 0x04, 0x4E, 0xFF],
            Self::ExposureCompensationMode => vec![0x81, 0x09, 0x04, 0x3E, 0xFF],
            Self::Iris => vec![0x81, 0x09, 0x04, 0x4B, 0xFF],
            Self::Shutter => vec![0x81, 0x09, 0x04, 0x4A, 0xFF],
            Self::Bright => vec![0x81, 0x09, 0x04, 0x4D, 0xFF],
            Self::Gain => vec![0x81, 0x09, 0x04, 0x4C, 0xFF],
            Self::GainLimit => vec![0x81, 0x09, 0x04, 0x2C, 0xFF],
            Self::AntiFlicker => vec![0x81, 0x09, 0x04, 0x23, 0xFF],
            Self::Saturation => vec![0x81, 0x09, 0x04, 0x49, 0xFF],
            Self::Hue => vec![0x81, 0x09, 0x04, 0x4F, 0xFF],
            Self::RedGain => vec![0x81, 0x09, 0x0A, 0x12, 0xFF],
            Self::BlueGain => vec![0x81, 0x09, 0x0A, 0x13, 0xFF],
            Self::Backlight => vec![0x81, 0x09, 0x04, 0x33, 0xFF],
            Self::ImageFlip => vec![0x81, 0x09, 0x04, 0x61, 0xFF],
            // Additional inquiry commands
            Self::SharpnessMode => vec![0x81, 0x09, 0x04, 0x05, 0xFF],
            Self::ColorTemperature => vec![0x81, 0x09, 0x04, 0x20, 0xFF],
            Self::NoiseReduction2D => vec![0x81, 0x09, 0x04, 0x53, 0xFF],
            Self::NoiseReduction3D => vec![0x81, 0x09, 0x04, 0x54, 0xFF],
            Self::BlackWhite => vec![0x81, 0x09, 0x04, 0x01, 0xFF],
            Self::FocusZone => vec![0x81, 0x09, 0x04, 0x3C, 0xFF],
            Self::AutoFocusSensitivity => vec![0x81, 0x09, 0x04, 0x58, 0xFF],
            Self::FocusNearLimit => vec![0x81, 0x09, 0x04, 0x28, 0xFF],
            Self::DynamicRange => vec![0x81, 0x09, 0x04, 0x25, 0xFF],
        };
        Ok(bytes)
    }

    fn response_type(&self) -> Option<ResponseType> {
        match self {
            Self::Power => Some(ResponseType::Power),
            Self::PanTiltPosition => Some(ResponseType::PanTiltPosition),
            Self::ZoomPosition => Some(ResponseType::ZoomPosition),
            Self::FocusPosition => Some(ResponseType::FocusPosition),
            Self::ExposureMode => Some(ResponseType::ExposureMode),
            Self::WhiteBalanceMode => Some(ResponseType::WhiteBalanceMode),
            Self::Luminance => Some(ResponseType::Luminance),
            Self::Contrast => Some(ResponseType::Contrast),
            // New response types
            Self::Sharpness => Some(ResponseType::Sharpness),
            Self::ExposureCompensation => Some(ResponseType::ExposureCompensation),
            Self::ExposureCompensationMode => Some(ResponseType::ExposureCompensationMode),
            Self::Iris => Some(ResponseType::Iris),
            Self::Shutter => Some(ResponseType::Shutter),
            Self::Bright => Some(ResponseType::Bright),
            Self::Gain => Some(ResponseType::Gain),
            Self::GainLimit => Some(ResponseType::GainLimit),
            Self::AntiFlicker => Some(ResponseType::AntiFlicker),
            Self::Saturation => Some(ResponseType::Saturation),
            Self::Hue => Some(ResponseType::Hue),
            Self::RedGain => Some(ResponseType::RedGain),
            Self::BlueGain => Some(ResponseType::BlueGain),
            Self::Backlight => Some(ResponseType::Backlight),
            Self::ImageFlip => Some(ResponseType::ImageFlip),
            // Additional response types
            Self::SharpnessMode => Some(ResponseType::SharpnessMode),
            Self::ColorTemperature => Some(ResponseType::ColorTemperature),
            Self::NoiseReduction2D => Some(ResponseType::NoiseReduction2D),
            Self::NoiseReduction3D => Some(ResponseType::NoiseReduction3D),
            Self::BlackWhite => Some(ResponseType::BlackWhite),
            Self::FocusZone => Some(ResponseType::FocusZone),
            Self::AutoFocusSensitivity => Some(ResponseType::AutoFocusSensitivity),
            Self::FocusNearLimit => Some(ResponseType::FocusNearLimit),
            Self::DynamicRange => Some(ResponseType::DynamicRange),
        }
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_power_inquiry() {
        let cmd = InquiryCommand::Power;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x00, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::Power));
    }

    #[test]
    fn test_pan_tilt_position_inquiry() {
        let cmd = InquiryCommand::PanTiltPosition;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x06, 0x12, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::PanTiltPosition));
    }

    #[test]
    fn test_zoom_position_inquiry() {
        let cmd = InquiryCommand::ZoomPosition;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x47, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::ZoomPosition));
    }

    #[test]
    fn test_focus_position_inquiry() {
        let cmd = InquiryCommand::FocusPosition;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x48, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::FocusPosition));
    }

    #[test]
    fn test_exposure_mode_inquiry() {
        let cmd = InquiryCommand::ExposureMode;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x39, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::ExposureMode));
    }

    #[test]
    fn test_white_balance_mode_inquiry() {
        let cmd = InquiryCommand::WhiteBalanceMode;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x35, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::WhiteBalanceMode));
    }

    #[test]
    fn test_luminance_inquiry() {
        let cmd = InquiryCommand::Luminance;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0xA1, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::Luminance));
    }

    #[test]
    fn test_contrast_inquiry() {
        let cmd = InquiryCommand::Contrast;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0xA2, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::Contrast));
    }

    #[test]
    fn test_sharpness_inquiry() {
        let cmd = InquiryCommand::Sharpness;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x42, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::Sharpness));
    }

    #[test]
    fn test_exposure_compensation_inquiry() {
        let cmd = InquiryCommand::ExposureCompensation;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x4E, 0xFF]
        );
        assert_eq!(
            cmd.response_type(),
            Some(ResponseType::ExposureCompensation)
        );
    }

    #[test]
    fn test_exposure_compensation_mode_inquiry() {
        let cmd = InquiryCommand::ExposureCompensationMode;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x3E, 0xFF]
        );
        assert_eq!(
            cmd.response_type(),
            Some(ResponseType::ExposureCompensationMode)
        );
    }

    #[test]
    fn test_iris_inquiry() {
        let cmd = InquiryCommand::Iris;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x4B, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::Iris));
    }

    #[test]
    fn test_shutter_inquiry() {
        let cmd = InquiryCommand::Shutter;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x4A, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::Shutter));
    }

    #[test]
    fn test_bright_inquiry() {
        let cmd = InquiryCommand::Bright;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x4D, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::Bright));
    }

    #[test]
    fn test_gain_inquiry() {
        let cmd = InquiryCommand::Gain;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x4C, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::Gain));
    }

    #[test]
    fn test_gain_limit_inquiry() {
        let cmd = InquiryCommand::GainLimit;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x2C, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::GainLimit));
    }

    #[test]
    fn test_anti_flicker_inquiry() {
        let cmd = InquiryCommand::AntiFlicker;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x23, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::AntiFlicker));
    }

    #[test]
    fn test_saturation_inquiry() {
        let cmd = InquiryCommand::Saturation;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x49, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::Saturation));
    }

    #[test]
    fn test_hue_inquiry() {
        let cmd = InquiryCommand::Hue;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x4F, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::Hue));
    }

    #[test]
    fn test_red_gain_inquiry() {
        let cmd = InquiryCommand::RedGain;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x0A, 0x12, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::RedGain));
    }

    #[test]
    fn test_blue_gain_inquiry() {
        let cmd = InquiryCommand::BlueGain;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x0A, 0x13, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::BlueGain));
    }

    #[test]
    fn test_backlight_inquiry() {
        let cmd = InquiryCommand::Backlight;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x33, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::Backlight));
    }

    #[test]
    fn test_image_flip_inquiry() {
        let cmd = InquiryCommand::ImageFlip;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x61, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::ImageFlip));
    }

    #[test]
    fn test_sharpness_mode_inquiry() {
        let cmd = InquiryCommand::SharpnessMode;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x05, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::SharpnessMode));
    }

    #[test]
    fn test_color_temperature_inquiry() {
        let cmd = InquiryCommand::ColorTemperature;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x20, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::ColorTemperature));
    }

    #[test]
    fn test_noise_reduction_2d_inquiry() {
        let cmd = InquiryCommand::NoiseReduction2D;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x53, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::NoiseReduction2D));
    }

    #[test]
    fn test_noise_reduction_3d_inquiry() {
        let cmd = InquiryCommand::NoiseReduction3D;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x54, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::NoiseReduction3D));
    }

    #[test]
    fn test_black_white_inquiry() {
        let cmd = InquiryCommand::BlackWhite;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x01, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::BlackWhite));
    }

    #[test]
    fn test_focus_zone_inquiry() {
        let cmd = InquiryCommand::FocusZone;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x3C, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::FocusZone));
    }

    #[test]
    fn test_auto_focus_sensitivity_inquiry() {
        let cmd = InquiryCommand::AutoFocusSensitivity;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x58, 0xFF]
        );
        assert_eq!(
            cmd.response_type(),
            Some(ResponseType::AutoFocusSensitivity)
        );
    }

    #[test]
    fn test_focus_near_limit_inquiry() {
        let cmd = InquiryCommand::FocusNearLimit;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x28, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::FocusNearLimit));
    }

    #[test]
    fn test_dynamic_range_inquiry() {
        let cmd = InquiryCommand::DynamicRange;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x09, 0x04, 0x25, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::DynamicRange));
    }

    #[test]
    fn test_all_inquiries_have_response_types() {
        // Test that all inquiry variants have a corresponding response type
        let inquiries = vec![
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

        for inquiry in inquiries {
            assert!(
                inquiry.response_type().is_some(),
                "Inquiry {inquiry:?} should have a response type"
            );
        }
    }

    #[test]
    fn test_command_category() {
        // All inquiry commands should be Quick category
        let inquiries = vec![
            InquiryCommand::Power,
            InquiryCommand::PanTiltPosition,
            InquiryCommand::ZoomPosition,
            InquiryCommand::FocusPosition,
            InquiryCommand::ExposureMode,
            InquiryCommand::WhiteBalanceMode,
            InquiryCommand::Luminance,
            InquiryCommand::Contrast,
        ];

        for inquiry in inquiries {
            assert_eq!(inquiry.command_category(), CommandCategory::Quick);
        }
    }

    #[test]
    fn test_inquiry_command_debug() {
        // Test Debug trait implementation
        let cmd = InquiryCommand::Power;
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("Power"));

        let cmd = InquiryCommand::ZoomPosition;
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("ZoomPosition"));
    }

    #[test]
    fn test_inquiry_command_clone() {
        // Test Copy/Clone traits
        let cmd1 = InquiryCommand::Power;
        let cmd2 = cmd1; // Copy
        let cmd3 = cmd1; // Copy (clone() not needed for Copy types)

        assert_eq!(
            cmd1.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            cmd2.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        );
        assert_eq!(
            cmd1.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            cmd3.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        );
    }
}
