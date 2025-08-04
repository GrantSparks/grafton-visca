//! VISCA response parsing and handling.
//!
//! This module provides response parsing functionality for VISCA protocol responses,
//! including ACK/completion messages, error responses, and inquiry data parsing.

#![allow(dead_code)]

// Standard library imports
use std::borrow::Cow;

// Third-party crate imports

// Workspace / local-crate imports
use crate::{
    command::{
        image_adjustment::{BlackWhiteMode, NrMode, NrSpeed, SharpnessMode},
        system::{MotionSyncMode, MotionSyncSpeed},
        AutoFocusSensitivity, AutoWhiteBalanceSensitivity, ExposureMode, FocusMode, FocusZone,
        InquiryResponse, WhiteBalanceMode,
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
    CmdAck,
    /// Command completed successfully (no data returned)
    Completion,
    /// Command failed with an error
    Error(Error),
    /// Inquiry command response containing requested data
    Inquiry(InquiryResponse),
    /// Unknown response format with type information and raw data
    Unknown {
        /// The response type that could not be parsed
        response_type: Option<ResponseType>,
        /// Raw response data for debugging
        data: Vec<u8>,
    },
}

impl Response {
    /// Convert response to a Result, treating Completion as Ok and Error as Err.
    ///
    /// Note: ACK responses are treated as an error because they only indicate
    /// the command was queued, not completed. Callers should wait for the
    /// subsequent Completion response.
    pub fn into_result(self) -> Result<(), Error> {
        match self {
            Response::Completion => Ok(()),
            Response::CmdAck => Err(Error::CommandPending), // ACK means command is queued, not completed
            Response::Error(e) => Err(e),
            Response::Inquiry(_) => Ok(()), // Inquiry responses are success
            Response::Unknown { data, .. } => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("Known response type"),
                actual: data,
            }),
        }
    }

    /// Parse a response from raw bytes.
    ///
    /// This method is for parsing basic responses (ACK, Completion, Error).
    /// For inquiry responses, use `parse_with_type` instead.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        // Check for common non-inquiry responses first
        if bytes.is_empty() {
            return Err(Error::InvalidResponse {
                expected: Cow::Borrowed("Non-empty response"),
                actual: bytes.to_vec(),
            });
        }

        // ACK: 9x 4y FF (where x = socket, y = ack type)
        if bytes.len() == 3
            && (bytes[0] & 0xF0) == 0x90
            && (bytes[1] & 0xF0) == 0x40
            && bytes[2] == 0xFF
        {
            return Ok(Response::CmdAck);
        }

        // Completion: 9x 5y FF (where x = socket, y = completion type)
        if bytes.len() == 3
            && (bytes[0] & 0xF0) == 0x90
            && (bytes[1] & 0xF0) == 0x50
            && bytes[2] == 0xFF
        {
            return Ok(Response::Completion);
        }

        // Error: 9x 6y zz FF (where x = socket, y = error type, zz = error code)
        if bytes.len() >= 4 && (bytes[0] & 0xF0) == 0x90 && (bytes[1] & 0xF0) == 0x60 {
            return Ok(Response::Error(Error::from_code(bytes[2])));
        }

        // If it's an inquiry response (9x 50 ...), it needs a specific type
        if bytes.len() > 3 && (bytes[0] & 0xF0) == 0x90 && bytes[1] == 0x50 {
            return Err(Error::InvalidResponse {
                expected: Cow::Borrowed("Use parse_with_type for inquiry responses"),
                actual: bytes.to_vec(),
            });
        }

        // Unknown response format
        Ok(Response::Unknown {
            response_type: None,
            data: bytes.to_vec(),
        })
    }

    /// Parse an inquiry response with a specific expected type.
    pub fn parse_with_type(bytes: &[u8], response_type: &ResponseType) -> Result<Self, Error> {
        parse_response(bytes, response_type)
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
    /// Backlight compensation inquiry response.
    Backlight,
    /// Dynamic range inquiry response.
    DynamicRange,
    /// White balance mode inquiry response.
    WhiteBalanceMode,
    /// Color temperature inquiry response.
    ColorTemperature,
    /// Red gain inquiry response.
    RedChannel,
    /// Blue gain inquiry response.
    BlueChannel,
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
    /// Sharpness position inquiry response.
    SharpnessPosition,
    /// Black and white mode state inquiry response.
    BlackWhiteMode,
    /// Exposure compensation position inquiry response.
    ExposureCompensationPosition,
    /// Red tuning level inquiry response.
    RedTuning,
    /// Blue tuning level inquiry response.
    BlueTuning,
    /// Gamma curve setting inquiry response.
    Gamma,
    /// Auto white balance sensitivity inquiry response.
    AutoWhiteBalanceSensitivity,
    /// Motion sync mode inquiry response.
    MotionSyncMode,
    /// Motion sync speed inquiry response.
    MotionSyncSpeed,
    /// Focus mode inquiry response.
    FocusMode,
    /// Focus range inquiry response.
    FocusRange,
    /// Menu open/close state inquiry response.
    MenuOpenClose,
    /// USB audio state inquiry response.
    UsbAudio,
    /// RTMP state inquiry response.
    Rtmp,
    /// Auto focus state inquiry response.
    AutoFocus,
    /// Focus unlock state inquiry response.
    FocusUnlock,
    /// Zoom out state inquiry response.
    ZoomOut,
    /// Zoom in state inquiry response.
    ZoomIn,
    /// Iris up state inquiry response.
    IrisUp,
    /// Iris down state inquiry response.
    IrisDown,
    /// Night/day mode inquiry response.
    NightDayMode,
    /// Night/day position inquiry response.
    NightDayPosition,
    /// Auto trace state inquiry response.
    AutoTrace,
    /// Two tone mode inquiry response.
    TwoToneMode,
    /// Defog mode inquiry response.
    DefogMode,
    /// Noise reduction level inquiry response.
    NrLevel,
    /// Noise reduction mode inquiry response.
    NrMode,
    /// Noise reduction speed inquiry response.
    NrSpeed,
    /// Broadcast domain inquiry response.
    BroadcastDomain,
    /// Resolution inquiry response.
    Resolution,
    /// ND filter state inquiry response.
    NdFilter,
    /// ND filter preset inquiry response.
    NdFilterPreset,
    /// Focus near/far state inquiry response.
    FocusNearFar,
    /// Zoom tele/wide state inquiry response.
    ZoomTeleWide,
    /// Standby state inquiry response.
    Standby,
    /// Digital PTZ state inquiry response.
    DigitalPtz,
    /// Digital mode inquiry response.
    Digital,
    /// Iris control inquiry response.
    IrisControl,
    /// Defog level inquiry response.
    DefogLevel,
    /// Night/day switch inquiry response.
    NightDaySwitch,
    /// Flip mode inquiry response.
    FlipMode,
    /// Tally status inquiry response.
    TallyStatus,
    /// Tally auto adjust inquiry response.
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
        0x40 => Ok(Response::CmdAck), // ACK responses
        0x50 => {
            // Completion or inquiry data response
            if data.len() == 3 {
                Ok(Response::Completion)
            } else {
                // Debug logging for inquiry responses
                log::debug!(
                    "Parsing inquiry response for {:?}, raw bytes: {:02X?}, payload bytes: {:02X?}",
                    expected_type,
                    data,
                    &data[2..data.len() - 1]
                );
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
        _ => Ok(Response::Unknown {
            response_type: None,
            data: data.to_vec(),
        }),
    }
}

fn parse_inquiry_response(payload: &[u8], expected_type: &ResponseType) -> Result<Response, Error> {
    match expected_type {
        ResponseType::Power => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(InquiryResponse::Power {
                on: payload[0] == 0x02,
            }))
        }
        ResponseType::ZoomPosition => {
            // Standard VISCA expects 4 bytes for zoom position
            // But some cameras may return 8 bytes (possibly including digital zoom info)
            if payload.len() == 4 {
                // Standard format: 0p 0q 0r 0s
                let position = combine_nibbles_u16(&payload[0..4]);
                Ok(Response::Inquiry(InquiryResponse::ZoomPosition {
                    position,
                }))
            } else if payload.len() == 8 {
                // Extended format: Some cameras return 8 bytes
                // This might include both optical and digital zoom info
                // For now, use the first 4 bytes as the zoom position
                log::warn!(
                    "ZoomPosition: Received extended format (8 bytes). Payload: {:02X?}. Using first 4 bytes.",
                    payload
                );
                let position = combine_nibbles_u16(&payload[0..4]);
                Ok(Response::Inquiry(InquiryResponse::ZoomPosition {
                    position,
                }))
            } else {
                log::error!(
                    "ZoomPosition: Invalid response length. Expected 4 or 8 bytes, got {}. Payload: {:02X?}",
                    payload.len(),
                    payload
                );
                return Err(Error::InvalidResponseLength);
            }
        }
        ResponseType::PanTiltPosition => {
            // Standard VISCA expects 8 bytes (4 for pan, 4 for tilt)
            // But some cameras may return 4 bytes with combined values
            if payload.len() == 8 {
                // Standard format: PP PP PP PP TT TT TT TT
                let pan = combine_nibbles_i16(&payload[0..4]);
                let tilt = combine_nibbles_i16(&payload[4..8]);
                Ok(Response::Inquiry(InquiryResponse::PanTiltPosition {
                    pan,
                    tilt,
                }))
            } else if payload.len() == 4 {
                // Compact format: Some cameras return PP PP TT TT
                // or all zeros when at home position
                log::warn!(
                    "PanTiltPosition: Received compact format (4 bytes). Payload: {:02X?}. Treating as home position.",
                    payload
                );
                // For now, treat 4-byte response as home position (0, 0)
                // This may need adjustment based on specific camera models
                let pan = if payload.len() >= 2 {
                    ((payload[0] as i16) << 8) | (payload[1] as i16)
                } else {
                    0
                };
                let tilt = if payload.len() >= 4 {
                    ((payload[2] as i16) << 8) | (payload[3] as i16)
                } else {
                    0
                };
                Ok(Response::Inquiry(InquiryResponse::PanTiltPosition {
                    pan,
                    tilt,
                }))
            } else {
                log::error!(
                    "PanTiltPosition: Invalid response length. Expected 8 or 4 bytes, got {}. Payload: {:02X?}",
                    payload.len(),
                    payload
                );
                return Err(Error::InvalidResponseLength);
            }
        }
        ResponseType::FocusPosition => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            let position = combine_nibbles_u16(&payload[0..4]);
            Ok(Response::Inquiry(InquiryResponse::FocusPosition {
                position,
            }))
        }
        ResponseType::FocusNearLimit => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            let position = combine_nibbles_u16(&payload[0..4]);
            Ok(Response::Inquiry(InquiryResponse::FocusNearLimit {
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
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "exposure_mode",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed("Unknown exposure mode value"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::ExposureMode { mode }))
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
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "white_balance_mode",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed("Unknown white balance mode value"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::WhiteBalanceMode {
                mode,
            }))
        }
        ResponseType::FocusZone => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let zone = match payload[0] {
                0x00 => FocusZone::Top,
                0x01 => FocusZone::Center,
                0x02 => FocusZone::Bottom,
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "focus_zone",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed("Unknown focus zone value"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::FocusZone { zone }))
        }
        ResponseType::AutoFocusSensitivity => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let sensitivity = match payload[0] {
                0x00 => AutoFocusSensitivity::Low,
                0x01 => AutoFocusSensitivity::Normal,
                0x02 => AutoFocusSensitivity::High,
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "auto_focus_sensitivity",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed("Unknown auto focus sensitivity value"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::AutoFocusSensitivity {
                sensitivity,
            }))
        }
        ResponseType::ExposureCompensationMode => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(
                InquiryResponse::ExposureCompensationMode {
                    on: payload[0] == 0x02,
                },
            ))
        }
        ResponseType::SharpnessMode => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let mode = match payload[0] {
                0x02 => SharpnessMode::Auto,
                0x03 => SharpnessMode::Manual,
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "sharpness_mode",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed("Unknown sharpness mode value"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::SharpnessMode { mode }))
        }
        ResponseType::BlackWhite => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(InquiryResponse::BlackWhite {
                on: payload[0] == 0x04,
            }))
        }
        ResponseType::GainLimit => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(InquiryResponse::GainLimit {
                limit: payload[0],
            }))
        }
        ResponseType::RedChannel => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(InquiryResponse::RedChannel {
                gain: payload[0] as i8 - 10,
            }))
        }
        ResponseType::BlueChannel => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(InquiryResponse::BlueChannel {
                gain: payload[0] as i8 - 10,
            }))
        }
        ResponseType::Sharpness => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            let value = combine_nibbles_u8(&payload[2..4]);
            Ok(Response::Inquiry(InquiryResponse::Sharpness { value }))
        }
        ResponseType::ExposureCompensation => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            let raw_value = combine_nibbles_u8(&payload[2..4]);
            Ok(Response::Inquiry(InquiryResponse::ExposureCompensation {
                value: raw_value as i8 - 7,
            }))
        }
        ResponseType::Shutter => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            let position = combine_nibbles_u8(&payload[2..4]) as u16;
            Ok(Response::Inquiry(InquiryResponse::Shutter { position }))
        }
        ResponseType::ImageFlip => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let value = payload[0];
            Ok(Response::Inquiry(InquiryResponse::ImageFlip {
                horizontal: (value & 0x01) != 0,
                vertical: (value & 0x02) != 0,
            }))
        }
        ResponseType::Backlight => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(InquiryResponse::Backlight {
                status: payload[0] == 0x02,
            }))
        }
        ResponseType::Luminance => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(InquiryResponse::Luminance(payload[0])))
        }
        ResponseType::Contrast => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(InquiryResponse::Contrast(payload[0])))
        }
        ResponseType::TallyRed => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(InquiryResponse::TallyRed {
                on: payload[0] == 0x02,
            }))
        }
        ResponseType::TallyGreen => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(InquiryResponse::TallyGreen {
                on: payload[0] == 0x02,
            }))
        }
        ResponseType::Bright => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            let position = combine_nibbles_u16(&payload[0..4]);
            Ok(Response::Inquiry(InquiryResponse::Bright { position }))
        }
        ResponseType::Gain => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            // Extract the gain value from the last nibble
            let gain = payload[3];
            Ok(Response::Inquiry(InquiryResponse::GainLevel { gain }))
        }
        ResponseType::Iris => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            // Extract the iris position from the last nibble
            let position = payload[3];
            Ok(Response::Inquiry(InquiryResponse::Iris { position }))
        }
        ResponseType::Saturation => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            // Extract the saturation level from the last nibble
            let level = payload[3];
            Ok(Response::Inquiry(InquiryResponse::Saturation { level }))
        }
        ResponseType::ColorTemperature => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            // Extract the color temperature from nibbles 2 and 3
            let temperature = ((payload[2] as u16) << 4) | (payload[3] as u16);
            Ok(Response::Inquiry(InquiryResponse::ColorTemperature {
                temperature,
            }))
        }
        ResponseType::Hue => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            // Extract the hue value from the last nibble
            let hue = payload[3];
            Ok(Response::Inquiry(InquiryResponse::Hue { hue }))
        }
        ResponseType::Version => {
            // Version response format: VV VV MM MM FF FF KK
            // VV VV = Vendor ID (2 bytes)
            // MM MM = Model ID (2 bytes)
            // FF FF = ROM version (2 bytes)
            // KK = Max socket number (1 byte)
            if payload.len() != 7 {
                return Err(Error::InvalidResponseLength);
            }
            let vendor = ((payload[0] as u16) << 8) | (payload[1] as u16);
            let model = ((payload[2] as u16) << 8) | (payload[3] as u16);
            let rom_version = ((payload[4] as u32) << 8) | (payload[5] as u32);
            let max_socket = payload[6];
            Ok(Response::Inquiry(InquiryResponse::Version {
                vendor,
                model,
                rom_version,
                max_socket,
            }))
        }
        ResponseType::FocusMode => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let mode = match payload[0] {
                0x02 => FocusMode::Auto,
                0x03 => FocusMode::Manual,
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "focus_mode",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed("Unknown focus mode value"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::FocusMode { mode }))
        }
        ResponseType::DynamicRange => {
            // Dynamic range level response
            // Single byte level value (0x0=0 to 0x8=8)
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let level = payload[0];
            Ok(Response::Inquiry(InquiryResponse::DynamicRange { level }))
        }
        ResponseType::NoiseReduction2D => {
            // 2D noise reduction level response
            // Based on common VISCA patterns, expecting single byte level value
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let level = payload[0];
            Ok(Response::Inquiry(InquiryResponse::NoiseReduction2D {
                level,
            }))
        }
        ResponseType::NoiseReduction3D => {
            // 3D noise reduction level response
            // Based on common VISCA patterns, expecting single byte level value
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let level = payload[0];
            Ok(Response::Inquiry(InquiryResponse::NoiseReduction3D {
                level,
            }))
        }
        ResponseType::MenuOpenClose => {
            // Menu open/close status response
            // Single byte: 0x02 = closed, 0x03 = open
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let is_open = match payload[0] {
                0x02 => false, // Menu closed
                0x03 => true,  // Menu open
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "menu_status",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed(
                            "Invalid menu status value. Expected 0x02 (closed) or 0x03 (open)",
                        ),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::MenuOpenClose {
                is_open,
            }))
        }
        ResponseType::AutoFocus => {
            // AutoFocus on/off status response
            // Single byte: 0x02 = off, 0x03 = on
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let enabled = match payload[0] {
                0x02 => false, // AF off
                0x03 => true,  // AF on
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "autofocus_status",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed(
                            "Invalid autofocus status value. Expected 0x02 (off) or 0x03 (on)",
                        ),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::AutoFocus { enabled }))
        }
        ResponseType::TallyStatus => {
            // Tally light status response
            // Two bytes: first for red, second for green
            // Each byte: 0x02 = off, 0x03 = on
            if payload.len() != 2 {
                return Err(Error::InvalidResponseLength);
            }
            let red_on = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "tally_red_status",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed(
                            "Invalid tally status value. Expected 0x02 (off) or 0x03 (on)",
                        ),
                    })
                }
            };
            let green_on = match payload[1] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "tally_green_status",
                        value: Cow::Owned(format!("{:02X}", payload[1])),
                        reason: Cow::Borrowed(
                            "Invalid tally status value. Expected 0x02 (off) or 0x03 (on)",
                        ),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::TallyStatus {
                red_on,
                green_on,
            }))
        }
        ResponseType::Resolution => {
            // Resolution inquiry response
            // Single byte indicating resolution mode
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            // Resolution values based on common PTZ camera patterns:
            // 0x00 = 1080p60, 0x01 = 1080p30, 0x02 = 720p60, 0x03 = 720p30, etc.
            let resolution_mode = payload[0];
            Ok(Response::Inquiry(InquiryResponse::Resolution(
                resolution_mode,
            )))
        }
        ResponseType::NightDayMode => {
            // Night/Day mode inquiry response
            // Single byte: 0x02 = Day mode, 0x03 = Night mode
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let is_night = match payload[0] {
                0x02 => false, // Day mode
                0x03 => true,  // Night mode
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "night_day_mode",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed(
                            "Invalid night/day mode value. Expected 0x02 (day) or 0x03 (night)",
                        ),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::NightDayMode {
                is_night,
            }))
        }
        ResponseType::NdFilter => {
            // ND filter position inquiry response
            // Single byte indicating filter position
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            // Common ND filter positions:
            // 0x00 = Clear (no filter)
            // 0x01 = 1/4 ND
            // 0x02 = 1/8 ND
            // 0x03 = 1/16 ND
            // 0x04 = 1/32 ND
            // 0x05 = 1/64 ND
            let position = payload[0];
            Ok(Response::Inquiry(InquiryResponse::NdFilter { position }))
        }
        ResponseType::PictureEffect => {
            // Picture effect inquiry response
            // Single byte indicating current effect
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            // Picture effect values:
            // 0x00 = Off (normal)
            // 0x01 = Negative
            // 0x02 = B&W
            // Other values are camera-specific effects
            let effect = payload[0];
            Ok(Response::Inquiry(InquiryResponse::PictureEffect { effect }))
        }
        ResponseType::FlipMode => {
            // Combined flip mode inquiry response
            // Single byte encoding both horizontal and vertical flip
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            // Flip mode encoding:
            // 0x00 = No flip
            // 0x01 = Horizontal flip only
            // 0x02 = Vertical flip only
            // 0x03 = Both horizontal and vertical flip
            let mode = payload[0];
            let horizontal = (mode & 0x01) != 0;
            let vertical = (mode & 0x02) != 0;
            Ok(Response::Inquiry(InquiryResponse::FlipMode {
                horizontal,
                vertical,
            }))
        }
        ResponseType::Standby => {
            // Standby mode inquiry response
            // Single byte: 0x02 = Active, 0x03 = Standby
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let in_standby =
                match payload[0] {
                    0x02 => false, // Active
                    0x03 => true,  // Standby
                    _ => return Err(Error::InvalidParameter {
                        parameter: "standby_mode",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed(
                            "Invalid standby mode value. Expected 0x02 (active) or 0x03 (standby)",
                        ),
                    }),
                };
            Ok(Response::Inquiry(InquiryResponse::Standby { in_standby }))
        }
        ResponseType::FocusRange => {
            // Focus range inquiry response
            // Single byte indicating focus range mode
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            // Parse custom focus range response
            parse_focus_range(payload).map(Response::Inquiry)
        }
        ResponseType::IrisControl => {
            // Iris control inquiry response
            // Single byte: 0x02 = Manual control, 0x03 = Auto control
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let auto = match payload[0] {
                0x02 => false, // Manual control
                0x03 => true,  // Auto control
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "iris_control",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed(
                            "Invalid iris control value. Expected 0x02 (manual) or 0x03 (auto)",
                        ),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::IrisControl { auto }))
        }
        ResponseType::DefogMode => {
            // Defog mode inquiry response
            // Single byte: 0x02 = Off, 0x03 = On
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let enabled = match payload[0] {
                0x02 => false, // Defog off
                0x03 => true,  // Defog on
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "defog_mode",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed(
                            "Invalid defog mode value. Expected 0x02 (off) or 0x03 (on)",
                        ),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::DefogMode { enabled }))
        }
        ResponseType::DefogLevel => {
            // Defog level inquiry response
            // Single byte indicating defog strength level
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(InquiryResponse::DefogLevel {
                level: payload[0],
            }))
        }
        ResponseType::DigitalPtz => {
            // Digital PTZ enable/disable inquiry response
            // Single byte: 0x02 = Off, 0x03 = On
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let enabled = match payload[0] {
                0x02 => false, // Digital PTZ off
                0x03 => true,  // Digital PTZ on
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "digital_ptz",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed(
                            "Invalid digital PTZ value. Expected 0x02 (off) or 0x03 (on)",
                        ),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::DigitalPtz { enabled }))
        }
        ResponseType::AutoWhiteBalanceSensitivity => {
            // Auto white balance sensitivity inquiry response
            // Single byte: 0x00 = Low, 0x01 = Normal, 0x02 = High
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let sensitivity = match payload[0] {
                0x00 => AutoWhiteBalanceSensitivity::Low,
                0x01 => AutoWhiteBalanceSensitivity::Normal,
                0x02 => AutoWhiteBalanceSensitivity::High,
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "auto_wb_sensitivity",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed("Invalid auto white balance sensitivity. Expected 0x00 (Low), 0x01 (Normal), or 0x02 (High)"),
                    })
                }
            };
            Ok(Response::Inquiry(
                InquiryResponse::AutoWhiteBalanceSensitivity { sensitivity },
            ))
        }
        ResponseType::ExposureCompensationPosition => {
            // Exposure compensation position inquiry response
            // 4 bytes: PP PP (position as nibbles)
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            let position = combine_nibbles_u16(&payload[0..4]);
            Ok(Response::Inquiry(
                InquiryResponse::ExposureCompensationPosition { position },
            ))
        }
        ResponseType::RedTuning => {
            // Red channel tuning inquiry response
            // Single byte: tuning level
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(InquiryResponse::RedTuning {
                level: payload[0],
            }))
        }
        ResponseType::BlueTuning => {
            // Blue channel tuning inquiry response
            // Single byte: tuning level
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(InquiryResponse::BlueTuning {
                level: payload[0],
            }))
        }
        ResponseType::Gamma => {
            // Gamma curve setting inquiry response
            // Single byte: gamma setting (0=Standard, 1-4=different curves)
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(InquiryResponse::Gamma {
                value: payload[0],
            }))
        }
        ResponseType::AutoTrace => {
            // Auto trace mode inquiry response
            // Single byte: 0x02 = Off, 0x03 = On
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let enabled = match payload[0] {
                0x02 => false, // Auto trace off
                0x03 => true,  // Auto trace on
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "auto_trace",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed(
                            "Invalid auto trace value. Expected 0x02 (off) or 0x03 (on)",
                        ),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::AutoTrace { enabled }))
        }
        ResponseType::FocusUnlock => {
            // Focus unlock state inquiry response
            // Single byte: 0x02 = Locked, 0x03 = Unlocked
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let unlocked =
                match payload[0] {
                    0x02 => false, // Focus locked
                    0x03 => true,  // Focus unlocked
                    _ => return Err(Error::InvalidParameter {
                        parameter: "focus_unlock",
                        value: Cow::Owned(format!("{:02X}", payload[0])),
                        reason: Cow::Borrowed(
                            "Invalid focus unlock value. Expected 0x02 (locked) or 0x03 (unlocked)",
                        ),
                    }),
                };
            Ok(Response::Inquiry(InquiryResponse::FocusUnlock { unlocked }))
        }
        ResponseType::SharpnessPosition => {
            if payload.len() != 4 {
                return Err(Error::InvalidResponseLength);
            }
            let position = combine_nibbles_u16(&payload[0..4]);
            Ok(Response::Inquiry(InquiryResponse::SharpnessPosition {
                position,
            }))
        }
        ResponseType::NrLevel => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(InquiryResponse::NrLevel(payload[0])))
        }
        ResponseType::BroadcastDomain => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(InquiryResponse::BroadcastDomain(
                payload[0],
            )))
        }
        ResponseType::MotionSyncMode => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let mode = MotionSyncMode::try_from(payload[0])?;
            Ok(Response::Inquiry(InquiryResponse::MotionSyncMode { mode }))
        }
        ResponseType::MotionSyncSpeed => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let speed = MotionSyncSpeed::try_from(payload[0])?;
            Ok(Response::Inquiry(InquiryResponse::MotionSyncSpeed {
                speed,
            }))
        }
        ResponseType::NrMode => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let mode = NrMode::try_from(payload[0])?;
            Ok(Response::Inquiry(InquiryResponse::NrMode { mode }))
        }
        ResponseType::NrSpeed => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let speed = NrSpeed::try_from(payload[0])?;
            Ok(Response::Inquiry(InquiryResponse::NrSpeed { speed }))
        }
        ResponseType::BlackWhiteMode => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let mode = BlackWhiteMode::try_from(payload[0])?;
            Ok(Response::Inquiry(InquiryResponse::BlackWhiteMode { mode }))
        }
        ResponseType::UsbAudio => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let on = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "UsbAudio status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
                        reason: Cow::Borrowed("Expected 0x02 (off) or 0x03 (on)"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::UsbAudio { on }))
        }
        ResponseType::TwoToneMode => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let on = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "TwoToneMode status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
                        reason: Cow::Borrowed("Expected 0x02 (off) or 0x03 (on)"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::TwoToneMode { on }))
        }
        ResponseType::NdFilterPreset => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            Ok(Response::Inquiry(InquiryResponse::NdFilterPreset {
                preset: payload[0],
            }))
        }
        ResponseType::Digital => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let on = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "Digital mode status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
                        reason: Cow::Borrowed("Expected 0x02 (off) or 0x03 (on)"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::Digital { on }))
        }
        ResponseType::TallyAutoAdjust => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let on = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "TallyAutoAdjust status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
                        reason: Cow::Borrowed("Expected 0x02 (off) or 0x03 (on)"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::TallyAutoAdjust { on }))
        }
        ResponseType::Rtmp => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let on = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "RTMP status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
                        reason: Cow::Borrowed("Expected 0x02 (off) or 0x03 (on)"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::Rtmp { on }))
        }
        ResponseType::ZoomOut => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let active = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "ZoomOut status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
                        reason: Cow::Borrowed("Expected 0x02 (inactive) or 0x03 (active)"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::ZoomOut { active }))
        }
        ResponseType::ZoomIn => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let active = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "ZoomIn status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
                        reason: Cow::Borrowed("Expected 0x02 (inactive) or 0x03 (active)"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::ZoomIn { active }))
        }
        ResponseType::IrisUp => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let active = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "IrisUp status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
                        reason: Cow::Borrowed("Expected 0x02 (inactive) or 0x03 (active)"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::IrisUp { active }))
        }
        ResponseType::IrisDown => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let active = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "IrisDown status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
                        reason: Cow::Borrowed("Expected 0x02 (inactive) or 0x03 (active)"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::IrisDown { active }))
        }
        ResponseType::NightDayPosition => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let position = payload[0];
            Ok(Response::Inquiry(InquiryResponse::NightDayPosition {
                position,
            }))
        }
        ResponseType::FocusNearFar => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let near = match payload[0] {
                0x02 => false, // Far active
                0x03 => true,  // Near active
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "FocusNearFar status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
                        reason: Cow::Borrowed("Expected 0x02 (far) or 0x03 (near)"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::FocusNearFar { near }))
        }
        ResponseType::ZoomTeleWide => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let tele = match payload[0] {
                0x02 => false, // Wide active
                0x03 => true,  // Tele active
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "ZoomTeleWide status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
                        reason: Cow::Borrowed("Expected 0x02 (wide) or 0x03 (tele)"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::ZoomTeleWide { tele }))
        }
        ResponseType::NightDaySwitch => {
            if payload.len() != 1 {
                return Err(Error::InvalidResponseLength);
            }
            let enabled = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Err(Error::InvalidParameter {
                        parameter: "NightDaySwitch status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
                        reason: Cow::Borrowed("Expected 0x02 (disabled) or 0x03 (enabled)"),
                    })
                }
            };
            Ok(Response::Inquiry(InquiryResponse::NightDaySwitch {
                enabled,
            }))
        }
    }
}

fn combine_nibbles_u16(nibbles: &[u8]) -> u16 {
    if nibbles.len() < 4 {
        return 0;
    }
    ((nibbles[0] as u16) << 12)
        | ((nibbles[1] as u16) << 8)
        | ((nibbles[2] as u16) << 4)
        | (nibbles[3] as u16)
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

// Custom parser helper functions for inquiry responses
// These are used by the derive macro when custom_fn is specified

/// Parse the last nibble from a 4-byte payload for Gain
pub fn parse_gain_last_nibble(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.len() < 4 {
        return Err(Error::InvalidResponseLength);
    }
    Ok(InquiryResponse::GainLevel {
        gain: data[3] & 0x0F,
    })
}

/// Parse the last nibble from a 4-byte payload for Iris
pub fn parse_iris_last_nibble(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.len() < 4 {
        return Err(Error::InvalidResponseLength);
    }
    Ok(InquiryResponse::Iris {
        position: data[3] & 0x0F,
    })
}

/// Parse the last nibble from a 4-byte payload for Saturation
pub fn parse_saturation_last_nibble(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.len() < 4 {
        return Err(Error::InvalidResponseLength);
    }
    Ok(InquiryResponse::Saturation {
        level: data[3] & 0x0F,
    })
}

/// Parse the last nibble from a 4-byte payload for Hue
pub fn parse_hue_last_nibble(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.len() < 4 {
        return Err(Error::InvalidResponseLength);
    }
    Ok(InquiryResponse::Hue {
        hue: data[3] & 0x0F,
    })
}

/// Parse middle nibbles from payload (used for Sharpness)
pub fn parse_middle_nibbles(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.len() < 4 {
        return Err(Error::InvalidResponseLength);
    }
    let value = combine_nibbles_u8(&data[2..4]);
    Ok(InquiryResponse::Sharpness { value })
}

/// Parse exposure compensation value with offset
pub fn parse_exposure_compensation(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.len() < 4 {
        return Err(Error::InvalidResponseLength);
    }
    let value = combine_nibbles_u8(&data[2..4]) as i8 - 7;
    Ok(InquiryResponse::ExposureCompensation { value })
}

/// Parse shutter value from middle nibbles
pub fn parse_shutter(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.len() < 4 {
        return Err(Error::InvalidResponseLength);
    }
    let position = combine_nibbles_u8(&data[2..4]) as u16;
    Ok(InquiryResponse::Shutter { position })
}

/// Parse color temperature from middle nibbles
pub fn parse_color_temperature(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.len() < 4 {
        return Err(Error::InvalidResponseLength);
    }
    let temperature = ((data[2] as u16) << 4) | (data[3] as u16);
    Ok(InquiryResponse::ColorTemperature { temperature })
}

/// Parse sharpness mode (0x02 = Auto, 0x03 = Manual)
pub fn parse_sharpness_mode(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    match data[0] {
        0x02 => Ok(InquiryResponse::SharpnessMode {
            mode: SharpnessMode::Auto,
        }),
        0x03 => Ok(InquiryResponse::SharpnessMode {
            mode: SharpnessMode::Manual,
        }),
        _ => Err(Error::InvalidResponse {
            expected: Cow::Borrowed("0x02 (Auto) or 0x03 (Manual)"),
            actual: vec![data[0]],
        }),
    }
}

/// Parse menu open/close status
pub fn parse_menu_open_close(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    let is_open = match data[0] {
        0x02 => false, // Menu closed
        0x03 => true,  // Menu open
        _ => {
            return Err(Error::InvalidParameter {
                parameter: "menu_status",
                value: Cow::Owned(format!("{:02X}", data[0])),
                reason: Cow::Borrowed(
                    "Invalid menu status value. Expected 0x02 (closed) or 0x03 (open)",
                ),
            })
        }
    };
    Ok(InquiryResponse::MenuOpenClose { is_open })
}

/// Parse auto focus on/off status
pub fn parse_auto_focus(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    let enabled = match data[0] {
        0x02 => false, // AF off
        0x03 => true,  // AF on
        _ => {
            return Err(Error::InvalidParameter {
                parameter: "autofocus_status",
                value: Cow::Owned(format!("{:02X}", data[0])),
                reason: Cow::Borrowed(
                    "Invalid autofocus status value. Expected 0x02 (off) or 0x03 (on)",
                ),
            })
        }
    };
    Ok(InquiryResponse::AutoFocus { enabled })
}

/// Parse tally light status (red and green)
pub fn parse_tally_status(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.len() < 2 {
        return Err(Error::InvalidResponseLength);
    }
    let red_on = match data[0] {
        0x02 => false,
        0x03 => true,
        _ => {
            return Err(Error::InvalidParameter {
                parameter: "tally_red_status",
                value: Cow::Owned(format!("{:02X}", data[0])),
                reason: Cow::Borrowed(
                    "Invalid tally status value. Expected 0x02 (off) or 0x03 (on)",
                ),
            })
        }
    };
    let green_on = match data[1] {
        0x02 => false,
        0x03 => true,
        _ => {
            return Err(Error::InvalidParameter {
                parameter: "tally_green_status",
                value: Cow::Owned(format!("{:02X}", data[1])),
                reason: Cow::Borrowed(
                    "Invalid tally status value. Expected 0x02 (off) or 0x03 (on)",
                ),
            })
        }
    };
    Ok(InquiryResponse::TallyStatus { red_on, green_on })
}

/// Parse night/day mode status
pub fn parse_night_day_mode(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    let is_night = match data[0] {
        0x02 => false, // Day mode
        0x03 => true,  // Night mode
        _ => {
            return Err(Error::InvalidParameter {
                parameter: "night_day_mode",
                value: Cow::Owned(format!("{:02X}", data[0])),
                reason: Cow::Borrowed(
                    "Invalid night/day mode value. Expected 0x02 (day) or 0x03 (night)",
                ),
            })
        }
    };
    Ok(InquiryResponse::NightDayMode { is_night })
}

/// Parse flip mode (combined horizontal/vertical)
pub fn parse_flip_mode(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    // Flip mode encoding:
    // 0x00 = No flip
    // 0x01 = Horizontal flip only
    // 0x02 = Vertical flip only
    // 0x03 = Both horizontal and vertical flip
    let mode = data[0];
    let horizontal = (mode & 0x01) != 0;
    let vertical = (mode & 0x02) != 0;
    Ok(InquiryResponse::FlipMode {
        horizontal,
        vertical,
    })
}

/// Parse standby mode status
pub fn parse_standby(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    let in_standby = match data[0] {
        0x02 => false, // Active
        0x03 => true,  // Standby
        _ => {
            return Err(Error::InvalidParameter {
                parameter: "standby_mode",
                value: Cow::Owned(format!("{:02X}", data[0])),
                reason: Cow::Borrowed(
                    "Invalid standby mode value. Expected 0x02 (active) or 0x03 (standby)",
                ),
            })
        }
    };
    Ok(InquiryResponse::Standby { in_standby })
}

/// Parse green tally light status (FR7 only)
pub fn parse_tally_green(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    let on = match data[0] {
        0x02 => true,  // On
        0x03 => false, // Off
        _ => {
            return Err(Error::InvalidParameter {
                parameter: "tally_green_status",
                value: Cow::Owned(format!("{:02X}", data[0])),
                reason: Cow::Borrowed(
                    "Invalid green tally status value. Expected 0x02 (on) or 0x03 (off)",
                ),
            })
        }
    };
    Ok(InquiryResponse::TallyGreen { on })
}

/// Parse ND filter position
pub fn parse_nd_filter(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    Ok(InquiryResponse::NdFilter { position: data[0] })
}

/// Parse picture effect mode
pub fn parse_picture_effect(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    Ok(InquiryResponse::PictureEffect { effect: data[0] })
}

/// Parse iris control mode
pub fn parse_iris_control(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    let auto = match data[0] {
        0x02 => false, // Manual control
        0x03 => true,  // Auto control
        _ => {
            return Err(Error::InvalidParameter {
                parameter: "iris_control",
                value: Cow::Owned(format!("{:02X}", data[0])),
                reason: Cow::Borrowed(
                    "Invalid iris control value. Expected 0x02 (manual) or 0x03 (auto)",
                ),
            })
        }
    };
    Ok(InquiryResponse::IrisControl { auto })
}

/// Parse defog mode
pub fn parse_defog_mode(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    let enabled = match data[0] {
        0x02 => false, // Defog off
        0x03 => true,  // Defog on
        _ => {
            return Err(Error::InvalidParameter {
                parameter: "defog_mode",
                value: Cow::Owned(format!("{:02X}", data[0])),
                reason: Cow::Borrowed("Invalid defog mode value. Expected 0x02 (off) or 0x03 (on)"),
            })
        }
    };
    Ok(InquiryResponse::DefogMode { enabled })
}

/// Parse digital PTZ mode
pub fn parse_digital_ptz(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    let enabled = match data[0] {
        0x02 => false, // Digital PTZ off
        0x03 => true,  // Digital PTZ on
        _ => {
            return Err(Error::InvalidParameter {
                parameter: "digital_ptz",
                value: Cow::Owned(format!("{:02X}", data[0])),
                reason: Cow::Borrowed(
                    "Invalid digital PTZ value. Expected 0x02 (off) or 0x03 (on)",
                ),
            })
        }
    };
    Ok(InquiryResponse::DigitalPtz { enabled })
}

/// Parse defog level
pub fn parse_defog_level(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    Ok(InquiryResponse::DefogLevel { level: data[0] })
}

/// Parse auto white balance sensitivity
pub fn parse_auto_wb_sensitivity(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    let sensitivity = match data[0] {
        0x00 => AutoWhiteBalanceSensitivity::Low,
        0x01 => AutoWhiteBalanceSensitivity::Normal,
        0x02 => AutoWhiteBalanceSensitivity::High,
        _ => {
            return Err(Error::InvalidParameter {
                parameter: "auto_wb_sensitivity",
                value: Cow::Owned(format!("{:02X}", data[0])),
                reason: Cow::Borrowed("Invalid auto white balance sensitivity. Expected 0x00 (Low), 0x01 (Normal), or 0x02 (High)"),
            })
        }
    };
    Ok(InquiryResponse::AutoWhiteBalanceSensitivity { sensitivity })
}

/// Parse exposure compensation position
pub fn parse_exposure_compensation_position(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.len() < 4 {
        return Err(Error::InvalidResponseLength);
    }
    let position = combine_nibbles_u16(&data[0..4]);
    Ok(InquiryResponse::ExposureCompensationPosition { position })
}

/// Parse red tuning level
pub fn parse_red_tuning(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    Ok(InquiryResponse::RedTuning { level: data[0] })
}

/// Parse blue tuning level
pub fn parse_blue_tuning(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    Ok(InquiryResponse::BlueTuning { level: data[0] })
}

/// Parse gamma curve setting
pub fn parse_gamma(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    Ok(InquiryResponse::Gamma { value: data[0] })
}

/// Parse auto trace mode
pub fn parse_auto_trace(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    let enabled = match data[0] {
        0x02 => false, // Auto trace off
        0x03 => true,  // Auto trace on
        _ => {
            return Err(Error::InvalidParameter {
                parameter: "auto_trace",
                value: Cow::Owned(format!("{:02X}", data[0])),
                reason: Cow::Borrowed("Invalid auto trace value. Expected 0x02 (off) or 0x03 (on)"),
            })
        }
    };
    Ok(InquiryResponse::AutoTrace { enabled })
}

/// Parse focus unlock state
pub fn parse_focus_unlock(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    let unlocked = match data[0] {
        0x02 => false, // Focus locked
        0x03 => true,  // Focus unlocked
        _ => {
            return Err(Error::InvalidParameter {
                parameter: "focus_unlock",
                value: Cow::Owned(format!("{:02X}", data[0])),
                reason: Cow::Borrowed(
                    "Invalid focus unlock value. Expected 0x02 (locked) or 0x03 (unlocked)",
                ),
            })
        }
    };
    Ok(InquiryResponse::FocusUnlock { unlocked })
}

/// Parse focus range mode
pub fn parse_focus_range(data: &[u8]) -> Result<InquiryResponse, Error> {
    use crate::command::FocusRange;

    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    let range = FocusRange::try_from(data[0])?;
    Ok(InquiryResponse::FocusRange { range })
}

// Note: Complex response parsing tests moved to tests/response_parsing_comprehensive.rs
// Only basic unit tests remain here for fast feedback during development

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_response_debug() {
        let ack = Response::CmdAck;
        assert_eq!(format!("{ack:?}"), "CmdAck");

        let completion = Response::Completion;
        assert_eq!(format!("{completion:?}"), "Completion");
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
        assert!(matches!(result, Response::CmdAck));
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
            Response::Inquiry(InquiryResponse::Power { on }) => assert!(on),
            _ => panic!("Expected Power inquiry response"),
        }

        // Power Off
        let response = vec![0x90, 0x50, 0x03, 0xFF];
        let result = parse_response(&response, &ResponseType::Power).unwrap();
        match result {
            Response::Inquiry(InquiryResponse::Power { on }) => assert!(!on),
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

    #[test]
    fn test_parse_ack_response() {
        // ACK for socket 0
        let ack_bytes = &[0x90, 0x40, 0xFF];
        let response = parse_response(ack_bytes, &ResponseType::PanTiltPosition);
        assert!(matches!(response, Ok(Response::CmdAck)));

        // ACK for socket 1
        let ack_bytes = &[0x90, 0x41, 0xFF];
        let response = parse_response(ack_bytes, &ResponseType::ZoomPosition);
        assert!(matches!(response, Ok(Response::CmdAck)));
    }

    #[test]
    fn test_parse_completion_response() {
        // Completion for socket 0
        let completion_bytes = &[0x90, 0x50, 0xFF];
        let response = parse_response(completion_bytes, &ResponseType::PanTiltPosition);
        assert!(matches!(response, Ok(Response::Completion)));

        // Completion for socket 1
        let completion_bytes = &[0x90, 0x51, 0xFF];
        let response = parse_response(completion_bytes, &ResponseType::ZoomPosition);
        assert!(matches!(response, Ok(Response::Completion)));
    }

    #[test]
    fn test_parse_error_responses() {
        // Test Syntax Error
        let error_bytes = &[0x90, 0x60, 0x02, 0xFF];
        let response = parse_response(error_bytes, &ResponseType::PanTiltPosition);
        assert!(matches!(response, Err(Error::SyntaxError)));

        // Test Command Buffer Full
        let error_bytes = &[0x90, 0x60, 0x03, 0xFF];
        let response = parse_response(error_bytes, &ResponseType::PanTiltPosition);
        assert!(matches!(response, Err(Error::CommandBufferFull)));

        // Test Command Not Executable
        let error_bytes = &[0x90, 0x61, 0x41, 0xFF];
        let response = parse_response(error_bytes, &ResponseType::PanTiltPosition);
        assert!(matches!(response, Err(Error::CommandNotExecutable)));
    }

    #[test]
    fn test_parse_pan_tilt_position_response() {
        let pt_response_bytes = &[
            0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0xFF,
        ];
        let response = parse_response(pt_response_bytes, &ResponseType::PanTiltPosition);
        match response {
            Ok(Response::Inquiry(InquiryResponse::PanTiltPosition { pan, tilt })) => {
                assert_eq!(pan, 0x1234);
                assert_eq!(tilt, 0x5678);
            }
            _ => panic!("Expected PanTiltPosition inquiry response"),
        }
    }

    #[test]
    fn test_parse_zoom_position_response() {
        let zoom_response_bytes = &[0x90, 0x50, 0x0A, 0x0B, 0x0C, 0x0D, 0xFF];
        let response = parse_response(zoom_response_bytes, &ResponseType::ZoomPosition);
        match response {
            Ok(Response::Inquiry(InquiryResponse::ZoomPosition { position })) => {
                assert_eq!(position, 0xABCD);
            }
            _ => panic!("Expected ZoomPosition inquiry response"),
        }
    }

    #[test]
    fn test_parse_focus_position_response() {
        let focus_response_bytes = &[0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xFF];
        let response = parse_response(focus_response_bytes, &ResponseType::FocusPosition);
        match response {
            Ok(Response::Inquiry(InquiryResponse::FocusPosition { position })) => {
                assert_eq!(position, 0x1234);
            }
            _ => panic!("Expected FocusPosition inquiry response"),
        }
    }

    #[test]
    fn test_parse_exposure_mode_response() {
        // Auto exposure mode
        let exposure_response_bytes = &[0x90, 0x50, 0x00, 0xFF];
        let response = parse_response(exposure_response_bytes, &ResponseType::ExposureMode);
        match response {
            Ok(Response::Inquiry(InquiryResponse::ExposureMode { mode })) => {
                assert_eq!(mode as u8, 0x00); // Auto mode
            }
            _ => panic!("Expected ExposureMode inquiry response"),
        }

        // Manual exposure mode
        let exposure_response_bytes = &[0x90, 0x50, 0x03, 0xFF];
        let response = parse_response(exposure_response_bytes, &ResponseType::ExposureMode);
        match response {
            Ok(Response::Inquiry(InquiryResponse::ExposureMode { mode })) => {
                assert_eq!(mode as u8, 0x03); // Manual mode
            }
            _ => panic!("Expected ExposureMode inquiry response"),
        }
    }

    #[test]
    fn test_parse_luminance_response() {
        // Test minimum luminance value (0)
        let luminance_response_bytes = &[0x90, 0x50, 0x00, 0xFF];
        let response = parse_response(luminance_response_bytes, &ResponseType::Luminance);
        match response {
            Ok(Response::Inquiry(InquiryResponse::Luminance(value))) => {
                assert_eq!(value, 0x00);
            }
            _ => panic!("Expected Luminance inquiry response"),
        }

        // Test middle luminance value (7)
        let luminance_response_bytes = &[0x90, 0x50, 0x07, 0xFF];
        let response = parse_response(luminance_response_bytes, &ResponseType::Luminance);
        match response {
            Ok(Response::Inquiry(InquiryResponse::Luminance(value))) => {
                assert_eq!(value, 0x07);
            }
            _ => panic!("Expected Luminance inquiry response"),
        }

        // Test maximum luminance value (14)
        let luminance_response_bytes = &[0x90, 0x50, 0x0E, 0xFF];
        let response = parse_response(luminance_response_bytes, &ResponseType::Luminance);
        match response {
            Ok(Response::Inquiry(InquiryResponse::Luminance(value))) => {
                assert_eq!(value, 0x0E);
            }
            _ => panic!("Expected Luminance inquiry response"),
        }
    }

    #[test]
    fn test_parse_contrast_response() {
        // Test minimum contrast value (0)
        let contrast_response_bytes = &[0x90, 0x50, 0x00, 0xFF];
        let response = parse_response(contrast_response_bytes, &ResponseType::Contrast);
        match response {
            Ok(Response::Inquiry(InquiryResponse::Contrast(value))) => {
                assert_eq!(value, 0x00);
            }
            _ => panic!("Expected Contrast inquiry response"),
        }

        // Test middle contrast value (7)
        let contrast_response_bytes = &[0x90, 0x50, 0x07, 0xFF];
        let response = parse_response(contrast_response_bytes, &ResponseType::Contrast);
        match response {
            Ok(Response::Inquiry(InquiryResponse::Contrast(value))) => {
                assert_eq!(value, 0x07);
            }
            _ => panic!("Expected Contrast inquiry response"),
        }

        // Test maximum contrast value (14)
        let contrast_response_bytes = &[0x90, 0x50, 0x0E, 0xFF];
        let response = parse_response(contrast_response_bytes, &ResponseType::Contrast);
        match response {
            Ok(Response::Inquiry(InquiryResponse::Contrast(value))) => {
                assert_eq!(value, 0x0E);
            }
            _ => panic!("Expected Contrast inquiry response"),
        }
    }

    #[test]
    fn test_parse_invalid_response() {
        // Test response that doesn't start with 0x90
        let invalid_bytes = &[0x80, 0x50, 0xFF];
        let response = parse_response(invalid_bytes, &ResponseType::PanTiltPosition);
        assert!(response.is_err());

        // Test response that doesn't end with 0xFF
        let invalid_bytes = &[0x90, 0x50, 0x00];
        let response = parse_response(invalid_bytes, &ResponseType::PanTiltPosition);
        assert!(response.is_err());

        // Test empty response
        let invalid_bytes = &[];
        let response = parse_response(invalid_bytes, &ResponseType::PanTiltPosition);
        assert!(response.is_err());
    }

    #[test]
    fn test_parse_response_with_wrong_type() {
        // Try to parse an ACK as a data response
        let ack_bytes = &[0x90, 0x40, 0xFF];
        let response = parse_response(ack_bytes, &ResponseType::PanTiltPosition);
        // ACK is still recognized regardless of expected response type
        assert!(matches!(response, Ok(Response::CmdAck)));
    }

    #[test]
    fn test_parse_sharpness_response() {
        // Test Sharpness response
        let sharpness_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x0B, 0xFF];
        let response = parse_response(sharpness_bytes, &ResponseType::Sharpness);
        match response {
            Ok(Response::Inquiry(InquiryResponse::Sharpness { value })) => {
                assert_eq!(value, 0x0B);
            }
            _ => panic!("Expected Sharpness inquiry response"),
        }
    }

    #[test]
    fn test_parse_exposure_compensation_responses() {
        // Test Exposure Compensation value -7
        let exp_comp_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
        let response = parse_response(exp_comp_bytes, &ResponseType::ExposureCompensation);
        match response {
            Ok(Response::Inquiry(InquiryResponse::ExposureCompensation { value })) => {
                assert_eq!(value, -7);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation value 0
        let exp_comp_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
        let response = parse_response(exp_comp_bytes, &ResponseType::ExposureCompensation);
        match response {
            Ok(Response::Inquiry(InquiryResponse::ExposureCompensation { value })) => {
                assert_eq!(value, 0);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation value +7
        let exp_comp_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x0E, 0xFF];
        let response = parse_response(exp_comp_bytes, &ResponseType::ExposureCompensation);
        match response {
            Ok(Response::Inquiry(InquiryResponse::ExposureCompensation { value })) => {
                assert_eq!(value, 7);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation Mode On
        let exp_comp_mode_bytes = &[0x90, 0x50, 0x02, 0xFF];
        let response = parse_response(exp_comp_mode_bytes, &ResponseType::ExposureCompensationMode);
        match response {
            Ok(Response::Inquiry(InquiryResponse::ExposureCompensationMode { on })) => {
                assert!(on);
            }
            _ => panic!("Expected ExposureCompensationMode inquiry response"),
        }

        // Test Exposure Compensation Mode Off
        let exp_comp_mode_bytes = &[0x90, 0x50, 0x03, 0xFF];
        let response = parse_response(exp_comp_mode_bytes, &ResponseType::ExposureCompensationMode);
        match response {
            Ok(Response::Inquiry(InquiryResponse::ExposureCompensationMode { on })) => {
                assert!(!on);
            }
            _ => panic!("Expected ExposureCompensationMode inquiry response"),
        }
    }

    #[test]
    fn test_parse_iris_responses() {
        // Test Iris Close (0x00)
        let iris_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
        let response = parse_response(iris_bytes, &ResponseType::Iris);
        match response {
            Ok(Response::Inquiry(InquiryResponse::Iris { position })) => {
                assert_eq!(position, 0x00);
            }
            _ => panic!("Expected Iris inquiry response"),
        }

        // Test Iris F1.8 (0x0C)
        let iris_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x0C, 0xFF];
        let response = parse_response(iris_bytes, &ResponseType::Iris);
        match response {
            Ok(Response::Inquiry(InquiryResponse::Iris { position })) => {
                assert_eq!(position, 0x0C);
            }
            _ => panic!("Expected Iris inquiry response"),
        }
    }

    #[test]
    fn test_parse_shutter_responses() {
        // Test Shutter 1/30 (0x01)
        let shutter_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x01, 0xFF];
        let response = parse_response(shutter_bytes, &ResponseType::Shutter);
        match response {
            Ok(Response::Inquiry(InquiryResponse::Shutter { position })) => {
                assert_eq!(position, 0x01);
            }
            _ => panic!("Expected Shutter inquiry response"),
        }

        // Test Shutter 1/10000 (0x11)
        let shutter_bytes = &[0x90, 0x50, 0x00, 0x00, 0x01, 0x01, 0xFF];
        let response = parse_response(shutter_bytes, &ResponseType::Shutter);
        match response {
            Ok(Response::Inquiry(InquiryResponse::Shutter { position })) => {
                assert_eq!(position, 0x11);
            }
            _ => panic!("Expected Shutter inquiry response"),
        }
    }

    #[test]
    fn test_parse_gain_responses() {
        // Test Gain response
        let gain_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
        let response = parse_response(gain_bytes, &ResponseType::Gain);
        match response {
            Ok(Response::Inquiry(InquiryResponse::GainLevel { gain })) => {
                assert_eq!(gain, 0x07);
            }
            _ => panic!("Expected Gain inquiry response"),
        }

        // Test GainLimit response
        let gain_limit_bytes = &[0x90, 0x50, 0x0F, 0xFF];
        let response = parse_response(gain_limit_bytes, &ResponseType::GainLimit);
        match response {
            Ok(Response::Inquiry(InquiryResponse::GainLimit { limit })) => {
                assert_eq!(limit, 0x0F);
            }
            _ => panic!("Expected GainLimit inquiry response"),
        }
    }

    #[test]
    fn test_parse_image_flip_responses() {
        // Test ImageFlip Off (0x00)
        let flip_bytes = &[0x90, 0x50, 0x00, 0xFF];
        let response = parse_response(flip_bytes, &ResponseType::ImageFlip);
        match response {
            Ok(Response::Inquiry(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            })) => {
                assert!(!vertical);
                assert!(!horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }

        // Test ImageFlip Horizontal only (0x01)
        let flip_bytes = &[0x90, 0x50, 0x01, 0xFF];
        let response = parse_response(flip_bytes, &ResponseType::ImageFlip);
        match response {
            Ok(Response::Inquiry(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            })) => {
                assert!(!vertical);
                assert!(horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }

        // Test ImageFlip Vertical only (0x02)
        let flip_bytes = &[0x90, 0x50, 0x02, 0xFF];
        let response = parse_response(flip_bytes, &ResponseType::ImageFlip);
        match response {
            Ok(Response::Inquiry(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            })) => {
                assert!(vertical);
                assert!(!horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }

        // Test ImageFlip Both (0x03)
        let flip_bytes = &[0x90, 0x50, 0x03, 0xFF];
        let response = parse_response(flip_bytes, &ResponseType::ImageFlip);
        match response {
            Ok(Response::Inquiry(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            })) => {
                assert!(vertical);
                assert!(horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }
    }

    #[test]
    fn test_response_length_validation() {
        // Test various response types with incorrect lengths

        // Sharpness with wrong length (should be 7 bytes)
        let invalid_sharpness = &[0x90, 0x50, 0x0B, 0xFF];
        let response = parse_response(invalid_sharpness, &ResponseType::Sharpness);
        assert!(matches!(response, Err(Error::InvalidResponseLength)));

        // Exposure compensation with wrong length (should be 7 bytes)
        let invalid_exp_comp = &[0x90, 0x50, 0x07, 0xFF];
        let response = parse_response(invalid_exp_comp, &ResponseType::ExposureCompensation);
        assert!(matches!(response, Err(Error::InvalidResponseLength)));
    }
}
