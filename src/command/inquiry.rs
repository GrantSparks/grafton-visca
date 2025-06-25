//! Inquiry commands for VISCA cameras.
//!
//! This module provides commands for querying the current state of various camera settings,
//! including power status, position, zoom, focus, and other camera parameters.

// Crate imports
use crate::{
    command::{
        inquiry_structs::*, Command, InquiryResponse, ResponseType,
    },
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
        match self {
            Self::Power => PowerInquiry.to_bytes(),
            Self::PanTiltPosition => PanTiltPositionInquiry.to_bytes(),
            Self::ZoomPosition => ZoomPositionInquiry.to_bytes(),
            Self::FocusPosition => FocusPositionInquiry.to_bytes(),
            Self::ExposureMode => ExposureModeInquiry.to_bytes(),
            Self::WhiteBalanceMode => WhiteBalanceModeInquiry.to_bytes(),
            Self::Luminance => LuminanceInquiry.to_bytes(),
            Self::Contrast => ContrastInquiry.to_bytes(),
            Self::Sharpness => SharpnessInquiry.to_bytes(),
            Self::ExposureCompensation => ExposureCompensationInquiry.to_bytes(),
            Self::ExposureCompensationMode => ExposureCompensationModeInquiry.to_bytes(),
            Self::Iris => IrisInquiry.to_bytes(),
            Self::Shutter => ShutterInquiry.to_bytes(),
            Self::Bright => BrightInquiry.to_bytes(),
            Self::Gain => GainInquiry.to_bytes(),
            Self::GainLimit => GainLimitInquiry.to_bytes(),
            Self::AntiFlicker => Ok(vec![0x81, 0x09, 0x04, 0x23, 0xFF]),
            Self::Saturation => SaturationInquiry.to_bytes(),
            Self::Hue => HueInquiry.to_bytes(),
            Self::RedGain => RedGainInquiry.to_bytes(),
            Self::BlueGain => BlueGainInquiry.to_bytes(),
            Self::Backlight => BacklightInquiry.to_bytes(),
            Self::ImageFlip => ImageFlipInquiry.to_bytes(),
            Self::SharpnessMode => Ok(vec![0x81, 0x09, 0x04, 0x05, 0xFF]),
            Self::ColorTemperature => ColorTemperatureInquiry.to_bytes(),
            Self::NoiseReduction2D => NoiseReduction2DInquiry.to_bytes(),
            Self::NoiseReduction3D => NoiseReduction3DInquiry.to_bytes(),
            Self::BlackWhite => BlackWhiteInquiry.to_bytes(),
            Self::FocusZone => Ok(vec![0x81, 0x09, 0x04, 0x3C, 0xFF]),
            Self::AutoFocusSensitivity => Ok(vec![0x81, 0x09, 0x04, 0x58, 0xFF]),
            Self::FocusNearLimit => FocusNearLimitInquiry.to_bytes(),
            Self::DynamicRange => DynamicRangeInquiry.to_bytes(),
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        match self {
            Self::Power => PowerInquiry.response_type(),
            Self::PanTiltPosition => PanTiltPositionInquiry.response_type(),
            Self::ZoomPosition => ZoomPositionInquiry.response_type(),
            Self::FocusPosition => FocusPositionInquiry.response_type(),
            Self::ExposureMode => ExposureModeInquiry.response_type(),
            Self::WhiteBalanceMode => WhiteBalanceModeInquiry.response_type(),
            Self::Luminance => LuminanceInquiry.response_type(),
            Self::Contrast => ContrastInquiry.response_type(),
            Self::Sharpness => SharpnessInquiry.response_type(),
            Self::ExposureCompensation => ExposureCompensationInquiry.response_type(),
            Self::ExposureCompensationMode => ExposureCompensationModeInquiry.response_type(),
            Self::Iris => IrisInquiry.response_type(),
            Self::Shutter => ShutterInquiry.response_type(),
            Self::Bright => BrightInquiry.response_type(),
            Self::Gain => GainInquiry.response_type(),
            Self::GainLimit => GainLimitInquiry.response_type(),
            Self::AntiFlicker => Some(ResponseType::AntiFlicker),
            Self::Saturation => SaturationInquiry.response_type(),
            Self::Hue => HueInquiry.response_type(),
            Self::RedGain => RedGainInquiry.response_type(),
            Self::BlueGain => BlueGainInquiry.response_type(),
            Self::Backlight => BacklightInquiry.response_type(),
            Self::ImageFlip => ImageFlipInquiry.response_type(),
            Self::SharpnessMode => Some(ResponseType::SharpnessMode),
            Self::ColorTemperature => ColorTemperatureInquiry.response_type(),
            Self::NoiseReduction2D => NoiseReduction2DInquiry.response_type(),
            Self::NoiseReduction3D => NoiseReduction3DInquiry.response_type(),
            Self::BlackWhite => BlackWhiteInquiry.response_type(),
            Self::FocusZone => Some(ResponseType::FocusZone),
            Self::AutoFocusSensitivity => Some(ResponseType::AutoFocusSensitivity),
            Self::FocusNearLimit => FocusNearLimitInquiry.response_type(),
            Self::DynamicRange => DynamicRangeInquiry.response_type(),
        }
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

impl InquiryCommand {
    /// Parse the response data for this inquiry command.
    ///
    /// This method delegates to the individual inquiry structs that have
    /// parser implementations via the InquiryCommand derive macro.
    pub fn parse_response(&self, data: &[u8]) -> Result<InquiryResponse, Error> {
        match self {
            Self::Power => PowerInquiry.parse_response(data),
            Self::PanTiltPosition => PanTiltPositionInquiry.parse_response(data),
            Self::ZoomPosition => ZoomPositionInquiry.parse_response(data),
            Self::FocusPosition => FocusPositionInquiry.parse_response(data),
            Self::ExposureMode => ExposureModeInquiry.parse_response(data),
            Self::WhiteBalanceMode => WhiteBalanceModeInquiry.parse_response(data),
            Self::Luminance => LuminanceInquiry.parse_response(data),
            Self::Contrast => ContrastInquiry.parse_response(data),
            Self::Sharpness => SharpnessInquiry.parse_response(data),
            Self::ExposureCompensation => ExposureCompensationInquiry.parse_response(data),
            Self::ExposureCompensationMode => ExposureCompensationModeInquiry.parse_response(data),
            Self::Iris => IrisInquiry.parse_response(data),
            Self::Shutter => ShutterInquiry.parse_response(data),
            Self::Bright => BrightInquiry.parse_response(data),
            Self::Gain => GainInquiry.parse_response(data),
            Self::GainLimit => GainLimitInquiry.parse_response(data),
            Self::AntiFlicker => Err(Error::InvalidResponse { expected: "AntiFlicker parser not implemented".to_string(), actual: data.to_vec() }),
            Self::Saturation => SaturationInquiry.parse_response(data),
            Self::Hue => HueInquiry.parse_response(data),
            Self::RedGain => RedGainInquiry.parse_response(data),
            Self::BlueGain => BlueGainInquiry.parse_response(data),
            Self::Backlight => BacklightInquiry.parse_response(data),
            Self::ImageFlip => ImageFlipInquiry.parse_response(data),
            Self::SharpnessMode => Err(Error::InvalidResponse { expected: "SharpnessMode parser not implemented".to_string(), actual: data.to_vec() }),
            Self::ColorTemperature => ColorTemperatureInquiry.parse_response(data),
            Self::NoiseReduction2D => NoiseReduction2DInquiry.parse_response(data),
            Self::NoiseReduction3D => NoiseReduction3DInquiry.parse_response(data),
            Self::BlackWhite => BlackWhiteInquiry.parse_response(data),
            Self::FocusZone => Err(Error::InvalidResponse { expected: "FocusZone parser not implemented".to_string(), actual: data.to_vec() }),
            Self::AutoFocusSensitivity => Err(Error::InvalidResponse { expected: "AutoFocusSensitivity parser not implemented".to_string(), actual: data.to_vec() }),
            Self::FocusNearLimit => FocusNearLimitInquiry.parse_response(data),
            Self::DynamicRange => DynamicRangeInquiry.parse_response(data),
        }
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
