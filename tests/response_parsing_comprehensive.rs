//! Comprehensive response parsing tests moved from src/command/response.rs
//!
//! These tests cover all VISCA response parsing functionality using the
//! enhanced testing infrastructure with MockTransport and validation.

mod common;

use crate::common::{ProtocolValidator, ValidationMode};
use grafton_visca::{
    command::{
        gain::AntiFlickerMode, response::parse_response, AutoFocusSensitivity, ExposureMode,
        FocusMode, FocusZone, InquiryResponse, Response, ResponseType, WhiteBalanceMode,
    },
    Error,
};

#[test]
fn test_response_debug() {
    let ack = Response::CmdAck;
    assert_eq!(format!("{:?}", ack), "CmdAck");

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
fn test_parse_error_response() {
    let response = vec![0x90, 0x60, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::Power);
    assert!(result.is_err());
    assert!(matches!(result, Err(Error::SyntaxError)));

    // Test various error codes
    let error_codes = vec![
        (0x02, Error::SyntaxError),
        (0x03, Error::CommandBufferFull),
        (0x04, Error::CommandCanceled),
        (0x05, Error::NoSocket),
        (0x41, Error::CommandNotExecutable),
    ];

    for (code, expected_error) in error_codes {
        let response = vec![0x90, 0x60, code, 0xFF];
        let result = parse_response(&response, &ResponseType::Power);
        match result {
            Err(err) => assert_eq!(
                format!("{:?}", err),
                format!("{:?}", expected_error),
                "Error code 0x{:02X} should map to {:?}",
                code,
                expected_error
            ),
            _ => panic!("Expected error for code 0x{:02X}", code),
        }
    }
}

#[test]
fn test_invalid_response_formats() {
    // Empty response
    let response = vec![];
    let result = parse_response(&response, &ResponseType::Power);
    assert!(matches!(result, Err(Error::InvalidResponseFormat)));

    // Too short
    let response = vec![0x90, 0xFF];
    let result = parse_response(&response, &ResponseType::Power);
    assert!(matches!(result, Err(Error::InvalidResponseFormat)));

    // Wrong first byte
    let response = vec![0x80, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::Power);
    assert!(matches!(result, Err(Error::InvalidResponseFormat)));

    // Missing terminator
    let response = vec![0x90, 0x50, 0x02, 0x00];
    let result = parse_response(&response, &ResponseType::Power);
    assert!(matches!(result, Err(Error::InvalidResponseFormat)));
}

#[test]
fn test_parse_power_inquiry_response() {
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

    // Test invalid data length
    let response = vec![0x90, 0x50, 0x02, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::Power);
    assert!(matches!(result, Err(Error::InvalidResponseLength)));
}

#[test]
fn test_parse_zoom_position_response() {
    // Test zoom position 0x0000
    let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::ZoomPosition).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ZoomPosition { position }) => {
            assert_eq!(position, 0x0000);
        }
        _ => panic!("Expected ZoomPosition inquiry response"),
    }

    // Test zoom position 0x1234
    let response = vec![0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::ZoomPosition).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ZoomPosition { position }) => {
            assert_eq!(position, 0x1234);
        }
        _ => panic!("Expected ZoomPosition inquiry response"),
    }

    // Test zoom position 0xFFFF
    let response = vec![0x90, 0x50, 0x0F, 0x0F, 0x0F, 0x0F, 0xFF];
    let result = parse_response(&response, &ResponseType::ZoomPosition).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ZoomPosition { position }) => {
            assert_eq!(position, 0xFFFF);
        }
        _ => panic!("Expected ZoomPosition inquiry response"),
    }
}

#[test]
fn test_parse_pan_tilt_position_response() {
    // Test zero position
    let response = vec![
        0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF,
    ];
    let result = parse_response(&response, &ResponseType::PanTiltPosition).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt }) => {
            assert_eq!(pan, 0);
            assert_eq!(tilt, 0);
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
fn test_parse_focus_position_response() {
    // Test minimum focus position
    let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::FocusPosition).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::FocusPosition { position }) => {
            assert_eq!(position, 0x0000);
        }
        _ => panic!("Expected FocusPosition inquiry response"),
    }

    // Test focus position 0x9000
    let response = vec![0x90, 0x50, 0x09, 0x00, 0x00, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::FocusPosition).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::FocusPosition { position }) => {
            assert_eq!(position, 0x9000);
        }
        _ => panic!("Expected FocusPosition inquiry response"),
    }
}

#[test]
fn test_parse_exposure_mode_response() {
    let test_cases = vec![
        (0x00, ExposureMode::Auto),
        (0x03, ExposureMode::Manual),
        (0x0A, ExposureMode::Shutter),
        (0x0B, ExposureMode::Iris),
        (0x0D, ExposureMode::Bright),
    ];

    for (value, expected_mode) in test_cases {
        let response = vec![0x90, 0x50, value, 0xFF];
        let result = parse_response(&response, &ResponseType::ExposureMode).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::ExposureMode { mode }) => {
                assert_eq!(
                    mode, expected_mode,
                    "Value 0x{:02X} should map to {:?}",
                    value, expected_mode
                );
            }
            _ => panic!("Expected ExposureMode inquiry response"),
        }
    }

    // Test invalid exposure mode
    let response = vec![0x90, 0x50, 0x0F, 0xFF];
    let result = parse_response(&response, &ResponseType::ExposureMode);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_parse_white_balance_mode_response() {
    let test_cases = vec![
        (0x00, WhiteBalanceMode::Auto),
        (0x01, WhiteBalanceMode::Indoor),
        (0x02, WhiteBalanceMode::Outdoor),
        (0x03, WhiteBalanceMode::OnePush),
        (0x05, WhiteBalanceMode::Manual),
        (0x20, WhiteBalanceMode::ColorTemperature),
    ];

    for (value, expected_mode) in test_cases {
        let response = vec![0x90, 0x50, value, 0xFF];
        let result = parse_response(&response, &ResponseType::WhiteBalanceMode).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::WhiteBalanceMode { mode }) => {
                assert_eq!(
                    mode, expected_mode,
                    "Value 0x{:02X} should map to {:?}",
                    value, expected_mode
                );
            }
            _ => panic!("Expected WhiteBalanceMode inquiry response"),
        }
    }
}

#[test]
fn test_parse_anti_flicker_response() {
    let test_cases = vec![
        (0x00, AntiFlickerMode::Off),
        (0x01, AntiFlickerMode::Hz50),
        (0x02, AntiFlickerMode::Hz60),
    ];

    for (value, expected_mode) in test_cases {
        let response = vec![0x90, 0x50, value, 0xFF];
        let result = parse_response(&response, &ResponseType::AntiFlicker).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::AntiFlicker { mode }) => {
                assert_eq!(
                    mode, expected_mode,
                    "Value 0x{:02X} should map to {:?}",
                    value, expected_mode
                );
            }
            _ => panic!("Expected AntiFlicker inquiry response"),
        }
    }
}

#[test]
fn test_parse_focus_zone_response() {
    let test_cases = vec![
        (0x00, FocusZone::Top),
        (0x01, FocusZone::Center),
        (0x02, FocusZone::Bottom),
    ];

    for (value, expected_zone) in test_cases {
        let response = vec![0x90, 0x50, value, 0xFF];
        let result = parse_response(&response, &ResponseType::FocusZone).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::FocusZone { zone }) => {
                assert_eq!(
                    zone, expected_zone,
                    "Value 0x{:02X} should map to {:?}",
                    value, expected_zone
                );
            }
            _ => panic!("Expected FocusZone inquiry response"),
        }
    }
}

#[test]
fn test_parse_auto_focus_sensitivity_response() {
    let test_cases = vec![
        (0x00, AutoFocusSensitivity::Low),
        (0x01, AutoFocusSensitivity::Normal),
        (0x02, AutoFocusSensitivity::High),
    ];

    for (value, expected_sensitivity) in test_cases {
        let response = vec![0x90, 0x50, value, 0xFF];
        let result = parse_response(&response, &ResponseType::AutoFocusSensitivity).unwrap();
        match result {
            Response::InquiryResponse(InquiryResponse::AutoFocusSensitivity { sensitivity }) => {
                assert_eq!(
                    sensitivity, expected_sensitivity,
                    "Value 0x{:02X} should map to {:?}",
                    value, expected_sensitivity
                );
            }
            _ => panic!("Expected AutoFocusSensitivity inquiry response"),
        }
    }
}

#[test]
fn test_parse_exposure_compensation_mode_response() {
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
fn test_parse_sharpness_response() {
    // Test sharpness value 0x00
    let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::Sharpness).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::Sharpness { value }) => {
            assert_eq!(value, 0x00);
        }
        _ => panic!("Expected Sharpness inquiry response"),
    }

    // Test sharpness value 0x0F
    let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0F, 0xFF];
    let result = parse_response(&response, &ResponseType::Sharpness).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::Sharpness { value }) => {
            assert_eq!(value, 0x0F);
        }
        _ => panic!("Expected Sharpness inquiry response"),
    }
}

#[test]
fn test_parse_exposure_compensation_response() {
    // Test compensation value 0 (raw value 7)
    let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
    let result = parse_response(&response, &ResponseType::ExposureCompensation).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ExposureCompensation { value }) => {
            assert_eq!(value, 0);
        }
        _ => panic!("Expected ExposureCompensation inquiry response"),
    }

    // Test compensation value -7 (raw value 0)
    let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::ExposureCompensation).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ExposureCompensation { value }) => {
            assert_eq!(value, -7);
        }
        _ => panic!("Expected ExposureCompensation inquiry response"),
    }

    // Test compensation value +7 (raw value 14)
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
fn test_parse_shutter_response() {
    // Test shutter position 0x00
    let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::Shutter).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::Shutter { position }) => {
            assert_eq!(position, 0x00);
        }
        _ => panic!("Expected Shutter inquiry response"),
    }

    // Test shutter position 0x11
    let response = vec![0x90, 0x50, 0x00, 0x00, 0x01, 0x01, 0xFF];
    let result = parse_response(&response, &ResponseType::Shutter).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::Shutter { position }) => {
            assert_eq!(position, 0x11);
        }
        _ => panic!("Expected Shutter inquiry response"),
    }
}

#[test]
fn test_parse_image_flip_response() {
    // Test no flip
    let response = vec![0x90, 0x50, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::ImageFlip).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ImageFlip {
            horizontal,
            vertical,
        }) => {
            assert!(!horizontal);
            assert!(!vertical);
        }
        _ => panic!("Expected ImageFlip inquiry response"),
    }

    // Test horizontal flip only
    let response = vec![0x90, 0x50, 0x01, 0xFF];
    let result = parse_response(&response, &ResponseType::ImageFlip).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ImageFlip {
            horizontal,
            vertical,
        }) => {
            assert!(horizontal);
            assert!(!vertical);
        }
        _ => panic!("Expected ImageFlip inquiry response"),
    }

    // Test vertical flip only
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::ImageFlip).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ImageFlip {
            horizontal,
            vertical,
        }) => {
            assert!(!horizontal);
            assert!(vertical);
        }
        _ => panic!("Expected ImageFlip inquiry response"),
    }

    // Test both flips
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::ImageFlip).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ImageFlip {
            horizontal,
            vertical,
        }) => {
            assert!(horizontal);
            assert!(vertical);
        }
        _ => panic!("Expected ImageFlip inquiry response"),
    }
}

#[test]
fn test_parse_gain_level_response() {
    // Test gain level from last nibble
    let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x05, 0xFF];
    let result = parse_response(&response, &ResponseType::Gain).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::GainLevel { gain }) => {
            assert_eq!(gain, 0x05);
        }
        _ => panic!("Expected GainLevel inquiry response"),
    }
}

#[test]
fn test_parse_iris_response() {
    // Test iris position from last nibble
    let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0C, 0xFF];
    let result = parse_response(&response, &ResponseType::Iris).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::Iris { position }) => {
            assert_eq!(position, 0x0C);
        }
        _ => panic!("Expected Iris inquiry response"),
    }
}

#[test]
fn test_parse_color_temperature_response() {
    // Test color temperature from nibbles 2 and 3
    let response = vec![0x90, 0x50, 0x00, 0x00, 0x0A, 0x0F, 0xFF];
    let result = parse_response(&response, &ResponseType::ColorTemperature).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ColorTemperature { temperature }) => {
            assert_eq!(temperature, 0x00AF); // (0x0A << 4) | 0x0F
        }
        _ => panic!("Expected ColorTemperature inquiry response"),
    }
}

#[test]
fn test_response_into_result() {
    let _validator = ProtocolValidator::new(ValidationMode::Strict);

    // Completion should convert to Ok
    let completion = Response::Completion;
    assert!(completion.into_result().is_ok());

    // CmdAck should convert to Err (command pending)
    let ack = Response::CmdAck;
    assert!(matches!(ack.into_result(), Err(Error::CommandPending)));

    // Error response should convert to Err
    let error = Response::Error(Error::SyntaxError);
    assert!(matches!(error.into_result(), Err(Error::SyntaxError)));

    // InquiryResponse should convert to Ok
    let inquiry = Response::InquiryResponse(InquiryResponse::Power { on: true });
    assert!(inquiry.into_result().is_ok());

    // Unknown response should convert to Err
    let unknown = Response::Unknown {
        response_type: None,
        data: vec![0x90, 0x50, 0xFF],
    };
    assert!(matches!(unknown.into_result(), Err(Error::InvalidResponse { .. })));
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
        ),
        (
            ResponseType::ExposureCompensationMode,
            vec![0x90, 0x50, 0x02, 0xFF],
        ),
        (ResponseType::SharpnessMode, vec![0x90, 0x50, 0x02, 0xFF]), // Auto
        (ResponseType::GainLimit, vec![0x90, 0x50, 0x05, 0xFF]),
        (ResponseType::RedChannel, vec![0x90, 0x50, 0x0A, 0xFF]), // 0 offset from 10
        (ResponseType::BlueChannel, vec![0x90, 0x50, 0x0A, 0xFF]), // 0 offset from 10
        (
            ResponseType::Sharpness,
            vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x05, 0xFF],
        ),
        (
            ResponseType::ExposureCompensation,
            vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF],
        ), // 0 offset from 7
        (
            ResponseType::Bright,
            vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF],
        ),
        (
            ResponseType::Gain,
            vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x05, 0xFF],
        ),
        (
            ResponseType::Iris,
            vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0C, 0xFF],
        ),
        (
            ResponseType::Saturation,
            vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF],
        ),
        (
            ResponseType::ColorTemperature,
            vec![0x90, 0x50, 0x00, 0x00, 0x28, 0x00, 0xFF],
        ), // 2800K
        (
            ResponseType::Hue,
            vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF],
        ),
        (ResponseType::AntiFlicker, vec![0x90, 0x50, 0x00, 0xFF]), // Off
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
        // Newly implemented types
        (
            ResponseType::Version,
            vec![0x90, 0x50, 0x00, 0x01, 0x00, 0x02, 0x03, 0x04, 0x02, 0xFF],
        ), // Vendor: 0x0001, Model: 0x0002, ROM: 0x0304, Socket: 2
        (ResponseType::FocusMode, vec![0x90, 0x50, 0x02, 0xFF]), // Auto
        (ResponseType::DynamicRange, vec![0x90, 0x50, 0x05, 0xFF]), // Level 5
        (ResponseType::NoiseReduction2D, vec![0x90, 0x50, 0x03, 0xFF]), // Level 3
        (ResponseType::NoiseReduction3D, vec![0x90, 0x50, 0x04, 0xFF]), // Level 4
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
        ResponseType::PictureEffect,
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

#[test]
fn test_version_inquiry_response() {
    // Test Version response parsing
    let response = vec![0x90, 0x50, 0x00, 0x01, 0x00, 0x02, 0x03, 0x04, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::Version).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::Version {
            vendor,
            model,
            rom_version,
            max_socket,
        }) => {
            assert_eq!(vendor, 0x0001);
            assert_eq!(model, 0x0002);
            assert_eq!(rom_version, 0x0304);
            assert_eq!(max_socket, 0x02);
        }
        _ => panic!("Expected Version inquiry response"),
    }

    // Test invalid length
    let response = vec![0x90, 0x50, 0x00, 0x01, 0xFF];
    let result = parse_response(&response, &ResponseType::Version);
    assert!(matches!(result, Err(Error::InvalidResponseLength)));
}

#[test]
fn test_focus_mode_inquiry_response() {
    // Test Auto focus mode
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::FocusMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::FocusMode { mode }) => {
            assert_eq!(mode, FocusMode::Auto);
        }
        _ => panic!("Expected FocusMode inquiry response"),
    }

    // Test Manual focus mode
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::FocusMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::FocusMode { mode }) => {
            assert_eq!(mode, FocusMode::Manual);
        }
        _ => panic!("Expected FocusMode inquiry response"),
    }

    // Test invalid focus mode value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::FocusMode);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_dynamic_range_inquiry_response() {
    // Test DynamicRange response
    let response = vec![0x90, 0x50, 0x05, 0xFF];
    let result = parse_response(&response, &ResponseType::DynamicRange).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::DynamicRange { level }) => {
            assert_eq!(level, 0x05);
        }
        _ => panic!("Expected DynamicRange inquiry response"),
    }
}

#[test]
fn test_noise_reduction_inquiry_responses() {
    // Test NoiseReduction2D
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::NoiseReduction2D).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NoiseReduction2D { level }) => {
            assert_eq!(level, 0x03);
        }
        _ => panic!("Expected NoiseReduction2D inquiry response"),
    }

    // Test NoiseReduction3D
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::NoiseReduction3D).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NoiseReduction3D { level }) => {
            assert_eq!(level, 0x04);
        }
        _ => panic!("Expected NoiseReduction3D inquiry response"),
    }
}