//! Inquiry command structs using derive macro.
//!
//! This module uses the InquiryCommand derive macro to generate
//! Command implementations for all inquiry types.

use grafton_visca_macros::InquiryCommand;

// Power and System Inquiries

/// Inquiry command to get the current power state of the camera.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x00, response = "Power")]
pub struct PowerInquiry;

/// Inquiry command to get the camera version information.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x02, sub_command = 0x00, response = "Version")]
pub struct VersionInquiry;

// Position Inquiries

/// Inquiry command to get the current pan/tilt position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x12, sub_command = 0x06, response = "PanTiltPosition")]
pub struct PanTiltPositionInquiry;

/// Inquiry command to get the current zoom position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x47, response = "ZoomPosition")]
pub struct ZoomPositionInquiry;

/// Inquiry command to get the current focus position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x48, response = "FocusPosition")]
pub struct FocusPositionInquiry;

// Exposure Inquiries

/// Inquiry command to get the current exposure mode setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x39, response = "ExposureMode")]
pub struct ExposureModeInquiry;

/// Inquiry command to get the current exposure compensation value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x4E, response = "ExposureCompensation")]
pub struct ExposureCompensationInquiry;

/// Inquiry command to get the exposure compensation mode on/off status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x3E, response = "ExposureCompensationMode")]
pub struct ExposureCompensationModeInquiry;

/// Inquiry command to get the current iris position value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x4B, response = "Iris")]
pub struct IrisInquiry;

/// Inquiry command to get the current shutter speed setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x4A, response = "Shutter")]
pub struct ShutterInquiry;

/// Inquiry command to get the current brightness adjustment value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x4D, response = "Bright")]
pub struct BrightInquiry;

// White Balance and Color Inquiries

/// Inquiry command to get the current white balance mode.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x35, response = "WhiteBalanceMode")]
pub struct WhiteBalanceModeInquiry;

/// Inquiry command to get the current color temperature value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x20, response = "ColorTemperature")]
pub struct ColorTemperatureInquiry;

/// Inquiry command to get the current red gain value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x12, sub_command = 0x0A, response = "RedGain")]
pub struct RedGainInquiry;

/// Inquiry command to get the current blue gain value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x13, sub_command = 0x0A, response = "BlueGain")]
pub struct BlueGainInquiry;

// Image Adjustment Inquiries

/// Inquiry command to get the current luminance setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0xA1, response = "Luminance")]
pub struct LuminanceInquiry;

/// Inquiry command to get the current contrast level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0xA2, response = "Contrast")]
pub struct ContrastInquiry;

/// Inquiry command to get the current sharpness level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x42, response = "Sharpness")]
pub struct SharpnessInquiry;

/// Inquiry command to get the current sharpness mode on/off status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x05, response = "SharpnessMode")]
pub struct SharpnessModeInquiry;

/// Inquiry command to get the current color saturation level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x49, response = "Saturation")]
pub struct SaturationInquiry;

/// Inquiry command to get the current hue adjustment value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x4F, response = "Hue")]
pub struct HueInquiry;

// Gain Inquiries

/// Inquiry command to get the current gain value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x4C, response = "Gain")]
pub struct GainInquiry;

/// Inquiry command to get the current gain limit setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x2C, response = "GainLimit")]
pub struct GainLimitInquiry;

/// Inquiry command to get the anti-flicker mode setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x23, response = "AntiFlicker")]
pub struct AntiFlickerInquiry;

// Image Processing Inquiries

/// Inquiry command to get the backlight compensation mode.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x33, response = "Backlight")]
pub struct BacklightInquiry;

/// Inquiry command to get the image flip (mirror/reverse) settings.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x61, response = "ImageFlip")]
pub struct ImageFlipInquiry;

/// Inquiry command to get the black and white mode on/off status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x01, response = "BlackWhite")]
pub struct BlackWhiteInquiry;

/// Inquiry command to get the 2D noise reduction level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x53, response = "NoiseReduction2D")]
pub struct NoiseReduction2DInquiry;

/// Inquiry command to get the 3D noise reduction level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x54, response = "NoiseReduction3D")]
pub struct NoiseReduction3DInquiry;

/// Inquiry command to get the dynamic range mode/level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x25, response = "DynamicRange")]
pub struct DynamicRangeInquiry;

// Focus Inquiries

/// Inquiry command to get the current focus zone selection.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x3C, response = "FocusZone")]
pub struct FocusZoneInquiry;

/// Inquiry command to get the auto-focus sensitivity setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x58, response = "AutoFocusSensitivity")]
pub struct AutoFocusSensitivityInquiry;

/// Inquiry command to get the focus near limit position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x28, response = "FocusNearLimit")]
pub struct FocusNearLimitInquiry;


