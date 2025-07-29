//! Inquiry command structs using derive macro.
//!
//! This module uses the InquiryCommand derive macro to generate
//! Command implementations for all inquiry types.

use grafton_visca_macros::InquiryCommand;

// Power and System Inquiries

/// Inquiry command to get the current power state of the camera.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x00, response = "Power", parser = "bool")]
pub struct PowerInquiry;

/// Inquiry command to get the camera version information.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x02, sub_command = 0x00, response = "Version")]
pub struct VersionInquiry;

// Position Inquiries

/// Inquiry command to get the current pan/tilt position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x12,
    sub_command = 0x06,
    response = "PanTiltPosition",
    parser = "pan_tilt"
)]
pub struct PanTiltPositionInquiry;

/// Inquiry command to get the current zoom position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x47, response = "ZoomPosition", parser = "position")]
pub struct ZoomPositionInquiry;

/// Inquiry command to get the current focus position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x48, response = "FocusPosition", parser = "position")]
pub struct FocusPositionInquiry;

// Exposure Inquiries

/// Inquiry command to get the current exposure mode setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x39,
    response = "ExposureMode",
    parser = "mode",
    type = "ExposureMode"
)]
pub struct ExposureModeInquiry;

/// Inquiry command to get the current exposure compensation value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x4E,
    response = "ExposureCompensation",
    parser = "custom",
    custom_fn = "parse_exposure_compensation"
)]
pub struct ExposureCompensationInquiry;

/// Inquiry command to get the exposure compensation mode on/off status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x3E, response = "ExposureCompensationMode", parser = "bool")]
pub struct ExposureCompensationModeInquiry;

/// Inquiry command to get the current iris position value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x4B,
    response = "Iris",
    parser = "custom",
    custom_fn = "parse_iris_last_nibble"
)]
pub struct IrisInquiry;

/// Inquiry command to get the current shutter speed setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x4A,
    response = "Shutter",
    parser = "custom",
    custom_fn = "parse_shutter"
)]
pub struct ShutterInquiry;

/// Inquiry command to get the current brightness adjustment value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x4D, response = "Bright", parser = "position")]
pub struct BrightInquiry;

// White Balance and Color Inquiries

/// Inquiry command to get the current white balance mode.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x35,
    response = "WhiteBalanceMode",
    parser = "mode",
    type = "WhiteBalanceMode"
)]
pub struct WhiteBalanceModeInquiry;

/// Inquiry command to get the current color temperature value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x20,
    response = "ColorTemperature",
    parser = "custom",
    custom_fn = "parse_color_temperature"
)]
pub struct ColorTemperatureInquiry;

/// Inquiry command to get the current red gain value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x12,
    sub_command = 0x0A,
    response = "RedChannel",
    parser = "offset",
    field = "gain",
    offset = 10
)]
pub struct RedGainInquiry;

/// Inquiry command to get the current blue gain value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x13,
    sub_command = 0x0A,
    response = "BlueChannel",
    parser = "offset",
    field = "gain",
    offset = 10
)]
pub struct BlueGainInquiry;

// Image Adjustment Inquiries

/// Inquiry command to get the current luminance setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0xA1, response = "Luminance", parser = "byte")]
pub struct LuminanceInquiry;

/// Inquiry command to get the current contrast level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0xA2, response = "Contrast", parser = "byte")]
pub struct ContrastInquiry;

/// Inquiry command to get the current sharpness level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x42,
    response = "Sharpness",
    parser = "custom",
    custom_fn = "parse_middle_nibbles"
)]
pub struct SharpnessInquiry;

/// Inquiry command to get the current sharpness mode on/off status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x05,
    response = "SharpnessMode",
    parser = "custom",
    custom_fn = "parse_sharpness_mode"
)]
pub struct SharpnessModeInquiry;

/// Inquiry command to get the current color saturation level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x49,
    response = "Saturation",
    parser = "custom",
    custom_fn = "parse_saturation_last_nibble"
)]
pub struct SaturationInquiry;

/// Inquiry command to get the current hue adjustment value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x4F,
    response = "Hue",
    parser = "custom",
    custom_fn = "parse_hue_last_nibble"
)]
pub struct HueInquiry;

// Gain Inquiries

/// Inquiry command to get the current gain value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x4C,
    response = "Gain",
    parser = "custom",
    custom_fn = "parse_gain_last_nibble"
)]
pub struct GainInquiry;

/// Inquiry command to get the current gain limit setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x2C, response = "GainLimit", parser = "byte")]
pub struct GainLimitInquiry;

// Image Processing Inquiries

/// Inquiry command to get the backlight compensation mode.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x33, response = "Backlight", parser = "bool")]
pub struct BacklightInquiry;

/// Inquiry command to get the image flip (mirror/reverse) settings.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x61, response = "ImageFlip", parser = "flags")]
pub struct ImageFlipInquiry;

/// Inquiry command to get the black and white mode on/off status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x01, response = "BlackWhite", parser = "bool")]
pub struct BlackWhiteInquiry;

/// Inquiry command to get the 2D noise reduction level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x53, response = "NoiseReduction2D", parser = "byte")]
pub struct NoiseReduction2DInquiry;

/// Inquiry command to get the 3D noise reduction level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x54, response = "NoiseReduction3D", parser = "byte")]
pub struct NoiseReduction3DInquiry;

/// Inquiry command to get the dynamic range mode/level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x25, response = "DynamicRange", parser = "byte")]
pub struct DynamicRangeInquiry;

// Focus Inquiries

/// Inquiry command to get the current focus zone selection.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x3C,
    response = "FocusZone",
    parser = "mode",
    type = "FocusZone"
)]
pub struct FocusZoneInquiry;

/// Inquiry command to get the auto-focus sensitivity setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x58,
    response = "AutoFocusSensitivity",
    parser = "mode",
    type = "AutoFocusSensitivity"
)]
pub struct AutoFocusSensitivityInquiry;

/// Inquiry command to get the focus near limit position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x28, response = "FocusNearLimit", parser = "position")]
pub struct FocusNearLimitInquiry;

/// Inquiry command to get the current focus mode (Auto/Manual).
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x38,
    response = "FocusMode",
    parser = "mode",
    type = "FocusMode"
)]
pub struct FocusModeInquiry;

// System State Inquiries

/// Inquiry command to get the menu open/close status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x06,
    response = "MenuOpenClose",
    parser = "custom",
    custom_fn = "parse_menu_open_close"
)]
pub struct MenuOpenCloseInquiry;

/// Inquiry command to get the auto focus on/off status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x18,
    response = "AutoFocus",
    parser = "custom",
    custom_fn = "parse_auto_focus"
)]
pub struct AutoFocusInquiry;

/// Inquiry command to get the tally light status (red and green).
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0xA8,
    response = "TallyStatus",
    parser = "custom",
    custom_fn = "parse_tally_status"
)]
pub struct TallyStatusInquiry;

// The TallyGreenInquiry is manually implemented below due to its special format

/// Inquiry command to get the current video resolution mode.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x63, response = "Resolution", parser = "byte")]
pub struct ResolutionInquiry;

/// Inquiry command to get the night/day mode status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x60,
    response = "NightDayMode",
    parser = "custom",
    custom_fn = "parse_night_day_mode"
)]
pub struct NightDayModeInquiry;

/// Inquiry command to get the ND filter position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x64,
    response = "NdFilter",
    parser = "custom",
    custom_fn = "parse_nd_filter"
)]
pub struct NdFilterInquiry;

/// Inquiry command to get the current picture effect mode.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x32,
    response = "PictureEffect",
    parser = "custom",
    custom_fn = "parse_picture_effect"
)]
pub struct PictureEffectInquiry;

/// Inquiry command to get the current flip mode (combined horizontal/vertical).
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x65,
    response = "FlipMode",
    parser = "custom",
    custom_fn = "parse_flip_mode"
)]
pub struct FlipModeInquiry;

/// Inquiry command to get the standby mode status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x70,
    response = "Standby",
    parser = "custom",
    custom_fn = "parse_standby"
)]
pub struct StandbyInquiry;

/// Inquiry command to get the focus range setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x2A,
    response = "FocusRange",
    parser = "custom",
    custom_fn = "parse_focus_range"
)]
pub struct FocusRangeInquiry;

/// Inquiry command to get the iris control mode.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x2B,
    response = "IrisControl",
    parser = "custom",
    custom_fn = "parse_iris_control"
)]
pub struct IrisControlInquiry;

/// Inquiry command to get the defog mode status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x37,
    response = "DefogMode",
    parser = "custom",
    custom_fn = "parse_defog_mode"
)]
pub struct DefogModeInquiry;

/// Inquiry command to get the defog level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0xA0,
    response = "DefogLevel",
    parser = "custom",
    custom_fn = "parse_defog_level"
)]
pub struct DefogLevelInquiry;

/// Inquiry command to get the digital PTZ mode status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x6B,
    response = "DigitalPtz",
    parser = "custom",
    custom_fn = "parse_digital_ptz"
)]
pub struct DigitalPtzInquiry;

// Additional Inquiries

/// Inquiry command to get the auto white balance sensitivity setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x59,
    response = "AutoWhiteBalanceSensitivity",
    parser = "custom",
    custom_fn = "parse_auto_wb_sensitivity"
)]
pub struct AutoWhiteBalanceSensitivityInquiry;

/// Inquiry command to get the exposure compensation position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x4E,
    response = "ExposureCompensationPosition",
    parser = "custom",
    custom_fn = "parse_exposure_compensation_position"
)]
pub struct ExposureCompensationPositionInquiry;

/// Inquiry command to get the red channel tuning level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x43,
    response = "RedTuning",
    parser = "custom",
    custom_fn = "parse_red_tuning"
)]
pub struct RedTuningInquiry;

/// Inquiry command to get the blue channel tuning level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x44,
    response = "BlueTuning",
    parser = "custom",
    custom_fn = "parse_blue_tuning"
)]
pub struct BlueTuningInquiry;

/// Inquiry command to get the current gamma curve setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x5B,
    response = "Gamma",
    parser = "custom",
    custom_fn = "parse_gamma"
)]
pub struct GammaInquiry;

/// Inquiry command to get the auto trace mode status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x09,
    sub_command = 0x50,
    response = "AutoTrace",
    parser = "custom",
    custom_fn = "parse_auto_trace"
)]
pub struct AutoTraceInquiry;

/// Inquiry command to get the focus unlock state.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x08,
    sub_command = 0x54,
    response = "FocusUnlock",
    parser = "custom",
    custom_fn = "parse_focus_unlock"
)]
pub struct FocusUnlockInquiry;

/// Inquiry command to get the current sharpness position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x42, response = "SharpnessPosition", parser = "position")]
pub struct SharpnessPositionInquiry;

/// Inquiry command to get the noise reduction level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x52, response = "NrLevel", parser = "byte")]
pub struct NrLevelInquiry;

/// Inquiry command to get the broadcast domain setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x75, response = "BroadcastDomain", parser = "byte")]
pub struct BroadcastDomainInquiry;

// System and Image Processing Inquiries

/// Inquiry command to get the motion sync mode setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x56,
    response = "MotionSyncMode",
    parser = "mode",
    type = "MotionSyncMode"
)]
pub struct MotionSyncModeInquiry;

/// Inquiry command to get the motion sync speed setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x57,
    response = "MotionSyncSpeed",
    parser = "speed",
    type = "MotionSyncSpeed"
)]
pub struct MotionSyncSpeedInquiry;

/// Inquiry command to get the noise reduction mode setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x53, response = "NrMode", parser = "mode", type = "NrMode")]
pub struct NrModeInquiry;

/// Inquiry command to get the noise reduction speed setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x54,
    response = "NrSpeed",
    parser = "speed",
    type = "NrSpeed"
)]
pub struct NrSpeedInquiry;

/// Inquiry command to get the black and white mode setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x73,
    response = "BlackWhiteMode",
    parser = "mode",
    type = "BlackWhiteMode"
)]
pub struct BlackWhiteModeInquiry;

// Additional System Inquiries

/// Inquiry command to get the USB audio state.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x7A, response = "UsbAudio", parser = "bool")]
pub struct UsbAudioInquiry;

/// Inquiry command to get the two tone mode state.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x74, response = "TwoToneMode", parser = "bool")]
pub struct TwoToneModeInquiry;

/// Inquiry command to get the ND filter preset setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x66, response = "NdFilterPreset", parser = "byte")]
pub struct NdFilterPresetInquiry;

/// Inquiry command to get the digital mode state.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x7B, response = "Digital", parser = "bool")]
pub struct DigitalInquiry;

/// Inquiry command to get the tally auto adjust state.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0xA9, response = "TallyAutoAdjust", parser = "bool")]
pub struct TallyAutoAdjustInquiry;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::visca_test;

    visca_test!(
        ZoomPositionInquiry,
        test_zoom_position_inquiry,
        ZoomPositionInquiry,
        &[0x81, 0x09, 0x04, 0x47, 0xFF]
    );
    visca_test!(
        FocusPositionInquiry,
        test_focus_position_inquiry,
        FocusPositionInquiry,
        &[0x81, 0x09, 0x04, 0x48, 0xFF]
    );
    visca_test!(
        PowerInquiry,
        test_power_inquiry,
        PowerInquiry,
        &[0x81, 0x09, 0x04, 0x00, 0xFF]
    );
    visca_test!(
        FocusModeInquiry,
        test_focus_mode_inquiry,
        FocusModeInquiry,
        &[0x81, 0x09, 0x04, 0x38, 0xFF]
    );
    visca_test!(
        MenuOpenCloseInquiry,
        test_menu_open_close_inquiry,
        MenuOpenCloseInquiry,
        &[0x81, 0x09, 0x04, 0x06, 0xFF]
    );
    visca_test!(
        AutoFocusInquiry,
        test_auto_focus_inquiry,
        AutoFocusInquiry,
        &[0x81, 0x09, 0x04, 0x18, 0xFF]
    );
    visca_test!(
        TallyStatusInquiry,
        test_tally_status_inquiry,
        TallyStatusInquiry,
        &[0x81, 0x09, 0x04, 0xA8, 0xFF]
    );
    visca_test!(
        TallyGreenInquiry,
        test_tally_green_inquiry,
        TallyGreenInquiry,
        &[0x81, 0x09, 0x7E, 0x04, 0x1A, 0x00, 0xFF]
    );
    visca_test!(
        ResolutionInquiry,
        test_resolution_inquiry,
        ResolutionInquiry,
        &[0x81, 0x09, 0x04, 0x63, 0xFF]
    );
    visca_test!(
        NightDayModeInquiry,
        test_night_day_mode_inquiry,
        NightDayModeInquiry,
        &[0x81, 0x09, 0x04, 0x60, 0xFF]
    );
    visca_test!(
        NdFilterInquiry,
        test_nd_filter_inquiry,
        NdFilterInquiry,
        &[0x81, 0x09, 0x04, 0x64, 0xFF]
    );
    visca_test!(
        PictureEffectInquiry,
        test_picture_effect_inquiry,
        PictureEffectInquiry,
        &[0x81, 0x09, 0x04, 0x32, 0xFF]
    );
    visca_test!(
        FlipModeInquiry,
        test_flip_mode_inquiry,
        FlipModeInquiry,
        &[0x81, 0x09, 0x04, 0x65, 0xFF]
    );
    visca_test!(
        StandbyInquiry,
        test_standby_inquiry,
        StandbyInquiry,
        &[0x81, 0x09, 0x04, 0x70, 0xFF]
    );
    visca_test!(
        FocusRangeInquiry,
        test_focus_range_inquiry,
        FocusRangeInquiry,
        &[0x81, 0x09, 0x04, 0x2A, 0xFF]
    );
    visca_test!(
        IrisControlInquiry,
        test_iris_control_inquiry,
        IrisControlInquiry,
        &[0x81, 0x09, 0x04, 0x2B, 0xFF]
    );
    visca_test!(
        DefogModeInquiry,
        test_defog_mode_inquiry,
        DefogModeInquiry,
        &[0x81, 0x09, 0x04, 0x37, 0xFF]
    );
    visca_test!(
        DefogLevelInquiry,
        test_defog_level_inquiry,
        DefogLevelInquiry,
        &[0x81, 0x09, 0x04, 0xA0, 0xFF]
    );
    visca_test!(
        DigitalPtzInquiry,
        test_digital_ptz_inquiry,
        DigitalPtzInquiry,
        &[0x81, 0x09, 0x04, 0x6B, 0xFF]
    );
    visca_test!(
        AutoWhiteBalanceSensitivityInquiry,
        test_auto_wb_sensitivity_inquiry,
        AutoWhiteBalanceSensitivityInquiry,
        &[0x81, 0x09, 0x04, 0x59, 0xFF]
    );
    visca_test!(
        ExposureCompensationPositionInquiry,
        test_exposure_compensation_position_inquiry,
        ExposureCompensationPositionInquiry,
        &[0x81, 0x09, 0x04, 0x4E, 0xFF]
    );
    visca_test!(
        RedTuningInquiry,
        test_red_tuning_inquiry,
        RedTuningInquiry,
        &[0x81, 0x09, 0x04, 0x43, 0xFF]
    );
    visca_test!(
        BlueTuningInquiry,
        test_blue_tuning_inquiry,
        BlueTuningInquiry,
        &[0x81, 0x09, 0x04, 0x44, 0xFF]
    );
    visca_test!(
        AutoTraceInquiry,
        test_auto_trace_inquiry,
        AutoTraceInquiry,
        &[0x81, 0x09, 0x50, 0x09, 0xFF]
    );
    visca_test!(
        FocusUnlockInquiry,
        test_focus_unlock_inquiry,
        FocusUnlockInquiry,
        &[0x81, 0x09, 0x54, 0x08, 0xFF]
    );
    visca_test!(
        SharpnessPositionInquiry,
        test_sharpness_position_inquiry,
        SharpnessPositionInquiry,
        &[0x81, 0x09, 0x04, 0x42, 0xFF]
    );
    visca_test!(
        NrLevelInquiry,
        test_nr_level_inquiry,
        NrLevelInquiry,
        &[0x81, 0x09, 0x04, 0x52, 0xFF]
    );
    visca_test!(
        BroadcastDomainInquiry,
        test_broadcast_domain_inquiry,
        BroadcastDomainInquiry,
        &[0x81, 0x09, 0x04, 0x75, 0xFF]
    );
    visca_test!(
        MotionSyncModeInquiry,
        test_motion_sync_mode_inquiry,
        MotionSyncModeInquiry,
        &[0x81, 0x09, 0x04, 0x56, 0xFF]
    );
    visca_test!(
        MotionSyncSpeedInquiry,
        test_motion_sync_speed_inquiry,
        MotionSyncSpeedInquiry,
        &[0x81, 0x09, 0x04, 0x57, 0xFF]
    );
    visca_test!(
        NrModeInquiry,
        test_nr_mode_inquiry,
        NrModeInquiry,
        &[0x81, 0x09, 0x04, 0x53, 0xFF]
    );
    visca_test!(
        NrSpeedInquiry,
        test_nr_speed_inquiry,
        NrSpeedInquiry,
        &[0x81, 0x09, 0x04, 0x54, 0xFF]
    );
    visca_test!(
        BlackWhiteModeInquiry,
        test_black_white_mode_inquiry,
        BlackWhiteModeInquiry,
        &[0x81, 0x09, 0x04, 0x73, 0xFF]
    );
}

// Manual implementation for TallyGreenInquiry due to special format
/// Inquiry command to get the green tally light status (FR7 only).
/// Returns 0x02 for On, 0x03 for Off.
#[derive(Debug, Copy, Clone)]
pub struct TallyGreenInquiry;

impl crate::command::encode_visca::EncodeVisca for TallyGreenInquiry {
    type Response = crate::command::InquiryResponse;
    const MAX_SIZE: usize = 7;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::error::Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(crate::error::Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        // Special format for green tally inquiry
        buffer[0] = camera_id.to_address_byte();
        buffer[1] = 0x09;
        buffer[2] = 0x7E;
        buffer[3] = 0x04;
        buffer[4] = 0x1A;
        buffer[5] = 0x00;
        buffer[6] = 0xFF;

        Ok(Self::MAX_SIZE)
    }

    fn response_type(&self) -> Option<crate::command::response::ResponseType> {
        None
    }

    fn timeout_kind(&self) -> crate::timeout::CommandCategory {
        crate::timeout::CommandCategory::Quick
    }
}

impl crate::capabilities::CommandFeatures for TallyGreenInquiry {
    fn required_features(&self) -> &[crate::capabilities::CameraFeature] {
        &[crate::capabilities::CameraFeature::Tally]
    }
}
