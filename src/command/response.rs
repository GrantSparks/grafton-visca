//! VISCA response parsing and handling.
//!
//! This module provides response parsing functionality for VISCA protocol responses,
//! including ACK/completion messages, error responses, and inquiry data parsing.

// Third-party imports
use log::error;

// Crate imports
use crate::{
    command::{
        gain::AntiFlickerMode, luminance_contrast_sharpness::SharpnessMode, AFSensitivity,
        ExposureMode, FocusZone, ViscaInquiryResponse, WhiteBalanceMode,
    },
    error::ViscaError,
};

/// Response from a VISCA command.
///
/// Represents all possible responses from the camera including acknowledgments,
/// completions, errors, and inquiry data.
#[derive(Debug)]
pub enum ViscaResponse {
    /// Acknowledgment that the command was received and is being processed
    Ack,
    /// Command completed successfully (no data returned)
    Completion,
    /// Command failed with an error
    Error(ViscaError),
    /// Inquiry command response containing requested data
    InquiryResponse(ViscaInquiryResponse),
    /// Unknown response format (raw bytes provided for debugging)
    Unknown(Vec<u8>),
}

/// Type of expected response for inquiry commands.
///
/// Used to indicate what kind of data parser should expect in the response payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViscaResponseType {
    Power,
    PanTiltPosition,
    ZoomPosition,
    FocusPosition,
    ExposureMode,
    WhiteBalanceMode,
    Luminance,
    Contrast,
    Sharpness,
    SharpnessMode,
    SharpnessPosition,
    HorizontalFlip,
    VerticalFlip,
    ImageFlip,
    BlackWhiteMode,
    ExposureCompensation,
    ExposureCompensationMode,
    ExposureCompensationPosition,
    Backlight,
    Iris,
    Shutter,
    Bright,
    Gain,
    GainLimit,
    AntiFlicker,
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
    MotionSyncMode,
    MotionSyncSpeed,
    FocusMode,
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
    NoiseReduction2D,
    NoiseReduction3D,
    BlackWhite,
    AFSensitivity,
    FocusNearLimit,
    DynamicRange,
}

/// Parse a VISCA response from raw bytes.
///
/// # Errors
///
/// Returns `ViscaError::InvalidResponseFormat` if the response format is invalid.
/// Returns `ViscaError::InvalidResponseLength` if the response length doesn't match expected.
/// Returns `ViscaError::UnexpectedResponseType` if the response data is invalid for the type.
/// Returns a specific VISCA error code if the response indicates an error (0x60-0x6F).
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
            if response.len() == 3 {
                return Ok(ViscaResponse::Completion);
            }
            parse_inquiry_response(response, *response_type)
        }
        0x60..=0x6F => Err(ViscaError::from_code(response[2])),
        _ => {
            error!("Unknown response: {response:02X?}");
            Ok(ViscaResponse::Unknown(response.to_vec()))
        }
    }
}

fn parse_inquiry_response(
    response: &[u8],
    response_type: ViscaResponseType,
) -> Result<ViscaResponse, ViscaError> {
    match response_type {
        ViscaResponseType::Power => parse_power_response(response),
        ViscaResponseType::PanTiltPosition => parse_pan_tilt_position(response),
        ViscaResponseType::ZoomPosition => parse_position_response(response, PositionType::Zoom),
        ViscaResponseType::FocusPosition => parse_position_response(response, PositionType::Focus),
        ViscaResponseType::ExposureMode => parse_mode_response(response, ModeType::Exposure),
        ViscaResponseType::WhiteBalanceMode => {
            parse_mode_response(response, ModeType::WhiteBalance)
        }
        ViscaResponseType::ExposureCompensationMode => {
            parse_mode_response(response, ModeType::ExposureCompensation)
        }
        ViscaResponseType::SharpnessMode => parse_mode_response(response, ModeType::Sharpness),
        ViscaResponseType::BlackWhite => parse_mode_response(response, ModeType::BlackWhite),
        ViscaResponseType::Sharpness => {
            parse_value_response(response, ValueType::Extended(ExtendedValueType::Sharpness))
        }
        ViscaResponseType::ExposureCompensation => parse_value_response(
            response,
            ValueType::Extended(ExtendedValueType::ExposureCompensation),
        ),
        ViscaResponseType::Iris => {
            parse_value_response(response, ValueType::Extended(ExtendedValueType::Iris))
        }
        ViscaResponseType::Shutter => {
            parse_value_response(response, ValueType::Extended(ExtendedValueType::Shutter))
        }
        ViscaResponseType::Bright => {
            parse_value_response(response, ValueType::Extended(ExtendedValueType::Bright))
        }
        ViscaResponseType::Gain => {
            parse_value_response(response, ValueType::Extended(ExtendedValueType::Gain))
        }
        ViscaResponseType::GainLimit => {
            parse_value_response(response, ValueType::Simple(SimpleValueType::GainLimit))
        }
        ViscaResponseType::AntiFlicker => {
            parse_value_response(response, ValueType::Simple(SimpleValueType::AntiFlicker))
        }
        ViscaResponseType::Saturation => {
            parse_value_response(response, ValueType::Extended(ExtendedValueType::Saturation))
        }
        ViscaResponseType::Hue => {
            parse_value_response(response, ValueType::Extended(ExtendedValueType::Hue))
        }
        ViscaResponseType::RedGain => {
            parse_value_response(response, ValueType::Simple(SimpleValueType::RedGain))
        }
        ViscaResponseType::BlueGain => {
            parse_value_response(response, ValueType::Simple(SimpleValueType::BlueGain))
        }
        ViscaResponseType::ImageFlip => {
            parse_value_response(response, ValueType::Simple(SimpleValueType::ImageFlip))
        }
        ViscaResponseType::ColorTemperature => parse_value_response(
            response,
            ValueType::Extended(ExtendedValueType::ColorTemperature),
        ),
        ViscaResponseType::NoiseReduction2D => parse_value_response(
            response,
            ValueType::Simple(SimpleValueType::NoiseReduction2D),
        ),
        ViscaResponseType::NoiseReduction3D => parse_value_response(
            response,
            ValueType::Simple(SimpleValueType::NoiseReduction3D),
        ),
        ViscaResponseType::FocusZone => {
            parse_value_response(response, ValueType::Simple(SimpleValueType::FocusZone))
        }
        ViscaResponseType::AFSensitivity => {
            parse_value_response(response, ValueType::Simple(SimpleValueType::AFSensitivity))
        }
        ViscaResponseType::FocusNearLimit => {
            parse_position_response(response, PositionType::FocusNearLimit)
        }
        ViscaResponseType::DynamicRange => parse_value_response(
            response,
            ValueType::Extended(ExtendedValueType::DynamicRange),
        ),
        _ => Ok(ViscaResponse::Completion),
    }
}

#[allow(clippy::missing_const_for_fn)] // ViscaError contains String fields
fn parse_power_response(response: &[u8]) -> Result<ViscaResponse, ViscaError> {
    if response.len() != 4 {
        return Err(ViscaError::InvalidResponseLength);
    }
    let on = response[2] == 0x02;
    Ok(ViscaResponse::InquiryResponse(
        ViscaInquiryResponse::Power { on },
    ))
}

fn parse_pan_tilt_position(response: &[u8]) -> Result<ViscaResponse, ViscaError> {
    if response.len() != 11 {
        return Err(ViscaError::InvalidResponseLength);
    }

    let mut pan = i16::from(response[2]) << 12;
    pan |= i16::from(response[3]) << 8;
    pan |= i16::from(response[4]) << 4;
    pan |= i16::from(response[5]);

    let mut tilt = i16::from(response[6]) << 12;
    tilt |= i16::from(response[7]) << 8;
    tilt |= i16::from(response[8]) << 4;
    tilt |= i16::from(response[9]);

    Ok(ViscaResponse::InquiryResponse(
        ViscaInquiryResponse::PanTiltPosition { pan, tilt },
    ))
}

fn parse_position_response(
    response: &[u8],
    position_type: PositionType,
) -> Result<ViscaResponse, ViscaError> {
    if response.len() != 7 {
        return Err(ViscaError::InvalidResponseLength);
    }

    let mut position = u16::from(response[2]) << 12;
    position |= u16::from(response[3]) << 8;
    position |= u16::from(response[4]) << 4;
    position |= u16::from(response[5]);

    match position_type {
        PositionType::Zoom => Ok(ViscaResponse::InquiryResponse(
            ViscaInquiryResponse::ZoomPosition { position },
        )),
        PositionType::Focus => Ok(ViscaResponse::InquiryResponse(
            ViscaInquiryResponse::FocusPosition { position },
        )),
        PositionType::FocusNearLimit => Ok(ViscaResponse::InquiryResponse(
            ViscaInquiryResponse::FocusNearLimit { position },
        )),
    }
}

fn parse_mode_response(response: &[u8], mode_type: ModeType) -> Result<ViscaResponse, ViscaError> {
    if response.len() != 4 {
        return Err(ViscaError::InvalidResponseLength);
    }

    match mode_type {
        ModeType::Exposure => {
            let mode = ExposureMode::try_from(response[2])
                .map_err(|()| ViscaError::UnexpectedResponseType)?;
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::ExposureMode { mode },
            ))
        }
        ModeType::WhiteBalance => {
            let mode = WhiteBalanceMode::try_from(response[2])
                .map_err(|()| ViscaError::UnexpectedResponseType)?;
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::WhiteBalance { mode },
            ))
        }
        ModeType::ExposureCompensation => {
            let on = response[2] == 0x02;
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::ExposureCompensationMode { on },
            ))
        }
        ModeType::Sharpness => {
            let mode = match response[2] {
                0x02 => SharpnessMode::Auto,
                0x03 => SharpnessMode::Manual,
                _ => return Err(ViscaError::UnexpectedResponseType),
            };
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::SharpnessMode { mode },
            ))
        }
        ModeType::BlackWhite => {
            let on = response[2] == 0x04;
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::BlackWhite { on },
            ))
        }
    }
}

fn parse_value_response(
    response: &[u8],
    value_type: ValueType,
) -> Result<ViscaResponse, ViscaError> {
    match value_type {
        ValueType::Simple(simple_type) => parse_simple_value(response, simple_type),
        ValueType::Extended(extended_type) => parse_extended_value(response, extended_type),
    }
}

#[allow(clippy::missing_const_for_fn)] // ViscaError contains String fields
fn parse_simple_value(
    response: &[u8],
    value_type: SimpleValueType,
) -> Result<ViscaResponse, ViscaError> {
    if response.len() != 4 {
        return Err(ViscaError::InvalidResponseLength);
    }

    match value_type {
        SimpleValueType::GainLimit => {
            let limit = response[2];
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::GainLimit { limit },
            ))
        }
        SimpleValueType::AntiFlicker => {
            let mode = match response[2] {
                0x00 => AntiFlickerMode::Off,
                0x01 => AntiFlickerMode::Hz50,
                0x02 => AntiFlickerMode::Hz60,
                _ => return Err(ViscaError::UnexpectedResponseType),
            };
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::AntiFlicker { mode },
            ))
        }
        SimpleValueType::RedGain => {
            let gain = (response[2] as i8) - 10;
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::RedGain { gain },
            ))
        }
        SimpleValueType::BlueGain => {
            let gain = (response[2] as i8) - 10;
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::BlueGain { gain },
            ))
        }
        SimpleValueType::ImageFlip => {
            let vertical = (response[2] & 0x02) != 0;
            let horizontal = (response[2] & 0x01) != 0;
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::ImageFlip {
                    vertical,
                    horizontal,
                },
            ))
        }
        SimpleValueType::NoiseReduction2D => {
            let level = response[2];
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::NoiseReduction2D { level },
            ))
        }
        SimpleValueType::NoiseReduction3D => {
            let level = response[2];
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::NoiseReduction3D { level },
            ))
        }
        SimpleValueType::FocusZone => {
            let zone = match response[2] {
                0x00 => FocusZone::Top,
                0x01 => FocusZone::Center,
                0x02 => FocusZone::Bottom,
                _ => return Err(ViscaError::UnexpectedResponseType),
            };
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::FocusZone { zone },
            ))
        }
        SimpleValueType::AFSensitivity => {
            let sensitivity = match response[2] {
                0x02 => AFSensitivity::High,
                0x01 => AFSensitivity::Normal,
                0x00 => AFSensitivity::Low,
                _ => return Err(ViscaError::UnexpectedResponseType),
            };
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::AFSensitivity { sensitivity },
            ))
        }
    }
}

fn parse_extended_value(
    response: &[u8],
    value_type: ExtendedValueType,
) -> Result<ViscaResponse, ViscaError> {
    if response.len() != 7 {
        return Err(ViscaError::InvalidResponseLength);
    }

    match value_type {
        ExtendedValueType::Sharpness => {
            let value = (response[4] << 4) | response[5];
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::Sharpness { value },
            ))
        }
        ExtendedValueType::ExposureCompensation => {
            let raw_value = response[5];
            let value = (raw_value as i8) - 7;
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::ExposureCompensation { value },
            ))
        }
        ExtendedValueType::Iris => {
            let position = response[5];
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::Iris {
                position,
            }))
        }
        ExtendedValueType::Shutter => {
            let position = (u16::from(response[4]) << 4) | u16::from(response[5]);
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::Shutter { position },
            ))
        }
        ExtendedValueType::Bright => {
            let position = (u16::from(response[4]) << 4) | u16::from(response[5]);
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::Bright { position },
            ))
        }
        ExtendedValueType::Gain => {
            let gain = (response[4] << 4) | response[5];
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::Gain {
                gain,
            }))
        }
        ExtendedValueType::Saturation => {
            let level = response[5];
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::Saturation { level },
            ))
        }
        ExtendedValueType::Hue => {
            let hue = response[5];
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::Hue {
                hue,
            }))
        }
        ExtendedValueType::ColorTemperature => {
            let temperature = (u16::from(response[4]) << 4) | u16::from(response[5]);
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::ColorTemperature { temperature },
            ))
        }
        ExtendedValueType::DynamicRange => {
            let level = response[5];
            Ok(ViscaResponse::InquiryResponse(
                ViscaInquiryResponse::DynamicRange { level },
            ))
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum PositionType {
    Zoom,
    Focus,
    FocusNearLimit,
}

#[derive(Debug, Clone, Copy)]
enum ModeType {
    Exposure,
    WhiteBalance,
    ExposureCompensation,
    Sharpness,
    BlackWhite,
}

#[derive(Debug, Clone, Copy)]
enum ValueType {
    Simple(SimpleValueType),
    Extended(ExtendedValueType),
}

#[derive(Debug, Clone, Copy)]
enum SimpleValueType {
    GainLimit,
    AntiFlicker,
    RedGain,
    BlueGain,
    ImageFlip,
    NoiseReduction2D,
    NoiseReduction3D,
    FocusZone,
    AFSensitivity,
}

#[derive(Debug, Clone, Copy)]
enum ExtendedValueType {
    Sharpness,
    ExposureCompensation,
    Iris,
    Shutter,
    Bright,
    Gain,
    Saturation,
    Hue,
    ColorTemperature,
    DynamicRange,
}
