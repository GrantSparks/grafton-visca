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
    assert!(matches!(
        unknown.into_result(),
        Err(Error::InvalidResponse { .. })
    ));
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
        // Newly implemented types - Phase 2
        (ResponseType::MenuOpenClose, vec![0x90, 0x50, 0x02, 0xFF]), // Menu closed
        (ResponseType::AutoFocus, vec![0x90, 0x50, 0x03, 0xFF]),     // AF on
        (
            ResponseType::TallyStatus,
            vec![0x90, 0x50, 0x02, 0x02, 0xFF],
        ), // Both off
        (ResponseType::Resolution, vec![0x90, 0x50, 0x00, 0xFF]),    // Mode 0 (1080p60)
        // Newly implemented types - Phase 3
        (ResponseType::NightDayMode, vec![0x90, 0x50, 0x02, 0xFF]), // Day mode
        (ResponseType::NdFilter, vec![0x90, 0x50, 0x00, 0xFF]),     // Clear filter
        (ResponseType::PictureEffect, vec![0x90, 0x50, 0x00, 0xFF]), // Off
        (ResponseType::FlipMode, vec![0x90, 0x50, 0x00, 0xFF]),     // No flip
        (ResponseType::Standby, vec![0x90, 0x50, 0x02, 0xFF]),      // Active
        // Newly implemented types - Phase 4
        (ResponseType::FocusRange, vec![0x90, 0x50, 0x00, 0xFF]), // Normal
        (ResponseType::IrisControl, vec![0x90, 0x50, 0x02, 0xFF]), // Manual
        (ResponseType::DefogMode, vec![0x90, 0x50, 0x02, 0xFF]),  // Off
        (ResponseType::DefogLevel, vec![0x90, 0x50, 0x00, 0xFF]), // Level 0
        (ResponseType::DigitalPtz, vec![0x90, 0x50, 0x02, 0xFF]), // Off
        // Newly implemented types - Phase 6
        (
            ResponseType::AutoWhiteBalanceSensitivity,
            vec![0x90, 0x50, 0x01, 0xFF],
        ), // Normal
        (
            ResponseType::ExposureCompensationPosition,
            vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF],
        ), // Position 0
        (ResponseType::RedTuning, vec![0x90, 0x50, 0x80, 0xFF]), // Level 128
        (ResponseType::BlueTuning, vec![0x90, 0x50, 0x80, 0xFF]), // Level 128
        (ResponseType::AutoTrace, vec![0x90, 0x50, 0x02, 0xFF]), // Off
        (ResponseType::FocusUnlock, vec![0x90, 0x50, 0x02, 0xFF]), // Locked
        // Newly implemented types - Phase 7
        (
            ResponseType::SharpnessPosition,
            vec![0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xFF],
        ), // Position 0x1234
        (ResponseType::NrLevel, vec![0x90, 0x50, 0x05, 0xFF]), // Level 5
        (ResponseType::BroadcastDomain, vec![0x90, 0x50, 0x01, 0xFF]), // Domain 1
        (ResponseType::MotionSyncMode, vec![0x90, 0x50, 0x02, 0xFF]), // Off
        (ResponseType::MotionSyncSpeed, vec![0x90, 0x50, 0x01, 0xFF]), // Normal
        (ResponseType::NrMode, vec![0x90, 0x50, 0x03, 0xFF]),  // On
        (ResponseType::NrSpeed, vec![0x90, 0x50, 0x02, 0xFF]), // Fast
        (ResponseType::BlackWhiteMode, vec![0x90, 0x50, 0x03, 0xFF]), // BlackWhite
        // Newly implemented types - Phase 9
        (ResponseType::UsbAudio, vec![0x90, 0x50, 0x02, 0xFF]), // Off
        (ResponseType::TwoToneMode, vec![0x90, 0x50, 0x03, 0xFF]), // On
        (ResponseType::NdFilterPreset, vec![0x90, 0x50, 0x01, 0xFF]), // Preset 1
        (ResponseType::Digital, vec![0x90, 0x50, 0x02, 0xFF]),  // Off
        (ResponseType::TallyAutoAdjust, vec![0x90, 0x50, 0x03, 0xFF]), // On
        // Newly implemented types - Phase 10
        (ResponseType::Rtmp, vec![0x90, 0x50, 0x02, 0xFF]), // Off
        (ResponseType::ZoomOut, vec![0x90, 0x50, 0x03, 0xFF]), // Active
        (ResponseType::ZoomIn, vec![0x90, 0x50, 0x02, 0xFF]), // Inactive
        (ResponseType::IrisUp, vec![0x90, 0x50, 0x03, 0xFF]), // Active
        (ResponseType::IrisDown, vec![0x90, 0x50, 0x02, 0xFF]), // Inactive
        (ResponseType::NightDayPosition, vec![0x90, 0x50, 0x05, 0xFF]), // Position 5
        (ResponseType::FocusNearFar, vec![0x90, 0x50, 0x03, 0xFF]), // Near active
        (ResponseType::ZoomTeleWide, vec![0x90, 0x50, 0x02, 0xFF]), // Wide active
        (ResponseType::NightDaySwitch, vec![0x90, 0x50, 0x03, 0xFF]), // Enabled
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
    let unimplemented_types: Vec<ResponseType> = vec![
        // All response types are now implemented!
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

#[test]
fn test_menu_open_close_inquiry_response() {
    // Test menu closed
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::MenuOpenClose).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::MenuOpenClose { is_open }) => {
            assert!(!is_open);
        }
        _ => panic!("Expected MenuOpenClose inquiry response"),
    }

    // Test menu open
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::MenuOpenClose).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::MenuOpenClose { is_open }) => {
            assert!(is_open);
        }
        _ => panic!("Expected MenuOpenClose inquiry response"),
    }

    // Test invalid menu status value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::MenuOpenClose);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_auto_focus_inquiry_response() {
    // Test AF off
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::AutoFocus).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::AutoFocus { enabled }) => {
            assert!(!enabled);
        }
        _ => panic!("Expected AutoFocus inquiry response"),
    }

    // Test AF on
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::AutoFocus).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::AutoFocus { enabled }) => {
            assert!(enabled);
        }
        _ => panic!("Expected AutoFocus inquiry response"),
    }

    // Test invalid AF status value
    let response = vec![0x90, 0x50, 0x05, 0xFF];
    let result = parse_response(&response, &ResponseType::AutoFocus);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_tally_status_inquiry_response() {
    // Test both tally lights off
    let response = vec![0x90, 0x50, 0x02, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::TallyStatus).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::TallyStatus { red_on, green_on }) => {
            assert!(!red_on);
            assert!(!green_on);
        }
        _ => panic!("Expected TallyStatus inquiry response"),
    }

    // Test red on, green off
    let response = vec![0x90, 0x50, 0x03, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::TallyStatus).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::TallyStatus { red_on, green_on }) => {
            assert!(red_on);
            assert!(!green_on);
        }
        _ => panic!("Expected TallyStatus inquiry response"),
    }

    // Test red off, green on
    let response = vec![0x90, 0x50, 0x02, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::TallyStatus).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::TallyStatus { red_on, green_on }) => {
            assert!(!red_on);
            assert!(green_on);
        }
        _ => panic!("Expected TallyStatus inquiry response"),
    }

    // Test both tally lights on
    let response = vec![0x90, 0x50, 0x03, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::TallyStatus).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::TallyStatus { red_on, green_on }) => {
            assert!(red_on);
            assert!(green_on);
        }
        _ => panic!("Expected TallyStatus inquiry response"),
    }

    // Test invalid length
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::TallyStatus);
    assert!(matches!(result, Err(Error::InvalidResponseLength)));
}

#[test]
fn test_resolution_inquiry_response() {
    // Test resolution mode 0 (1080p60)
    let response = vec![0x90, 0x50, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::Resolution).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::Resolution(mode)) => {
            assert_eq!(mode, 0x00);
        }
        _ => panic!("Expected Resolution inquiry response"),
    }

    // Test resolution mode 1 (1080p30)
    let response = vec![0x90, 0x50, 0x01, 0xFF];
    let result = parse_response(&response, &ResponseType::Resolution).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::Resolution(mode)) => {
            assert_eq!(mode, 0x01);
        }
        _ => panic!("Expected Resolution inquiry response"),
    }

    // Test resolution mode 2 (720p60)
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::Resolution).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::Resolution(mode)) => {
            assert_eq!(mode, 0x02);
        }
        _ => panic!("Expected Resolution inquiry response"),
    }
}

#[test]
fn test_night_day_mode_response() {
    // Test day mode
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::NightDayMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NightDayMode { is_night }) => {
            assert!(!is_night);
        }
        _ => panic!("Expected NightDayMode inquiry response"),
    }

    // Test night mode
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::NightDayMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NightDayMode { is_night }) => {
            assert!(is_night);
        }
        _ => panic!("Expected NightDayMode inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::NightDayMode);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_nd_filter_response() {
    // Test clear (no filter)
    let response = vec![0x90, 0x50, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::NdFilter).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NdFilter { position }) => {
            assert_eq!(position, 0x00);
        }
        _ => panic!("Expected NdFilter inquiry response"),
    }

    // Test 1/4 ND
    let response = vec![0x90, 0x50, 0x01, 0xFF];
    let result = parse_response(&response, &ResponseType::NdFilter).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NdFilter { position }) => {
            assert_eq!(position, 0x01);
        }
        _ => panic!("Expected NdFilter inquiry response"),
    }

    // Test 1/64 ND
    let response = vec![0x90, 0x50, 0x05, 0xFF];
    let result = parse_response(&response, &ResponseType::NdFilter).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NdFilter { position }) => {
            assert_eq!(position, 0x05);
        }
        _ => panic!("Expected NdFilter inquiry response"),
    }
}

#[test]
fn test_picture_effect_response() {
    // Test off (normal)
    let response = vec![0x90, 0x50, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::PictureEffect).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::PictureEffect { effect }) => {
            assert_eq!(effect, 0x00);
        }
        _ => panic!("Expected PictureEffect inquiry response"),
    }

    // Test negative
    let response = vec![0x90, 0x50, 0x01, 0xFF];
    let result = parse_response(&response, &ResponseType::PictureEffect).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::PictureEffect { effect }) => {
            assert_eq!(effect, 0x01);
        }
        _ => panic!("Expected PictureEffect inquiry response"),
    }

    // Test B&W
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::PictureEffect).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::PictureEffect { effect }) => {
            assert_eq!(effect, 0x02);
        }
        _ => panic!("Expected PictureEffect inquiry response"),
    }
}

#[test]
fn test_flip_mode_response() {
    // Test no flip
    let response = vec![0x90, 0x50, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::FlipMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::FlipMode {
            horizontal,
            vertical,
        }) => {
            assert!(!horizontal);
            assert!(!vertical);
        }
        _ => panic!("Expected FlipMode inquiry response"),
    }

    // Test horizontal flip only
    let response = vec![0x90, 0x50, 0x01, 0xFF];
    let result = parse_response(&response, &ResponseType::FlipMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::FlipMode {
            horizontal,
            vertical,
        }) => {
            assert!(horizontal);
            assert!(!vertical);
        }
        _ => panic!("Expected FlipMode inquiry response"),
    }

    // Test vertical flip only
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::FlipMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::FlipMode {
            horizontal,
            vertical,
        }) => {
            assert!(!horizontal);
            assert!(vertical);
        }
        _ => panic!("Expected FlipMode inquiry response"),
    }

    // Test both flips
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::FlipMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::FlipMode {
            horizontal,
            vertical,
        }) => {
            assert!(horizontal);
            assert!(vertical);
        }
        _ => panic!("Expected FlipMode inquiry response"),
    }
}

#[test]
fn test_standby_response() {
    // Test active mode
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::Standby).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::Standby { in_standby }) => {
            assert!(!in_standby);
        }
        _ => panic!("Expected Standby inquiry response"),
    }

    // Test standby mode
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::Standby).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::Standby { in_standby }) => {
            assert!(in_standby);
        }
        _ => panic!("Expected Standby inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::Standby);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_focus_range_response() {
    use grafton_visca::command::FocusRange;

    // Test normal focus range
    let response = vec![0x90, 0x50, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::FocusRange).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::FocusRange { range }) => {
            assert_eq!(range, FocusRange::Normal);
        }
        _ => panic!("Expected FocusRange inquiry response"),
    }

    // Test 10x focus range
    let response = vec![0x90, 0x50, 0x01, 0xFF];
    let result = parse_response(&response, &ResponseType::FocusRange).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::FocusRange { range }) => {
            assert_eq!(range, FocusRange::Range10x);
        }
        _ => panic!("Expected FocusRange inquiry response"),
    }

    // Test 4.3x focus range
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::FocusRange).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::FocusRange { range }) => {
            assert_eq!(range, FocusRange::Range4_3x);
        }
        _ => panic!("Expected FocusRange inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x06, 0xFF];
    let result = parse_response(&response, &ResponseType::FocusRange);
    assert!(result.is_err());
}

#[test]
fn test_iris_control_response() {
    // Test manual control
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::IrisControl).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::IrisControl { auto }) => {
            assert!(!auto);
        }
        _ => panic!("Expected IrisControl inquiry response"),
    }

    // Test auto control
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::IrisControl).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::IrisControl { auto }) => {
            assert!(auto);
        }
        _ => panic!("Expected IrisControl inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::IrisControl);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_defog_mode_response() {
    // Test defog off
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::DefogMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::DefogMode { enabled }) => {
            assert!(!enabled);
        }
        _ => panic!("Expected DefogMode inquiry response"),
    }

    // Test defog on
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::DefogMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::DefogMode { enabled }) => {
            assert!(enabled);
        }
        _ => panic!("Expected DefogMode inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::DefogMode);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_defog_level_response() {
    // Test defog level 0
    let response = vec![0x90, 0x50, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::DefogLevel).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::DefogLevel { level }) => {
            assert_eq!(level, 0);
        }
        _ => panic!("Expected DefogLevel inquiry response"),
    }

    // Test defog level 8 (maximum)
    let response = vec![0x90, 0x50, 0x08, 0xFF];
    let result = parse_response(&response, &ResponseType::DefogLevel).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::DefogLevel { level }) => {
            assert_eq!(level, 8);
        }
        _ => panic!("Expected DefogLevel inquiry response"),
    }

    // Test defog level 4 (medium)
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::DefogLevel).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::DefogLevel { level }) => {
            assert_eq!(level, 4);
        }
        _ => panic!("Expected DefogLevel inquiry response"),
    }
}

#[test]
fn test_digital_ptz_response() {
    // Test digital PTZ off
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::DigitalPtz).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::DigitalPtz { enabled }) => {
            assert!(!enabled);
        }
        _ => panic!("Expected DigitalPtz inquiry response"),
    }

    // Test digital PTZ on
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::DigitalPtz).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::DigitalPtz { enabled }) => {
            assert!(enabled);
        }
        _ => panic!("Expected DigitalPtz inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::DigitalPtz);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_auto_white_balance_sensitivity_response() {
    use grafton_visca::command::AutoWhiteBalanceSensitivity;

    // Test Low sensitivity
    let response = vec![0x90, 0x50, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::AutoWhiteBalanceSensitivity).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::AutoWhiteBalanceSensitivity { sensitivity }) => {
            assert_eq!(sensitivity, AutoWhiteBalanceSensitivity::Low);
        }
        _ => panic!("Expected AutoWhiteBalanceSensitivity inquiry response"),
    }

    // Test Normal sensitivity
    let response = vec![0x90, 0x50, 0x01, 0xFF];
    let result = parse_response(&response, &ResponseType::AutoWhiteBalanceSensitivity).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::AutoWhiteBalanceSensitivity { sensitivity }) => {
            assert_eq!(sensitivity, AutoWhiteBalanceSensitivity::Normal);
        }
        _ => panic!("Expected AutoWhiteBalanceSensitivity inquiry response"),
    }

    // Test High sensitivity
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::AutoWhiteBalanceSensitivity).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::AutoWhiteBalanceSensitivity { sensitivity }) => {
            assert_eq!(sensitivity, AutoWhiteBalanceSensitivity::High);
        }
        _ => panic!("Expected AutoWhiteBalanceSensitivity inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::AutoWhiteBalanceSensitivity);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_exposure_compensation_position_response() {
    // Test position 0x0000
    let response = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::ExposureCompensationPosition).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ExposureCompensationPosition { position }) => {
            assert_eq!(position, 0x0000);
        }
        _ => panic!("Expected ExposureCompensationPosition inquiry response"),
    }

    // Test position 0x1234
    let response = vec![0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::ExposureCompensationPosition).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ExposureCompensationPosition { position }) => {
            assert_eq!(position, 0x1234);
        }
        _ => panic!("Expected ExposureCompensationPosition inquiry response"),
    }

    // Test invalid length
    let response = vec![0x90, 0x50, 0x01, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::ExposureCompensationPosition);
    assert!(matches!(result, Err(Error::InvalidResponseLength)));
}

#[test]
fn test_red_tuning_response() {
    // Test level 0
    let response = vec![0x90, 0x50, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::RedTuning).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::RedTuning { level }) => {
            assert_eq!(level, 0);
        }
        _ => panic!("Expected RedTuning inquiry response"),
    }

    // Test level 255
    let response = vec![0x90, 0x50, 0xFF, 0xFF];
    let result = parse_response(&response, &ResponseType::RedTuning).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::RedTuning { level }) => {
            assert_eq!(level, 0xFF);
        }
        _ => panic!("Expected RedTuning inquiry response"),
    }
}

#[test]
fn test_blue_tuning_response() {
    // Test level 0
    let response = vec![0x90, 0x50, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::BlueTuning).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::BlueTuning { level }) => {
            assert_eq!(level, 0);
        }
        _ => panic!("Expected BlueTuning inquiry response"),
    }

    // Test level 128
    let response = vec![0x90, 0x50, 0x80, 0xFF];
    let result = parse_response(&response, &ResponseType::BlueTuning).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::BlueTuning { level }) => {
            assert_eq!(level, 0x80);
        }
        _ => panic!("Expected BlueTuning inquiry response"),
    }
}

#[test]
fn test_auto_trace_response() {
    // Test auto trace off
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::AutoTrace).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::AutoTrace { enabled }) => {
            assert!(!enabled);
        }
        _ => panic!("Expected AutoTrace inquiry response"),
    }

    // Test auto trace on
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::AutoTrace).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::AutoTrace { enabled }) => {
            assert!(enabled);
        }
        _ => panic!("Expected AutoTrace inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::AutoTrace);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_focus_unlock_response() {
    // Test focus locked
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::FocusUnlock).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::FocusUnlock { unlocked }) => {
            assert!(!unlocked);
        }
        _ => panic!("Expected FocusUnlock inquiry response"),
    }

    // Test focus unlocked
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::FocusUnlock).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::FocusUnlock { unlocked }) => {
            assert!(unlocked);
        }
        _ => panic!("Expected FocusUnlock inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::FocusUnlock);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_sharpness_position_response() {
    // Test valid sharpness position
    let response = vec![0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::SharpnessPosition).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::SharpnessPosition { position }) => {
            assert_eq!(position, 0x1234);
        }
        _ => panic!("Expected SharpnessPosition inquiry response"),
    }

    // Test invalid length
    let response = vec![0x90, 0x50, 0x01, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::SharpnessPosition);
    assert!(matches!(result, Err(Error::InvalidResponseLength)));
}

#[test]
fn test_nr_level_response() {
    // Test noise reduction level
    let response = vec![0x90, 0x50, 0x05, 0xFF];
    let result = parse_response(&response, &ResponseType::NrLevel).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NrLevel(level)) => {
            assert_eq!(level, 0x05);
        }
        _ => panic!("Expected NrLevel inquiry response"),
    }

    // Test invalid length
    let response = vec![0x90, 0x50, 0x05, 0x06, 0xFF];
    let result = parse_response(&response, &ResponseType::NrLevel);
    assert!(matches!(result, Err(Error::InvalidResponseLength)));
}

#[test]
fn test_broadcast_domain_response() {
    // Test broadcast domain
    let response = vec![0x90, 0x50, 0x01, 0xFF];
    let result = parse_response(&response, &ResponseType::BroadcastDomain).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::BroadcastDomain(domain)) => {
            assert_eq!(domain, 0x01);
        }
        _ => panic!("Expected BroadcastDomain inquiry response"),
    }

    // Test invalid length
    let response = vec![0x90, 0x50, 0x01, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::BroadcastDomain);
    assert!(matches!(result, Err(Error::InvalidResponseLength)));
}

#[test]
fn test_motion_sync_mode_response() {
    use grafton_visca::command::system::MotionSyncMode;

    // Test motion sync off
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::MotionSyncMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::MotionSyncMode { mode }) => {
            assert_eq!(mode, MotionSyncMode::Off);
        }
        _ => panic!("Expected MotionSyncMode inquiry response"),
    }

    // Test motion sync on
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::MotionSyncMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::MotionSyncMode { mode }) => {
            assert_eq!(mode, MotionSyncMode::On);
        }
        _ => panic!("Expected MotionSyncMode inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::MotionSyncMode);
    assert!(result.is_err());
}

#[test]
fn test_motion_sync_speed_response() {
    use grafton_visca::command::system::MotionSyncSpeed;

    // Test slow speed
    let response = vec![0x90, 0x50, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::MotionSyncSpeed).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::MotionSyncSpeed { speed }) => {
            assert_eq!(speed, MotionSyncSpeed::Slow);
        }
        _ => panic!("Expected MotionSyncSpeed inquiry response"),
    }

    // Test normal speed
    let response = vec![0x90, 0x50, 0x01, 0xFF];
    let result = parse_response(&response, &ResponseType::MotionSyncSpeed).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::MotionSyncSpeed { speed }) => {
            assert_eq!(speed, MotionSyncSpeed::Normal);
        }
        _ => panic!("Expected MotionSyncSpeed inquiry response"),
    }

    // Test fast speed
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::MotionSyncSpeed).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::MotionSyncSpeed { speed }) => {
            assert_eq!(speed, MotionSyncSpeed::Fast);
        }
        _ => panic!("Expected MotionSyncSpeed inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::MotionSyncSpeed);
    assert!(result.is_err());
}

#[test]
fn test_nr_mode_response() {
    use grafton_visca::command::image_adjustment::NrMode;

    // Test noise reduction off
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::NrMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NrMode { mode }) => {
            assert_eq!(mode, NrMode::Off);
        }
        _ => panic!("Expected NrMode inquiry response"),
    }

    // Test noise reduction on
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::NrMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NrMode { mode }) => {
            assert_eq!(mode, NrMode::On);
        }
        _ => panic!("Expected NrMode inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::NrMode);
    assert!(result.is_err());
}

#[test]
fn test_nr_speed_response() {
    use grafton_visca::command::image_adjustment::NrSpeed;

    // Test slow speed
    let response = vec![0x90, 0x50, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::NrSpeed).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NrSpeed { speed }) => {
            assert_eq!(speed, NrSpeed::Slow);
        }
        _ => panic!("Expected NrSpeed inquiry response"),
    }

    // Test normal speed
    let response = vec![0x90, 0x50, 0x01, 0xFF];
    let result = parse_response(&response, &ResponseType::NrSpeed).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NrSpeed { speed }) => {
            assert_eq!(speed, NrSpeed::Normal);
        }
        _ => panic!("Expected NrSpeed inquiry response"),
    }

    // Test fast speed
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::NrSpeed).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NrSpeed { speed }) => {
            assert_eq!(speed, NrSpeed::Fast);
        }
        _ => panic!("Expected NrSpeed inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::NrSpeed);
    assert!(result.is_err());
}

#[test]
fn test_black_white_mode_response() {
    use grafton_visca::command::image_adjustment::BlackWhiteMode;

    // Test color mode
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::BlackWhiteMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::BlackWhiteMode { mode }) => {
            assert_eq!(mode, BlackWhiteMode::Color);
        }
        _ => panic!("Expected BlackWhiteMode inquiry response"),
    }

    // Test black and white mode
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::BlackWhiteMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::BlackWhiteMode { mode }) => {
            assert_eq!(mode, BlackWhiteMode::BlackWhite);
        }
        _ => panic!("Expected BlackWhiteMode inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::BlackWhiteMode);
    assert!(result.is_err());
}

#[test]
fn test_usb_audio_response() {
    // Test USB audio off
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::UsbAudio).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::UsbAudio { on }) => {
            assert!(!on);
        }
        _ => panic!("Expected UsbAudio inquiry response"),
    }

    // Test USB audio on
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::UsbAudio).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::UsbAudio { on }) => {
            assert!(on);
        }
        _ => panic!("Expected UsbAudio inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::UsbAudio);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_two_tone_mode_response() {
    // Test two tone mode off
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::TwoToneMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::TwoToneMode { on }) => {
            assert!(!on);
        }
        _ => panic!("Expected TwoToneMode inquiry response"),
    }

    // Test two tone mode on
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::TwoToneMode).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::TwoToneMode { on }) => {
            assert!(on);
        }
        _ => panic!("Expected TwoToneMode inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::TwoToneMode);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_nd_filter_preset_response() {
    // Test preset 0 (clear)
    let response = vec![0x90, 0x50, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::NdFilterPreset).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NdFilterPreset { preset }) => {
            assert_eq!(preset, 0x00);
        }
        _ => panic!("Expected NdFilterPreset inquiry response"),
    }

    // Test preset 1
    let response = vec![0x90, 0x50, 0x01, 0xFF];
    let result = parse_response(&response, &ResponseType::NdFilterPreset).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NdFilterPreset { preset }) => {
            assert_eq!(preset, 0x01);
        }
        _ => panic!("Expected NdFilterPreset inquiry response"),
    }

    // Test preset 5
    let response = vec![0x90, 0x50, 0x05, 0xFF];
    let result = parse_response(&response, &ResponseType::NdFilterPreset).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NdFilterPreset { preset }) => {
            assert_eq!(preset, 0x05);
        }
        _ => panic!("Expected NdFilterPreset inquiry response"),
    }

    // Test invalid length
    let response = vec![0x90, 0x50, 0x01, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::NdFilterPreset);
    assert!(matches!(result, Err(Error::InvalidResponseLength)));
}

#[test]
fn test_digital_response() {
    // Test digital mode off
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::Digital).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::Digital { on }) => {
            assert!(!on);
        }
        _ => panic!("Expected Digital inquiry response"),
    }

    // Test digital mode on
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::Digital).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::Digital { on }) => {
            assert!(on);
        }
        _ => panic!("Expected Digital inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::Digital);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_tally_auto_adjust_response() {
    // Test tally auto adjust off
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::TallyAutoAdjust).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::TallyAutoAdjust { on }) => {
            assert!(!on);
        }
        _ => panic!("Expected TallyAutoAdjust inquiry response"),
    }

    // Test tally auto adjust on
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::TallyAutoAdjust).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::TallyAutoAdjust { on }) => {
            assert!(on);
        }
        _ => panic!("Expected TallyAutoAdjust inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::TallyAutoAdjust);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_rtmp_response() {
    // Test RTMP off
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::Rtmp).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::Rtmp { on }) => {
            assert!(!on);
        }
        _ => panic!("Expected Rtmp inquiry response"),
    }

    // Test RTMP on
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::Rtmp).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::Rtmp { on }) => {
            assert!(on);
        }
        _ => panic!("Expected Rtmp inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::Rtmp);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_zoom_out_response() {
    // Test zoom out inactive
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::ZoomOut).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ZoomOut { active }) => {
            assert!(!active);
        }
        _ => panic!("Expected ZoomOut inquiry response"),
    }

    // Test zoom out active
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::ZoomOut).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ZoomOut { active }) => {
            assert!(active);
        }
        _ => panic!("Expected ZoomOut inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::ZoomOut);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_zoom_in_response() {
    // Test zoom in inactive
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::ZoomIn).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ZoomIn { active }) => {
            assert!(!active);
        }
        _ => panic!("Expected ZoomIn inquiry response"),
    }

    // Test zoom in active
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::ZoomIn).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ZoomIn { active }) => {
            assert!(active);
        }
        _ => panic!("Expected ZoomIn inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::ZoomIn);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_iris_up_response() {
    // Test iris up inactive
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::IrisUp).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::IrisUp { active }) => {
            assert!(!active);
        }
        _ => panic!("Expected IrisUp inquiry response"),
    }

    // Test iris up active
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::IrisUp).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::IrisUp { active }) => {
            assert!(active);
        }
        _ => panic!("Expected IrisUp inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::IrisUp);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_iris_down_response() {
    // Test iris down inactive
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::IrisDown).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::IrisDown { active }) => {
            assert!(!active);
        }
        _ => panic!("Expected IrisDown inquiry response"),
    }

    // Test iris down active
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::IrisDown).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::IrisDown { active }) => {
            assert!(active);
        }
        _ => panic!("Expected IrisDown inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::IrisDown);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_night_day_position_response() {
    // Test night/day position 0
    let response = vec![0x90, 0x50, 0x00, 0xFF];
    let result = parse_response(&response, &ResponseType::NightDayPosition).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NightDayPosition { position }) => {
            assert_eq!(position, 0x00);
        }
        _ => panic!("Expected NightDayPosition inquiry response"),
    }

    // Test night/day position 5
    let response = vec![0x90, 0x50, 0x05, 0xFF];
    let result = parse_response(&response, &ResponseType::NightDayPosition).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NightDayPosition { position }) => {
            assert_eq!(position, 0x05);
        }
        _ => panic!("Expected NightDayPosition inquiry response"),
    }

    // Test night/day position 255
    let response = vec![0x90, 0x50, 0xFF, 0xFF];
    let result = parse_response(&response, &ResponseType::NightDayPosition).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NightDayPosition { position }) => {
            assert_eq!(position, 0xFF);
        }
        _ => panic!("Expected NightDayPosition inquiry response"),
    }

    // Test invalid length
    let response = vec![0x90, 0x50, 0x05, 0x06, 0xFF];
    let result = parse_response(&response, &ResponseType::NightDayPosition);
    assert!(matches!(result, Err(Error::InvalidResponseLength)));
}

#[test]
fn test_focus_near_far_response() {
    // Test focus far
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::FocusNearFar).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::FocusNearFar { near }) => {
            assert!(!near); // Far active
        }
        _ => panic!("Expected FocusNearFar inquiry response"),
    }

    // Test focus near
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::FocusNearFar).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::FocusNearFar { near }) => {
            assert!(near); // Near active
        }
        _ => panic!("Expected FocusNearFar inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::FocusNearFar);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_zoom_tele_wide_response() {
    // Test zoom wide
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::ZoomTeleWide).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ZoomTeleWide { tele }) => {
            assert!(!tele); // Wide active
        }
        _ => panic!("Expected ZoomTeleWide inquiry response"),
    }

    // Test zoom tele
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::ZoomTeleWide).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::ZoomTeleWide { tele }) => {
            assert!(tele); // Tele active
        }
        _ => panic!("Expected ZoomTeleWide inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::ZoomTeleWide);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

#[test]
fn test_night_day_switch_response() {
    // Test night/day switch disabled
    let response = vec![0x90, 0x50, 0x02, 0xFF];
    let result = parse_response(&response, &ResponseType::NightDaySwitch).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NightDaySwitch { enabled }) => {
            assert!(!enabled);
        }
        _ => panic!("Expected NightDaySwitch inquiry response"),
    }

    // Test night/day switch enabled
    let response = vec![0x90, 0x50, 0x03, 0xFF];
    let result = parse_response(&response, &ResponseType::NightDaySwitch).unwrap();
    match result {
        Response::InquiryResponse(InquiryResponse::NightDaySwitch { enabled }) => {
            assert!(enabled);
        }
        _ => panic!("Expected NightDaySwitch inquiry response"),
    }

    // Test invalid value
    let response = vec![0x90, 0x50, 0x04, 0xFF];
    let result = parse_response(&response, &ResponseType::NightDaySwitch);
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}
