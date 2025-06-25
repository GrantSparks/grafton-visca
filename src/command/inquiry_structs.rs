//! Individual inquiry command structs using the InquiryCommand derive macro.
//!
//! This module contains all the individual structs that represent inquiry commands,
//! using the InquiryCommand derive macro to eliminate boilerplate code.

use grafton_visca_macros::InquiryCommand;
// Note: These imports are reserved for future use when TryFrom implementations are added
// use crate::command::{ExposureMode, AntiFlickerMode, WhiteBalanceMode, SharpnessMode, FocusZone, AutoFocusSensitivity};

// Power and Basic Controls
#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x00, response = "Power", inquiry_variant = "Power", parser = "bool")]
pub struct PowerInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x12, sub_command = 0x06, response = "PanTiltPosition", inquiry_variant = "PanTiltPosition", parser = "pan_tilt")]
pub struct PanTiltPositionInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x47, response = "ZoomPosition", inquiry_variant = "ZoomPosition", parser = "position")]
pub struct ZoomPositionInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x48, response = "FocusPosition", inquiry_variant = "FocusPosition", parser = "position")]
pub struct FocusPositionInquiry;

// Exposure Controls
#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x39, response = "ExposureMode", inquiry_variant = "ExposureMode", parser = "mode", mode_type = "ExposureMode")]
pub struct ExposureModeInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x3E, response = "ExposureCompensationMode", inquiry_variant = "ExposureCompensationMode", parser = "bool")]
pub struct ExposureCompensationModeInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x4E, response = "ExposureCompensation", inquiry_variant = "ExposureCompensation", parser = "offset", field = "value", offset = 7)]
pub struct ExposureCompensationInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x4B, response = "Iris", inquiry_variant = "Iris", parser = "direct_byte", field = "position")]
pub struct IrisInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x4A, response = "Shutter", inquiry_variant = "Shutter", parser = "nibble", field = "position")]
pub struct ShutterInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x4D, response = "Bright", inquiry_variant = "Bright", parser = "nibble", field = "position")]
pub struct BrightInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x4C, response = "Gain", inquiry_variant = "Gain", parser = "direct_byte", field = "gain")]
pub struct GainInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x2C, response = "GainLimit", inquiry_variant = "GainLimit", parser = "direct_byte", field = "limit")]
pub struct GainLimitInquiry;

// #[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
// #[visca(command = 0x23, response = "AntiFlicker", inquiry_variant = "AntiFlicker", parser = "mode", mode_type = "AntiFlickerMode")]
// pub struct AntiFlickerInquiry;

// White Balance Controls
#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x35, response = "WhiteBalanceMode", response_variant = "WhiteBalance", inquiry_variant = "WhiteBalanceMode", parser = "mode", mode_type = "WhiteBalanceMode", field = "mode")]
pub struct WhiteBalanceModeInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x20, response = "ColorTemperature", inquiry_variant = "ColorTemperature", parser = "nibble", field = "temperature")]
pub struct ColorTemperatureInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x12, sub_command = 0x0A, response = "RedGain", inquiry_variant = "RedGain", parser = "offset", field = "gain", offset = 10)]
pub struct RedGainInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x13, sub_command = 0x0A, response = "BlueGain", inquiry_variant = "BlueGain", parser = "offset", field = "gain", offset = 10)]
pub struct BlueGainInquiry;

// Image Adjustment Controls
#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0xA1, response = "Luminance", inquiry_variant = "Luminance", parser = "direct_byte")]
pub struct LuminanceInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0xA2, response = "Contrast", inquiry_variant = "Contrast", parser = "direct_byte")]
pub struct ContrastInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x42, response = "Sharpness", inquiry_variant = "Sharpness", parser = "direct_byte", field = "value")]
pub struct SharpnessInquiry;

// #[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
// #[visca(command = 0x05, response = "SharpnessMode", inquiry_variant = "SharpnessMode", parser = "mode", mode_type = "SharpnessMode")]
// pub struct SharpnessModeInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x49, response = "Saturation", inquiry_variant = "Saturation", parser = "direct_byte", field = "level")]
pub struct SaturationInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x4F, response = "Hue", inquiry_variant = "Hue", parser = "direct_byte", field = "hue")]
pub struct HueInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x33, response = "Backlight", inquiry_variant = "Backlight", parser = "bool", field = "status")]
pub struct BacklightInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x61, response = "ImageFlip", inquiry_variant = "ImageFlip", parser = "flags")]
pub struct ImageFlipInquiry;

// Noise Reduction and Effects
#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x53, response = "NoiseReduction2D", inquiry_variant = "NoiseReduction2D", parser = "direct_byte", field = "level")]
pub struct NoiseReduction2DInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x54, response = "NoiseReduction3D", inquiry_variant = "NoiseReduction3D", parser = "direct_byte", field = "level")]
pub struct NoiseReduction3DInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x01, response = "BlackWhite", inquiry_variant = "BlackWhite", parser = "bool")]
pub struct BlackWhiteInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x25, response = "DynamicRange", inquiry_variant = "DynamicRange", parser = "direct_byte", field = "level")]
pub struct DynamicRangeInquiry;

// Focus Controls
// #[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
// #[visca(command = 0x3C, response = "FocusZone", inquiry_variant = "FocusZone", parser = "mode", mode_type = "FocusZone", field = "zone")]
// pub struct FocusZoneInquiry;

// #[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
// #[visca(command = 0x58, response = "AutoFocusSensitivity", inquiry_variant = "AutoFocusSensitivity", parser = "mode", mode_type = "AutoFocusSensitivity", field = "sensitivity")]
// pub struct AutoFocusSensitivityInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone, PartialEq)]
#[visca(command = 0x28, response = "FocusNearLimit", inquiry_variant = "FocusNearLimit", parser = "position")]
pub struct FocusNearLimitInquiry;