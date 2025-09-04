//! VISCA response parsing and handling.
//!
//! This module provides response parsing functionality for VISCA protocol responses,
//! including ACK/completion messages, error responses, and inquiry data parsing.

mod decoders;
mod lift;
mod nibbles;
pub mod types;

use std::borrow::Cow;

use crate::{
    command::{image::SharpnessMode, AutoWhiteBalanceSensitivity, InquiryResponse},
    error::Error,
};

// Import from local modules
use self::nibbles::{combine_nibbles_u16, combine_nibbles_u8};

// Re-export key types and functions
pub use self::{
    lift::{lift_inquiry, parse_inquiry_payload},
    types::{ViscaResponse, ViscaResponseType},
};

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

/// Parse digital Ptz mode
pub fn parse_digital_ptz(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    let enabled = match data[0] {
        0x02 => false, // Digital Ptz off
        0x03 => true,  // Digital Ptz on
        _ => {
            return Err(Error::InvalidParameter {
                parameter: "digital_ptz",
                value: Cow::Owned(format!("{:02X}", data[0])),
                reason: Cow::Borrowed(
                    "Invalid digital Ptz value. Expected 0x02 (off) or 0x03 (on)",
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
    use crate::command::bytes::VISCA_TERMINATOR;

    #[test]
    fn test_response_debug() {
        let ack = ViscaResponse::CmdAck { socket: None };
        assert_eq!(format!("{ack:?}"), "CmdAck { socket: None }");

        let completion = ViscaResponse::Completion { socket: None };
        assert_eq!(format!("{completion:?}"), "Completion { socket: None }");
    }

    #[test]
    fn test_response_type_equality() {
        assert_eq!(ViscaResponseType::Power, ViscaResponseType::Power);
        assert_ne!(ViscaResponseType::Power, ViscaResponseType::ZoomPosition);
    }

    #[test]
    fn test_basic_ack_parsing() {
        let response = vec![0x90, 0x41, VISCA_TERMINATOR];
        let result = ViscaResponse::parse_with_type(&response, &ViscaResponseType::Power).unwrap();
        assert!(matches!(result, ViscaResponse::CmdAck { .. }));
    }

    #[test]
    fn test_basic_completion_parsing() {
        let response = vec![0x90, 0x51, VISCA_TERMINATOR];
        let result = ViscaResponse::parse_with_type(&response, &ViscaResponseType::Power).unwrap();
        assert!(matches!(result, ViscaResponse::Completion { .. }));
    }

    #[test]
    fn test_basic_error_parsing() {
        let response = vec![0x90, 0x60, 0x02, VISCA_TERMINATOR];
        let result = ViscaResponse::parse_with_type(&response, &ViscaResponseType::Power);
        // Error responses are wrapped in Ok(ViscaResponse::Error) for parse_with_type
        assert!(matches!(result, Ok(ViscaResponse::Error(_))));
    }

    #[test]
    fn test_invalid_format() {
        // Empty response
        let response = vec![];
        let result = ViscaResponse::parse_with_type(&response, &ViscaResponseType::Power);
        assert!(result.is_err());

        // Too short
        let response = vec![0x90, VISCA_TERMINATOR];
        let result = ViscaResponse::parse_with_type(&response, &ViscaResponseType::Power);
        assert!(result.is_err());
    }

    #[test]
    fn test_simple_power_response() {
        // Power On
        let response = vec![0x90, 0x50, 0x02, VISCA_TERMINATOR];
        let result = ViscaResponse::parse_with_type(&response, &ViscaResponseType::Power).unwrap();
        match result {
            ViscaResponse::Inquiry(InquiryResponse::Power { on }) => assert!(on),
            _ => panic!("Expected Power inquiry response"),
        }

        // Power Off
        let response = vec![0x90, 0x50, 0x03, VISCA_TERMINATOR];
        let result = ViscaResponse::parse_with_type(&response, &ViscaResponseType::Power).unwrap();
        match result {
            ViscaResponse::Inquiry(InquiryResponse::Power { on }) => assert!(!on),
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
        let ack_bytes = &[0x90, 0x40, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(ack_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(matches!(response, Ok(ViscaResponse::CmdAck { .. })));

        // ACK for socket 1
        let ack_bytes = &[0x90, 0x41, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(ack_bytes, &ViscaResponseType::ZoomPosition);
        assert!(matches!(response, Ok(ViscaResponse::CmdAck { .. })));
    }

    #[test]
    fn test_parse_completion_response() {
        // Completion for socket 0
        let completion_bytes = &[0x90, 0x50, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(completion_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(matches!(response, Ok(ViscaResponse::Completion { .. })));

        // Completion for socket 1
        let completion_bytes = &[0x90, 0x51, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(completion_bytes, &ViscaResponseType::ZoomPosition);
        assert!(matches!(response, Ok(ViscaResponse::Completion { .. })));
    }

    #[test]
    fn test_parse_error_responses() {
        // Test Syntax Error
        let error_bytes = &[0x90, 0x60, 0x02, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(error_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(matches!(
            response,
            Ok(ViscaResponse::Error(Error::SyntaxError))
        ));

        // Test Command Buffer Full
        let error_bytes = &[0x90, 0x60, 0x03, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(error_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(matches!(
            response,
            Ok(ViscaResponse::Error(Error::CommandBufferFull))
        ));

        // Test Command Not Executable (0x41 - command invalid in current state)
        let error_bytes = &[0x90, 0x61, 0x41, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(error_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(matches!(
            response,
            Ok(ViscaResponse::Error(Error::CommandNotExecutable))
        ));
    }

    #[test]
    fn test_parse_pan_tilt_position_response() {
        let pt_response_bytes = &[
            0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0xFF,
        ];
        let response =
            ViscaResponse::parse_with_type(pt_response_bytes, &ViscaResponseType::PanTiltPosition);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::PanTiltPosition { pan, tilt })) => {
                assert_eq!(pan, 0x1234);
                assert_eq!(tilt, 0x5678);
            }
            _ => panic!("Expected PanTiltPosition inquiry response"),
        }
    }

    #[test]
    fn test_parse_zoom_position_response() {
        let zoom_response_bytes = &[0x90, 0x50, 0x0A, 0x0B, 0x0C, 0x0D, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(zoom_response_bytes, &ViscaResponseType::ZoomPosition);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::ZoomPosition { position })) => {
                assert_eq!(position, 0xABCD);
            }
            _ => panic!("Expected ZoomPosition inquiry response"),
        }
    }

    #[test]
    fn test_parse_focus_position_response() {
        let focus_response_bytes = &[0x90, 0x50, 0x01, 0x02, 0x03, 0x04, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(focus_response_bytes, &ViscaResponseType::FocusPosition);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::FocusPosition { position })) => {
                assert_eq!(position, 0x1234);
            }
            _ => panic!("Expected FocusPosition inquiry response"),
        }
    }

    #[test]
    fn test_parse_exposure_mode_response() {
        // Auto exposure mode
        let exposure_response_bytes = &[0x90, 0x50, 0x00, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(
            exposure_response_bytes,
            &ViscaResponseType::ExposureMode,
        );
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::ExposureMode { mode })) => {
                assert_eq!(mode as u8, 0x00); // Auto mode
            }
            _ => panic!("Expected ExposureMode inquiry response"),
        }

        // Manual exposure mode
        let exposure_response_bytes = &[0x90, 0x50, 0x03, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(
            exposure_response_bytes,
            &ViscaResponseType::ExposureMode,
        );
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::ExposureMode { mode })) => {
                assert_eq!(mode as u8, 0x03); // Manual mode
            }
            _ => panic!("Expected ExposureMode inquiry response"),
        }
    }

    #[test]
    fn test_parse_luminance_response() {
        // Test minimum luminance value (0)
        let luminance_response_bytes = &[0x90, 0x50, 0x00, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(luminance_response_bytes, &ViscaResponseType::Luminance);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::Luminance(value))) => {
                assert_eq!(value, 0x00);
            }
            _ => panic!("Expected Luminance inquiry response"),
        }

        // Test middle luminance value (7)
        let luminance_response_bytes = &[0x90, 0x50, 0x07, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(luminance_response_bytes, &ViscaResponseType::Luminance);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::Luminance(value))) => {
                assert_eq!(value, 0x07);
            }
            _ => panic!("Expected Luminance inquiry response"),
        }

        // Test maximum luminance value (14)
        let luminance_response_bytes = &[0x90, 0x50, 0x0E, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(luminance_response_bytes, &ViscaResponseType::Luminance);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::Luminance(value))) => {
                assert_eq!(value, 0x0E);
            }
            _ => panic!("Expected Luminance inquiry response"),
        }
    }

    #[test]
    fn test_parse_contrast_response() {
        // Test minimum contrast value (0)
        let contrast_response_bytes = &[0x90, 0x50, 0x00, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(contrast_response_bytes, &ViscaResponseType::Contrast);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::Contrast(value))) => {
                assert_eq!(value, 0x00);
            }
            _ => panic!("Expected Contrast inquiry response"),
        }

        // Test middle contrast value (7)
        let contrast_response_bytes = &[0x90, 0x50, 0x07, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(contrast_response_bytes, &ViscaResponseType::Contrast);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::Contrast(value))) => {
                assert_eq!(value, 0x07);
            }
            _ => panic!("Expected Contrast inquiry response"),
        }

        // Test maximum contrast value (14)
        let contrast_response_bytes = &[0x90, 0x50, 0x0E, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(contrast_response_bytes, &ViscaResponseType::Contrast);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::Contrast(value))) => {
                assert_eq!(value, 0x0E);
            }
            _ => panic!("Expected Contrast inquiry response"),
        }
    }

    #[test]
    fn test_parse_invalid_response() {
        // Test response that doesn't start with 0x90
        let invalid_bytes = &[0x80, 0x50, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(invalid_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(response.is_err());

        // Test response that doesn't end with 0xFF
        let invalid_bytes = &[0x90, 0x50, 0x00];
        let response =
            ViscaResponse::parse_with_type(invalid_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(response.is_err());

        // Test empty response
        let invalid_bytes = &[];
        let response =
            ViscaResponse::parse_with_type(invalid_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(response.is_err());
    }

    #[test]
    fn test_parse_response_with_wrong_type() {
        // Try to parse an ACK as a data response
        let ack_bytes = &[0x90, 0x40, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(ack_bytes, &ViscaResponseType::PanTiltPosition);
        // ACK is still recognized regardless of expected response type
        assert!(matches!(response, Ok(ViscaResponse::CmdAck { .. })));
    }

    #[test]
    fn test_parse_sharpness_response() {
        // Test Sharpness response
        let sharpness_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x0B, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(sharpness_bytes, &ViscaResponseType::Sharpness);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::Sharpness { value })) => {
                assert_eq!(value, 0x0B);
            }
            _ => panic!("Expected Sharpness inquiry response"),
        }
    }

    #[test]
    fn test_parse_exposure_compensation_responses() {
        // Test Exposure Compensation value -7
        let exp_comp_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x00, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(
            exp_comp_bytes,
            &ViscaResponseType::ExposureCompensation,
        );
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::ExposureCompensation { value })) => {
                assert_eq!(value, -7);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation value 0
        let exp_comp_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x07, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(
            exp_comp_bytes,
            &ViscaResponseType::ExposureCompensation,
        );
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::ExposureCompensation { value })) => {
                assert_eq!(value, 0);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation value +7
        let exp_comp_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x0E, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(
            exp_comp_bytes,
            &ViscaResponseType::ExposureCompensation,
        );
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::ExposureCompensation { value })) => {
                assert_eq!(value, 7);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation Mode On
        let exp_comp_mode_bytes = &[0x90, 0x50, 0x02, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(
            exp_comp_mode_bytes,
            &ViscaResponseType::ExposureCompensationMode,
        );
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::ExposureCompensationMode { on })) => {
                assert!(on);
            }
            _ => panic!("Expected ExposureCompensationMode inquiry response"),
        }

        // Test Exposure Compensation Mode Off
        let exp_comp_mode_bytes = &[0x90, 0x50, 0x03, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(
            exp_comp_mode_bytes,
            &ViscaResponseType::ExposureCompensationMode,
        );
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::ExposureCompensationMode { on })) => {
                assert!(!on);
            }
            _ => panic!("Expected ExposureCompensationMode inquiry response"),
        }
    }

    #[test]
    fn test_parse_iris_responses() {
        // Test Iris Close (0x00)
        let iris_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x00, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(iris_bytes, &ViscaResponseType::Iris);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::Iris { position })) => {
                assert_eq!(position, 0x00);
            }
            _ => panic!("Expected Iris inquiry response"),
        }

        // Test Iris F1.8 (0x0C)
        let iris_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x0C, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(iris_bytes, &ViscaResponseType::Iris);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::Iris { position })) => {
                assert_eq!(position, 0x0C);
            }
            _ => panic!("Expected Iris inquiry response"),
        }
    }

    #[test]
    fn test_parse_shutter_responses() {
        // Test Shutter 1/30 (0x01)
        let shutter_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x01, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(shutter_bytes, &ViscaResponseType::Shutter);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::Shutter { position })) => {
                assert_eq!(position, 0x01);
            }
            _ => panic!("Expected Shutter inquiry response"),
        }

        // Test Shutter 1/10000 (0x11)
        let shutter_bytes = &[0x90, 0x50, 0x00, 0x00, 0x01, 0x01, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(shutter_bytes, &ViscaResponseType::Shutter);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::Shutter { position })) => {
                assert_eq!(position, 0x11);
            }
            _ => panic!("Expected Shutter inquiry response"),
        }
    }

    #[test]
    fn test_parse_gain_responses() {
        // Test Gain response
        let gain_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x07, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(gain_bytes, &ViscaResponseType::Gain);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::GainLevel { gain })) => {
                assert_eq!(gain, 0x07);
            }
            _ => panic!("Expected Gain inquiry response"),
        }

        // Test GainLimit response
        let gain_limit_bytes = &[0x90, 0x50, 0x0F, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(gain_limit_bytes, &ViscaResponseType::GainLimit);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::GainLimit { limit })) => {
                assert_eq!(limit, 0x0F);
            }
            _ => panic!("Expected GainLimit inquiry response"),
        }
    }

    #[test]
    fn test_parse_image_flip_responses() {
        // Test ImageFlip Off (0x00)
        let flip_bytes = &[0x90, 0x50, 0x00, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(flip_bytes, &ViscaResponseType::ImageFlip);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            })) => {
                assert!(!vertical);
                assert!(!horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }

        // Test ImageFlip Horizontal only (0x01)
        let flip_bytes = &[0x90, 0x50, 0x01, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(flip_bytes, &ViscaResponseType::ImageFlip);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            })) => {
                assert!(!vertical);
                assert!(horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }

        // Test ImageFlip Vertical only (0x02)
        let flip_bytes = &[0x90, 0x50, 0x02, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(flip_bytes, &ViscaResponseType::ImageFlip);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            })) => {
                assert!(vertical);
                assert!(!horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }

        // Test ImageFlip Both (0x03)
        let flip_bytes = &[0x90, 0x50, 0x03, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(flip_bytes, &ViscaResponseType::ImageFlip);
        match response {
            Ok(ViscaResponse::Inquiry(InquiryResponse::ImageFlip {
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
        let invalid_sharpness = &[0x90, 0x50, 0x0B, VISCA_TERMINATOR];
        let response =
            ViscaResponse::parse_with_type(invalid_sharpness, &ViscaResponseType::Sharpness);
        assert!(matches!(response, Err(Error::InvalidResponseLength)));

        // Exposure compensation with wrong length (should be 7 bytes)
        let invalid_exp_comp = &[0x90, 0x50, 0x07, VISCA_TERMINATOR];
        let response = ViscaResponse::parse_with_type(
            invalid_exp_comp,
            &ViscaResponseType::ExposureCompensation,
        );
        assert!(matches!(response, Err(Error::InvalidResponseLength)));
    }
}
