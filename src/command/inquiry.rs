//! Inquiry commands for VISCA cameras.
//!
//! This module provides commands for querying the current state of various camera settings,
//! including power status, position, zoom, focus, and other camera parameters.

// Crate imports
use crate::{
    command::{ViscaCommand, ViscaResponseType},
    error::ViscaError,
    timeout::CommandCategory,
};

/// Commands for querying the current state of camera settings.
///
/// These commands allow reading the current values of various camera parameters
/// without modifying them. Each command returns the specific type of inquiry response
/// appropriate for the requested parameter.
#[derive(Debug)]
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
    AFSensitivity,
    /// Query the focus near limit position.
    FocusNearLimit,
    /// Query the dynamic range control level (0-8).
    DynamicRange,
}

impl ViscaCommand for InquiryCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let bytes = match self {
            InquiryCommand::Power => vec![0x81, 0x09, 0x04, 0x00, 0xFF],
            InquiryCommand::PanTiltPosition => vec![0x81, 0x09, 0x06, 0x12, 0xFF],
            InquiryCommand::ZoomPosition => vec![0x81, 0x09, 0x04, 0x47, 0xFF],
            InquiryCommand::FocusPosition => vec![0x81, 0x09, 0x04, 0x48, 0xFF],
            InquiryCommand::ExposureMode => vec![0x81, 0x09, 0x04, 0x39, 0xFF],
            InquiryCommand::WhiteBalanceMode => vec![0x81, 0x09, 0x04, 0x35, 0xFF],
            InquiryCommand::Luminance => vec![0x81, 0x09, 0x04, 0xA1, 0xFF],
            InquiryCommand::Contrast => vec![0x81, 0x09, 0x04, 0xA2, 0xFF],
            // New inquiry commands
            InquiryCommand::Sharpness => vec![0x81, 0x09, 0x04, 0x42, 0xFF],
            InquiryCommand::ExposureCompensation => vec![0x81, 0x09, 0x04, 0x4E, 0xFF],
            InquiryCommand::ExposureCompensationMode => vec![0x81, 0x09, 0x04, 0x3E, 0xFF],
            InquiryCommand::Iris => vec![0x81, 0x09, 0x04, 0x4B, 0xFF],
            InquiryCommand::Shutter => vec![0x81, 0x09, 0x04, 0x4A, 0xFF],
            InquiryCommand::Bright => vec![0x81, 0x09, 0x04, 0x4D, 0xFF],
            InquiryCommand::Gain => vec![0x81, 0x09, 0x04, 0x4C, 0xFF],
            InquiryCommand::GainLimit => vec![0x81, 0x09, 0x04, 0x2C, 0xFF],
            InquiryCommand::AntiFlicker => vec![0x81, 0x09, 0x04, 0x23, 0xFF],
            InquiryCommand::Saturation => vec![0x81, 0x09, 0x04, 0x49, 0xFF],
            InquiryCommand::Hue => vec![0x81, 0x09, 0x04, 0x4F, 0xFF],
            InquiryCommand::RedGain => vec![0x81, 0x09, 0x0A, 0x12, 0xFF],
            InquiryCommand::BlueGain => vec![0x81, 0x09, 0x0A, 0x13, 0xFF],
            InquiryCommand::Backlight => vec![0x81, 0x09, 0x04, 0x33, 0xFF],
            InquiryCommand::ImageFlip => vec![0x81, 0x09, 0x04, 0x61, 0xFF],
            // Additional inquiry commands
            InquiryCommand::SharpnessMode => vec![0x81, 0x09, 0x04, 0x05, 0xFF],
            InquiryCommand::ColorTemperature => vec![0x81, 0x09, 0x04, 0x20, 0xFF],
            InquiryCommand::NoiseReduction2D => vec![0x81, 0x09, 0x04, 0x53, 0xFF],
            InquiryCommand::NoiseReduction3D => vec![0x81, 0x09, 0x04, 0x54, 0xFF],
            InquiryCommand::BlackWhite => vec![0x81, 0x09, 0x04, 0x01, 0xFF],
            InquiryCommand::FocusZone => vec![0x81, 0x09, 0x04, 0x3C, 0xFF],
            InquiryCommand::AFSensitivity => vec![0x81, 0x09, 0x04, 0x58, 0xFF],
            InquiryCommand::FocusNearLimit => vec![0x81, 0x09, 0x04, 0x28, 0xFF],
            InquiryCommand::DynamicRange => vec![0x81, 0x09, 0x04, 0x25, 0xFF],
        };
        Ok(bytes)
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        match self {
            InquiryCommand::Power => Some(ViscaResponseType::Power),
            InquiryCommand::PanTiltPosition => Some(ViscaResponseType::PanTiltPosition),
            InquiryCommand::ZoomPosition => Some(ViscaResponseType::ZoomPosition),
            InquiryCommand::FocusPosition => Some(ViscaResponseType::FocusPosition),
            InquiryCommand::ExposureMode => Some(ViscaResponseType::ExposureMode),
            InquiryCommand::WhiteBalanceMode => Some(ViscaResponseType::WhiteBalanceMode),
            InquiryCommand::Luminance => Some(ViscaResponseType::Luminance),
            InquiryCommand::Contrast => Some(ViscaResponseType::Contrast),
            // New response types
            InquiryCommand::Sharpness => Some(ViscaResponseType::Sharpness),
            InquiryCommand::ExposureCompensation => Some(ViscaResponseType::ExposureCompensation),
            InquiryCommand::ExposureCompensationMode => {
                Some(ViscaResponseType::ExposureCompensationMode)
            }
            InquiryCommand::Iris => Some(ViscaResponseType::Iris),
            InquiryCommand::Shutter => Some(ViscaResponseType::Shutter),
            InquiryCommand::Bright => Some(ViscaResponseType::Bright),
            InquiryCommand::Gain => Some(ViscaResponseType::Gain),
            InquiryCommand::GainLimit => Some(ViscaResponseType::GainLimit),
            InquiryCommand::AntiFlicker => Some(ViscaResponseType::AntiFlicker),
            InquiryCommand::Saturation => Some(ViscaResponseType::Saturation),
            InquiryCommand::Hue => Some(ViscaResponseType::Hue),
            InquiryCommand::RedGain => Some(ViscaResponseType::RedGain),
            InquiryCommand::BlueGain => Some(ViscaResponseType::BlueGain),
            InquiryCommand::Backlight => Some(ViscaResponseType::Backlight),
            InquiryCommand::ImageFlip => Some(ViscaResponseType::ImageFlip),
            // Additional response types
            InquiryCommand::SharpnessMode => Some(ViscaResponseType::SharpnessMode),
            InquiryCommand::ColorTemperature => Some(ViscaResponseType::ColorTemperature),
            InquiryCommand::NoiseReduction2D => Some(ViscaResponseType::NoiseReduction2D),
            InquiryCommand::NoiseReduction3D => Some(ViscaResponseType::NoiseReduction3D),
            InquiryCommand::BlackWhite => Some(ViscaResponseType::BlackWhite),
            InquiryCommand::FocusZone => Some(ViscaResponseType::FocusZone),
            InquiryCommand::AFSensitivity => Some(ViscaResponseType::AFSensitivity),
            InquiryCommand::FocusNearLimit => Some(ViscaResponseType::FocusNearLimit),
            InquiryCommand::DynamicRange => Some(ViscaResponseType::DynamicRange),
        }
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}
