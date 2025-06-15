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
        gain::AntiFlickerMode, luminance_contrast_sharpness::SharpnessMode, AutoFocusSensitivity,
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
    /// Zoom out standard operation inquiry response.
    ZoomOutStandard,
    /// Zoom in standard operation inquiry response.
    ZoomInStandard,
    /// 2D noise reduction setting inquiry response.
    NoiseReduction2D,
    /// 3D noise reduction setting inquiry response.
    NoiseReduction3D,
    /// Black and white mode inquiry response.
    BlackWhite,
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
pub fn parse_response(response: &[u8], response_type: &ResponseType) -> Result<Response, Error> {
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
        ResponseType::Luminance => parse_luminance_response(response),
        ResponseType::Contrast => parse_contrast_response(response),
        ResponseType::Backlight => parse_backlight_response(response),
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
        ResponseType::AutoFocusSensitivity => parse_value_response(
            response,
            ValueType::Simple(SimpleValueType::AutoFocusSensitivity),
        ),
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

#[allow(clippy::missing_const_for_fn)] // Error contains String fields
fn parse_luminance_response(response: &[u8]) -> Result<Response, Error> {
    if response.len() != 4 {
        return Err(Error::InvalidResponseLength);
    }
    let value = response[2];
    Ok(Response::InquiryResponse(InquiryResponse::Luminance(value)))
}

#[allow(clippy::missing_const_for_fn)] // Error contains String fields
fn parse_contrast_response(response: &[u8]) -> Result<Response, Error> {
    if response.len() != 4 {
        return Err(Error::InvalidResponseLength);
    }
    let value = response[2];
    Ok(Response::InquiryResponse(InquiryResponse::Contrast(value)))
}

#[allow(clippy::missing_const_for_fn)] // Error contains String fields
fn parse_backlight_response(response: &[u8]) -> Result<Response, Error> {
    if response.len() != 4 {
        return Err(Error::InvalidResponseLength);
    }
    let status = response[2] == 0x02;
    Ok(Response::InquiryResponse(InquiryResponse::Backlight {
        status,
    }))
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
        SimpleValueType::AutoFocusSensitivity => {
            let sensitivity = match response[2] {
                0x02 => AutoFocusSensitivity::High,
                0x01 => AutoFocusSensitivity::Normal,
                0x00 => AutoFocusSensitivity::Low,
                _ => return Err(Error::UnexpectedResponseType),
            };
            Ok(Response::InquiryResponse(
                InquiryResponse::AutoFocusSensitivity { sensitivity },
            ))
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
    AutoFocusSensitivity,
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::uninlined_format_args,
    clippy::match_same_arms
)]
mod tests {
    use super::*;

    #[test]
    fn test_response_debug() {
        let ack = Response::Ack;
        assert_eq!(format!("{:?}", ack), "Ack");

        let completion = Response::Completion;
        assert_eq!(format!("{:?}", completion), "Completion");

        let unknown = Response::Unknown(vec![0x90, 0x50, 0xFF]);
        assert_eq!(format!("{:?}", unknown), "Unknown([144, 80, 255])");
    }

    #[test]
    fn test_response_type_equality() {
        assert_eq!(ResponseType::Power, ResponseType::Power);
        assert_ne!(ResponseType::Power, ResponseType::ZoomPosition);
    }

    #[test]
    fn test_parse_ack_response() {
        let response = vec![0x90, 0x41, 0xFF];
        let result = parse_response(&response, &ResponseType::Power).unwrap();
        assert!(matches!(result, Response::Ack));

        // Test all socket variations
        for socket in 0x40..=0x4F {
            let response = vec![0x90, socket, 0xFF];
            let result = parse_response(&response, &ResponseType::Power).unwrap();
            assert!(matches!(result, Response::Ack));
        }
    }

    #[test]
    fn test_parse_completion_response() {
        let response = vec![0x90, 0x51, 0xFF];
        let result = parse_response(&response, &ResponseType::Power).unwrap();
        assert!(matches!(result, Response::Completion));

        // Test all socket variations
        for socket in 0x50..=0x5F {
            let response = vec![0x90, socket, 0xFF];
            let result = parse_response(&response, &ResponseType::Power).unwrap();
            assert!(matches!(result, Response::Completion));
        }
    }

    #[test]
    fn test_parse_error_responses() {
        // Test all error codes
        let error_codes = vec![
            (0x01, "Syntax Error"),
            (0x02, "Command Buffer Full"),
            (0x03, "Command Canceled"),
            (0x04, "No Socket"),
            (0x05, "Not Executable"),
            (0x41, "Command Not Executable"),
        ];

        for (code, _desc) in error_codes {
            let response = vec![0x90, 0x60, code, 0xFF];
            let result = parse_response(&response, &ResponseType::Power);
            assert!(result.is_err());

            // Verify the error matches the expected code
            match (code, &result) {
                (0x01, Err(Error::Unknown(0x01))) => {}
                (0x02, Err(Error::SyntaxError)) => {}
                (0x03, Err(Error::CommandBufferFull)) => {}
                (0x04, Err(Error::CommandCanceled)) => {}
                (0x05, Err(Error::NoSocket)) => {}
                (0x41, Err(Error::CommandNotExecutable)) => {}
                (code_val, Err(Error::Unknown(c))) if *c == code_val => {}
                other => panic!("Expected error for code {:#02X}, got {:?}", code, other),
            }
        }
    }

    #[test]
    fn test_parse_invalid_response_format() {
        // Too short
        let response = vec![0x90, 0xFF];
        let result = parse_response(&response, &ResponseType::Power);
        assert!(matches!(result, Err(Error::InvalidResponseFormat)));

        // Wrong start byte
        let response = vec![0x80, 0x50, 0xFF];
        let result = parse_response(&response, &ResponseType::Power);
        assert!(matches!(result, Err(Error::InvalidResponseFormat)));

        // Wrong end byte
        let response = vec![0x90, 0x50, 0xFE];
        let result = parse_response(&response, &ResponseType::Power);
        assert!(matches!(result, Err(Error::InvalidResponseFormat)));

        // Empty response
        let response = vec![];
        let result = parse_response(&response, &ResponseType::Power);
        assert!(matches!(result, Err(Error::InvalidResponseFormat)));
    }

    #[test]
    fn test_parse_unknown_response() {
        let response = vec![0x90, 0x30, 0xFF]; // Unknown socket byte
        let result = parse_response(&response, &ResponseType::Power).unwrap();
        match result {
            Response::Unknown(bytes) => assert_eq!(bytes, vec![0x90, 0x30, 0xFF]),
            _ => panic!("Expected Unknown response"),
        }
    }

    #[test]
    fn test_parse_power_response() {
        // Power On
        let response = vec![0x90, 0x50, 0x02, 0xFF];
        let result = parse_response(&response, &ResponseType::Power).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::Power { on }) => assert!(on),
            _ => panic!("Expected Power inquiry response"),
        }

        // Power Off
        let response = vec![0x90, 0x50, 0x03, 0xFF];
        let result = parse_response(&response, &ResponseType::Power).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::Power { on }) => assert!(!on),
            _ => panic!("Expected Power inquiry response"),
        }

        // Invalid length
        let response = vec![0x90, 0x50, 0x02, 0x00, 0xFF];
        let result = parse_response(&response, &ResponseType::Power);
        assert!(matches!(result, Err(Error::InvalidResponseLength)));
    }

    #[test]
    fn test_parse_pan_tilt_position() {
        let response = vec![
            0x90, 0x50, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0xFF,
        ];
        let result = parse_response(&response, &ResponseType::PanTiltPosition).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt }) => {
                assert_eq!(pan, -1);
                assert_eq!(tilt, -1);
            }
            _ => panic!("Expected PanTiltPosition inquiry response"),
        }

        // Test positive values
        let response = vec![
            0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0xFF,
        ];
        let result = parse_response(&response, &ResponseType::PanTiltPosition).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt }) => {
                assert_eq!(pan, 0x1234);
                assert_eq!(tilt, 0x5678);
            }
            _ => panic!("Expected PanTiltPosition inquiry response"),
        }

        // Invalid length
        let response = vec![0x90, 0x50, 0x00, 0x00, 0xFF];
        let result = parse_response(&response, &ResponseType::PanTiltPosition);
        assert!(matches!(result, Err(Error::InvalidResponseLength)));
    }

    #[test]
    fn test_parse_zoom_position() {
        let response = vec![0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xFF];
        let result = parse_response(&response, &ResponseType::ZoomPosition).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::ZoomPosition { position }) => {
                assert_eq!(position, 0x1234);
            }
            _ => panic!("Expected ZoomPosition inquiry response"),
        }
    }

    #[test]
    fn test_parse_focus_position() {
        let response = vec![0x90, 0x50, 0x0A, 0x0B, 0x0C, 0x0D, 0xFF];
        let result = parse_response(&response, &ResponseType::FocusPosition).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::FocusPosition { position }) => {
                assert_eq!(position, 0xABCD);
            }
            _ => panic!("Expected FocusPosition inquiry response"),
        }
    }

    #[test]
    fn test_parse_focus_near_limit() {
        let response = vec![0x90, 0x50, 0x05, 0x05, 0x05, 0x05, 0xFF];
        let result = parse_response(&response, &ResponseType::FocusNearLimit).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::FocusNearLimit { position }) => {
                assert_eq!(position, 0x5555);
            }
            _ => panic!("Expected FocusNearLimit inquiry response"),
        }
    }

    #[test]
    fn test_parse_exposure_mode() {
        // Test all valid exposure modes
        let modes = vec![
            (0x00, ExposureMode::Auto),
            (0x03, ExposureMode::Manual),
            (0x0A, ExposureMode::Shutter),
            (0x0B, ExposureMode::Iris),
            (0x0D, ExposureMode::Bright),
        ];

        for (byte, expected_mode) in modes {
            let response = vec![0x90, 0x50, byte, 0xFF];
            let result = parse_response(&response, &ResponseType::ExposureMode).unwrap();
            match result {
                Response::InquiryResponse(InquiryResponse::ExposureMode { mode }) => {
                    assert_eq!(mode, expected_mode);
                }
                _ => panic!("Expected ExposureMode inquiry response"),
            }
        }

        // Invalid mode
        let response = vec![0x90, 0x50, 0xFF, 0xFF];
        let result = parse_response(&response, &ResponseType::ExposureMode);
        assert!(matches!(result, Err(Error::UnexpectedResponseType)));
    }

    #[test]
    fn test_parse_white_balance_mode() {
        // Test all valid white balance modes
        let modes = vec![
            (0x00, WhiteBalanceMode::Auto),
            (0x01, WhiteBalanceMode::Indoor),
            (0x02, WhiteBalanceMode::Outdoor),
            (0x03, WhiteBalanceMode::OnePush),
            (0x05, WhiteBalanceMode::Manual),
            (0x20, WhiteBalanceMode::ColorTemperature),
        ];

        for (byte, expected_mode) in modes {
            let response = vec![0x90, 0x50, byte, 0xFF];
            let result = parse_response(&response, &ResponseType::WhiteBalanceMode).unwrap();
            match result {
                Response::InquiryResponse(InquiryResponse::WhiteBalance { mode }) => {
                    assert_eq!(mode, expected_mode);
                }
                _ => panic!("Expected WhiteBalance inquiry response"),
            }
        }
    }

    #[test]
    fn test_parse_exposure_compensation_mode() {
        // On
        let response = vec![0x90, 0x50, 0x02, 0xFF];
        let result = parse_response(&response, &ResponseType::ExposureCompensationMode).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::ExposureCompensationMode { on }) => {
                assert!(on);
            }
            _ => panic!("Expected ExposureCompensationMode inquiry response"),
        }

        // Off
        let response = vec![0x90, 0x50, 0x03, 0xFF];
        let result = parse_response(&response, &ResponseType::ExposureCompensationMode).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::ExposureCompensationMode { on }) => {
                assert!(!on);
            }
            _ => panic!("Expected ExposureCompensationMode inquiry response"),
        }
    }

    #[test]
    fn test_parse_sharpness_mode() {
        // Auto
        let response = vec![0x90, 0x50, 0x02, 0xFF];
        let result = parse_response(&response, &ResponseType::SharpnessMode).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::SharpnessMode { mode }) => {
                assert!(matches!(mode, SharpnessMode::Auto));
            }
            _ => panic!("Expected SharpnessMode inquiry response"),
        }

        // Manual
        let response = vec![0x90, 0x50, 0x03, 0xFF];
        let result = parse_response(&response, &ResponseType::SharpnessMode).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::SharpnessMode { mode }) => {
                assert!(matches!(mode, SharpnessMode::Manual));
            }
            _ => panic!("Expected SharpnessMode inquiry response"),
        }

        // Invalid mode
        let response = vec![0x90, 0x50, 0x01, 0xFF];
        let result = parse_response(&response, &ResponseType::SharpnessMode);
        assert!(matches!(result, Err(Error::UnexpectedResponseType)));
    }

    #[test]
    fn test_parse_black_white_mode() {
        // On
        let response = vec![0x90, 0x50, 0x04, 0xFF];
        let result = parse_response(&response, &ResponseType::BlackWhite).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::BlackWhite { on }) => assert!(on),
            _ => panic!("Expected BlackWhite inquiry response"),
        }

        // Off
        let response = vec![0x90, 0x50, 0x00, 0xFF];
        let result = parse_response(&response, &ResponseType::BlackWhite).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::BlackWhite { on }) => assert!(!on),
            _ => panic!("Expected BlackWhite inquiry response"),
        }
    }

    #[test]
    fn test_parse_simple_values() {
        // GainLimit
        let response = vec![0x90, 0x50, 0x07, 0xFF];
        let result = parse_response(&response, &ResponseType::GainLimit).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::GainLimit { limit }) => {
                assert_eq!(limit, 7);
            }
            _ => panic!("Expected GainLimit inquiry response"),
        }

        // AntiFlicker
        let response = vec![0x90, 0x50, 0x00, 0xFF];
        let result = parse_response(&response, &ResponseType::AntiFlicker).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::AntiFlicker { mode }) => {
                assert!(matches!(mode, AntiFlickerMode::Off));
            }
            _ => panic!("Expected AntiFlicker inquiry response"),
        }

        // RedGain
        let response = vec![0x90, 0x50, 0x0A, 0xFF];
        let result = parse_response(&response, &ResponseType::RedGain).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::RedGain { gain }) => {
                assert_eq!(gain, 0);
            }
            _ => panic!("Expected RedGain inquiry response"),
        }

        // BlueGain
        let response = vec![0x90, 0x50, 0x14, 0xFF];
        let result = parse_response(&response, &ResponseType::BlueGain).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::BlueGain { gain }) => {
                assert_eq!(gain, 10);
            }
            _ => panic!("Expected BlueGain inquiry response"),
        }

        // ImageFlip
        let response = vec![0x90, 0x50, 0x03, 0xFF];
        let result = parse_response(&response, &ResponseType::ImageFlip).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            }) => {
                assert!(vertical);
                assert!(horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }

        // NoiseReduction2D
        let response = vec![0x90, 0x50, 0x05, 0xFF];
        let result = parse_response(&response, &ResponseType::NoiseReduction2D).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::NoiseReduction2D { level }) => {
                assert_eq!(level, 5);
            }
            _ => panic!("Expected NoiseReduction2D inquiry response"),
        }

        // FocusZone
        let response = vec![0x90, 0x50, 0x01, 0xFF];
        let result = parse_response(&response, &ResponseType::FocusZone).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::FocusZone { zone }) => {
                assert!(matches!(zone, FocusZone::Center));
            }
            _ => panic!("Expected FocusZone inquiry response"),
        }

        // AutoFocusSensitivity
        let response = vec![0x90, 0x50, 0x02, 0xFF];
        let result = parse_response(&response, &ResponseType::AutoFocusSensitivity).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::AutoFocusSensitivity { sensitivity }) => {
                assert!(matches!(sensitivity, AutoFocusSensitivity::High));
            }
            _ => panic!("Expected AutoFocusSensitivity inquiry response"),
        }
    }

    #[test]
    fn test_parse_extended_values() {
        // Sharpness
        let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
        let result = parse_response(&response, &ResponseType::Sharpness).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::Sharpness { value }) => {
                assert_eq!(value, 7);
            }
            _ => panic!("Expected Sharpness inquiry response"),
        }

        // ExposureCompensation
        let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
        let result = parse_response(&response, &ResponseType::ExposureCompensation).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::ExposureCompensation { value }) => {
                assert_eq!(value, 0);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Iris
        let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0C, 0xFF];
        let result = parse_response(&response, &ResponseType::Iris).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::Iris { position }) => {
                assert_eq!(position, 0x0C);
            }
            _ => panic!("Expected Iris inquiry response"),
        }

        // Shutter
        let response = vec![0x90, 0x50, 0x00, 0x00, 0x01, 0x01, 0xFF];
        let result = parse_response(&response, &ResponseType::Shutter).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::Shutter { position }) => {
                assert_eq!(position, 0x11);
            }
            _ => panic!("Expected Shutter inquiry response"),
        }

        // Saturation
        let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x08, 0xFF];
        let result = parse_response(&response, &ResponseType::Saturation).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::Saturation { level }) => {
                assert_eq!(level, 8);
            }
            _ => panic!("Expected Saturation inquiry response"),
        }

        // ColorTemperature
        let response = vec![0x90, 0x50, 0x00, 0x00, 0x03, 0x02, 0xFF];
        let result = parse_response(&response, &ResponseType::ColorTemperature).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::ColorTemperature { temperature }) => {
                assert_eq!(temperature, 0x32);
            }
            _ => panic!("Expected ColorTemperature inquiry response"),
        }

        // DynamicRange
        let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x08, 0xFF];
        let result = parse_response(&response, &ResponseType::DynamicRange).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::DynamicRange { level }) => {
                assert_eq!(level, 8);
            }
            _ => panic!("Expected DynamicRange inquiry response"),
        }
    }

    #[test]
    fn test_anti_flicker_modes() {
        let modes = vec![
            (0x00, AntiFlickerMode::Off),
            (0x01, AntiFlickerMode::Hz50),
            (0x02, AntiFlickerMode::Hz60),
        ];

        for (byte, expected_mode) in modes {
            let response = vec![0x90, 0x50, byte, 0xFF];
            let result = parse_response(&response, &ResponseType::AntiFlicker).unwrap();
            match result {
                Response::InquiryResponse(InquiryResponse::AntiFlicker { mode }) => {
                    assert_eq!(mode, expected_mode);
                }
                _ => panic!("Expected AntiFlicker inquiry response"),
            }
        }

        // Invalid mode
        let response = vec![0x90, 0x50, 0x03, 0xFF];
        let result = parse_response(&response, &ResponseType::AntiFlicker);
        assert!(matches!(result, Err(Error::UnexpectedResponseType)));
    }

    #[test]
    fn test_focus_zone_values() {
        let zones = vec![
            (0x00, FocusZone::Top),
            (0x01, FocusZone::Center),
            (0x02, FocusZone::Bottom),
        ];

        for (byte, expected_zone) in zones {
            let response = vec![0x90, 0x50, byte, 0xFF];
            let result = parse_response(&response, &ResponseType::FocusZone).unwrap();
            match result {
                Response::InquiryResponse(InquiryResponse::FocusZone { zone }) => {
                    assert_eq!(zone, expected_zone);
                }
                _ => panic!("Expected FocusZone inquiry response"),
            }
        }

        // Invalid zone
        let response = vec![0x90, 0x50, 0x03, 0xFF];
        let result = parse_response(&response, &ResponseType::FocusZone);
        assert!(matches!(result, Err(Error::UnexpectedResponseType)));
    }

    #[test]
    fn test_auto_focus_sensitivity_values() {
        let sensitivities = vec![
            (0x00, AutoFocusSensitivity::Low),
            (0x01, AutoFocusSensitivity::Normal),
            (0x02, AutoFocusSensitivity::High),
        ];

        for (byte, expected_sensitivity) in sensitivities {
            let response = vec![0x90, 0x50, byte, 0xFF];
            let result = parse_response(&response, &ResponseType::AutoFocusSensitivity).unwrap();
            match result {
                Response::InquiryResponse(InquiryResponse::AutoFocusSensitivity {
                    sensitivity,
                }) => {
                    assert_eq!(sensitivity, expected_sensitivity);
                }
                _ => panic!("Expected AutoFocusSensitivity inquiry response"),
            }
        }

        // Invalid sensitivity
        let response = vec![0x90, 0x50, 0x03, 0xFF];
        let result = parse_response(&response, &ResponseType::AutoFocusSensitivity);
        assert!(matches!(result, Err(Error::UnexpectedResponseType)));
    }

    #[test]
    fn test_response_length_validation() {
        // Test simple values with wrong length
        let response = vec![0x90, 0x50, 0x00, 0x00, 0xFF];
        let result = parse_response(&response, &ResponseType::GainLimit);
        assert!(matches!(result, Err(Error::InvalidResponseLength)));

        // Test extended values with wrong length
        let response = vec![0x90, 0x50, 0x00, 0x00, 0xFF];
        let result = parse_response(&response, &ResponseType::Sharpness);
        assert!(matches!(result, Err(Error::InvalidResponseLength)));

        // Test mode values with wrong length
        let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0xFF];
        let result = parse_response(&response, &ResponseType::ExposureMode);
        assert!(matches!(result, Err(Error::InvalidResponseLength)));
    }

    #[test]
    fn test_image_flip_combinations() {
        // Neither flipped
        let response = vec![0x90, 0x50, 0x00, 0xFF];
        let result = parse_response(&response, &ResponseType::ImageFlip).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            }) => {
                assert!(!vertical);
                assert!(!horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }

        // Horizontal only
        let response = vec![0x90, 0x50, 0x01, 0xFF];
        let result = parse_response(&response, &ResponseType::ImageFlip).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            }) => {
                assert!(!vertical);
                assert!(horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }

        // Vertical only
        let response = vec![0x90, 0x50, 0x02, 0xFF];
        let result = parse_response(&response, &ResponseType::ImageFlip).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            }) => {
                assert!(vertical);
                assert!(!horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }

        // Both flipped
        let response = vec![0x90, 0x50, 0x03, 0xFF];
        let result = parse_response(&response, &ResponseType::ImageFlip).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            }) => {
                assert!(vertical);
                assert!(horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }
    }

    #[test]
    fn test_gain_value_conversions() {
        // Test RedGain conversion
        for raw_value in 0..=20 {
            let response = vec![0x90, 0x50, raw_value, 0xFF];
            let result = parse_response(&response, &ResponseType::RedGain).unwrap();
            match result {
                Response::InquiryResponse(InquiryResponse::RedGain { gain }) => {
                    assert_eq!(gain, i16::from(raw_value) as i8 - 10);
                }
                _ => panic!("Expected RedGain inquiry response"),
            }
        }

        // Test BlueGain conversion
        for raw_value in 0..=20 {
            let response = vec![0x90, 0x50, raw_value, 0xFF];
            let result = parse_response(&response, &ResponseType::BlueGain).unwrap();
            match result {
                Response::InquiryResponse(InquiryResponse::BlueGain { gain }) => {
                    assert_eq!(gain, i16::from(raw_value) as i8 - 10);
                }
                _ => panic!("Expected BlueGain inquiry response"),
            }
        }
    }

    #[test]
    fn test_exposure_compensation_conversion() {
        // Test all valid exposure compensation values
        for raw_value in 0..=14 {
            let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, raw_value, 0xFF];
            let result = parse_response(&response, &ResponseType::ExposureCompensation).unwrap();
            match result {
                Response::InquiryResponse(InquiryResponse::ExposureCompensation { value }) => {
                    assert_eq!(value, i16::from(raw_value) as i8 - 7);
                }
                _ => panic!("Expected ExposureCompensation inquiry response"),
            }
        }
    }

    #[test]
    fn test_extended_value_nibble_combination() {
        // Test Sharpness nibble combination
        let response = vec![0x90, 0x50, 0x00, 0x00, 0x01, 0x05, 0xFF];
        let result = parse_response(&response, &ResponseType::Sharpness).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::Sharpness { value }) => {
                assert_eq!(value, 0x15);
            }
            _ => panic!("Expected Sharpness inquiry response"),
        }

        // Test Shutter nibble combination
        let response = vec![0x90, 0x50, 0x00, 0x00, 0x0A, 0x0B, 0xFF];
        let result = parse_response(&response, &ResponseType::Shutter).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::Shutter { position }) => {
                assert_eq!(position, 0xAB);
            }
            _ => panic!("Expected Shutter inquiry response"),
        }

        // Test ColorTemperature nibble combination
        let response = vec![0x90, 0x50, 0x00, 0x00, 0x01, 0x02, 0xFF];
        let result = parse_response(&response, &ResponseType::ColorTemperature).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::ColorTemperature { temperature }) => {
                assert_eq!(temperature, 0x12);
            }
            _ => panic!("Expected ColorTemperature inquiry response"),
        }
    }

    #[test]
    fn test_edge_case_values() {
        // Test maximum sharpness value
        let response = vec![0x90, 0x50, 0x00, 0x00, 0x0F, 0x0F, 0xFF];
        let result = parse_response(&response, &ResponseType::Sharpness).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::Sharpness { value }) => {
                assert_eq!(value, 0xFF);
            }
            _ => panic!("Expected Sharpness inquiry response"),
        }

        // Test maximum gain limit
        let response = vec![0x90, 0x50, 0x0F, 0xFF];
        let result = parse_response(&response, &ResponseType::GainLimit).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::GainLimit { limit }) => {
                assert_eq!(limit, 15);
            }
            _ => panic!("Expected GainLimit inquiry response"),
        }

        // Test boundary exposure compensation values
        let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
        let result = parse_response(&response, &ResponseType::ExposureCompensation).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::ExposureCompensation { value }) => {
                assert_eq!(value, -7);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0E, 0xFF];
        let result = parse_response(&response, &ResponseType::ExposureCompensation).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::ExposureCompensation { value }) => {
                assert_eq!(value, 7);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }
    }

    #[test]
    fn test_parse_luminance_response() {
        // Test minimum value
        let response = vec![0x90, 0x50, 0x00, 0xFF];
        let result = parse_response(&response, &ResponseType::Luminance).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::Luminance(value)) => {
                assert_eq!(value, 0);
            }
            _ => panic!("Expected Luminance inquiry response"),
        }

        // Test maximum value
        let response = vec![0x90, 0x50, 0x0E, 0xFF];
        let result = parse_response(&response, &ResponseType::Luminance).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::Luminance(value)) => {
                assert_eq!(value, 14);
            }
            _ => panic!("Expected Luminance inquiry response"),
        }

        // Test invalid length
        let response = vec![0x90, 0x50, 0x00, 0x00, 0xFF];
        let result = parse_response(&response, &ResponseType::Luminance);
        assert!(matches!(result, Err(Error::InvalidResponseLength)));
    }

    #[test]
    fn test_parse_contrast_response() {
        // Test minimum value
        let response = vec![0x90, 0x50, 0x00, 0xFF];
        let result = parse_response(&response, &ResponseType::Contrast).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::Contrast(value)) => {
                assert_eq!(value, 0);
            }
            _ => panic!("Expected Contrast inquiry response"),
        }

        // Test maximum value
        let response = vec![0x90, 0x50, 0x0E, 0xFF];
        let result = parse_response(&response, &ResponseType::Contrast).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::Contrast(value)) => {
                assert_eq!(value, 14);
            }
            _ => panic!("Expected Contrast inquiry response"),
        }

        // Test invalid length
        let response = vec![0x90, 0x50, 0x00, 0x00, 0xFF];
        let result = parse_response(&response, &ResponseType::Contrast);
        assert!(matches!(result, Err(Error::InvalidResponseLength)));
    }

    #[test]
    fn test_parse_backlight_response() {
        // Test Backlight On
        let response = vec![0x90, 0x50, 0x02, 0xFF];
        let result = parse_response(&response, &ResponseType::Backlight).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::Backlight { status }) => {
                assert!(status);
            }
            _ => panic!("Expected Backlight inquiry response"),
        }

        // Test Backlight Off
        let response = vec![0x90, 0x50, 0x03, 0xFF];
        let result = parse_response(&response, &ResponseType::Backlight).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::Backlight { status }) => {
                assert!(!status);
            }
            _ => panic!("Expected Backlight inquiry response"),
        }

        // Test invalid length
        let response = vec![0x90, 0x50, 0x02, 0x00, 0xFF];
        let result = parse_response(&response, &ResponseType::Backlight);
        assert!(matches!(result, Err(Error::InvalidResponseLength)));
    }

    #[test]
    fn test_unhandled_response_types() {
        // Test response types that return Completion
        let unhandled_types = vec![
            ResponseType::SharpnessPosition,
            ResponseType::HorizontalFlip,
            ResponseType::VerticalFlip,
            ResponseType::BlackWhiteMode,
            ResponseType::ExposureCompensationPosition,
            ResponseType::RedTuning,
            ResponseType::BlueTuning,
            ResponseType::AutoWhiteBalanceSensitivity,
            ResponseType::ThreeDNoiseReduction,
            ResponseType::TwoDNoiseReduction,
            ResponseType::MotionSyncMode,
            ResponseType::MotionSyncSpeed,
            ResponseType::FocusMode,
            ResponseType::FocusRange,
            ResponseType::MenuOpenClose,
            ResponseType::UsbAudio,
            ResponseType::Rtmp,
            ResponseType::BlockLens,
            ResponseType::BlockColorExposure,
            ResponseType::BlockPowerImageEffect,
            ResponseType::BlockImage,
            ResponseType::ZoomOutStandard,
            ResponseType::ZoomInStandard,
        ];

        for response_type in unhandled_types {
            let response = vec![0x90, 0x50, 0x00, 0x00, 0xFF];
            let result = parse_response(&response, &response_type).unwrap();
            assert!(matches!(result, Response::Completion));
        }
    }
}
