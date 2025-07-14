//! Enhanced response parsing tests using the new testing infrastructure.
//!
//! These tests use MockTransport, ProtocolValidator, and ResponseBuilder
//! to thoroughly test VISCA response parsing with simulated camera interactions.

mod common;

use crate::common::{
    patterns, MockResponse, MockTransport, ProtocolValidator, ResponseBuilder, ScenarioBuilder,
    ValidationMode,
};
use grafton_visca::{
    blocking::*,
    camera::{Camera, ProfileId},
    command::{
        gain::AntiFlickerMode, AutoFocusSensitivity, ExposureMode, FocusZone, WhiteBalanceMode,
    },
};
use std::time::Duration;

#[test]
fn test_power_inquiry_with_mock_transport() {
    let mut mock = MockTransport::new();
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // For inquiry commands, the camera expects:
    // 1. ACK response first
    // 2. Then the inquiry data response
    // The mock transport processes responses in order
    mock.expect_command(patterns::inquiry::POWER)
        .described_as("power inquiry")
        .will_ack(1)
        .will_return_data(&[0x02]); // Power on - creates [0x90, 0x50, 0x02, 0xFF]

    let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

    // Execute inquiry
    let result = camera.get_power_state();
    match result {
        Ok(on) => {
            println!("Power state: {}", on);
            assert!(on);
        }
        Err(e) => {
            println!("Error: {:?}", e);
            println!("Sent commands: {:?}", mock.sent_history());
            println!("Response history: {:?}", mock.response_history());
            panic!("Failed to get power state: {:?}", e);
        }
    }

    // Validate protocol compliance
    let sent_commands = mock.sent_history();
    for cmd in &sent_commands {
        assert!(validator.validate_command(cmd).is_ok());
    }

    mock.verify().unwrap();
}

#[test]
fn test_zoom_position_inquiry_with_validation() {
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // Test multiple zoom positions using MockTransportBuilder
    let zoom_positions = vec![
        (0x0000, "minimum zoom"),
        (0x4000, "middle zoom"),
        (0x7000, "maximum zoom"),
    ];

    let mut builder = crate::common::MockTransportBuilder::new().connected(true);

    for (position, _description) in &zoom_positions {
        builder = builder.expect(
            patterns::inquiry::ZOOM_POSITION,
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Immediate(
                    ResponseBuilder::inquiry()
                        .add_byte(((position >> 12) & 0x0F) as u8)
                        .add_byte(((position >> 8) & 0x0F) as u8)
                        .add_byte(((position >> 4) & 0x0F) as u8)
                        .add_byte((position & 0x0F) as u8)
                        .build(),
                ),
            ],
        );
    }

    let mock = builder.build();
    let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

    // Execute inquiries
    for (expected_position, _) in &zoom_positions {
        let result = camera.get_zoom_position().unwrap();
        assert_eq!(result, *expected_position);
    }

    // Validate all sent commands
    let sent_commands = mock.sent_history();
    for cmd in &sent_commands {
        assert!(validator.validate_command(cmd).is_ok());
    }

    // MockTransportBuilder automatically verifies expectations when dropped
}

#[test]
fn test_pan_tilt_position_inquiry_comprehensive() {
    let scenario = ScenarioBuilder::new("Pan/Tilt Position Inquiry Test")
        .description("Test pan/tilt position inquiry with various positions")
        .expect_command(
            patterns::inquiry::PAN_TILT_POSITION,
            vec![
                MockResponse::Immediate(ResponseBuilder::ack(1)),
                MockResponse::Immediate(
                    ResponseBuilder::inquiry()
                        .add_u16_nibbles(0x0000) // Pan: 0
                        .add_u16_nibbles(0x0000) // Tilt: 0
                        .build(),
                ),
            ],
        )
        .expect_command(
            patterns::inquiry::PAN_TILT_POSITION,
            vec![
                MockResponse::Immediate(ResponseBuilder::ack(1)),
                MockResponse::Immediate(
                    ResponseBuilder::inquiry()
                        .add_u16_nibbles(0x1000) // Pan: positive
                        .add_u16_nibbles(0x0800) // Tilt: positive
                        .build(),
                ),
            ],
        )
        .build();

    let mut mock = MockTransport::new();
    scenario.apply_to(&mut mock).unwrap();

    let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

    // Test center position
    let (pan, tilt) = camera.get_pan_tilt_position().unwrap();
    assert_eq!(pan, 0);
    assert_eq!(tilt, 0);

    // Test offset position
    let (pan, tilt) = camera.get_pan_tilt_position().unwrap();
    assert_eq!(pan, 0x1000);
    assert_eq!(tilt, 0x0800);

    mock.verify().unwrap();
}

#[test]
fn test_error_response_handling() {
    // Use MockTransportBuilder for error scenarios
    let mock = crate::common::MockTransportBuilder::new()
        .connected(true)
        .expect(
            patterns::inquiry::POWER,
            vec![MockResponse::Immediate(ResponseBuilder::error(0x02))], // Syntax error
        )
        .expect(
            patterns::inquiry::ZOOM_POSITION,
            vec![MockResponse::Immediate(ResponseBuilder::error(0x03))], // Buffer full
        )
        .expect(
            patterns::inquiry::PAN_TILT_POSITION,
            vec![MockResponse::Immediate(ResponseBuilder::error(0x41))], // Not executable
        )
        .build();

    let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

    // Test error handling
    assert!(camera.get_power_state().is_err());
    assert!(camera.get_zoom_position().is_err());
    assert!(camera.get_pan_tilt_position().is_err());

    // MockTransportBuilder automatically verifies expectations when dropped
}

#[test]
fn test_exposure_mode_inquiry_all_modes() {
    // Test all exposure modes using MockTransportBuilder
    let exposure_modes = vec![
        (0x00, ExposureMode::Auto, "auto exposure"),
        (0x03, ExposureMode::Manual, "manual exposure"),
        (0x0A, ExposureMode::Shutter, "shutter priority"),
        (0x0B, ExposureMode::Iris, "iris priority"),
        (0x0D, ExposureMode::Bright, "bright mode"),
    ];

    let mut builder = crate::common::MockTransportBuilder::new().connected(true);

    for (mode_byte, _expected_mode, _description) in &exposure_modes {
        builder = builder.expect(
            patterns::inquiry::EXPOSURE_MODE,
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Immediate(ResponseBuilder::inquiry().add_byte(*mode_byte).build()),
            ],
        );
    }

    let mock = builder.build();
    let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

    // Execute inquiries and verify results
    for (_, expected_mode, _) in &exposure_modes {
        let result = camera.get_exposure_mode().unwrap();
        assert_eq!(result, *expected_mode);
    }

    // MockTransportBuilder automatically verifies expectations when dropped
}

#[test]
fn test_white_balance_inquiry_with_protocol_validation() {
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // Test white balance modes using MockTransportBuilder
    let wb_modes = vec![
        (0x00, WhiteBalanceMode::Auto),
        (0x01, WhiteBalanceMode::Indoor),
        (0x02, WhiteBalanceMode::Outdoor),
        (0x03, WhiteBalanceMode::OnePush),
        (0x05, WhiteBalanceMode::Manual),
    ];

    let mut builder = crate::common::MockTransportBuilder::new().connected(true);

    for (mode_byte, _) in &wb_modes {
        builder = builder.expect(
            patterns::inquiry::WHITE_BALANCE_MODE,
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Immediate(ResponseBuilder::inquiry().add_byte(*mode_byte).build()),
            ],
        );
    }

    let mock = builder.build();
    let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

    for (_, expected_mode) in &wb_modes {
        let result = camera.get_white_balance_mode().unwrap();
        assert_eq!(result, *expected_mode);
    }

    // Validate protocol compliance for all commands
    let sent_commands = mock.sent_history();
    for cmd in &sent_commands {
        validator.validate_command(cmd).unwrap();
    }

    // Check that all sockets are free after completion
    assert!(validator.all_sockets_free());

    // MockTransportBuilder automatically verifies expectations when dropped
}

#[test]
fn test_complex_inquiry_sequence_with_timing() {
    let scenario = ScenarioBuilder::new("Complex Inquiry Sequence")
        .description("Test realistic sequence of camera inquiries with timing")
        .expect_command(
            patterns::inquiry::POWER,
            vec![
                MockResponse::Immediate(ResponseBuilder::ack(1)),
                MockResponse::Delayed(
                    ResponseBuilder::inquiry()
                        .add_byte(0x02) // Power on
                        .build(),
                    Duration::from_millis(10),
                ),
            ],
        )
        .expect_command(
            patterns::inquiry::ZOOM_POSITION,
            vec![
                MockResponse::Immediate(ResponseBuilder::ack(1)),
                MockResponse::Delayed(
                    ResponseBuilder::inquiry()
                        .add_u16_nibbles(0x4000) // Mid zoom
                        .build(),
                    Duration::from_millis(20),
                ),
            ],
        )
        .expect_command(
            patterns::inquiry::PAN_TILT_POSITION,
            vec![
                MockResponse::Immediate(ResponseBuilder::ack(1)),
                MockResponse::Delayed(
                    ResponseBuilder::inquiry()
                        .add_u16_nibbles(0x0000) // Pan: center
                        .add_u16_nibbles(0x0000) // Tilt: center
                        .build(),
                    Duration::from_millis(30),
                ),
            ],
        )
        .build();

    let mut mock = MockTransport::new();
    scenario.apply_to(&mut mock).unwrap();

    let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

    // Execute inquiry sequence
    let power_on = camera.get_power_state().unwrap();
    assert!(power_on);

    let zoom_pos = camera.get_zoom_position().unwrap();
    assert_eq!(zoom_pos, 0x4000);

    let (pan, tilt) = camera.get_pan_tilt_position().unwrap();
    assert_eq!(pan, 0);
    assert_eq!(tilt, 0);

    mock.verify().unwrap();
}

#[test]
fn test_anti_flicker_mode_parsing() {
    // Test all anti-flicker modes
    let modes = vec![
        (0x00, AntiFlickerMode::Off),
        (0x01, AntiFlickerMode::Hz50),
        (0x02, AntiFlickerMode::Hz60),
    ];

    for (mode_byte, expected_mode) in &modes {
        // Create a fresh mock for each test to avoid ordering issues
        let mut mock = MockTransport::new();

        mock.expect_command(patterns::inquiry::ANTI_FLICKER)
            .described_as(&format!("anti-flicker mode {:?}", expected_mode))
            .will_ack(1)
            .will_return_data(&[*mode_byte]);

        let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

        let result = camera.get_anti_flicker();
        match result {
            Ok(mode) => assert_eq!(mode, *expected_mode),
            Err(e) => {
                // Print debug info
                println!("Error getting anti-flicker mode: {:?}", e);
                println!("Sent commands: {:?}", mock.sent_history());
                println!("Response history: {:?}", mock.response_history());
                panic!("Failed to get anti-flicker mode");
            }
        }

        mock.verify().unwrap();
    }
}

#[test]
fn test_focus_zone_inquiry_comprehensive() {
    // Test all focus zones
    let zones = vec![
        (0x00, FocusZone::Top),
        (0x01, FocusZone::Center),
        (0x02, FocusZone::Bottom),
    ];

    for (zone_byte, expected_zone) in &zones {
        // Create a fresh mock for each test to avoid ordering issues
        let mut mock = MockTransport::new();

        mock.expect_command(patterns::inquiry::FOCUS_ZONE)
            .described_as(&format!("focus zone {:?}", expected_zone))
            .will_ack(1)
            .will_return_data(&[*zone_byte]);

        let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

        let result = camera.get_focus_zone().unwrap();
        assert_eq!(result, *expected_zone);

        mock.verify().unwrap();
    }
}

#[test]
fn test_auto_focus_sensitivity_inquiry() {
    // Test all sensitivity levels
    let sensitivities = vec![
        (0x00, AutoFocusSensitivity::Low),
        (0x01, AutoFocusSensitivity::Normal),
        (0x02, AutoFocusSensitivity::High),
    ];

    for (sens_byte, expected_sens) in &sensitivities {
        // Create a fresh mock for each test to avoid ordering issues
        let mut mock = MockTransport::new();

        mock.expect_command(patterns::inquiry::AUTO_FOCUS_SENSITIVITY)
            .described_as(&format!("auto focus sensitivity {:?}", expected_sens))
            .will_ack(1)
            .will_return_data(&[*sens_byte]);

        let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

        let result = camera.get_auto_focus_sensitivity().unwrap();
        assert_eq!(result, *expected_sens);

        mock.verify().unwrap();
    }
}

#[test]
fn test_malformed_response_handling() {
    let mut mock = MockTransport::new();

    // Test various malformed responses
    mock.expect_command(patterns::inquiry::POWER)
        .described_as("power inquiry - malformed response")
        .will_respond(MockResponse::Immediate(vec![0x90, 0x50, 0xFF])); // Missing data

    mock.expect_command(patterns::inquiry::ZOOM_POSITION)
        .described_as("zoom inquiry - truncated response")
        .will_respond(MockResponse::Immediate(vec![0x90, 0x50, 0x01, 0xFF])); // Incomplete data

    let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

    // These should result in errors
    assert!(camera.get_power_state().is_err());
    assert!(camera.get_zoom_position().is_err());

    mock.verify().unwrap();
}

#[test]
fn test_timeout_handling() {
    let mut mock = MockTransport::new();

    // Set up command that will timeout
    mock.expect_command(patterns::inquiry::POWER)
        .described_as("power inquiry - timeout")
        .will_respond(MockResponse::Timeout);

    let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

    // This should result in a timeout error
    let result = camera.get_power_state();
    assert!(result.is_err());

    mock.verify().unwrap();
}
