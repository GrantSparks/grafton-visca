use log::error;

use crate::error::ViscaError;
use std::convert::TryFrom;

pub trait ViscaCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError>;
    fn response_type(&self) -> Option<ViscaResponseType>;
}

#[derive(Debug, Copy, Clone)]
pub enum Power {
    On = 0x02,
    Standby = 0x03,
}

#[derive(Debug, Copy, Clone)]
pub enum Flip {
    On = 0x02,
    Off = 0x03,
}

#[derive(Debug, Copy, Clone)]
pub enum ExposureMode {
    Auto = 0x00,
    Manual = 0x03,
    Shutter = 0x0A,
    Iris = 0x0B,
    Bright = 0x0D,
}

#[derive(Debug, Copy, Clone)]
pub enum WhiteBalanceMode {
    Auto = 0x00,
    Indoor = 0x01,
    Outdoor = 0x02,
    OnePush = 0x03,
    Manual = 0x05,
    ColorTemperature = 0x20,
}

#[derive(Debug, Copy, Clone)]
pub enum FocusMode {
    Auto = 0x02,
    Manual = 0x03,
}

#[derive(Debug, Copy, Clone)]
pub struct PanSpeed(u8);

impl PanSpeed {
    pub const STOP: PanSpeed = PanSpeed(0x00);
    pub const LOW_SPEED: PanSpeed = PanSpeed(0x01);
    pub const HIGH_SPEED: PanSpeed = PanSpeed(0x18);

    pub fn new(value: u8) -> Result<Self, ViscaError> {
        if Self::is_valid_value(value) {
            Ok(PanSpeed(value))
        } else {
            Err(ViscaError::InvalidParameter(
                "Pan speed must be in the range 0x00..=0x18".into(),
            ))
        }
    }

    fn is_valid_value(value: u8) -> bool {
        value == Self::STOP.0 || (0x01..=Self::HIGH_SPEED.0).contains(&value)
    }

    pub fn get_value(&self) -> u8 {
        self.0
    }
}

#[derive(Debug, Copy, Clone)]
pub struct TiltSpeed(u8);

impl TiltSpeed {
    pub const STOP: TiltSpeed = TiltSpeed(0x00);
    pub const LOW_SPEED: TiltSpeed = TiltSpeed(0x01);
    pub const HIGH_SPEED: TiltSpeed = TiltSpeed(0x14);

    pub fn new(value: u8) -> Result<Self, ViscaError> {
        if Self::is_valid_value(value) {
            Ok(TiltSpeed(value))
        } else {
            Err(ViscaError::InvalidParameter(
                "Tilt speed must be in the range 0x00..=0x14".into(),
            ))
        }
    }

    fn is_valid_value(value: u8) -> bool {
        value == Self::STOP.0 || (0x01..=Self::HIGH_SPEED.0).contains(&value)
    }

    pub fn get_value(&self) -> u8 {
        self.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PanTiltDirection {
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
    Stop,
    Home,
}

impl PanTiltDirection {
    pub fn to_bytes(self) -> (u8, u8) {
        match self {
            PanTiltDirection::Up => (0x03, 0x01),
            PanTiltDirection::Down => (0x03, 0x02),
            PanTiltDirection::Left => (0x01, 0x03),
            PanTiltDirection::Right => (0x02, 0x03),
            PanTiltDirection::UpLeft => (0x01, 0x01),
            PanTiltDirection::UpRight => (0x02, 0x01),
            PanTiltDirection::DownLeft => (0x01, 0x02),
            PanTiltDirection::DownRight => (0x02, 0x02),
            PanTiltDirection::Stop => (0x03, 0x03),
            PanTiltDirection::Home => (0x04, 0x04), // This is a special case for the Home command
        }
    }
}

// PanTiltCommand
pub struct PanTiltCommand {
    pub direction: PanTiltDirection,
    pub pan_speed: PanSpeed,
    pub tilt_speed: TiltSpeed,
}

impl ViscaCommand for PanTiltCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let (dir_byte1, dir_byte2) = self.direction.to_bytes();
        if self.direction == PanTiltDirection::Home {
            Ok(vec![0x81, 0x01, 0x06, 0x04, 0xFF])
        } else {
            Ok(vec![
                0x81,
                0x01,
                0x06,
                0x01,
                self.pan_speed.get_value(),
                self.tilt_speed.get_value(),
                dir_byte1,
                dir_byte2,
                0xFF,
            ])
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

// ZoomCommand
#[derive(Debug)]
pub enum ZoomCommand {
    Stop,
    TeleStandard,
    WideStandard,
    TeleVariable(u8),
    WideVariable(u8),
    Direct(u16),
}

impl ViscaCommand for ZoomCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        match self {
            ZoomCommand::Stop => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]),
            ZoomCommand::TeleStandard => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]),
            ZoomCommand::WideStandard => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x03, 0xFF]),
            ZoomCommand::TeleVariable(speed) => {
                if *speed <= 7 {
                    Ok(vec![0x81, 0x01, 0x04, 0x07, 0x20 | speed, 0xFF])
                } else {
                    Err(ViscaError::InvalidParameter(
                        "Zoom speed must be in the range 0..=7".into(),
                    ))
                }
            }
            ZoomCommand::WideVariable(speed) => {
                if *speed <= 7 {
                    Ok(vec![0x81, 0x01, 0x04, 0x07, 0x30 | speed, 0xFF])
                } else {
                    Err(ViscaError::InvalidParameter(
                        "Zoom speed must be in the range 0..=7".into(),
                    ))
                }
            }
            ZoomCommand::Direct(position) => {
                let p = (*position >> 12) as u8;
                let q = (*position >> 8) as u8;
                let r = (*position >> 4) as u8;
                let s = (*position & 0x0F) as u8;
                Ok(vec![0x81, 0x01, 0x04, 0x47, p, q, r, s, 0xFF])
            }
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        match self {
            ZoomCommand::WideStandard => Some(ViscaResponseType::ZoomWideStandard),
            ZoomCommand::TeleStandard => Some(ViscaResponseType::ZoomTeleStandard),
            _ => None,
        }
    }
}

// FocusCommand
#[derive(Debug)]
pub enum FocusCommand {
    Stop,
    FarStandard,
    NearStandard,
    FarVariable(u8),
    NearVariable(u8),
    Direct(u16),
    Auto,
    Manual,
    OnePushTrigger,
    Infinity,
}

impl ViscaCommand for FocusCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        match self {
            FocusCommand::Stop => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x00, 0xFF]),
            FocusCommand::FarStandard => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x02, 0xFF]),
            FocusCommand::NearStandard => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x03, 0xFF]),
            FocusCommand::FarVariable(speed) => {
                if *speed <= 7 {
                    Ok(vec![0x81, 0x01, 0x04, 0x08, 0x20 | speed, 0xFF])
                } else {
                    Err(ViscaError::InvalidParameter(
                        "Focus speed must be in the range 0..=7".into(),
                    ))
                }
            }
            FocusCommand::NearVariable(speed) => {
                if *speed <= 7 {
                    Ok(vec![0x81, 0x01, 0x04, 0x08, 0x30 | speed, 0xFF])
                } else {
                    Err(ViscaError::InvalidParameter(
                        "Focus speed must be in the range 0..=7".into(),
                    ))
                }
            }
            FocusCommand::Direct(position) => {
                let p = (*position >> 12) as u8;
                let q = (*position >> 8) as u8;
                let r = (*position >> 4) as u8;
                let s = (*position & 0x0F) as u8;
                Ok(vec![0x81, 0x01, 0x04, 0x48, p, q, r, s, 0xFF])
            }
            FocusCommand::Auto => Ok(vec![0x81, 0x01, 0x04, 0x38, 0x02, 0xFF]),
            FocusCommand::Manual => Ok(vec![0x81, 0x01, 0x04, 0x38, 0x03, 0xFF]),
            FocusCommand::OnePushTrigger => Ok(vec![0x81, 0x01, 0x04, 0x18, 0x01, 0xFF]),
            FocusCommand::Infinity => Ok(vec![0x81, 0x01, 0x04, 0x18, 0x02, 0xFF]),
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

// ExposureCommand
pub struct ExposureCommand {
    pub mode: ExposureMode,
}

impl ViscaCommand for ExposureCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(vec![0x81, 0x01, 0x04, 0x39, self.mode as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

// WhiteBalanceCommand
pub struct WhiteBalanceCommand {
    pub mode: WhiteBalanceMode,
}

impl ViscaCommand for WhiteBalanceCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(vec![0x81, 0x01, 0x04, 0x35, self.mode as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

// InquiryCommand
#[derive(Debug)]
pub enum InquiryCommand {
    PanTiltPosition,
    ZoomPosition,
    FocusPosition,
    ExposureMode,
    WhiteBalanceMode,
    Luminance,
    Contrast,
    // Add other inquiry commands as needed
}

impl ViscaCommand for InquiryCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let bytes = match self {
            InquiryCommand::PanTiltPosition => vec![0x81, 0x09, 0x06, 0x12, 0xFF],
            InquiryCommand::ZoomPosition => vec![0x81, 0x09, 0x04, 0x47, 0xFF],
            InquiryCommand::FocusPosition => vec![0x81, 0x09, 0x04, 0x48, 0xFF],
            InquiryCommand::ExposureMode => vec![0x81, 0x09, 0x04, 0x39, 0xFF],
            InquiryCommand::WhiteBalanceMode => vec![0x81, 0x09, 0x04, 0x35, 0xFF],
            InquiryCommand::Luminance => vec![0x81, 0x09, 0x04, 0xA1, 0xFF],
            InquiryCommand::Contrast => vec![0x81, 0x09, 0x04, 0xA2, 0xFF],
        };
        Ok(bytes)
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        match self {
            InquiryCommand::PanTiltPosition => Some(ViscaResponseType::PanTiltPosition),
            InquiryCommand::ZoomPosition => Some(ViscaResponseType::ZoomPosition),
            InquiryCommand::FocusPosition => Some(ViscaResponseType::FocusPosition),
            InquiryCommand::ExposureMode => Some(ViscaResponseType::ExposureMode),
            InquiryCommand::WhiteBalanceMode => Some(ViscaResponseType::WhiteBalanceMode),
            InquiryCommand::Luminance => Some(ViscaResponseType::Luminance),
            InquiryCommand::Contrast => Some(ViscaResponseType::Contrast),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViscaResponseType {
    Luminance,
    Contrast,
    SharpnessMode,
    SharpnessPosition,
    HorizontalFlip,
    VerticalFlip,
    ImageFlip,
    BlackWhiteMode,
    ExposureMode,
    ExposureCompensationMode,
    ExposureCompensationPosition,
    Backlight,
    Iris,
    Shutter,
    GainLimit,
    AntiFlicker,
    WhiteBalanceMode,
    RedTuning,
    BlueTuning,
    Saturation,
    Hue,
    RedGain,
    BlueGain,
    ColorTemperature,
    AutoWhiteBalanceSensitivity,
    ThreeDNoiseReduction,
    TwoDNoiseReduction,
    PanTiltPosition,
    ZoomPosition,
    MotionSyncMode,
    MotionSyncSpeed,
    FocusMode,
    FocusPosition,
    FocusZone,
    AutoFocusSensitivity,
    FocusRange,
    MenuOpenClose,
    UsbAudio,
    Rtmp,
    BlockLens,
    BlockColorExposure,
    BlockPowerImageEffect,
    BlockImage,
    ZoomWideStandard,
    ZoomTeleStandard,
}

// Define ViscaInquiryResponse to capture various inquiry responses.
#[derive(Debug)]
pub enum ViscaInquiryResponse {
    PanTiltPosition { pan: u32, tilt: u32 },
    Luminance(u8),
    Contrast(u8),
    ZoomPosition { position: u32 },
    FocusPosition { position: u32 },
    Gain { gain: u8 },
    WhiteBalance { mode: WhiteBalanceMode },
    ExposureMode { mode: ExposureMode },
    ExposureCompensation { value: i8 },
    Backlight { status: bool },
    ColorTemperature { temperature: u16 },
    Hue { hue: u8 },
    // Add other specific inquiry responses as needed.
}

#[derive(Debug)]
pub enum ViscaResponse {
    Ack,
    Completion,
    Error(ViscaError),
    InquiryResponse(ViscaInquiryResponse),
    Unknown(Vec<u8>),
}

// Additional command structures

pub struct LuminanceCommand {
    pub value: u8,
}

impl ViscaCommand for LuminanceCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        if self.value <= 14 {
            Ok(vec![
                0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00, self.value, 0xFF,
            ])
        } else {
            Err(ViscaError::InvalidParameter(
                "Luminance value must be in the range 0..=14".into(),
            ))
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

pub struct ContrastCommand {
    pub value: u8,
}

impl ViscaCommand for ContrastCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        if self.value <= 14 {
            Ok(vec![
                0x81, 0x01, 0x04, 0xA2, 0x00, 0x00, 0x00, self.value, 0xFF,
            ])
        } else {
            Err(ViscaError::InvalidParameter(
                "Contrast value must be in the range 0..=14".into(),
            ))
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

pub struct SharpnessCommand {
    pub value: u8,
}

impl ViscaCommand for SharpnessCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        if self.value <= 11 {
            Ok(vec![
                0x81, 0x01, 0x04, 0x42, 0x00, 0x00, 0x00, self.value, 0xFF,
            ])
        } else {
            Err(ViscaError::InvalidParameter(
                "Sharpness value must be in the range 0..=11".into(),
            ))
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

pub struct ImageFlipCommand {
    pub flip: Flip,
}

impl ViscaCommand for ImageFlipCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(vec![0x81, 0x01, 0x04, 0x66, self.flip as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

pub struct BacklightCommand {
    pub status: bool,
}

impl ViscaCommand for BacklightCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let status_byte = if self.status { 0x02 } else { 0x03 };
        Ok(vec![0x81, 0x01, 0x04, 0x33, status_byte, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

pub struct PowerCommand {
    pub power: Power,
}

impl ViscaCommand for PowerCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(vec![0x81, 0x01, 0x04, 0x00, self.power as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

pub fn parse_visca_response(
    response: &[u8],
    response_type: &ViscaResponseType,
) -> Result<ViscaResponse, ViscaError> {
    if response.len() < 3 || response[0] != 0x90 || response[response.len() - 1] != 0xFF {
        return Err(ViscaError::InvalidResponseFormat);
    }

    match response[1] {
        0x40..=0x4F => Ok(ViscaResponse::Ack),
        0x50..=0x5F => {
            // Handle simple completion response early
            if response.len() == 3 {
                return Ok(ViscaResponse::Completion);
            }

            match response_type {
                ViscaResponseType::PanTiltPosition => {
                    if response.len() != 11 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let pan =
                        u32::from_be_bytes([response[2], response[3], response[4], response[5]]);
                    let tilt =
                        u32::from_be_bytes([response[6], response[7], response[8], response[9]]);
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::PanTiltPosition { pan, tilt },
                    ))
                }
                ViscaResponseType::ZoomPosition => {
                    if response.len() != 7 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let position = u32::from_be_bytes([0, response[2], response[3], response[4]]);
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::ZoomPosition { position },
                    ))
                }
                ViscaResponseType::FocusPosition => {
                    if response.len() != 7 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let position = u32::from_be_bytes([0, response[2], response[3], response[4]]);
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::FocusPosition { position },
                    ))
                }
                ViscaResponseType::ExposureMode => {
                    if response.len() != 4 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let mode = ExposureMode::try_from(response[2])
                        .map_err(|_| ViscaError::UnexpectedResponseType)?;
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::ExposureMode { mode },
                    ))
                }
                ViscaResponseType::WhiteBalanceMode => {
                    if response.len() != 4 {
                        return Err(ViscaError::InvalidResponseLength);
                    }
                    let mode = WhiteBalanceMode::try_from(response[2])
                        .map_err(|_| ViscaError::UnexpectedResponseType)?;
                    Ok(ViscaResponse::InquiryResponse(
                        ViscaInquiryResponse::WhiteBalance { mode },
                    ))
                }
                // Add more response type parsing as needed
                _ => Ok(ViscaResponse::Completion),
            }
        }
        0x60..=0x6F => Err(ViscaError::from_code(response[1])),
        _ => {
            // Log unknown responses for future debugging
            error!("Unknown response: {:02X?}", response);
            Ok(ViscaResponse::Unknown(response.to_vec()))
        }
    }
}

// Implement TryFrom for ExposureMode and WhiteBalanceMode
impl TryFrom<u8> for ExposureMode {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0x00 => Ok(ExposureMode::Auto),
            0x03 => Ok(ExposureMode::Manual),
            0x0A => Ok(ExposureMode::Shutter),
            0x0B => Ok(ExposureMode::Iris),
            0x0D => Ok(ExposureMode::Bright),
            _ => Err(()),
        }
    }
}

impl TryFrom<u8> for WhiteBalanceMode {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0x00 => Ok(WhiteBalanceMode::Auto),
            0x01 => Ok(WhiteBalanceMode::Indoor),
            0x02 => Ok(WhiteBalanceMode::Outdoor),
            0x03 => Ok(WhiteBalanceMode::OnePush),
            0x05 => Ok(WhiteBalanceMode::Manual),
            0x20 => Ok(WhiteBalanceMode::ColorTemperature),
            _ => Err(()),
        }
    }
}
