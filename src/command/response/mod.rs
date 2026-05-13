//! VISCA response parsing and handling.
//!
//! This module provides response parsing functionality for VISCA protocol responses,
//! including ACK/completion messages, error responses, and inquiry data parsing.

mod decoders;
mod lift;
mod payload;
mod types;

#[cfg(test)]
pub(crate) use self::lift::lift_inquiry;
pub(crate) use self::lift::lift_inquiry_for;
pub use self::{
    lift::parse_inquiry_payload,
    payload::{BoolConvention, Nibbles, Nibbles4Or8, Payload},
    types::Response,
};

// Re-export InquiryKind from the registry for crate-internal response code.
pub use crate::command::inquiry_registry::InquiryKind;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::command::InquiryData;
    use crate::error::Error;

    #[test]
    fn test_response_debug() {
        let ack = Response::CmdAck { socket: None };
        assert_eq!(format!("{ack:?}"), "CmdAck { socket: None }");

        let completion = Response::Completion { socket: None };
        assert_eq!(format!("{completion:?}"), "Completion { socket: None }");
    }

    #[test]
    fn test_response_type_equality() {
        assert_eq!(InquiryKind::Power, InquiryKind::Power);
        assert_ne!(InquiryKind::Power, InquiryKind::ZoomPosition);
    }

    #[test]
    fn test_basic_ack_parsing() {
        let response = vec![0x90, 0x41, VISCA_TERMINATOR];
        let result = Response::parse_with_type(&response, &InquiryKind::Power).unwrap();
        assert!(matches!(result, Response::CmdAck { .. }));
    }

    #[test]
    fn test_basic_completion_parsing() {
        let response = vec![0x90, 0x51, VISCA_TERMINATOR];
        let result = Response::parse_with_type(&response, &InquiryKind::Power).unwrap();
        assert!(matches!(result, Response::Completion { .. }));
    }

    #[test]
    fn test_basic_error_parsing() {
        let response = vec![0x90, 0x60, 0x02, VISCA_TERMINATOR];
        let result = Response::parse_with_type(&response, &InquiryKind::Power);
        // Error responses are wrapped in Ok(Response::Error) for parse_with_type
        assert!(matches!(result, Ok(Response::Error(_))));
    }

    #[test]
    fn test_invalid_format() {
        // Empty response
        let response = vec![];
        let result = Response::parse_with_type(&response, &InquiryKind::Power);
        assert!(result.is_err());

        // Too short
        let response = vec![0x90, VISCA_TERMINATOR];
        let result = Response::parse_with_type(&response, &InquiryKind::Power);
        assert!(result.is_err());
    }

    #[test]
    fn test_simple_power_response() {
        // Power On
        let response = vec![0x90, 0x50, 0x02, VISCA_TERMINATOR];
        let result = Response::parse_with_type(&response, &InquiryKind::Power).unwrap();
        match result {
            Response::Inquiry(InquiryData::Power { on }) => assert!(on),
            _ => panic!("Expected Power inquiry response"),
        }

        // Power Off
        let response = vec![0x90, 0x50, 0x03, VISCA_TERMINATOR];
        let result = Response::parse_with_type(&response, &InquiryKind::Power).unwrap();
        match result {
            Response::Inquiry(InquiryData::Power { on }) => assert!(!on),
            _ => panic!("Expected Power inquiry response"),
        }
    }

    #[test]
    fn test_parse_ack_response() {
        // ACK for socket 0
        let ack_bytes = &[0x90, 0x40, VISCA_TERMINATOR];
        let response = Response::parse_with_type(ack_bytes, &InquiryKind::PanTiltPosition);
        assert!(matches!(response, Ok(Response::CmdAck { .. })));

        // ACK for socket 1
        let ack_bytes = &[0x90, 0x41, VISCA_TERMINATOR];
        let response = Response::parse_with_type(ack_bytes, &InquiryKind::ZoomPosition);
        assert!(matches!(response, Ok(Response::CmdAck { .. })));
    }

    #[test]
    fn test_parse_completion_response() {
        // Completion for socket 0
        let completion_bytes = &[0x90, 0x50, VISCA_TERMINATOR];
        let response = Response::parse_with_type(completion_bytes, &InquiryKind::PanTiltPosition);
        assert!(matches!(response, Ok(Response::Completion { .. })));

        // Completion for socket 1
        let completion_bytes = &[0x90, 0x51, VISCA_TERMINATOR];
        let response = Response::parse_with_type(completion_bytes, &InquiryKind::ZoomPosition);
        assert!(matches!(response, Ok(Response::Completion { .. })));
    }

    #[test]
    fn test_parse_error_responses() {
        // Test Syntax Error
        let error_bytes = &[0x90, 0x60, 0x02, VISCA_TERMINATOR];
        let response = Response::parse_with_type(error_bytes, &InquiryKind::PanTiltPosition);
        assert!(matches!(response, Ok(Response::Error(Error::SyntaxError))));

        // Test Command Buffer Full
        let error_bytes = &[0x90, 0x60, 0x03, VISCA_TERMINATOR];
        let response = Response::parse_with_type(error_bytes, &InquiryKind::PanTiltPosition);
        assert!(matches!(
            response,
            Ok(Response::Error(Error::CommandBufferFull))
        ));

        // Test Command Not Executable (0x41 - command invalid in current state)
        let error_bytes = &[0x90, 0x61, 0x41, VISCA_TERMINATOR];
        let response = Response::parse_with_type(error_bytes, &InquiryKind::PanTiltPosition);
        assert!(matches!(
            response,
            Ok(Response::Error(Error::CommandNotExecutable))
        ));
    }

    #[test]
    fn test_parse_pan_tilt_position_response() {
        let pt_response_bytes = &[
            0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0xFF,
        ];
        let response = Response::parse_with_type(pt_response_bytes, &InquiryKind::PanTiltPosition);
        match response {
            Ok(Response::Inquiry(InquiryData::PanTiltPosition { pan, tilt })) => {
                assert_eq!(pan, 0x1234);
                assert_eq!(tilt, 0x5678);
            }
            _ => panic!("Expected PanTiltPosition inquiry response"),
        }
    }

    #[test]
    fn test_parse_zoom_position_response() {
        let zoom_response_bytes = &[0x90, 0x50, 0x0A, 0x0B, 0x0C, 0x0D, VISCA_TERMINATOR];
        let response = Response::parse_with_type(zoom_response_bytes, &InquiryKind::ZoomPosition);
        match response {
            Ok(Response::Inquiry(InquiryData::ZoomPosition { position })) => {
                assert_eq!(position, 0xABCD);
            }
            _ => panic!("Expected ZoomPosition inquiry response"),
        }
    }

    #[test]
    fn test_parse_focus_position_response() {
        let focus_response_bytes = &[0x90, 0x50, 0x01, 0x02, 0x03, 0x04, VISCA_TERMINATOR];
        let response = Response::parse_with_type(focus_response_bytes, &InquiryKind::FocusPosition);
        match response {
            Ok(Response::Inquiry(InquiryData::FocusPosition { position })) => {
                assert_eq!(position, 0x1234);
            }
            _ => panic!("Expected FocusPosition inquiry response"),
        }
    }

    #[test]
    fn test_parse_exposure_mode_response() {
        // Auto exposure mode
        let exposure_response_bytes = &[0x90, 0x50, 0x00, VISCA_TERMINATOR];
        let response =
            Response::parse_with_type(exposure_response_bytes, &InquiryKind::ExposureMode);
        match response {
            Ok(Response::Inquiry(InquiryData::ExposureMode { mode })) => {
                assert_eq!(mode as u8, 0x00); // Auto mode
            }
            _ => panic!("Expected ExposureMode inquiry response"),
        }

        // Manual exposure mode
        let exposure_response_bytes = &[0x90, 0x50, 0x03, VISCA_TERMINATOR];
        let response =
            Response::parse_with_type(exposure_response_bytes, &InquiryKind::ExposureMode);
        match response {
            Ok(Response::Inquiry(InquiryData::ExposureMode { mode })) => {
                assert_eq!(mode as u8, 0x03); // Manual mode
            }
            _ => panic!("Expected ExposureMode inquiry response"),
        }
    }

    #[test]
    fn test_parse_luminance_response() {
        // Test minimum luminance value (0)
        // Response format: y0 50 00 00 0p 0q FF where pq = position
        let luminance_response_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x00, VISCA_TERMINATOR];
        let response = Response::parse_with_type(luminance_response_bytes, &InquiryKind::Luminance);
        match response {
            Ok(Response::Inquiry(InquiryData::Luminance { level })) => {
                assert_eq!(level, 0x00);
            }
            _ => panic!("Expected Luminance inquiry response"),
        }

        // Test middle luminance value (7)
        let luminance_response_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x07, VISCA_TERMINATOR];
        let response = Response::parse_with_type(luminance_response_bytes, &InquiryKind::Luminance);
        match response {
            Ok(Response::Inquiry(InquiryData::Luminance { level })) => {
                assert_eq!(level, 0x07);
            }
            _ => panic!("Expected Luminance inquiry response"),
        }

        // Test maximum luminance value (14)
        let luminance_response_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x0E, VISCA_TERMINATOR];
        let response = Response::parse_with_type(luminance_response_bytes, &InquiryKind::Luminance);
        match response {
            Ok(Response::Inquiry(InquiryData::Luminance { level })) => {
                assert_eq!(level, 0x0E);
            }
            _ => panic!("Expected Luminance inquiry response"),
        }
    }

    #[test]
    fn test_parse_contrast_response() {
        // Test minimum contrast value (0)
        // Response format: y0 50 00 00 0p 0q FF where pq = position
        let contrast_response_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x00, VISCA_TERMINATOR];
        let response = Response::parse_with_type(contrast_response_bytes, &InquiryKind::Contrast);
        match response {
            Ok(Response::Inquiry(InquiryData::Contrast { level })) => {
                assert_eq!(level, 0x00);
            }
            _ => panic!("Expected Contrast inquiry response"),
        }

        // Test middle contrast value (7)
        let contrast_response_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x07, VISCA_TERMINATOR];
        let response = Response::parse_with_type(contrast_response_bytes, &InquiryKind::Contrast);
        match response {
            Ok(Response::Inquiry(InquiryData::Contrast { level })) => {
                assert_eq!(level, 0x07);
            }
            _ => panic!("Expected Contrast inquiry response"),
        }

        // Test maximum contrast value (14)
        let contrast_response_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x0E, VISCA_TERMINATOR];
        let response = Response::parse_with_type(contrast_response_bytes, &InquiryKind::Contrast);
        match response {
            Ok(Response::Inquiry(InquiryData::Contrast { level })) => {
                assert_eq!(level, 0x0E);
            }
            _ => panic!("Expected Contrast inquiry response"),
        }
    }

    #[test]
    fn test_parse_invalid_response() {
        // Test response that doesn't start with 0x90
        let invalid_bytes = &[0x80, 0x50, VISCA_TERMINATOR];
        let response = Response::parse_with_type(invalid_bytes, &InquiryKind::PanTiltPosition);
        assert!(response.is_err());

        // Test response that doesn't end with 0xFF
        let invalid_bytes = &[0x90, 0x50, 0x00];
        let response = Response::parse_with_type(invalid_bytes, &InquiryKind::PanTiltPosition);
        assert!(response.is_err());

        // Test empty response
        let invalid_bytes = &[];
        let response = Response::parse_with_type(invalid_bytes, &InquiryKind::PanTiltPosition);
        assert!(response.is_err());
    }

    #[test]
    fn test_parse_response_with_wrong_type() {
        // Try to parse an ACK as a data response
        let ack_bytes = &[0x90, 0x40, VISCA_TERMINATOR];
        let response = Response::parse_with_type(ack_bytes, &InquiryKind::PanTiltPosition);
        // ACK is still recognized regardless of expected response type
        assert!(matches!(response, Ok(Response::CmdAck { .. })));
    }

    #[test]
    fn test_parse_sharpness_response() {
        // Test Sharpness response
        let sharpness_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x0B, VISCA_TERMINATOR];
        let response = Response::parse_with_type(sharpness_bytes, &InquiryKind::Sharpness);
        match response {
            Ok(Response::Inquiry(InquiryData::Sharpness { value })) => {
                assert_eq!(value, 0x0B);
            }
            _ => panic!("Expected Sharpness inquiry response"),
        }
    }

    #[test]
    fn test_parse_sharpness_position_response() {
        // Test SharpnessPosition response with payload 00 00 00 03 (position = 0x0003)
        let sharpness_pos_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x03, VISCA_TERMINATOR];
        let response =
            Response::parse_with_type(sharpness_pos_bytes, &InquiryKind::SharpnessPosition);
        match response {
            Ok(Response::Inquiry(InquiryData::SharpnessPosition { position })) => {
                assert_eq!(position, 0x0003);
            }
            _ => panic!("Expected SharpnessPosition inquiry response"),
        }

        // Test SharpnessPosition response with larger value (e.g., 0x1234)
        let sharpness_pos_bytes = &[0x90, 0x50, 0x01, 0x02, 0x03, 0x04, VISCA_TERMINATOR];
        let response =
            Response::parse_with_type(sharpness_pos_bytes, &InquiryKind::SharpnessPosition);
        match response {
            Ok(Response::Inquiry(InquiryData::SharpnessPosition { position })) => {
                assert_eq!(position, 0x1234);
            }
            _ => panic!("Expected SharpnessPosition inquiry response"),
        }
    }

    #[test]
    fn test_parse_exposure_compensation_responses() {
        // Test Exposure Compensation value -7
        let exp_comp_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x00, VISCA_TERMINATOR];
        let response =
            Response::parse_with_type(exp_comp_bytes, &InquiryKind::ExposureCompensation);
        match response {
            Ok(Response::Inquiry(InquiryData::ExposureCompensation { value })) => {
                assert_eq!(value, -7);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation value 0
        let exp_comp_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x07, VISCA_TERMINATOR];
        let response =
            Response::parse_with_type(exp_comp_bytes, &InquiryKind::ExposureCompensation);
        match response {
            Ok(Response::Inquiry(InquiryData::ExposureCompensation { value })) => {
                assert_eq!(value, 0);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation value +7
        let exp_comp_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x0E, VISCA_TERMINATOR];
        let response =
            Response::parse_with_type(exp_comp_bytes, &InquiryKind::ExposureCompensation);
        match response {
            Ok(Response::Inquiry(InquiryData::ExposureCompensation { value })) => {
                assert_eq!(value, 7);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation Mode On
        let exp_comp_mode_bytes = &[0x90, 0x50, 0x02, VISCA_TERMINATOR];
        let response =
            Response::parse_with_type(exp_comp_mode_bytes, &InquiryKind::ExposureCompensationMode);
        match response {
            Ok(Response::Inquiry(InquiryData::ExposureCompensationMode { on })) => {
                assert!(on);
            }
            _ => panic!("Expected ExposureCompensationMode inquiry response"),
        }

        // Test Exposure Compensation Mode Off
        let exp_comp_mode_bytes = &[0x90, 0x50, 0x03, VISCA_TERMINATOR];
        let response =
            Response::parse_with_type(exp_comp_mode_bytes, &InquiryKind::ExposureCompensationMode);
        match response {
            Ok(Response::Inquiry(InquiryData::ExposureCompensationMode { on })) => {
                assert!(!on);
            }
            _ => panic!("Expected ExposureCompensationMode inquiry response"),
        }
    }

    #[test]
    fn test_parse_iris_responses() {
        // Test Iris Close (0x00)
        let iris_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x00, VISCA_TERMINATOR];
        let response = Response::parse_with_type(iris_bytes, &InquiryKind::Iris);
        match response {
            Ok(Response::Inquiry(InquiryData::Iris { position })) => {
                assert_eq!(position, 0x00);
            }
            _ => panic!("Expected Iris inquiry response"),
        }

        // Test Iris F1.8 (0x0C)
        let iris_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x0C, VISCA_TERMINATOR];
        let response = Response::parse_with_type(iris_bytes, &InquiryKind::Iris);
        match response {
            Ok(Response::Inquiry(InquiryData::Iris { position })) => {
                assert_eq!(position, 0x0C);
            }
            _ => panic!("Expected Iris inquiry response"),
        }
    }

    #[test]
    fn test_parse_shutter_responses() {
        // Test Shutter 1/30 (0x01)
        let shutter_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x01, VISCA_TERMINATOR];
        let response = Response::parse_with_type(shutter_bytes, &InquiryKind::Shutter);
        match response {
            Ok(Response::Inquiry(InquiryData::Shutter { position })) => {
                assert_eq!(position, 0x01);
            }
            _ => panic!("Expected Shutter inquiry response"),
        }

        // Test Shutter 1/10000 (0x11)
        let shutter_bytes = &[0x90, 0x50, 0x00, 0x00, 0x01, 0x01, VISCA_TERMINATOR];
        let response = Response::parse_with_type(shutter_bytes, &InquiryKind::Shutter);
        match response {
            Ok(Response::Inquiry(InquiryData::Shutter { position })) => {
                assert_eq!(position, 0x11);
            }
            _ => panic!("Expected Shutter inquiry response"),
        }
    }

    #[test]
    fn test_parse_gain_responses() {
        // Test Gain response
        let gain_bytes = &[0x90, 0x50, 0x00, 0x00, 0x00, 0x07, VISCA_TERMINATOR];
        let response = Response::parse_with_type(gain_bytes, &InquiryKind::Gain);
        match response {
            Ok(Response::Inquiry(InquiryData::Gain { gain })) => {
                assert_eq!(gain, 0x07);
            }
            _ => panic!("Expected Gain inquiry response"),
        }

        // Test GainLimit response
        let gain_limit_bytes = &[0x90, 0x50, 0x0F, VISCA_TERMINATOR];
        let response = Response::parse_with_type(gain_limit_bytes, &InquiryKind::GainLimit);
        match response {
            Ok(Response::Inquiry(InquiryData::GainLimit { limit })) => {
                assert_eq!(limit, 0x0F);
            }
            _ => panic!("Expected GainLimit inquiry response"),
        }
    }

    #[test]
    fn test_parse_image_flip_responses() {
        // Test ImageFlip Off (0x00)
        let flip_bytes = &[0x90, 0x50, 0x00, VISCA_TERMINATOR];
        let response = Response::parse_with_type(flip_bytes, &InquiryKind::FlipState);
        match response {
            Ok(Response::Inquiry(InquiryData::FlipState {
                vertical,
                horizontal,
            })) => {
                assert!(!vertical);
                assert!(!horizontal);
            }
            _ => panic!("Expected FlipState inquiry response"),
        }

        // Test ImageFlip Horizontal only (0x01)
        let flip_bytes = &[0x90, 0x50, 0x01, VISCA_TERMINATOR];
        let response = Response::parse_with_type(flip_bytes, &InquiryKind::FlipState);
        match response {
            Ok(Response::Inquiry(InquiryData::FlipState {
                vertical,
                horizontal,
            })) => {
                assert!(!vertical);
                assert!(horizontal);
            }
            _ => panic!("Expected FlipState inquiry response"),
        }

        // Test ImageFlip Vertical only (0x02)
        let flip_bytes = &[0x90, 0x50, 0x02, VISCA_TERMINATOR];
        let response = Response::parse_with_type(flip_bytes, &InquiryKind::FlipState);
        match response {
            Ok(Response::Inquiry(InquiryData::FlipState {
                vertical,
                horizontal,
            })) => {
                assert!(vertical);
                assert!(!horizontal);
            }
            _ => panic!("Expected FlipState inquiry response"),
        }

        // Test ImageFlip Both (0x03)
        let flip_bytes = &[0x90, 0x50, 0x03, VISCA_TERMINATOR];
        let response = Response::parse_with_type(flip_bytes, &InquiryKind::FlipState);
        match response {
            Ok(Response::Inquiry(InquiryData::FlipState {
                vertical,
                horizontal,
            })) => {
                assert!(vertical);
                assert!(horizontal);
            }
            _ => panic!("Expected FlipState inquiry response"),
        }
    }

    #[test]
    fn test_response_length_validation() {
        // Test various response types with incorrect lengths

        // Sharpness with wrong length (should be 7 bytes)
        let invalid_sharpness = &[0x90, 0x50, 0x0B, VISCA_TERMINATOR];
        let response = Response::parse_with_type(invalid_sharpness, &InquiryKind::Sharpness);
        assert!(matches!(response, Err(Error::InvalidResponseLength { .. })));

        // Exposure compensation with wrong length (should be 7 bytes)
        let invalid_exp_comp = &[0x90, 0x50, 0x07, VISCA_TERMINATOR];
        let response =
            Response::parse_with_type(invalid_exp_comp, &InquiryKind::ExposureCompensation);
        assert!(matches!(response, Err(Error::InvalidResponseLength { .. })));
    }

    #[test]
    fn test_parse_defog_mode_response() {
        // Test DefogMode off (0x02)
        let response_bytes = &[0x90, 0x50, 0x02, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::DefogMode);
        match response {
            Ok(Response::Inquiry(InquiryData::DefogMode { enabled })) => {
                assert!(!enabled);
            }
            _ => panic!("Expected DefogMode inquiry response"),
        }

        // Test DefogMode on (0x03)
        let response_bytes = &[0x90, 0x50, 0x03, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::DefogMode);
        match response {
            Ok(Response::Inquiry(InquiryData::DefogMode { enabled })) => {
                assert!(enabled);
            }
            _ => panic!("Expected DefogMode inquiry response"),
        }
    }

    #[test]
    fn test_parse_defog_level_response() {
        // Test DefogLevel 0
        let response_bytes = &[0x90, 0x50, 0x00, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::DefogLevel);
        assert!(matches!(
            response,
            Ok(Response::Inquiry(InquiryData::DefogLevel { .. }))
        ));

        // Test DefogLevel 5 (max)
        let response_bytes = &[0x90, 0x50, 0x05, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::DefogLevel);
        assert!(matches!(
            response,
            Ok(Response::Inquiry(InquiryData::DefogLevel { .. }))
        ));

        // Test DefogLevel out of range (6) - should fail
        let response_bytes = &[0x90, 0x50, 0x06, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::DefogLevel);
        assert!(matches!(response, Err(Error::InvalidParameter { .. })));
    }

    #[test]
    fn test_parse_digital_ptz_response() {
        // Test DigitalPtz off (0x02)
        let response_bytes = &[0x90, 0x50, 0x02, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::DigitalPtz);
        match response {
            Ok(Response::Inquiry(InquiryData::DigitalPtz { enabled })) => {
                assert!(!enabled);
            }
            _ => panic!("Expected DigitalPtz inquiry response"),
        }

        // Test DigitalPtz on (0x03)
        let response_bytes = &[0x90, 0x50, 0x03, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::DigitalPtz);
        match response {
            Ok(Response::Inquiry(InquiryData::DigitalPtz { enabled })) => {
                assert!(enabled);
            }
            _ => panic!("Expected DigitalPtz inquiry response"),
        }
    }

    #[test]
    fn test_parse_broadcast_domain_response() {
        // Test BroadcastDomain 0
        let response_bytes = &[0x90, 0x50, 0x00, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::BroadcastDomain);
        assert!(matches!(
            response,
            Ok(Response::Inquiry(InquiryData::BroadcastDomain(_)))
        ));

        // Test BroadcastDomain 3 (max)
        let response_bytes = &[0x90, 0x50, 0x03, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::BroadcastDomain);
        assert!(matches!(
            response,
            Ok(Response::Inquiry(InquiryData::BroadcastDomain(_)))
        ));

        // Test BroadcastDomain out of range (4) - should fail
        let response_bytes = &[0x90, 0x50, 0x04, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::BroadcastDomain);
        assert!(matches!(response, Err(Error::InvalidParameter { .. })));
    }

    #[test]
    fn test_parse_motion_sync_mode_response() {
        // Test MotionSyncMode On (0x02)
        let response_bytes = &[0x90, 0x50, 0x02, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::MotionSyncMode);
        match response {
            Ok(Response::Inquiry(InquiryData::MotionSyncMode { mode })) => {
                assert_eq!(mode, crate::command::system::MotionSyncMode::On);
            }
            _ => panic!("Expected MotionSyncMode inquiry response"),
        }

        // Test MotionSyncMode Off (0x03)
        let response_bytes = &[0x90, 0x50, 0x03, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::MotionSyncMode);
        match response {
            Ok(Response::Inquiry(InquiryData::MotionSyncMode { mode })) => {
                assert_eq!(mode, crate::command::system::MotionSyncMode::Off);
            }
            _ => panic!("Expected MotionSyncMode inquiry response"),
        }
    }

    #[test]
    fn test_parse_motion_sync_preset_response() {
        // Test MotionSyncPreset Slow (0x00)
        let response_bytes = &[0x90, 0x50, 0x00, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::MotionSyncPreset);
        match response {
            Ok(Response::Inquiry(InquiryData::MotionSyncPreset { speed })) => {
                assert_eq!(speed, crate::command::system::MotionSyncPreset::Slow);
            }
            _ => panic!("Expected MotionSyncPreset inquiry response"),
        }

        // Test MotionSyncPreset Normal (0x01)
        let response_bytes = &[0x90, 0x50, 0x01, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::MotionSyncPreset);
        match response {
            Ok(Response::Inquiry(InquiryData::MotionSyncPreset { speed })) => {
                assert_eq!(speed, crate::command::system::MotionSyncPreset::Normal);
            }
            _ => panic!("Expected MotionSyncPreset inquiry response"),
        }

        // Test MotionSyncPreset Fast (0x02)
        let response_bytes = &[0x90, 0x50, 0x02, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::MotionSyncPreset);
        match response {
            Ok(Response::Inquiry(InquiryData::MotionSyncPreset { speed })) => {
                assert_eq!(speed, crate::command::system::MotionSyncPreset::Fast);
            }
            _ => panic!("Expected MotionSyncPreset inquiry response"),
        }
    }

    #[test]
    fn test_parse_noise_reduction_level_response() {
        // Test NoiseReductionLevel 0
        let response_bytes = &[0x90, 0x50, 0x00, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::NoiseReductionLevel);
        match response {
            Ok(Response::Inquiry(InquiryData::NoiseReductionLevel(level))) => {
                assert_eq!(level, 0x00);
            }
            _ => panic!("Expected NoiseReductionLevel inquiry response"),
        }

        // Test NoiseReductionLevel 5
        let response_bytes = &[0x90, 0x50, 0x05, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::NoiseReductionLevel);
        match response {
            Ok(Response::Inquiry(InquiryData::NoiseReductionLevel(level))) => {
                assert_eq!(level, 0x05);
            }
            _ => panic!("Expected NoiseReductionLevel inquiry response"),
        }
    }

    #[test]
    fn test_parse_night_day_position_response() {
        // Test NightDayPosition 0
        let response_bytes = &[0x90, 0x50, 0x00, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::NightDayPosition);
        match response {
            Ok(Response::Inquiry(InquiryData::NightDayPosition { position })) => {
                assert_eq!(position, 0x00);
            }
            _ => panic!("Expected NightDayPosition inquiry response"),
        }

        // Test NightDayPosition 0x0F
        let response_bytes = &[0x90, 0x50, 0x0F, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::NightDayPosition);
        match response {
            Ok(Response::Inquiry(InquiryData::NightDayPosition { position })) => {
                assert_eq!(position, 0x0F);
            }
            _ => panic!("Expected NightDayPosition inquiry response"),
        }
    }

    #[test]
    fn test_parse_night_day_switch_response() {
        // Test NightDaySwitch off (0x02)
        let response_bytes = &[0x90, 0x50, 0x02, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::NightDaySwitch);
        match response {
            Ok(Response::Inquiry(InquiryData::NightDaySwitch { enabled })) => {
                assert!(!enabled);
            }
            _ => panic!("Expected NightDaySwitch inquiry response"),
        }

        // Test NightDaySwitch on (0x03)
        let response_bytes = &[0x90, 0x50, 0x03, VISCA_TERMINATOR];
        let response = Response::parse_with_type(response_bytes, &InquiryKind::NightDaySwitch);
        match response {
            Ok(Response::Inquiry(InquiryData::NightDaySwitch { enabled })) => {
                assert!(enabled);
            }
            _ => panic!("Expected NightDaySwitch inquiry response"),
        }
    }
}
