//! VISCA response parsing and handling.
//!
//! This module provides response parsing functionality for VISCA protocol responses,
//! including ACK/completion messages, error responses, and inquiry data parsing.

// Standard library imports
// (none)

// Third-party crate imports
use log::error;

// Workspace / local-crate imports
use crate::{
    command::{
        gain::AntiFlickerMode, luminance_contrast_sharpness::SharpnessMode, AFSensitivity,
        ExposureMode, FocusZone, InquiryResponse, WhiteBalanceMode,
    },
    error::Error,
};

/// Response from a VISCA command.
///
/// Represents all possible responses from the camera including acknowledgments,
/// completions, errors, and inquiry data.
#[derive(Debug)]
pub enum Response {
    /// Acknowledgment that the command was received and is being processed
    Ack,
    /// Command completed successfully (no data returned)
    Completion,
    /// Command failed with an error
    Error(Error),
    /// Inquiry command response containing requested data
    InquiryResponse(InquiryResponse),
    /// Unknown response format (raw bytes provided for debugging)
    Unknown(Vec<u8>),
}

/// Type of expected response for inquiry commands.
///
/// Used to indicate what kind of data parser should expect in the response payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseType {
    /// Power state inquiry response (On/Off).
    Power,
    /// Pan and tilt position inquiry response.
    PanTiltPosition,
    /// Zoom position inquiry response.
    ZoomPosition,
    /// Focus position inquiry response.
    FocusPosition,
    /// Exposure mode inquiry response (Auto/Manual/Shutter/Iris/Bright).
    ExposureMode,
    /// White balance mode inquiry response (Auto/Indoor/Outdoor/OnePush/Manual/ColorTemperature).
    WhiteBalanceMode,
    /// Luminance level inquiry response (0-14).
    Luminance,
    /// Contrast level inquiry response (0-14).
    Contrast,
    /// Sharpness level inquiry response (0-11).
    Sharpness,
    /// Sharpness mode inquiry response (Auto/Manual).
    SharpnessMode,
    /// Sharpness position inquiry response.
    SharpnessPosition,
    /// Horizontal flip state inquiry response.
    HorizontalFlip,
    /// Vertical flip state inquiry response.
    VerticalFlip,
    /// Combined image flip state inquiry response.
    ImageFlip,
    /// Black and white mode inquiry response.
    BlackWhiteMode,
    /// Exposure compensation value inquiry response (-7 to +7).
    ExposureCompensation,
    /// Exposure compensation mode inquiry response (On/Off).
    ExposureCompensationMode,
    /// Exposure compensation position inquiry response.
    ExposureCompensationPosition,
    /// Backlight compensation inquiry response (On/Off).
    Backlight,
    /// Iris setting inquiry response (0x0=Close to 0xC=F1.8).
    Iris,
    /// Shutter speed inquiry response (0x01=1/30 to 0x11=1/10000).
    Shutter,
    /// Brightness level inquiry response (0-17).
    Bright,
    /// Gain level inquiry response (0-7).
    Gain,
    /// Gain limit inquiry response (0-15).
    GainLimit,
    /// Anti-flicker mode inquiry response (Off/50Hz/60Hz).
    AntiFlicker,
    /// Red tuning value inquiry response (-10 to +10).
    RedTuning,
    /// Blue tuning value inquiry response (-10 to +10).
    BlueTuning,
    /// Saturation level inquiry response (60%-200%).
    Saturation,
    /// Hue level inquiry response (0-14).
    Hue,
    /// Red gain value inquiry response.
    RedGain,
    /// Blue gain value inquiry response.
    BlueGain,
    /// Color temperature value inquiry response.
    ColorTemperature,
    /// Auto white balance sensitivity inquiry response.
    AutoWhiteBalanceSensitivity,
    /// 3D noise reduction setting inquiry response.
    ThreeDNoiseReduction,
    /// 2D noise reduction setting inquiry response.
    TwoDNoiseReduction,
    /// Motion sync mode inquiry response.
    MotionSyncMode,
    /// Motion sync speed inquiry response.
    MotionSyncSpeed,
    /// Focus mode inquiry response (Auto/Manual).
    FocusMode,
    /// Focus zone setting inquiry response.
    FocusZone,
    /// Auto-focus sensitivity inquiry response.
    AutoFocusSensitivity,
    /// Focus range inquiry response.
    FocusRange,
    /// Menu open/close state inquiry response.
    MenuOpenClose,
    /// USB audio state inquiry response.
    UsbAudio,
    /// RTMP streaming state inquiry response.
    Rtmp,
    /// Block lens movement inquiry response.
    BlockLens,
    /// Block color/exposure control inquiry response.
    BlockColorExposure,
    /// Block power/image effect inquiry response.
    BlockPowerImageEffect,
    /// Block image control inquiry response.
    BlockImage,
    /// Zoom wide standard operation inquiry response.
    ZoomWideStandard,
    /// Zoom tele standard operation inquiry response.
    ZoomTeleStandard,
    /// 2D noise reduction setting inquiry response.
    NoiseReduction2D,
    /// 3D noise reduction setting inquiry response.
    NoiseReduction3D,
    /// Black and white mode inquiry response.
    BlackWhite,
    /// Auto-focus sensitivity inquiry response.
    AFSensitivity,
    /// Focus near limit position inquiry response.
    FocusNearLimit,
    /// Dynamic range control level inquiry response (0-8).
    DynamicRange,
}

/// Parse a VISCA response from raw bytes.
///
/// # Errors
///
/// Returns `Error::InvalidResponseFormat` if the response format is invalid.
/// Returns `Error::InvalidResponseLength` if the response length doesn't match expected.
/// Returns `Error::UnexpectedResponseType` if the response data is invalid for the type.
/// Returns a specific VISCA error code if the response indicates an error (0x60-0x6F).
pub fn parse_visca_response(
    response: &[u8],
    response_type: &ResponseType,
) -> Result<Response, Error> {
    if response.len() < 3 || response[0] != 0x90 || response[response.len() - 1] != 0xFF {
        return Err(Error::InvalidResponseFormat);
    }

    match response[1] {
        0x40..=0x4F => Ok(Response::Ack),
        0x50..=0x5F => {
            if response.len() == 3 {
                return Ok(Response::Completion);
            }
            parse_inquiry_response(response, *response_type)
        }
        0x60..=0x6F => Err(Error::from_code(response[2])),
        _ => {
            error!("Unknown response: {response:02X?}");
            Ok(Response::Unknown(response.to_vec()))
        }
    }
}

fn parse_inquiry_response(response: &[u8], response_type: ResponseType) -> Result<Response, Error> {
    match response_type {
        ResponseType::Power => parse_power_response(response),
        ResponseType::PanTiltPosition => parse_pan_tilt_position(response),
        ResponseType::ZoomPosition => parse_position_response(response, PositionType::Zoom),
        ResponseType::FocusPosition => parse_position_response(response, PositionType::Focus),
        ResponseType::ExposureMode => parse_mode_response(response, ModeType::Exposure),
        ResponseType::WhiteBalanceMode => parse_mode_response(response, ModeType::WhiteBalance),
        ResponseType::ExposureCompensationMode => {
            parse_mode_response(response, ModeType::ExposureCompensation)
        }
        ResponseType::SharpnessMode => parse_mode_response(response, ModeType::Sharpness),
        ResponseType::BlackWhite => parse_mode_response(response, ModeType::BlackWhite),
        ResponseType::Sharpness => {
            parse_value_response(response, ValueType::Extended(ExtendedValueType::Sharpness))
        }
        ResponseType::ExposureCompensation => parse_value_response(
            response,
            ValueType::Extended(ExtendedValueType::ExposureCompensation),
        ),
        ResponseType::Iris => {
            parse_value_response(response, ValueType::Extended(ExtendedValueType::Iris))
        }
        ResponseType::Shutter => {
            parse_value_response(response, ValueType::Extended(ExtendedValueType::Shutter))
        }
        ResponseType::Bright => {
            parse_value_response(response, ValueType::Extended(ExtendedValueType::Bright))
        }
        ResponseType::Gain => {
            parse_value_response(response, ValueType::Extended(ExtendedValueType::Gain))
        }
        ResponseType::GainLimit => {
            parse_value_response(response, ValueType::Simple(SimpleValueType::GainLimit))
        }
        ResponseType::AntiFlicker => {
            parse_value_response(response, ValueType::Simple(SimpleValueType::AntiFlicker))
        }
        ResponseType::Saturation => {
            parse_value_response(response, ValueType::Extended(ExtendedValueType::Saturation))
        }
        ResponseType::Hue => {
            parse_value_response(response, ValueType::Extended(ExtendedValueType::Hue))
        }
        ResponseType::RedGain => {
            parse_value_response(response, ValueType::Simple(SimpleValueType::RedGain))
        }
        ResponseType::BlueGain => {
            parse_value_response(response, ValueType::Simple(SimpleValueType::BlueGain))
        }
        ResponseType::ImageFlip => {
            parse_value_response(response, ValueType::Simple(SimpleValueType::ImageFlip))
        }
        ResponseType::ColorTemperature => parse_value_response(
            response,
            ValueType::Extended(ExtendedValueType::ColorTemperature),
        ),
        ResponseType::NoiseReduction2D => parse_value_response(
            response,
            ValueType::Simple(SimpleValueType::NoiseReduction2D),
        ),
        ResponseType::NoiseReduction3D => parse_value_response(
            response,
            ValueType::Simple(SimpleValueType::NoiseReduction3D),
        ),
        ResponseType::FocusZone => {
            parse_value_response(response, ValueType::Simple(SimpleValueType::FocusZone))
        }
        ResponseType::AFSensitivity => {
            parse_value_response(response, ValueType::Simple(SimpleValueType::AFSensitivity))
        }
        ResponseType::FocusNearLimit => {
            parse_position_response(response, PositionType::FocusNearLimit)
        }
        ResponseType::DynamicRange => parse_value_response(
            response,
            ValueType::Extended(ExtendedValueType::DynamicRange),
        ),
        _ => Ok(Response::Completion),
    }
}

#[allow(clippy::missing_const_for_fn)] // Error contains String fields
fn parse_power_response(response: &[u8]) -> Result<Response, Error> {
    if response.len() != 4 {
        return Err(Error::InvalidResponseLength);
    }
    let on = response[2] == 0x02;
    Ok(Response::InquiryResponse(InquiryResponse::Power { on }))
}

fn parse_pan_tilt_position(response: &[u8]) -> Result<Response, Error> {
    if response.len() != 11 {
        return Err(Error::InvalidResponseLength);
    }

    let mut pan = i16::from(response[2]) << 12;
    pan |= i16::from(response[3]) << 8;
    pan |= i16::from(response[4]) << 4;
    pan |= i16::from(response[5]);

    let mut tilt = i16::from(response[6]) << 12;
    tilt |= i16::from(response[7]) << 8;
    tilt |= i16::from(response[8]) << 4;
    tilt |= i16::from(response[9]);

    Ok(Response::InquiryResponse(
        InquiryResponse::PanTiltPosition { pan, tilt },
    ))
}

fn parse_position_response(
    response: &[u8],
    position_type: PositionType,
) -> Result<Response, Error> {
    if response.len() != 7 {
        return Err(Error::InvalidResponseLength);
    }

    let mut position = u16::from(response[2]) << 12;
    position |= u16::from(response[3]) << 8;
    position |= u16::from(response[4]) << 4;
    position |= u16::from(response[5]);

    match position_type {
        PositionType::Zoom => Ok(Response::InquiryResponse(InquiryResponse::ZoomPosition {
            position,
        })),
        PositionType::Focus => Ok(Response::InquiryResponse(InquiryResponse::FocusPosition {
            position,
        })),
        PositionType::FocusNearLimit => {
            Ok(Response::InquiryResponse(InquiryResponse::FocusNearLimit {
                position,
            }))
        }
    }
}

fn parse_mode_response(response: &[u8], mode_type: ModeType) -> Result<Response, Error> {
    if response.len() != 4 {
        return Err(Error::InvalidResponseLength);
    }

    match mode_type {
        ModeType::Exposure => {
            let mode =
                ExposureMode::try_from(response[2]).map_err(|()| Error::UnexpectedResponseType)?;
            Ok(Response::InquiryResponse(InquiryResponse::ExposureMode {
                mode,
            }))
        }
        ModeType::WhiteBalance => {
            let mode = WhiteBalanceMode::try_from(response[2])
                .map_err(|()| Error::UnexpectedResponseType)?;
            Ok(Response::InquiryResponse(InquiryResponse::WhiteBalance {
                mode,
            }))
        }
        ModeType::ExposureCompensation => {
            let on = response[2] == 0x02;
            Ok(Response::InquiryResponse(
                InquiryResponse::ExposureCompensationMode { on },
            ))
        }
        ModeType::Sharpness => {
            let mode = match response[2] {
                0x02 => SharpnessMode::Auto,
                0x03 => SharpnessMode::Manual,
                _ => return Err(Error::UnexpectedResponseType),
            };
            Ok(Response::InquiryResponse(InquiryResponse::SharpnessMode {
                mode,
            }))
        }
        ModeType::BlackWhite => {
            let on = response[2] == 0x04;
            Ok(Response::InquiryResponse(InquiryResponse::BlackWhite {
                on,
            }))
        }
    }
}

fn parse_value_response(response: &[u8], value_type: ValueType) -> Result<Response, Error> {
    match value_type {
        ValueType::Simple(simple_type) => parse_simple_value(response, simple_type),
        ValueType::Extended(extended_type) => parse_extended_value(response, extended_type),
    }
}

#[allow(clippy::missing_const_for_fn)] // Error contains String fields
fn parse_simple_value(response: &[u8], value_type: SimpleValueType) -> Result<Response, Error> {
    if response.len() != 4 {
        return Err(Error::InvalidResponseLength);
    }

    match value_type {
        SimpleValueType::GainLimit => {
            let limit = response[2];
            Ok(Response::InquiryResponse(InquiryResponse::GainLimit {
                limit,
            }))
        }
        SimpleValueType::AntiFlicker => {
            let mode = match response[2] {
                0x00 => AntiFlickerMode::Off,
                0x01 => AntiFlickerMode::Hz50,
                0x02 => AntiFlickerMode::Hz60,
                _ => return Err(Error::UnexpectedResponseType),
            };
            Ok(Response::InquiryResponse(InquiryResponse::AntiFlicker {
                mode,
            }))
        }
        SimpleValueType::RedGain => {
            let gain = i16::from(response[2]) - 10;
            #[allow(clippy::cast_possible_truncation)]
            let gain = gain as i8; // Safe: VISCA gain values are in valid range
            Ok(Response::InquiryResponse(InquiryResponse::RedGain { gain }))
        }
        SimpleValueType::BlueGain => {
            let gain = i16::from(response[2]) - 10;
            #[allow(clippy::cast_possible_truncation)]
            let gain = gain as i8; // Safe: VISCA gain values are in valid range
            Ok(Response::InquiryResponse(InquiryResponse::BlueGain {
                gain,
            }))
        }
        SimpleValueType::ImageFlip => {
            let vertical = (response[2] & 0x02) != 0;
            let horizontal = (response[2] & 0x01) != 0;
            Ok(Response::InquiryResponse(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            }))
        }
        SimpleValueType::NoiseReduction2D => {
            let level = response[2];
            Ok(Response::InquiryResponse(
                InquiryResponse::NoiseReduction2D { level },
            ))
        }
        SimpleValueType::NoiseReduction3D => {
            let level = response[2];
            Ok(Response::InquiryResponse(
                InquiryResponse::NoiseReduction3D { level },
            ))
        }
        SimpleValueType::FocusZone => {
            let zone = match response[2] {
                0x00 => FocusZone::Top,
                0x01 => FocusZone::Center,
                0x02 => FocusZone::Bottom,
                _ => return Err(Error::UnexpectedResponseType),
            };
            Ok(Response::InquiryResponse(InquiryResponse::FocusZone {
                zone,
            }))
        }
        SimpleValueType::AFSensitivity => {
            let sensitivity = match response[2] {
                0x02 => AFSensitivity::High,
                0x01 => AFSensitivity::Normal,
                0x00 => AFSensitivity::Low,
                _ => return Err(Error::UnexpectedResponseType),
            };
            Ok(Response::InquiryResponse(InquiryResponse::AFSensitivity {
                sensitivity,
            }))
        }
    }
}

fn parse_extended_value(response: &[u8], value_type: ExtendedValueType) -> Result<Response, Error> {
    if response.len() != 7 {
        return Err(Error::InvalidResponseLength);
    }

    match value_type {
        ExtendedValueType::Sharpness => {
            let value = (response[4] << 4) | response[5];
            Ok(Response::InquiryResponse(InquiryResponse::Sharpness {
                value,
            }))
        }
        ExtendedValueType::ExposureCompensation => {
            let raw_value = response[5];
            let value = i16::from(raw_value) - 7;
            #[allow(clippy::cast_possible_truncation)]
            let value = value as i8; // Safe: VISCA exposure compensation values are in valid range
            Ok(Response::InquiryResponse(
                InquiryResponse::ExposureCompensation { value },
            ))
        }
        ExtendedValueType::Iris => {
            let position = response[5];
            Ok(Response::InquiryResponse(InquiryResponse::Iris {
                position,
            }))
        }
        ExtendedValueType::Shutter => {
            let position = (u16::from(response[4]) << 4) | u16::from(response[5]);
            Ok(Response::InquiryResponse(InquiryResponse::Shutter {
                position,
            }))
        }
        ExtendedValueType::Bright => {
            let position = (u16::from(response[4]) << 4) | u16::from(response[5]);
            Ok(Response::InquiryResponse(InquiryResponse::Bright {
                position,
            }))
        }
        ExtendedValueType::Gain => {
            let gain = (response[4] << 4) | response[5];
            Ok(Response::InquiryResponse(InquiryResponse::Gain { gain }))
        }
        ExtendedValueType::Saturation => {
            let level = response[5];
            Ok(Response::InquiryResponse(InquiryResponse::Saturation {
                level,
            }))
        }
        ExtendedValueType::Hue => {
            let hue = response[5];
            Ok(Response::InquiryResponse(InquiryResponse::Hue { hue }))
        }
        ExtendedValueType::ColorTemperature => {
            let temperature = (u16::from(response[4]) << 4) | u16::from(response[5]);
            Ok(Response::InquiryResponse(
                InquiryResponse::ColorTemperature { temperature },
            ))
        }
        ExtendedValueType::DynamicRange => {
            let level = response[5];
            Ok(Response::InquiryResponse(InquiryResponse::DynamicRange {
                level,
            }))
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

// Temporary alias for backward compatibility
