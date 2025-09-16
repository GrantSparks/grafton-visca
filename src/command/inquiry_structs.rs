//! Inquiry command structs using derive macro.
//!
//! This module uses the InquiryCommand derive macro to generate
//! Command implementations for all inquiry types.

use grafton_visca_macros::InquiryCommand;

// Power and System Inquiries

/// Inquiry command to get the current power state of the camera.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x00,
    response = "Power",
    parser = "bool",
    constant = "POWER"
)]
pub struct PowerInquiry;

/// Inquiry command to get the camera version information.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x02,
    sub_command = 0x00,
    response = "Version",
    constant = "VERSION"
)]
pub struct VersionInquiry;

// Position Inquiries

/// Inquiry command to get the current pan/tilt position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x12,
    sub_command = 0x06,
    response = "PanTiltPosition",
    parser = "pan_tilt",
    constant = "PAN_TILT_POSITION"
)]
pub struct PanTiltPositionInquiry;

/// Inquiry command to get the current zoom position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x47,
    response = "ZoomPosition",
    parser = "position",
    constant = "ZOOM_POSITION"
)]
pub struct ZoomPositionInquiry;

/// Inquiry command to get the current focus position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x48,
    response = "FocusPosition",
    parser = "position",
    constant = "FOCUS_POSITION"
)]
pub struct FocusPositionInquiry;

// Exposure Inquiries

/// Inquiry command to get the current exposure mode setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x39,
    response = "ExposureMode",
    parser = "mode",
    type = "ExposureMode",
    constant = "EXPOSURE_MODE"
)]
pub struct ExposureModeInquiry;

/// Inquiry command to get the current exposure compensation value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x4E,
    response = "ExposureCompensation",
    parser = "custom",
    custom_fn = "parse_exposure_compensation",
    constant = "EXPOSURE_COMPENSATION"
)]
pub struct ExposureCompensationInquiry;

/// Inquiry command to get the exposure compensation mode on/off status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x3E,
    response = "ExposureCompensationMode",
    parser = "bool",
    constant = "EXPOSURE_COMPENSATION_MODE"
)]
pub struct ExposureCompensationModeInquiry;

/// Inquiry command to get the current iris position value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x4B,
    response = "Iris",
    parser = "custom",
    custom_fn = "parse_iris_last_nibble",
    constant = "IRIS"
)]
pub struct IrisInquiry;

/// Inquiry command to get the current shutter speed setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x4A,
    response = "Shutter",
    parser = "custom",
    custom_fn = "parse_shutter",
    constant = "SHUTTER"
)]
pub struct ShutterInquiry;

/// Inquiry command to get the current brightness adjustment value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x4D,
    response = "Bright",
    parser = "position",
    constant = "BRIGHT"
)]
pub struct BrightInquiry;

// White Balance and Color Inquiries

/// Inquiry command to get the current white balance mode.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x35,
    response = "WhiteBalanceMode",
    parser = "mode",
    type = "WhiteBalanceMode",
    constant = "WHITE_BALANCE_MODE"
)]
pub struct WhiteBalanceModeInquiry;

/// Inquiry command to get the current color temperature value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x20,
    response = "ColorTemperature",
    parser = "custom",
    custom_fn = "parse_color_temperature",
    constant = "COLOR_TEMPERATURE"
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
    offset = 10,
    constant = "RED_GAIN"
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
    offset = 10,
    constant = "BLUE_GAIN"
)]
pub struct BlueGainInquiry;

// Image Adjustment Inquiries

// NOTE: The following inquiry commands are not documented in the VISCA protocol specifications
// and may not work with actual cameras. They appear to be based on direct command opcodes
// rather than actual inquiry opcodes. Commenting out until proper documentation is found.

// /// Inquiry command to get the current luminance setting.
// #[derive(InquiryCommand, Debug, Copy, Clone)]
// #[visca(
//     command = 0x4D,
//     sub_command = 0x50,
//     response = "Luminance",
//     parser = "byte"
// )]
// pub struct LuminanceInquiry;

// /// Inquiry command to get the current contrast level.
// #[derive(InquiryCommand, Debug, Copy, Clone)]
// #[visca(
//     command = 0x4E,
//     sub_command = 0x50,
//     response = "Contrast",
//     parser = "byte"
// )]
// pub struct ContrastInquiry;

// /// Inquiry command to get the current sharpness level.
// #[derive(InquiryCommand, Debug, Copy, Clone)]
// #[visca(
//     command = 0x42,
//     response = "Sharpness",
//     parser = "custom",
//     custom_fn = "parse_middle_nibbles"
// )]
// pub struct SharpnessInquiry;

/// Inquiry command to get the current sharpness mode on/off status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x05,
    response = "SharpnessMode",
    parser = "custom",
    custom_fn = "parse_sharpness_mode",
    constant = "SHARPNESS_MODE"
)]
pub struct SharpnessModeInquiry;

/// Inquiry command to get the current color saturation level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x49,
    response = "Saturation",
    parser = "custom",
    custom_fn = "parse_saturation_last_nibble",
    constant = "SATURATION"
)]
pub struct SaturationInquiry;

/// Inquiry command to get the current hue adjustment value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x4F,
    response = "Hue",
    parser = "custom",
    custom_fn = "parse_hue_last_nibble",
    constant = "HUE"
)]
pub struct HueInquiry;

// Gain Inquiries

/// Inquiry command to get the current gain value.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x4C,
    response = "Gain",
    parser = "custom",
    custom_fn = "parse_gain_last_nibble",
    constant = "GAIN"
)]
pub struct GainInquiry;

/// Inquiry command to get the current gain limit setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x2C,
    response = "GainLimit",
    parser = "byte",
    constant = "GAIN_LIMIT"
)]
pub struct GainLimitInquiry;

// Image Processing Inquiries

/// Inquiry command to get the backlight compensation mode.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x33,
    response = "Backlight",
    parser = "bool",
    constant = "BACKLIGHT"
)]
pub struct BacklightInquiry;

/// Inquiry command to get the image flip (mirror/reverse) settings.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x66,
    response = "ImageFlip",
    parser = "flags",
    constant = "IMAGE_FLIP"
)]
pub struct ImageFlipInquiry;

/// Inquiry command to get the black and white mode on/off status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x01,
    response = "BlackWhite",
    parser = "bool",
    constant = "BLACK_WHITE"
)]
pub struct BlackWhiteInquiry;

/// Inquiry command to get the 2D noise reduction level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x53,
    response = "NoiseReduction2D",
    parser = "byte",
    constant = "NOISE_REDUCTION_2D"
)]
pub struct NoiseReduction2DInquiry;

/// Inquiry command to get the 3D noise reduction level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x54,
    response = "NoiseReduction3D",
    parser = "byte",
    constant = "NOISE_REDUCTION_3D"
)]
pub struct NoiseReduction3DInquiry;

/// Inquiry command to get the dynamic range mode/level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x25,
    response = "DynamicRange",
    parser = "byte",
    constant = "DYNAMIC_RANGE"
)]
pub struct DynamicRangeInquiry;

// Focus Inquiries

/// Inquiry command to get the current focus zone selection.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x3C,
    response = "FocusZone",
    parser = "mode",
    type = "FocusZone",
    constant = "FOCUS_ZONE"
)]
pub struct FocusZoneInquiry;

/// Inquiry command to get the auto-focus sensitivity setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x58,
    response = "AutoFocusSensitivity",
    parser = "mode",
    type = "AutoFocusSensitivity",
    constant = "AUTO_FOCUS_SENSITIVITY"
)]
pub struct AutoFocusSensitivityInquiry;

/// Inquiry command to get the focus near limit position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x28,
    response = "FocusNearLimit",
    parser = "position",
    constant = "FOCUS_NEAR_LIMIT"
)]
pub struct FocusNearLimitInquiry;

/// Inquiry command to get the current focus mode (Auto/Manual).
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x38,
    response = "FocusMode",
    parser = "mode",
    type = "FocusMode",
    constant = "FOCUS_MODE"
)]
pub struct FocusModeInquiry;

// System State Inquiries

/// Inquiry command to get the menu open/close status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x06,
    response = "MenuOpenClose",
    parser = "custom",
    custom_fn = "parse_menu_open_close",
    constant = "MENU_OPEN_CLOSE"
)]
pub struct MenuOpenCloseInquiry;

// Inquiry command to get the auto focus on/off status.
// NOTE: AutoFocus inquiry is not documented in VISCA specs
// and has been disabled until proper documentation is found.

// #[derive(InquiryCommand, Debug, Copy, Clone)]
// #[visca(
//     command = 0x18,
//     response = "AutoFocus",
//     parser = "custom",
//     custom_fn = "parse_auto_focus"
// )]
// pub struct AutoFocusInquiry;
/// Inquiry command to get the tally light status (red and green).
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0xA8,
    response = "TallyStatus",
    parser = "custom",
    custom_fn = "parse_tally_status",
    constant = "TALLY_STATUS"
)]
pub struct TallyStatusInquiry;

// The TallyGreenInquiry is manually implemented below due to its special format

/// Inquiry command to get the current video resolution mode.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x63,
    response = "Resolution",
    parser = "byte",
    constant = "RESOLUTION"
)]
pub struct ResolutionInquiry;

/// Inquiry command to get the night/day mode status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x60,
    response = "NightDayMode",
    parser = "custom",
    custom_fn = "parse_night_day_mode",
    constant = "NIGHT_DAY_MODE"
)]
pub struct NightDayModeInquiry;

/// Inquiry command to get the ND filter position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x64,
    response = "NdFilter",
    parser = "custom",
    custom_fn = "parse_nd_filter",
    constant = "ND_FILTER"
)]
pub struct NdFilterInquiry;

/// Inquiry command to get the current picture effect mode.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x32,
    response = "PictureEffect",
    parser = "custom",
    custom_fn = "parse_picture_effect",
    constant = "PICTURE_EFFECT"
)]
pub struct PictureEffectInquiry;

/// Inquiry command to get the current flip mode (combined horizontal/vertical).
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x65,
    response = "FlipMode",
    parser = "custom",
    custom_fn = "parse_flip_mode",
    constant = "FLIP_MODE"
)]
pub struct FlipModeInquiry;

/// Inquiry command to get the standby mode status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x70,
    response = "Standby",
    parser = "custom",
    custom_fn = "parse_standby",
    constant = "STANDBY"
)]
pub struct StandbyInquiry;

/// Inquiry command to get the focus range setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x2A,
    response = "FocusRange",
    parser = "custom",
    custom_fn = "parse_focus_range",
    constant = "FOCUS_RANGE"
)]
pub struct FocusRangeInquiry;

/// Inquiry command to get the iris control mode.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x2B,
    response = "IrisControl",
    parser = "custom",
    custom_fn = "parse_iris_control",
    constant = "IRIS_CONTROL"
)]
pub struct IrisControlInquiry;

/// Inquiry command to get the defog mode status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x37,
    response = "DefogMode",
    parser = "custom",
    custom_fn = "parse_defog_mode",
    constant = "DEFOG_MODE"
)]
pub struct DefogModeInquiry;

/// Inquiry command to get the defog level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0xA0,
    response = "DefogLevel",
    parser = "custom",
    custom_fn = "parse_defog_level",
    constant = "DEFOG_LEVEL"
)]
pub struct DefogLevelInquiry;

/// Inquiry command to get the digital Ptz mode status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x6B,
    response = "DigitalPtz",
    parser = "custom",
    custom_fn = "parse_digital_ptz",
    constant = "DIGITAL_PTZ"
)]
pub struct DigitalPtzInquiry;

// Additional Inquiries

/// Inquiry command to get the auto white balance sensitivity setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x59,
    response = "AutoWhiteBalanceSensitivity",
    parser = "custom",
    custom_fn = "parse_auto_wb_sensitivity",
    constant = "AUTO_WB_SENSITIVITY"
)]
pub struct AutoWhiteBalanceSensitivityInquiry;

/// Inquiry command to get the exposure compensation position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x4E,
    response = "ExposureCompensationPosition",
    parser = "custom",
    custom_fn = "parse_exposure_compensation_position",
    constant = "EXPOSURE_COMPENSATION_POSITION"
)]
pub struct ExposureCompensationPositionInquiry;

/// Inquiry command to get the red channel tuning level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x43,
    response = "RedTuning",
    parser = "custom",
    custom_fn = "parse_red_tuning",
    constant = "RED_TUNING"
)]
pub struct RedTuningInquiry;

/// Inquiry command to get the blue channel tuning level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x44,
    response = "BlueTuning",
    parser = "custom",
    custom_fn = "parse_blue_tuning",
    constant = "BLUE_TUNING"
)]
pub struct BlueTuningInquiry;

/// Inquiry command to get the current gamma curve setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x5B,
    response = "Gamma",
    parser = "custom",
    custom_fn = "parse_gamma",
    constant = "GAMMA"
)]
pub struct GammaInquiry;

/// Inquiry command to get the auto trace mode status.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x09,
    sub_command = 0x50,
    response = "AutoTrace",
    parser = "custom",
    custom_fn = "parse_auto_trace",
    constant = "AUTO_TRACE"
)]
pub struct AutoTraceInquiry;

/// Inquiry command to get the focus unlock state.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x08,
    sub_command = 0x54,
    response = "FocusUnlock",
    parser = "custom",
    custom_fn = "parse_focus_unlock",
    constant = "FOCUS_UNLOCK"
)]
pub struct FocusUnlockInquiry;

/// Inquiry command to get the current sharpness position.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x42,
    response = "SharpnessPosition",
    parser = "position",
    constant = "SHARPNESS_POSITION"
)]
pub struct SharpnessPositionInquiry;

/// Inquiry command to get the noise reduction level.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x52,
    response = "NrLevel",
    parser = "byte",
    constant = "NR_LEVEL"
)]
pub struct NrLevelInquiry;

/// Inquiry command to get the broadcast domain setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x75,
    response = "BroadcastDomain",
    parser = "byte",
    constant = "BROADCAST_DOMAIN"
)]
pub struct BroadcastDomainInquiry;

// System and Image Processing Inquiries

/// Inquiry command to get the motion sync mode setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x56,
    response = "MotionSyncMode",
    parser = "mode",
    type = "MotionSyncMode",
    constant = "MOTION_SYNC_MODE"
)]
pub struct MotionSyncModeInquiry;

/// Inquiry command to get the motion sync speed setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x57,
    response = "MotionSyncPreset",
    inquiry_variant = "MotionSyncPreset",
    parser = "speed",
    type = "MotionSyncPreset",
    constant = "MOTION_SYNC_SPEED"
)]
pub struct MotionSyncPresetInquiry;

/// Inquiry command to get the noise reduction mode setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x53,
    response = "NrMode",
    parser = "mode",
    type = "NrMode",
    constant = "NR_MODE"
)]
pub struct NrModeInquiry;

/// Inquiry command to get the noise reduction speed setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x54,
    response = "NrSpeed",
    parser = "speed",
    type = "NrSpeed",
    constant = "NR_SPEED"
)]
pub struct NrSpeedInquiry;

/// Inquiry command to get the black and white mode setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x73,
    response = "BlackWhiteMode",
    parser = "mode",
    type = "BlackWhiteMode",
    constant = "BLACK_WHITE_MODE"
)]
pub struct BlackWhiteModeInquiry;

// Additional System Inquiries

/// Inquiry command to get the USB audio state.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x7A,
    response = "UsbAudio",
    parser = "bool",
    constant = "USB_AUDIO"
)]
pub struct UsbAudioInquiry;

/// Inquiry command to get the two tone mode state.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x74,
    response = "TwoToneMode",
    parser = "bool",
    constant = "TWO_TONE_MODE"
)]
pub struct TwoToneModeInquiry;

/// Inquiry command to get the ND filter preset setting.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x66,
    response = "NdFilterPreset",
    parser = "byte",
    constant = "ND_FILTER_PRESET"
)]
pub struct NdFilterPresetInquiry;

/// Inquiry command to get the digital mode state.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x7B,
    response = "Digital",
    parser = "bool",
    constant = "DIGITAL"
)]
pub struct DigitalInquiry;

/// Inquiry command to get the tally auto adjust state.
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0xA9,
    response = "TallyAutoAdjust",
    parser = "bool",
    constant = "TALLY_AUTO_ADJUST"
)]
pub struct TallyAutoAdjustInquiry;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::bytes::constants;
    use crate::macros::test_utils::visca_test;

    visca_test!(
        ZoomPositionInquiry,
        test_zoom_position_inquiry,
        ZoomPositionInquiry,
        constants::inquiry::ZOOM_POSITION
    );
    visca_test!(
        FocusPositionInquiry,
        test_focus_position_inquiry,
        FocusPositionInquiry,
        constants::inquiry::FOCUS_POSITION
    );
    visca_test!(
        PowerInquiry,
        test_power_inquiry,
        PowerInquiry,
        constants::inquiry::POWER
    );
    visca_test!(
        FocusModeInquiry,
        test_focus_mode_inquiry,
        FocusModeInquiry,
        constants::inquiry::FOCUS_MODE
    );
    visca_test!(
        MenuOpenCloseInquiry,
        test_menu_open_close_inquiry,
        MenuOpenCloseInquiry,
        constants::inquiry::MENU_OPEN_CLOSE
    );
    // NOTE: AutoFocus inquiry is not documented in VISCA specs
    // visca_test!(
    //     AutoFocusInquiry,
    //     test_auto_focus_inquiry,
    //     AutoFocusInquiry,
    //     constants::inquiry::AUTO_FOCUS
    // );
    visca_test!(
        TallyStatusInquiry,
        test_tally_status_inquiry,
        TallyStatusInquiry,
        constants::inquiry::TALLY_STATUS
    );
    visca_test!(
        TallyGreenInquiry,
        test_tally_green_inquiry,
        TallyGreenInquiry,
        constants::inquiry::TALLY_GREEN
    );
    visca_test!(
        ResolutionInquiry,
        test_resolution_inquiry,
        ResolutionInquiry,
        constants::inquiry::RESOLUTION
    );
    visca_test!(
        NightDayModeInquiry,
        test_night_day_mode_inquiry,
        NightDayModeInquiry,
        constants::inquiry::NIGHT_DAY_MODE
    );
    visca_test!(
        NdFilterInquiry,
        test_nd_filter_inquiry,
        NdFilterInquiry,
        constants::inquiry::ND_FILTER
    );
    visca_test!(
        PictureEffectInquiry,
        test_picture_effect_inquiry,
        PictureEffectInquiry,
        constants::inquiry::PICTURE_EFFECT
    );
    visca_test!(
        FlipModeInquiry,
        test_flip_mode_inquiry,
        FlipModeInquiry,
        constants::inquiry::FLIP_MODE
    );
    visca_test!(
        StandbyInquiry,
        test_standby_inquiry,
        StandbyInquiry,
        constants::inquiry::STANDBY
    );
    visca_test!(
        FocusRangeInquiry,
        test_focus_range_inquiry,
        FocusRangeInquiry,
        constants::inquiry::FOCUS_RANGE
    );
    visca_test!(
        IrisControlInquiry,
        test_iris_control_inquiry,
        IrisControlInquiry,
        constants::inquiry::IRIS_CONTROL
    );
    visca_test!(
        DefogModeInquiry,
        test_defog_mode_inquiry,
        DefogModeInquiry,
        constants::inquiry::DEFOG_MODE
    );
    visca_test!(
        DefogLevelInquiry,
        test_defog_level_inquiry,
        DefogLevelInquiry,
        constants::inquiry::DEFOG_LEVEL
    );
    visca_test!(
        DigitalPtzInquiry,
        test_digital_ptz_inquiry,
        DigitalPtzInquiry,
        constants::inquiry::DIGITAL_PTZ
    );
    visca_test!(
        AutoWhiteBalanceSensitivityInquiry,
        test_auto_wb_sensitivity_inquiry,
        AutoWhiteBalanceSensitivityInquiry,
        constants::inquiry::AUTO_WB_SENSITIVITY
    );
    visca_test!(
        ExposureCompensationPositionInquiry,
        test_exposure_compensation_position_inquiry,
        ExposureCompensationPositionInquiry,
        constants::inquiry::EXPOSURE_COMPENSATION_POSITION
    );
    visca_test!(
        RedTuningInquiry,
        test_red_tuning_inquiry,
        RedTuningInquiry,
        constants::inquiry::RED_TUNING
    );
    visca_test!(
        BlueTuningInquiry,
        test_blue_tuning_inquiry,
        BlueTuningInquiry,
        constants::inquiry::BLUE_TUNING
    );
    visca_test!(
        AutoTraceInquiry,
        test_auto_trace_inquiry,
        AutoTraceInquiry,
        constants::inquiry::AUTO_TRACE
    );
    visca_test!(
        FocusUnlockInquiry,
        test_focus_unlock_inquiry,
        FocusUnlockInquiry,
        constants::inquiry::FOCUS_UNLOCK
    );
    visca_test!(
        SharpnessPositionInquiry,
        test_sharpness_position_inquiry,
        SharpnessPositionInquiry,
        constants::inquiry::SHARPNESS_POSITION
    );
    visca_test!(
        NrLevelInquiry,
        test_nr_level_inquiry,
        NrLevelInquiry,
        constants::inquiry::NR_LEVEL
    );
    visca_test!(
        BroadcastDomainInquiry,
        test_broadcast_domain_inquiry,
        BroadcastDomainInquiry,
        constants::inquiry::BROADCAST_DOMAIN
    );
    visca_test!(
        MotionSyncModeInquiry,
        test_motion_sync_mode_inquiry,
        MotionSyncModeInquiry,
        constants::inquiry::MOTION_SYNC_MODE
    );
    visca_test!(
        MotionSyncPresetInquiry,
        test_motion_sync_speed_inquiry,
        MotionSyncPresetInquiry,
        constants::inquiry::MOTION_SYNC_SPEED
    );
    visca_test!(
        NrModeInquiry,
        test_nr_mode_inquiry,
        NrModeInquiry,
        constants::inquiry::NR_MODE
    );
    visca_test!(
        NrSpeedInquiry,
        test_nr_speed_inquiry,
        NrSpeedInquiry,
        constants::inquiry::NR_SPEED
    );
    visca_test!(
        BlackWhiteModeInquiry,
        test_black_white_mode_inquiry,
        BlackWhiteModeInquiry,
        constants::inquiry::BLACK_WHITE_MODE
    );
}

// Manual implementation for TallyGreenInquiry due to special format
/// Inquiry command to get the green tally light status (FR7 only).
/// Returns 0x02 for On, 0x03 for Off.
///
/// Note: This uses a special extended inquiry format (0x7E 0x04 0x1A 0x00)
/// instead of the standard inquiry format, which is why it cannot use
/// the InquiryCommand derive macro.
#[derive(Debug, Copy, Clone)]
pub struct TallyGreenInquiry;

impl crate::command::encode::ViscaCommand for TallyGreenInquiry {
    type Response = crate::command::InquiryResponse;
    const MAX_SIZE: usize = 7;
    const TIMEOUT_CATEGORY: crate::timeout::CommandCategory =
        crate::timeout::CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::error::Error> {
        use crate::command::bytes::ConstCommandBuilder;

        // Special format for green tally inquiry
        let builder = ConstCommandBuilder::<7>::new()
            .append(crate::command::bytes::constants::inquiry::TALLY_GREEN)
            .with_camera_id(camera_id)
            .terminate();
        builder.build_into(buffer)
    }

    fn response_kind(&self) -> Option<crate::command::response::ResponseKind> {
        Some(crate::command::response::ResponseKind::TallyGreen)
    }
}

impl crate::command::typed::ResponseParser for TallyGreenInquiry {
    type Response = bool;

    fn from_response(resp: crate::command::Response) -> Result<Self::Response, crate::Error> {
        match resp {
            crate::command::Response::Inquiry(crate::command::InquiryResponse::TallyGreen {
                on,
            }) => Ok(on),
            crate::command::Response::Error(e) => Err(e),
            _ => Err(crate::Error::UnexpectedResponseType),
        }
    }
}
