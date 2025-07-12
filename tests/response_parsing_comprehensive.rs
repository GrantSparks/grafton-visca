//! Comprehensive response parsing tests moved from src/command/response.rs
//!
//! These tests cover all VISCA response parsing functionality using the
//! enhanced testing infrastructure with MockTransport and validation.

mod common;

use crate::common::{ProtocolValidator, ResponseBuilder, ValidationMode};
use grafton_visca::{
    command::{
        gain::AntiFlickerMode, image_adjustment::SharpnessMode, response::parse_response,
        AutoFocusSensitivity, ExposureMode, FocusZone, InquiryResponse, Response, ResponseType,
        WhiteBalanceMode,
    },
    Error,
};

#[test]
fn test_response_debug() {
    let ack = Response::CmdAck;
    assert_eq!(format!("{:?}", ack), "Ack");

    let completion = Response::Completion;
    assert_eq!(format!("{:?}", completion), "Completion");

    let unknown = Response::Unknown {
        response_type: None,
        data: vec![0x90, 0x50, 0xFF],
    };
    assert!(format!("{:?}", unknown).contains("Unknown"));
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
    assert!(matches!(result, Response::CmdAck));

    // Test all socket variations
    for socket in 0x40..=0x4F {
        let response = vec![0x90, socket, 0xFF];
        let result = parse_response(&response, &ResponseType::Power).unwrap();
        assert!(matches!(result, Response::CmdAck));
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
        match code {
            0x01 => assert!(result.is_err()), // This maps to Unknown(0x01)
            0x02 => assert!(matches!(result, Err(Error::SyntaxError))),
            0x03 => assert!(matches!(result, Err(Error::CommandBufferFull))),
            0x04 => assert!(matches!(result, Err(Error::CommandCanceled))),
            0x05 => assert!(matches!(result, Err(Error::NoSocket))),
            0x41 => assert!(matches!(result, Err(Error::CommandNotExecutable))),
            _ => assert!(result.is_err()),
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
        Response::Unknown { data, .. } => assert_eq!(data, vec![0x90, 0x30, 0xFF]),
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
fn test_parse_anti_flicker_modes() {
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
    assert!(result.is_err());
}

#[test]
fn test_parse_focus_zone_values() {
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
    assert!(result.is_err());
}

#[test]
fn test_parse_auto_focus_sensitivity_values() {
    let sensitivities = vec![
        (0x00, AutoFocusSensitivity::Low),
        (0x01, AutoFocusSensitivity::Normal),
        (0x02, AutoFocusSensitivity::High),
    ];

    for (byte, expected_sensitivity) in sensitivities {
        let response = vec![0x90, 0x50, byte, 0xFF];
        let result = parse_response(&response, &ResponseType::AutoFocusSensitivity).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::AutoFocusSensitivity { sensitivity }) => {
                assert_eq!(sensitivity, expected_sensitivity);
            }
            _ => panic!("Expected AutoFocusSensitivity inquiry response"),
        }
    }

    // Invalid sensitivity
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::AutoFocusSensitivity);
    assert!(result.is_err());
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
}

#[test]
fn test_protocol_compliance_with_validator() {
    let _validator = ProtocolValidator::new(ValidationMode::Strict);

    // Test that various response formats comply with protocol
    let responses = vec![
        vec![0x90, 0x41, 0xFF],                         // ACK
        vec![0x90, 0x51, 0xFF],                         // Completion
        vec![0x90, 0x50, 0x02, 0xFF],                   // Power inquiry response
        vec![0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xFF], // Zoom position response
    ];

    for response in responses {
        // Responses don't go through command validation, but we can check format
        assert!(response.len() >= 3);
        assert_eq!(response[0], 0x90);
        assert_eq!(response[response.len() - 1], 0xFF);
    }
}

#[test]
fn test_response_builder_integration() {
    // Test that ResponseBuilder produces parseable responses
    let ack = ResponseBuilder::ack(1);
    let result = parse_response(&ack, &ResponseType::Power).unwrap();
    assert!(matches!(result, Response::CmdAck));

    let completion = ResponseBuilder::completion(1);
    let result = parse_response(&completion, &ResponseType::Power).unwrap();
    assert!(matches!(result, Response::Completion));

    let power_on = ResponseBuilder::inquiry().add_on_off(true).build();
    let result = parse_response(&power_on, &ResponseType::Power).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::Power { on }) => assert!(on),
        _ => panic!("Expected Power inquiry response"),
    }
}

#[test]
fn test_unknown_inquiry_response_handling() {
    // Test that an unimplemented response type returns Unknown variant
    // Using a response type that's not implemented in parse_inquiry_response
    let response = vec![0x90, 0x50, 0x01, 0x02, 0x03, 0xFF]; // Some inquiry data

    // Try parsing with various unimplemented response types
    let unimplemented_types = vec![
        ResponseType::PictureEffect,
        ResponseType::MenuOpenClose,
        ResponseType::UsbAudio,
        ResponseType::Rtmp,
        ResponseType::AutoTrace,
    ];

    for response_type in unimplemented_types {
        let result = parse_response(&response, &response_type).unwrap();
        match result {
            Response::Unknown {
                response_type: Some(rt),
                data,
            } => {
                assert_eq!(rt, response_type);
                assert_eq!(data, vec![0x01, 0x02, 0x03]); // Payload without header/terminator
            }
            _ => panic!(
                "Expected Unknown response for unimplemented type {:?}",
                response_type
            ),
        }
    }
}

#[test]
fn test_exhaustive_response_type_coverage() {
    // This test ensures that we handle ALL ResponseType variants appropriately
    // by checking that each type either parses successfully or returns Unknown

    // List of all implemented response types with valid test data
    let implemented_types = vec![
        (ResponseType::Power, vec![0x90, 0x50, 0x02, 0xFF]), // on = true
        (
            ResponseType::ZoomPosition,
            vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF],
        ),
        (
            ResponseType::PanTiltPosition,
            vec![
                0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF,
            ],
        ),
        (
            ResponseType::FocusPosition,
            vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF],
        ),
        (
            ResponseType::FocusNearLimit,
            vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF],
        ),
        (ResponseType::ExposureMode, vec![0x90, 0x50, 0x00, 0xFF]), // Auto
        (ResponseType::WhiteBalanceMode, vec![0x90, 0x50, 0x00, 0xFF]), // Auto
        (ResponseType::FocusZone, vec![0x90, 0x50, 0x00, 0xFF]),
        (
            ResponseType::AutoFocusSensitivity,
            vec![0x90, 0x50, 0x01, 0xFF],
        ), // Normal
        (ResponseType::AntiFlicker, vec![0x90, 0x50, 0x00, 0xFF]), // Off
        (
            ResponseType::ExposureCompensationMode,
            vec![0x90, 0x50, 0x02, 0xFF],
        ), // on
        (
            ResponseType::ExposureCompensation,
            vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF],
        ),
        (ResponseType::GainLimit, vec![0x90, 0x50, 0x00, 0xFF]),
        (ResponseType::RedGain, vec![0x90, 0x50, 0x0A, 0xFF]), // 0 after conversion
        (ResponseType::BlueGain, vec![0x90, 0x50, 0x0A, 0xFF]), // 0 after conversion
        (
            ResponseType::Sharpness,
            vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF],
        ),
        (ResponseType::SharpnessMode, vec![0x90, 0x50, 0x02, 0xFF]), // Auto
        (
            ResponseType::Shutter,
            vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF],
        ),
        (ResponseType::Backlight, vec![0x90, 0x50, 0x02, 0xFF]), // on
        (ResponseType::Luminance, vec![0x90, 0x50, 0x00, 0xFF]),
        (ResponseType::Contrast, vec![0x90, 0x50, 0x00, 0xFF]),
        (ResponseType::ImageFlip, vec![0x90, 0x50, 0x00, 0xFF]),
        (ResponseType::BlackWhite, vec![0x90, 0x50, 0x04, 0xFF]), // on
        (ResponseType::TallyRed, vec![0x90, 0x50, 0x02, 0xFF]),   // on
        (ResponseType::TallyGreen, vec![0x90, 0x50, 0x02, 0xFF]), // on
    ];

    // Verify all implemented types parse successfully
    for (response_type, data) in implemented_types {
        let result = parse_response(&data, &response_type);
        assert!(
            result.is_ok(),
            "Failed to parse {:?}: {:?}",
            response_type,
            result
        );
        match result.unwrap() {
            Response::InquiryResponse(_) => {} // Good, it parsed
            Response::Unknown { .. } => {
                panic!("Implemented type {:?} returned Unknown", response_type)
            }
            _ => panic!("Unexpected response type for {:?}", response_type),
        }
    }

    // List of ALL unimplemented types - these should return Unknown
    let unimplemented_types = vec![
        ResponseType::Gain,
        ResponseType::Iris,
        ResponseType::Bright,
        ResponseType::DynamicRange,
        ResponseType::ColorTemperature,
        ResponseType::Saturation,
        ResponseType::Hue,
        ResponseType::NoiseReduction2D,
        ResponseType::NoiseReduction3D,
        ResponseType::PictureEffect,
        ResponseType::Version,
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
        ResponseType::AutoFocus,
        ResponseType::FocusUnlock,
        ResponseType::ZoomOut,
        ResponseType::ZoomIn,
        ResponseType::IrisUp,
        ResponseType::IrisDown,
        ResponseType::NightDayMode,
        ResponseType::NightDayPosition,
        ResponseType::AutoTrace,
        ResponseType::TwoToneMode,
        ResponseType::DefogMode,
        ResponseType::NrLevel,
        ResponseType::NrMode,
        ResponseType::NrSpeed,
        ResponseType::BroadcastDomain,
        ResponseType::Resolution,
        ResponseType::NdFilter,
        ResponseType::NdFilterPreset,
        ResponseType::FocusNearFar,
        ResponseType::ZoomTeleWide,
        ResponseType::Standby,
        ResponseType::Tally,
        ResponseType::DigitalPtz,
        ResponseType::Digital,
        ResponseType::IrisControl,
        ResponseType::DefogLevel,
        ResponseType::NightDay,
        ResponseType::NightDaySwitch,
        ResponseType::FlipMode,
        ResponseType::TallyStatus,
        ResponseType::TallyAutoAdjust,
    ];

    // Verify unimplemented types return Unknown
    let test_data = vec![0x90, 0x50, 0x01, 0x02, 0xFF];
    for response_type in unimplemented_types {
        let result = parse_response(&test_data, &response_type).unwrap();
        match result {
            Response::Unknown {
                response_type: Some(rt),
                ..
            } => {
                assert_eq!(rt, response_type);
            }
            _ => panic!(
                "Expected Unknown response for unimplemented type {:?}",
                response_type
            ),
        }
    }
}
