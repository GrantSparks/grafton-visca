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
        gain::AntiFlickerMode, image_adjustment::SharpnessMode, AutoFocusSensitivity, ExposureMode,
        FocusZone, InquiryResponse, WhiteBalanceMode,
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

impl Response {
    /// Convert response to a Result, treating Completion as Ok and Error as Err.
    pub fn into_result(self) -> Result<(), Error> {
        match self {
            Response::Completion => Ok(()),
            Response::Ack => Ok(()), // ACK is also considered success
            Response::Error(e) => Err(e),
            Response::InquiryResponse(_) => Ok(()), // Inquiry responses are success
            Response::Unknown(bytes) => Err(Error::InvalidResponse {
                expected: "Known response type".to_string(),
                actual: bytes,
            }),
        }
    }

    /// Parse a response from raw bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        parse_response(bytes, &ResponseType::Power) // Default type, will be replaced
    }
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
    /// Focus near limit inquiry response.
    FocusNearLimit,
    /// Focus zone inquiry response.
    FocusZone,
    /// Auto focus sensitivity inquiry response.
    AutoFocusSensitivity,
    /// Exposure mode inquiry response.
    ExposureMode,
    /// Exposure compensation mode inquiry response.
    ExposureCompensationMode,
    /// Exposure compensation value inquiry response.
    ExposureCompensation,
    /// Iris position inquiry response.
    Iris,
    /// Shutter speed inquiry response.
    Shutter,
    /// Brightness inquiry response.
    Bright,
    /// Gain inquiry response.
    Gain,
    /// Gain limit inquiry response.
    GainLimit,
    /// Anti-flicker mode inquiry response.
    AntiFlicker,
    /// Backlight compensation inquiry response.
    Backlight,
    /// Dynamic range inquiry response.
    DynamicRange,
    /// White balance mode inquiry response.
    WhiteBalanceMode,
    /// Color temperature inquiry response.
    ColorTemperature,
    /// Red gain inquiry response.
    RedGain,
    /// Blue gain inquiry response.
    BlueGain,
    /// Luminance inquiry response.
    Luminance,
    /// Contrast inquiry response.
    Contrast,
    /// Sharpness value inquiry response.
    Sharpness,
    /// Sharpness mode inquiry response.
    SharpnessMode,
    /// Saturation inquiry response.
    Saturation,
    /// Hue inquiry response.
    Hue,
    /// 2D noise reduction inquiry response.
    NoiseReduction2D,
    /// 3D noise reduction inquiry response.
    NoiseReduction3D,
    /// Image flip inquiry response.
    ImageFlip,
    /// Black and white mode inquiry response.
    BlackWhite,
    /// Picture effect inquiry response.
    PictureEffect,
    /// System version inquiry response.
    Version,
    /// Red tally light state inquiry response.
    TallyRed,
    /// Green tally light state inquiry response.
    TallyGreen,
    
    // Additional response types for completeness
    SharpnessPosition,
    HorizontalFlip,
    VerticalFlip,
    BlackWhiteMode,
    ExposureCompensationPosition,
    RedTuning,
    BlueTuning,
    AutoWhiteBalanceSensitivity,
    ThreeDNoiseReduction,
    TwoDNoiseReduction,
    MotionSyncMode,
    MotionSyncSpeed,
    FocusMode,
    FocusRange,
    MenuOpenClose,
    UsbAudio,
    Rtmp,
    AutoFocus,
    FocusUnlock,
    ZoomOut,
    ZoomIn,
    IrisUp,
    IrisDown,
    NightDayMode,
    NightDayPosition,
    AutoTrace,
    TwoToneMode,
    DefogMode,
    NrLevel,
    NrMode,
    NrSpeed,
    BroadcastDomain,
    Resolution,
    NdFilter,
    NdFilterPreset,
    FocusNearFar,
    ZoomTeleWide,
    Standby,
    Tally,
    DigitalPtz,
    Digital,
    IrisControl,
    DefogLevel,
    NightDay,
    NightDaySwitch,
    FlipMode,
    TallyStatus,
    TallyAutoAdjust,
}

/// Parse a raw VISCA response into a structured Response.
pub fn parse_response(data: &[u8], expected_type: &ResponseType) -> Result<Response, Error> {
    // Basic format validation
    if data.is_empty() || data.len() < 3 {
        return Err(Error::InvalidResponseFormat);
    }

    if data[0] != 0x90 {
        return Err(Error::InvalidResponseFormat);
    }

    if data[data.len() - 1] != 0xFF {
        return Err(Error::InvalidResponseFormat);
    }

    let second_byte = data[1];

    // Parse based on second byte
    match second_byte & 0xF0 {
        0x40 => Ok(Response::Ack), // ACK responses
        0x50 => {
            // Completion or inquiry data response
            if data.len() == 3 {
                Ok(Response::Completion)
            } else {
                parse_inquiry_response(&data[2..data.len() - 1], expected_type)
            }
        }
        0x60 => {
            // Error response
            if data.len() != 4 {
                return Err(Error::InvalidResponseFormat);
            }
            Err(Error::from_code(data[2]))
        }
        _ => Ok(Response::Unknown(data.to_vec())),
    }
}

fn parse_inquiry_response(payload: &[u8], expected_type: &ResponseType) -> Result<Response, Error> {
    match expected_type {
        ResponseType::Power => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::InquiryResponse(InquiryResponse::Power {
                on: payload[0] == 0x02,
            }))
        }
        ResponseType::ZoomPosition => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            let position = combine_nibbles_u16(&payload[0..4]);
            Ok(Response::InquiryResponse(InquiryResponse::ZoomPosition {
                position,
            }))
        }
        ResponseType::PanTiltPosition => {
            if payload.len() != 8 {
                return Err(Error::InvalidResponseLength);
            }
            let pan = combine_nibbles_i16(&payload[0..4]);
            let tilt = combine_nibbles_i16(&payload[4..8]);
            Ok(Response::InquiryResponse(InquiryResponse::PanTiltPosition {
                pan,
                tilt,
            }))
        }
        ResponseType::FocusPosition => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            let position = combine_nibbles_u16(&payload[0..4]);
            Ok(Response::InquiryResponse(InquiryResponse::FocusPosition {
                position,
            }))
        }
        ResponseType::FocusNearLimit => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            let position = combine_nibbles_u16(&payload[0..4]);
            Ok(Response::InquiryResponse(InquiryResponse::FocusNearLimit {
                position,
            }))
        }
        ResponseType::ExposureMode => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let mode = match payload[0] {
                0x00 => ExposureMode::Auto,
                0x03 => ExposureMode::Manual,
                0x0A => ExposureMode::Shutter,
                0x0B => ExposureMode::Iris,
                0x0D => ExposureMode::Bright,
                _ => return Err(Error::InvalidParameter(format!("Unknown exposure mode: {:02X}", payload[0]))),
            };
            Ok(Response::InquiryResponse(InquiryResponse::ExposureMode { mode }))
        }
        ResponseType::WhiteBalanceMode => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let mode = match payload[0] {
                0x00 => WhiteBalanceMode::Auto,
                0x01 => WhiteBalanceMode::Indoor,
                0x02 => WhiteBalanceMode::Outdoor,
                0x03 => WhiteBalanceMode::OnePush,
                0x05 => WhiteBalanceMode::Manual,
                0x20 => WhiteBalanceMode::ColorTemperature,
                _ => return Err(Error::InvalidParameter(format!("Unknown white balance mode: {:02X}", payload[0]))),
            };
            Ok(Response::InquiryResponse(InquiryResponse::WhiteBalance { mode }))
        }
        ResponseType::AntiFlicker => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let mode = match payload[0] {
                0x00 => AntiFlickerMode::Off,
                0x01 => AntiFlickerMode::Hz50,
                0x02 => AntiFlickerMode::Hz60,
                _ => return Err(Error::InvalidParameter(format!("Unknown anti-flicker mode: {:02X}", payload[0]))),
            };
            Ok(Response::InquiryResponse(InquiryResponse::AntiFlicker { mode }))
        }
        ResponseType::FocusZone => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let zone = match payload[0] {
                0x00 => FocusZone::Top,
                0x01 => FocusZone::Center,
                0x02 => FocusZone::Bottom,
                _ => return Err(Error::InvalidParameter(format!("Unknown focus zone: {:02X}", payload[0]))),
            };
            Ok(Response::InquiryResponse(InquiryResponse::FocusZone { zone }))
        }
        ResponseType::AutoFocusSensitivity => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let sensitivity = match payload[0] {
                0x00 => AutoFocusSensitivity::Low,
                0x01 => AutoFocusSensitivity::Normal,
                0x02 => AutoFocusSensitivity::High,
                _ => return Err(Error::InvalidParameter(format!("Unknown auto focus sensitivity: {:02X}", payload[0]))),
            };
            Ok(Response::InquiryResponse(InquiryResponse::AutoFocusSensitivity { sensitivity }))
        }
        ResponseType::ExposureCompensationMode => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::InquiryResponse(InquiryResponse::ExposureCompensationMode {
                on: payload[0] == 0x02,
            }))
        }
        ResponseType::SharpnessMode => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let mode = match payload[0] {
                0x02 => SharpnessMode::Auto,
                0x03 => SharpnessMode::Manual,
                _ => return Err(Error::InvalidParameter(format!("Unknown sharpness mode: {:02X}", payload[0]))),
            };
            Ok(Response::InquiryResponse(InquiryResponse::SharpnessMode { mode }))
        }
        ResponseType::BlackWhite => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::InquiryResponse(InquiryResponse::BlackWhite {
                on: payload[0] == 0x04,
            }))
        }
        ResponseType::GainLimit => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::InquiryResponse(InquiryResponse::GainLimit {
                limit: payload[0],
            }))
        }
        ResponseType::RedGain => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::InquiryResponse(InquiryResponse::RedGain {
                gain: payload[0] as i8 - 10,
            }))
        }
        ResponseType::BlueGain => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::InquiryResponse(InquiryResponse::BlueGain {
                gain: payload[0] as i8 - 10,
            }))
        }
        ResponseType::Sharpness => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            let value = combine_nibbles_u8(&payload[2..4]);
            Ok(Response::InquiryResponse(InquiryResponse::Sharpness { value }))
        }
        ResponseType::ExposureCompensation => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            let raw_value = combine_nibbles_u8(&payload[2..4]);
            Ok(Response::InquiryResponse(InquiryResponse::ExposureCompensation {
                value: raw_value as i8 - 7,
            }))
        }
        ResponseType::Shutter => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            let position = combine_nibbles_u16(&payload[2..4]);
            Ok(Response::InquiryResponse(InquiryResponse::Shutter { position }))
        }
        ResponseType::ImageFlip => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let value = payload[0];
            Ok(Response::InquiryResponse(InquiryResponse::ImageFlip {
                horizontal: (value & 0x01) != 0,
                vertical: (value & 0x02) != 0,
            }))
        }
        ResponseType::Backlight => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::InquiryResponse(InquiryResponse::Backlight {
                status: payload[0] == 0x02,
            }))
        }
        ResponseType::Luminance => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::InquiryResponse(InquiryResponse::Luminance(payload[0])))
        }
        ResponseType::Contrast => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::InquiryResponse(InquiryResponse::Contrast(payload[0])))
        }
        ResponseType::TallyRed => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::InquiryResponse(InquiryResponse::TallyRed {
                on: payload[0] == 0x02,
            }))
        }
        ResponseType::TallyGreen => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::InquiryResponse(InquiryResponse::TallyGreen {
                on: payload[0] == 0x02,
            }))
        }
        // For unimplemented response types, return completion
        _ => Ok(Response::Completion),
    }
}

fn combine_nibbles_u16(nibbles: &[u8]) -> u16 {
    if nibbles.len() < 4 {
        return 0;
    }
    ((nibbles[0] as u16) << 12) | ((nibbles[1] as u16) << 8) | ((nibbles[2] as u16) << 4) | (nibbles[3] as u16)
}

fn combine_nibbles_i16(nibbles: &[u8]) -> i16 {
    let unsigned = combine_nibbles_u16(nibbles);
    unsigned as i16
}

fn combine_nibbles_u8(nibbles: &[u8]) -> u8 {
    if nibbles.len() < 2 {
        return 0;
    }
    ((nibbles[0] & 0x0F) << 4) | (nibbles[1] & 0x0F)
}

// Note: Complex response parsing tests moved to tests/response_parsing_comprehensive.rs
// Only basic unit tests remain here for fast feedback during development

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_response_debug() {
        let ack = Response::Ack;
        assert_eq!(format!("{:?}", ack), "Ack");

        let completion = Response::Completion;
        assert_eq!(format!("{:?}", completion), "Completion");
    }

    #[test]
    fn test_response_type_equality() {
        assert_eq!(ResponseType::Power, ResponseType::Power);
        assert_ne!(ResponseType::Power, ResponseType::ZoomPosition);
    }

    #[test]
    fn test_basic_ack_parsing() {
        let response = vec![0x90, 0x41, 0xFF];
        let result = parse_response(&response, &ResponseType::Power).unwrap();
        assert!(matches!(result, Response::Ack));
    }

    #[test]
    fn test_basic_completion_parsing() {
        let response = vec![0x90, 0x51, 0xFF];
        let result = parse_response(&response, &ResponseType::Power).unwrap();
        assert!(matches!(result, Response::Completion));
    }

    #[test]
    fn test_basic_error_parsing() {
        let response = vec![0x90, 0x60, 0x02, 0xFF];
        let result = parse_response(&response, &ResponseType::Power);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_format() {
        // Empty response
        let response = vec![];
        let result = parse_response(&response, &ResponseType::Power);
        assert!(matches!(result, Err(Error::InvalidResponseFormat)));

        // Too short
        let response = vec![0x90, 0xFF];
        let result = parse_response(&response, &ResponseType::Power);
        assert!(matches!(result, Err(Error::InvalidResponseFormat)));
    }

    #[test]
    fn test_simple_power_response() {
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
    }

    #[test]
    fn test_combine_nibbles() {
        // Test u16 combination
        let nibbles = [0x01, 0x02, 0x03, 0x04];
        assert_eq!(combine_nibbles_u16(&nibbles), 0x1234);

        // Test u8 combination
        let nibbles = [0x0A, 0x0B];
        assert_eq!(combine_nibbles_u8(&nibbles), 0xAB);
    }
}