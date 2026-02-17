//! Inquiry command structs using derive macro.
//!
//! This module uses the ViscaInquiry derive macro to generate
//! Command implementations for all inquiry types.

use grafton_visca_macros::ViscaInquiry;

/// Inquiry command to get the current power state of the camera.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x00,
    response = "Power",
    parser = "bool",
    bytes_const = "POWER"
)]
pub struct PowerInquiry;

/// Inquiry command to get the camera version information.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x02,
    subcode = 0x00,
    response = "Version",
    bytes_const = "VERSION"
)]
pub struct VersionInquiry;

/// Inquiry command to get the current pan/tilt position.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x12,
    subcode = 0x06,
    response = "PanTiltPosition",
    parser = "pan_tilt",
    bytes_const = "PAN_TILT_POSITION"
)]
pub struct PanTiltPositionInquiry;

/// Inquiry command to get the current zoom position.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x47,
    response = "ZoomPosition",
    parser = "position",
    bytes_const = "ZOOM_POSITION"
)]
pub struct ZoomPositionInquiry;

/// Inquiry command to get the current focus position.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x48,
    response = "FocusPosition",
    parser = "position",
    bytes_const = "FOCUS_POSITION"
)]
pub struct FocusPositionInquiry;

/// Inquiry command to get the current exposure mode setting.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x39,
    response = "ExposureMode",
    parser = "mode",
    value_type = "ExposureMode",
    bytes_const = "EXPOSURE_MODE"
)]
pub struct ExposureModeInquiry;

/// Inquiry command to get the current exposure compensation value.
///
/// NOTE: This inquiry is handled directly in the exposure decoder module.
#[derive(Debug, Copy, Clone)]
pub struct ExposureCompensationInquiry;

impl crate::command::encode::ViscaCommand for ExposureCompensationInquiry {
    type Response = crate::command::InquiryData;
    const MAX_SIZE: usize = 7;
    const TIMEOUT_CATEGORY: crate::timeout::CommandCategory =
        crate::timeout::CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::error::Error> {
        use crate::command::bytes::ConstCommandBuilder;

        let builder = ConstCommandBuilder::<7>::new()
            .append(crate::command::bytes::constants::inquiry::EXPOSURE_COMPENSATION)
            .with_camera_id(camera_id)
            .terminate();
        builder.build_into(buffer)
    }

    fn response_kind(&self) -> Option<crate::command::response::InquiryKind> {
        Some(crate::command::response::InquiryKind::ExposureCompensation)
    }
}

impl crate::command::typed::ResponseParser for ExposureCompensationInquiry {
    type Response = crate::types::ExposureCompensationLevel;

    fn from_response(resp: crate::command::Response) -> Result<Self::Response, crate::Error> {
        match resp {
            crate::command::Response::Inquiry(
                crate::command::InquiryData::ExposureCompensation { value },
            ) => crate::types::ExposureCompensationLevel::new(value),
            crate::command::Response::Error(e) => Err(e),
            _ => Err(crate::Error::UnexpectedResponseType),
        }
    }
}

/// Inquiry command to get the exposure compensation mode on/off status.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x3E,
    response = "ExposureCompensationMode",
    parser = "bool",
    bytes_const = "EXPOSURE_COMPENSATION_MODE"
)]
pub struct ExposureCompensationModeInquiry;

/// Inquiry command to get the current iris position value.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x4B,
    response = "Iris",
    parser = "last_nibble",
    field = "position",
    bytes_const = "IRIS"
)]
pub struct IrisInquiry;

/// Inquiry command to get the current shutter speed setting.
///
/// NOTE: This inquiry is handled directly in the exposure decoder module.
#[derive(Debug, Copy, Clone)]
pub struct ShutterInquiry;

impl crate::command::encode::ViscaCommand for ShutterInquiry {
    type Response = crate::command::InquiryData;
    const MAX_SIZE: usize = 7;
    const TIMEOUT_CATEGORY: crate::timeout::CommandCategory =
        crate::timeout::CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::error::Error> {
        use crate::command::bytes::ConstCommandBuilder;

        let builder = ConstCommandBuilder::<7>::new()
            .append(crate::command::bytes::constants::inquiry::SHUTTER)
            .with_camera_id(camera_id)
            .terminate();
        builder.build_into(buffer)
    }

    fn response_kind(&self) -> Option<crate::command::response::InquiryKind> {
        Some(crate::command::response::InquiryKind::Shutter)
    }
}

impl crate::command::typed::ResponseParser for ShutterInquiry {
    type Response = crate::types::ShutterSpeed;

    fn from_response(resp: crate::command::Response) -> Result<Self::Response, crate::Error> {
        match resp {
            crate::command::Response::Inquiry(crate::command::InquiryData::Shutter {
                position,
            }) => crate::types::ShutterSpeed::new(position),
            crate::command::Response::Error(e) => Err(e),
            _ => Err(crate::Error::UnexpectedResponseType),
        }
    }
}

/// Inquiry command to get the current brightness adjustment value.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x4D,
    response = "Brightness",
    parser = "position",
    bytes_const = "BRIGHT"
)]
pub struct BrightnessInquiry;

/// Inquiry command to get the current white balance mode.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x35,
    response = "WhiteBalanceMode",
    parser = "mode",
    value_type = "WhiteBalanceMode",
    bytes_const = "WHITE_BALANCE_MODE"
)]
pub struct WhiteBalanceModeInquiry;

/// Inquiry command to get the current color temperature value.
///
/// NOTE: This inquiry is handled directly in the color decoder module.
#[derive(Debug, Copy, Clone)]
pub struct ColorTemperatureInquiry;

impl crate::command::encode::ViscaCommand for ColorTemperatureInquiry {
    type Response = crate::command::InquiryData;
    const MAX_SIZE: usize = 7;
    const TIMEOUT_CATEGORY: crate::timeout::CommandCategory =
        crate::timeout::CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::error::Error> {
        use crate::command::bytes::ConstCommandBuilder;

        let builder = ConstCommandBuilder::<7>::new()
            .append(crate::command::bytes::constants::inquiry::COLOR_TEMPERATURE)
            .with_camera_id(camera_id)
            .terminate();
        builder.build_into(buffer)
    }

    fn response_kind(&self) -> Option<crate::command::response::InquiryKind> {
        Some(crate::command::response::InquiryKind::ColorTemperature)
    }
}

impl crate::command::typed::ResponseParser for ColorTemperatureInquiry {
    type Response = crate::types::ColorTemp;

    fn from_response(resp: crate::command::Response) -> Result<Self::Response, crate::Error> {
        match resp {
            crate::command::Response::Inquiry(crate::command::InquiryData::ColorTemperature {
                temperature,
            }) => crate::types::ColorTemp::new(temperature),
            crate::command::Response::Error(e) => Err(e),
            _ => Err(crate::Error::UnexpectedResponseType),
        }
    }
}

/// Inquiry command to get the current red gain value.
///
/// Queries register 0x04 0x43 (same as red tuning). G2 cameras respond
/// with a 4-nibble payload encoding the absolute gain value.
#[derive(Debug, Copy, Clone)]
pub struct RedGainInquiry;

impl crate::command::encode::ViscaCommand for RedGainInquiry {
    type Response = crate::command::InquiryData;
    const MAX_SIZE: usize = 7;
    const TIMEOUT_CATEGORY: crate::timeout::CommandCategory =
        crate::timeout::CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::error::Error> {
        use crate::command::bytes::ConstCommandBuilder;

        let builder = ConstCommandBuilder::<7>::new()
            .append(crate::command::bytes::constants::inquiry::RED_GAIN)
            .with_camera_id(camera_id)
            .terminate();
        builder.build_into(buffer)
    }

    fn response_kind(&self) -> Option<crate::command::response::InquiryKind> {
        Some(crate::command::response::InquiryKind::RedChannel)
    }
}

impl crate::command::typed::ResponseParser for RedGainInquiry {
    type Response = crate::types::RedChannel;

    fn from_response(resp: crate::command::Response) -> Result<Self::Response, crate::Error> {
        match resp {
            crate::command::Response::Inquiry(crate::command::InquiryData::RedChannel { gain }) => {
                crate::types::RedChannel::new(gain)
            }
            crate::command::Response::Error(e) => Err(e),
            _ => Err(crate::Error::UnexpectedResponseType),
        }
    }
}

/// Inquiry command to get the current blue gain value.
///
/// Queries register 0x04 0x44 (same as blue tuning). G2 cameras respond
/// with a 4-nibble payload encoding the absolute gain value.
#[derive(Debug, Copy, Clone)]
pub struct BlueGainInquiry;

impl crate::command::encode::ViscaCommand for BlueGainInquiry {
    type Response = crate::command::InquiryData;
    const MAX_SIZE: usize = 7;
    const TIMEOUT_CATEGORY: crate::timeout::CommandCategory =
        crate::timeout::CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::error::Error> {
        use crate::command::bytes::ConstCommandBuilder;

        let builder = ConstCommandBuilder::<7>::new()
            .append(crate::command::bytes::constants::inquiry::BLUE_GAIN)
            .with_camera_id(camera_id)
            .terminate();
        builder.build_into(buffer)
    }

    fn response_kind(&self) -> Option<crate::command::response::InquiryKind> {
        Some(crate::command::response::InquiryKind::BlueChannel)
    }
}

impl crate::command::typed::ResponseParser for BlueGainInquiry {
    type Response = crate::types::BlueChannel;

    fn from_response(resp: crate::command::Response) -> Result<Self::Response, crate::Error> {
        match resp {
            crate::command::Response::Inquiry(crate::command::InquiryData::BlueChannel {
                gain,
            }) => crate::types::BlueChannel::new(gain),
            crate::command::Response::Error(e) => Err(e),
            _ => Err(crate::Error::UnexpectedResponseType),
        }
    }
}

/// Inquiry command to get the current sharpness mode on/off status.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x05,
    response = "SharpnessMode",
    parser = "sharpness_mode",
    bytes_const = "SHARPNESS_MODE"
)]
pub struct SharpnessModeInquiry;

/// Inquiry command to get the current color saturation level.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x49,
    response = "Saturation",
    parser = "last_nibble",
    field = "level",
    bytes_const = "SATURATION"
)]
pub struct SaturationInquiry;

/// Inquiry command to get the current hue adjustment value.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x4F,
    response = "Hue",
    parser = "last_nibble",
    field = "hue",
    bytes_const = "HUE"
)]
pub struct HueInquiry;

/// Inquiry command to get the current gain value.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x4C,
    response = "Gain",
    parser = "last_nibble",
    field = "gain",
    data_variant = "GainLevel",
    bytes_const = "GAIN"
)]
pub struct GainInquiry;

/// Inquiry command to get the current gain limit setting.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x2C,
    response = "GainLimit",
    parser = "byte",
    bytes_const = "GAIN_LIMIT"
)]
pub struct GainLimitInquiry;

/// Inquiry command to get the backlight compensation mode.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x33,
    response = "Backlight",
    parser = "bool",
    bytes_const = "BACKLIGHT"
)]
pub struct BacklightInquiry;

/// Inquiry command to get the image flip (mirror/reverse) settings.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x66,
    response = "FlipState",
    parser = "flags",
    bytes_const = "IMAGE_FLIP"
)]
pub struct ImageFlipInquiry;

/// Inquiry command to get the black and white mode on/off status.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x01,
    response = "BlackWhite",
    parser = "bool",
    bytes_const = "BLACK_WHITE"
)]
pub struct BlackWhiteInquiry;

/// Inquiry command to get the 2D noise reduction level.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x53,
    response = "NoiseReduction2D",
    parser = "byte",
    bytes_const = "NOISE_REDUCTION_2D"
)]
pub struct NoiseReduction2DInquiry;

/// Inquiry command to get the 3D noise reduction level.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x54,
    response = "NoiseReduction3D",
    parser = "byte",
    bytes_const = "NOISE_REDUCTION_3D"
)]
pub struct NoiseReduction3DInquiry;

/// Inquiry command to get the dynamic range mode/level.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x25,
    response = "DynamicRange",
    parser = "byte",
    bytes_const = "DYNAMIC_RANGE"
)]
pub struct DynamicRangeInquiry;

/// Inquiry command to get the current focus zone selection.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x3C,
    response = "FocusZone",
    parser = "mode",
    value_type = "FocusZone",
    bytes_const = "FOCUS_ZONE"
)]
pub struct FocusZoneInquiry;

/// Inquiry command to get the auto-focus sensitivity setting.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x58,
    response = "AutoFocusSensitivity",
    parser = "mode",
    value_type = "AutoFocusSensitivity",
    bytes_const = "AUTO_FOCUS_SENSITIVITY"
)]
pub struct AutoFocusSensitivityInquiry;

/// Inquiry command to get the focus near limit position.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x28,
    response = "FocusNearLimit",
    parser = "position",
    bytes_const = "FOCUS_NEAR_LIMIT"
)]
pub struct FocusNearLimitInquiry;

/// Inquiry command to get the current focus mode (Auto/Manual).
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x38,
    response = "FocusMode",
    parser = "mode",
    value_type = "FocusMode",
    bytes_const = "FOCUS_MODE"
)]
pub struct FocusModeInquiry;

/// Inquiry command to get the menu open/close status.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x06,
    response = "MenuOpenClose",
    parser = "bool_convention",
    convention = "OnIs02",
    field = "is_open",
    bytes_const = "MENU_OPEN_CLOSE"
)]
pub struct MenuOpenCloseInquiry;

/// Inquiry command to get combined tally light status (red and green).
///
/// **Vendor-Specific**: This is a PTZOptics extension, not part of baseline VISCA.
/// Returns a 2-byte packed response with red and green tally states.
/// For baseline VISCA compliance, use `TallyRedInquiry` and `TallyGreenInquiry` separately.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0xA8,
    response = "TallyStatus",
    parser = "tally_status",
    bytes_const = "TALLY_STATUS"
)]
pub struct TallyStatusInquiry;

/// Inquiry command to get the current video resolution mode.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x63,
    response = "Resolution",
    parser = "byte",
    bytes_const = "RESOLUTION"
)]
pub struct ResolutionInquiry;

/// Inquiry command to get the night/day mode status.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x60,
    response = "NightDayMode",
    parser = "bool_convention",
    convention = "OnIs03",
    field = "is_night",
    bytes_const = "NIGHT_DAY_MODE"
)]
pub struct NightDayModeInquiry;

/// Inquiry command to get the ND filter position.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x64,
    response = "NdFilter",
    parser = "nd_filter",
    bytes_const = "ND_FILTER"
)]
pub struct NdFilterInquiry;

/// Inquiry command to get the current picture effect mode.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x32,
    response = "PictureEffect",
    parser = "picture_effect",
    bytes_const = "PICTURE_EFFECT"
)]
pub struct PictureEffectInquiry;

/// Inquiry command to get the current flip mode (combined horizontal/vertical).
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x65,
    response = "FlipState",
    parser = "flags",
    bytes_const = "FLIP_MODE"
)]
pub struct FlipStateInquiry;

/// Inquiry command to get the standby mode status.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x70,
    response = "Standby",
    parser = "bool_convention",
    convention = "OnIs03",
    field = "in_standby",
    bytes_const = "STANDBY"
)]
pub struct StandbyInquiry;

/// Inquiry command to get the focus range setting.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x2A,
    response = "FocusRange",
    parser = "focus_range",
    bytes_const = "FOCUS_RANGE"
)]
pub struct FocusRangeInquiry;

/// Inquiry command to get the iris control mode.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x2B,
    response = "IrisControl",
    parser = "bool_convention",
    convention = "OnIs03",
    field = "auto",
    bytes_const = "IRIS_CONTROL"
)]
pub struct IrisControlInquiry;

/// Inquiry command to get the defog mode status.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x37,
    response = "DefogMode",
    parser = "bool_convention",
    convention = "OnIs03",
    field = "enabled",
    bytes_const = "DEFOG_MODE"
)]
pub struct DefogModeInquiry;

/// Inquiry command to get the defog level.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0xA0,
    response = "DefogLevel",
    parser = "defog_level",
    bytes_const = "DEFOG_LEVEL"
)]
pub struct DefogLevelInquiry;

/// Inquiry command to get the digital Ptz mode status.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x6B,
    response = "DigitalPtz",
    parser = "bool_convention",
    convention = "OnIs03",
    field = "enabled",
    bytes_const = "DIGITAL_PTZ"
)]
pub struct DigitalPtzInquiry;

/// Inquiry command to get the auto white balance sensitivity setting.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x59,
    response = "AutoWhiteBalanceSensitivity",
    parser = "auto_wb_sensitivity",
    bytes_const = "AUTO_WB_SENSITIVITY"
)]
pub struct AutoWhiteBalanceSensitivityInquiry;

/// Inquiry command to get the exposure compensation position.
///
/// NOTE: This inquiry is handled directly in the exposure decoder module.
#[derive(Debug, Copy, Clone)]
pub struct ExposureCompensationPositionInquiry;

impl crate::command::encode::ViscaCommand for ExposureCompensationPositionInquiry {
    type Response = crate::command::InquiryData;
    const MAX_SIZE: usize = 7;
    const TIMEOUT_CATEGORY: crate::timeout::CommandCategory =
        crate::timeout::CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::error::Error> {
        use crate::command::bytes::ConstCommandBuilder;

        let builder = ConstCommandBuilder::<7>::new()
            .append(crate::command::bytes::constants::inquiry::EXPOSURE_COMPENSATION_POSITION)
            .with_camera_id(camera_id)
            .terminate();
        builder.build_into(buffer)
    }

    fn response_kind(&self) -> Option<crate::command::response::InquiryKind> {
        Some(crate::command::response::InquiryKind::ExposureCompensationPosition)
    }
}

impl crate::command::typed::ResponseParser for ExposureCompensationPositionInquiry {
    type Response = crate::types::ExposureCompensationPosition;

    fn from_response(resp: crate::command::Response) -> Result<Self::Response, crate::Error> {
        match resp {
            crate::command::Response::Inquiry(
                crate::command::InquiryData::ExposureCompensationPosition { position },
            ) => Ok(position),
            crate::command::Response::Error(e) => Err(e),
            _ => Err(crate::Error::UnexpectedResponseType),
        }
    }
}

/// Inquiry command to get the red channel tuning level.
///
/// NOTE: This inquiry is handled with manual implementation due to i8 return type.
#[derive(Debug, Copy, Clone)]
pub struct RedTuningInquiry;

impl crate::command::encode::ViscaCommand for RedTuningInquiry {
    type Response = crate::command::InquiryData;
    const MAX_SIZE: usize = 7;
    const TIMEOUT_CATEGORY: crate::timeout::CommandCategory =
        crate::timeout::CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::error::Error> {
        use crate::command::bytes::ConstCommandBuilder;

        let builder = ConstCommandBuilder::<7>::new()
            .append(crate::command::bytes::constants::inquiry::RED_TUNING)
            .with_camera_id(camera_id)
            .terminate();
        builder.build_into(buffer)
    }

    fn response_kind(&self) -> Option<crate::command::response::InquiryKind> {
        Some(crate::command::response::InquiryKind::RedTuning)
    }
}

impl crate::command::typed::ResponseParser for RedTuningInquiry {
    type Response = crate::types::RedTuning;

    fn from_response(resp: crate::command::Response) -> Result<Self::Response, crate::Error> {
        match resp {
            crate::command::Response::Inquiry(crate::command::InquiryData::RedTuning { level }) => {
                crate::types::RedTuning::new(level)
            }
            crate::command::Response::Error(e) => Err(e),
            _ => Err(crate::Error::UnexpectedResponseType),
        }
    }
}

/// Inquiry command to get the blue channel tuning level.
///
/// NOTE: This inquiry is handled with manual implementation due to i8 return type.
#[derive(Debug, Copy, Clone)]
pub struct BlueTuningInquiry;

impl crate::command::encode::ViscaCommand for BlueTuningInquiry {
    type Response = crate::command::InquiryData;
    const MAX_SIZE: usize = 7;
    const TIMEOUT_CATEGORY: crate::timeout::CommandCategory =
        crate::timeout::CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::error::Error> {
        use crate::command::bytes::ConstCommandBuilder;

        let builder = ConstCommandBuilder::<7>::new()
            .append(crate::command::bytes::constants::inquiry::BLUE_TUNING)
            .with_camera_id(camera_id)
            .terminate();
        builder.build_into(buffer)
    }

    fn response_kind(&self) -> Option<crate::command::response::InquiryKind> {
        Some(crate::command::response::InquiryKind::BlueTuning)
    }
}

impl crate::command::typed::ResponseParser for BlueTuningInquiry {
    type Response = crate::types::BlueTuning;

    fn from_response(resp: crate::command::Response) -> Result<Self::Response, crate::Error> {
        match resp {
            crate::command::Response::Inquiry(crate::command::InquiryData::BlueTuning {
                level,
            }) => crate::types::BlueTuning::new(level),
            crate::command::Response::Error(e) => Err(e),
            _ => Err(crate::Error::UnexpectedResponseType),
        }
    }
}

/// Inquiry command to get the current gamma curve setting.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x5B,
    response = "Gamma",
    parser = "gamma",
    bytes_const = "GAMMA"
)]
pub struct GammaInquiry;

/// Inquiry command to get the auto trace mode status.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x09,
    subcode = 0x50,
    response = "AutoTrace",
    parser = "bool_convention",
    convention = "OnIs03",
    field = "enabled",
    bytes_const = "AUTO_TRACE"
)]
pub struct AutoTraceInquiry;

/// Inquiry command to get the focus unlock state.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x08,
    subcode = 0x54,
    response = "FocusUnlock",
    parser = "bool_convention",
    convention = "OnIs03",
    field = "unlocked",
    bytes_const = "FOCUS_UNLOCK"
)]
pub struct FocusUnlockInquiry;

/// Inquiry command to get the current sharpness position.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x42,
    response = "SharpnessPosition",
    parser = "position",
    bytes_const = "SHARPNESS_POSITION"
)]
pub struct SharpnessPositionInquiry;

/// Inquiry command to get the noise reduction level.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x52,
    response = "NoiseReductionLevel",
    parser = "byte",
    bytes_const = "NR_LEVEL"
)]
pub struct NrLevelInquiry;

/// Inquiry command to get the broadcast domain setting.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x75,
    response = "BroadcastDomain",
    parser = "byte",
    bytes_const = "BROADCAST_DOMAIN"
)]
pub struct BroadcastDomainInquiry;

/// Inquiry command to get the motion sync mode setting.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x56,
    response = "MotionSyncMode",
    parser = "mode",
    value_type = "MotionSyncMode",
    bytes_const = "MOTION_SYNC_MODE"
)]
pub struct MotionSyncModeInquiry;

/// Inquiry command to get the motion sync speed setting.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x57,
    response = "MotionSyncPreset",
    inquiry_variant = "MotionSyncPreset",
    parser = "speed",
    value_type = "MotionSyncPreset",
    bytes_const = "MOTION_SYNC_SPEED"
)]
pub struct MotionSyncPresetInquiry;

/// Inquiry command to get the noise reduction mode setting.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x53,
    response = "NoiseReductionMode",
    parser = "mode",
    value_type = "NoiseReductionMode",
    bytes_const = "NR_MODE"
)]
pub struct NrModeInquiry;

/// Inquiry command to get the noise reduction speed setting.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x54,
    response = "NoiseReductionSpeed",
    parser = "speed",
    value_type = "NoiseReductionSpeed",
    bytes_const = "NR_SPEED"
)]
pub struct NrSpeedInquiry;

/// Inquiry command to get the black and white mode setting.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x73,
    response = "BlackWhiteMode",
    parser = "mode",
    value_type = "BlackWhiteMode",
    bytes_const = "BLACK_WHITE_MODE"
)]
pub struct BlackWhiteModeInquiry;

/// Inquiry command to get the USB audio state.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x7A,
    response = "UsbAudio",
    parser = "bool",
    bytes_const = "USB_AUDIO"
)]
pub struct UsbAudioInquiry;

/// Inquiry command to get the two tone mode state.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x74,
    response = "TwoToneMode",
    parser = "bool",
    bytes_const = "TWO_TONE_MODE"
)]
pub struct TwoToneModeInquiry;

/// Inquiry command to get the ND filter preset setting.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x66,
    response = "NdFilterPreset",
    parser = "byte",
    bytes_const = "ND_FILTER_PRESET"
)]
pub struct NdFilterPresetInquiry;

/// Inquiry command to get the digital mode state.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x7B,
    response = "Digital",
    parser = "bool",
    bytes_const = "DIGITAL"
)]
pub struct DigitalInquiry;

/// Inquiry command to get the tally auto adjust state.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0xA9,
    response = "TallyAutoAdjust",
    parser = "bool",
    bytes_const = "TALLY_AUTO_ADJUST"
)]
pub struct TallyAutoAdjustInquiry;

/// Inquiry command to get the red tally light status (baseline VISCA).
/// Returns 0x02 for On, 0x03 for Off.
///
/// NOTE: This uses a special extended inquiry format (0x7E 0x01 0x0A 0x00)
/// instead of the standard inquiry format, which is why it cannot use
/// the ViscaInquiry derive macro.
#[derive(Debug, Copy, Clone)]
pub struct TallyRedInquiry;

impl crate::command::encode::ViscaCommand for TallyRedInquiry {
    type Response = crate::command::InquiryData;
    const MAX_SIZE: usize = 8;
    const TIMEOUT_CATEGORY: crate::timeout::CommandCategory =
        crate::timeout::CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::error::Error> {
        use crate::command::bytes::ConstCommandBuilder;

        let builder = ConstCommandBuilder::<8>::new()
            .append(crate::command::bytes::constants::inquiry::TALLY_RED)
            .with_camera_id(camera_id)
            .terminate();
        builder.build_into(buffer)
    }

    fn response_kind(&self) -> Option<crate::command::response::InquiryKind> {
        Some(crate::command::response::InquiryKind::TallyRed)
    }
}

impl crate::command::typed::ResponseParser for TallyRedInquiry {
    type Response = bool;

    fn from_response(resp: crate::command::Response) -> Result<Self::Response, crate::Error> {
        match resp {
            crate::command::Response::Inquiry(crate::command::InquiryData::TallyRed { on }) => {
                Ok(on)
            }
            crate::command::Response::Error(e) => Err(e),
            _ => Err(crate::Error::UnexpectedResponseType),
        }
    }
}

/// Inquiry command to get the green tally light status (Sony FR7 specific).
/// Returns 0x02 for On, 0x03 for Off.
///
/// NOTE: This uses a special extended inquiry format (0x7E 0x04 0x1A 0x00)
/// instead of the standard inquiry format, which is why it cannot use
/// the ViscaInquiry derive macro.
#[derive(Debug, Copy, Clone)]
pub struct TallyGreenInquiry;

impl crate::command::encode::ViscaCommand for TallyGreenInquiry {
    type Response = crate::command::InquiryData;
    const MAX_SIZE: usize = 7;
    const TIMEOUT_CATEGORY: crate::timeout::CommandCategory =
        crate::timeout::CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, crate::error::Error> {
        use crate::command::bytes::ConstCommandBuilder;

        let builder = ConstCommandBuilder::<7>::new()
            .append(crate::command::bytes::constants::inquiry::TALLY_GREEN)
            .with_camera_id(camera_id)
            .terminate();
        builder.build_into(buffer)
    }

    fn response_kind(&self) -> Option<crate::command::response::InquiryKind> {
        Some(crate::command::response::InquiryKind::TallyGreen)
    }
}

impl crate::command::typed::ResponseParser for TallyGreenInquiry {
    type Response = bool;

    fn from_response(resp: crate::command::Response) -> Result<Self::Response, crate::Error> {
        match resp {
            crate::command::Response::Inquiry(crate::command::InquiryData::TallyGreen { on }) => {
                Ok(on)
            }
            crate::command::Response::Error(e) => Err(e),
            _ => Err(crate::Error::UnexpectedResponseType),
        }
    }
}

/// Inquiry command to get the current flicker mode setting.
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0x55,
    response = "FlickerMode",
    parser = "mode",
    value_type = "AntiFlickerMode",
    bytes_const = "FLICKER_MODE"
)]
pub struct FlickerModeInquiry;

/// Inquiry command to get the current contrast level (image processing).
///
/// Queries register `0x04 0xA2`. The camera responds with a 4-nibble
/// payload encoding the contrast position (0–14).
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0xA2,
    response = "Contrast",
    parser = "last_nibble",
    field = "level",
    bytes_const = "CONTRAST"
)]
pub struct ContrastInquiry;

/// Inquiry command to get the current luminance (brightness) level (image processing).
///
/// Queries register `0x04 0xA1`. The camera responds with a 4-nibble
/// payload encoding the luminance position (0–14).
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(
    opcode = 0xA1,
    response = "Luminance",
    parser = "last_nibble",
    field = "level",
    bytes_const = "LUMINANCE_LEVEL"
)]
pub struct LuminanceInquiry;

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
    visca_test!(
        TallyRedInquiry,
        test_tally_red_inquiry,
        TallyRedInquiry,
        constants::inquiry::TALLY_RED
    );
    visca_test!(
        TallyGreenInquiry,
        test_tally_green_inquiry,
        TallyGreenInquiry,
        constants::inquiry::TALLY_GREEN
    );
    visca_test!(
        TallyStatusInquiry,
        test_tally_status_inquiry,
        TallyStatusInquiry,
        constants::inquiry::TALLY_STATUS
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
        FlipStateInquiry,
        test_flip_mode_inquiry,
        FlipStateInquiry,
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
    visca_test!(
        FlickerModeInquiry,
        test_flicker_mode_inquiry,
        FlickerModeInquiry,
        constants::inquiry::FLICKER_MODE
    );
    visca_test!(
        ContrastInquiry,
        test_contrast_inquiry,
        ContrastInquiry,
        constants::inquiry::CONTRAST
    );
    visca_test!(
        LuminanceInquiry,
        test_luminance_inquiry,
        LuminanceInquiry,
        constants::inquiry::LUMINANCE_LEVEL
    );
}
