//! Demonstration of test fixtures and generators usage.
//!
//! This test file shows how to use CommandFixtures and generators
//! from the test_utils module for comprehensive testing.

#![cfg(test)]

mod common;

use crate::common::{
    patterns,
    test_fixtures::{generators, CommandFixtures},
    MockResponse, MockTransport, MockTransportBuilder, ProtocolValidator, ScenarioBuilder,
    ValidationMode,
};
use grafton_visca::{
    blocking::*,
    camera::{Camera, CameraModel},
    command::preset::PresetNumber,
};
use std::time::Duration;

#[test]
fn test_all_power_commands_with_fixtures() {
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // Get all power command fixtures
    let power_cmds = CommandFixtures::power_commands();

    // Use MockTransportBuilder for power command expectations in exact order
    let mut builder = MockTransportBuilder::new().connected(true);

    // Only expect the commands we'll actually send in the correct order
    builder = builder.expect(
        &power_cmds["power_on"],
        vec![
            MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
            MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
        ],
    );
    builder = builder.expect(
        &power_cmds["power_off"],
        vec![
            MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
            MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
        ],
    );

    // Validate each command
    for (name, cmd_bytes) in &power_cmds {
        assert!(
            validator.validate_command(cmd_bytes).is_ok(),
            "Command '{}' should be protocol compliant",
            name
        );
    }

    let mock = builder.build();
    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, mock.clone()).blocking();

    // Execute the commands in exact order
    camera.power_on().unwrap();
    camera.power_off().unwrap();

    // MockTransportBuilder automatically verifies expectations when dropped
}

#[test]
fn test_zoom_commands_with_fixtures() {
    let zoom_cmds = CommandFixtures::zoom_commands();

    // Use MockTransportBuilder for zoom command expectations in correct order
    let mut builder = MockTransportBuilder::new().connected(true);

    // Set up expectations in the exact order we'll call them
    builder = builder.expect(
        &zoom_cmds["zoom_stop"],
        vec![
            MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
            MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
        ],
    );
    // zoom_in() sends TeleVariable with speed 4: [81, 01, 04, 07, 24, FF]
    builder = builder.expect(
        &[0x81, 0x01, 0x04, 0x07, 0x24, 0xFF],
        vec![
            MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
            MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
        ],
    );
    // zoom_out() sends WideVariable with speed 4: [81, 01, 04, 07, 34, FF]
    builder = builder.expect(
        &[0x81, 0x01, 0x04, 0x07, 0x34, 0xFF],
        vec![
            MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
            MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
        ],
    );

    let mock = builder.build();
    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, mock.clone()).blocking();

    // Execute zoom commands in the same order as expectations
    camera.zoom_stop().unwrap();
    camera.zoom_in().unwrap();
    camera.zoom_out().unwrap();

    // MockTransportBuilder automatically verifies expectations when dropped
}

#[test]
fn test_preset_commands_with_fixtures() {
    let preset_cmds = CommandFixtures::preset_commands();

    // Use MockTransportBuilder for preset command expectations in correct order
    let mut builder = MockTransportBuilder::new().connected(true);

    // Set up expectations in the exact order we'll call them
    builder = builder.expect(
        &preset_cmds["preset_set_1"],
        vec![
            MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
            MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
        ],
    );
    builder = builder.expect(
        &preset_cmds["preset_recall_1"],
        vec![
            MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
            MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
        ],
    );
    builder = builder.expect(
        &preset_cmds["preset_set_2"],
        vec![
            MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
            MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
        ],
    );
    builder = builder.expect(
        &preset_cmds["preset_recall_2"],
        vec![
            MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
            MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
        ],
    );

    let mock = builder.build();
    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, mock.clone()).blocking();

    // Test preset operations in the same order as expectations
    camera.preset_set(PresetNumber::new(1).unwrap()).unwrap();
    camera.preset_recall(PresetNumber::new(1).unwrap()).unwrap();
    camera.preset_set(PresetNumber::new(2).unwrap()).unwrap();
    camera.preset_recall(PresetNumber::new(2).unwrap()).unwrap();

    // MockTransportBuilder automatically verifies expectations when dropped
}

#[test]
fn test_edge_case_commands() {
    let edge_cases = CommandFixtures::edge_case_commands();
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    for (name, cmd_bytes) in &edge_cases {
        println!("Testing edge case: {}", name);

        // Validate protocol compliance even for edge cases
        let result = validator.validate_command(cmd_bytes);

        // Some edge cases might be intentionally invalid
        if result.is_err() {
            println!("  Edge case '{}' is invalid: {:?}", name, result);
        } else {
            println!("  Edge case '{}' is valid", name);
        }
    }
}

#[test]
fn test_zoom_positions_with_generator() {
    let zoom_positions = generators::zoom_positions();

    // Use MockTransportBuilder for zoom position expectations
    let mut builder = MockTransportBuilder::new().connected(true);

    // Test a subset of zoom positions
    for position in zoom_positions.iter().take(5) {
        // Calculate what the camera will actually send
        // The normalized value will be position / 0x7000, and the camera calculates zoom_pos = normalized * 0x7000
        let normalized = (*position as f32 / 0x7000 as f32).min(1.0);
        let zoom_pos = (normalized * 0x7000 as f32) as u16;

        // Build expected command bytes for the actual zoom position
        let cmd_bytes = vec![
            0x81,
            0x01,
            0x04,
            0x47,
            ((zoom_pos >> 12) & 0x0F) as u8,
            ((zoom_pos >> 8) & 0x0F) as u8,
            ((zoom_pos >> 4) & 0x0F) as u8,
            (zoom_pos & 0x0F) as u8,
            0xFF,
        ];

        builder = builder.expect(
            &cmd_bytes,
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
            ],
        );
    }

    let mock = builder.build();
    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, mock.clone()).blocking();

    // Execute zoom position commands
    for position in zoom_positions.iter().take(5) {
        // Convert position to normalized value (0.0 - 1.0)
        // Use 0x7000 (digital zoom max) as the maximum since that's what the camera implementation uses
        // Clamp to 1.0 to handle values that exceed the max
        let normalized_value = (*position as f32 / 0x7000 as f32).min(1.0);
        let normalized = grafton_visca::units::Normalized(normalized_value);
        camera.zoom_absolute(normalized).unwrap();
    }

    // MockTransportBuilder automatically verifies expectations when dropped
}

#[test]
fn test_pan_tilt_positions_with_generator() {
    let positions = generators::pan_tilt_positions();
    let speeds = generators::speed_values();

    // Create a scenario for pan/tilt testing
    let mut scenario = ScenarioBuilder::new("Pan/Tilt Position Test");

    // Add expectations for a few position movements
    for ((pan, tilt), &speed) in positions.iter().take(3).zip(speeds.iter()) {
        // Convert hardware position values to degrees for the command
        let pan_degrees = (*pan as f32) * 170.0 / 2448.0; // Scale to ±170 degrees
        let tilt_degrees = (*tilt as f32) * 90.0 / 1296.0; // Scale to ±90 degrees
                                                           // Convert back to hardware units for command bytes
        let pan_units = (pan_degrees * 14.4) as i16; // PTZOpticsG2 pan conversion
        let tilt_units = (tilt_degrees * 14.4) as i16; // PTZOpticsG2 tilt conversion

        // Convert speed using SpeedLevel to get the actual speeds the camera will use
        let speed_level = grafton_visca::types::SpeedLevel::from(speed);
        let pan_speed = speed_level.to_pan_speed();
        let tilt_speed = speed_level.to_tilt_speed();

        let cmd_bytes =
            build_pan_tilt_absolute_command(pan_units, tilt_units, pan_speed, tilt_speed);

        scenario = scenario.expect_command(
            &cmd_bytes,
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Delayed(
                    patterns::responses::COMPLETE_1.to_vec(),
                    Duration::from_millis(200),
                ),
            ],
        );
    }

    let mut mock = MockTransport::new();
    scenario.build().apply_to(&mut mock).unwrap();

    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, mock.clone()).blocking();

    // Execute movements
    for ((pan, tilt), &speed) in positions.iter().take(3).zip(speeds.iter()) {
        // Convert hardware position values to degrees
        // For PTZOpticsG2: pan range is approximately ±170 degrees, tilt range is approximately -30 to +90 degrees
        // The hardware values are scaled to fit within valid degree ranges
        let pan_degrees = (*pan as f32) * 170.0 / 2448.0; // Scale to ±170 degrees
        let tilt_degrees = (*tilt as f32) * 90.0 / 1296.0; // Scale to ±90 degrees
        camera
            .pan_tilt_absolute(
                grafton_visca::units::Degrees::new(pan_degrees),
                grafton_visca::units::Degrees::new(tilt_degrees),
                grafton_visca::types::SpeedLevel::from(speed),
            )
            .unwrap();
    }

    // MockTransportBuilder automatically verifies expectations when dropped
}

#[cfg(not(feature = "tokio"))]
#[test]
fn test_comprehensive_command_sequence() {
    // Test a realistic sequence of camera operations
    let scenario = ScenarioBuilder::new("Comprehensive Camera Test")
        .description("Initialize camera and perform various operations")
        // Power on
        .expect_power_on()
        // Home position
        .expect_home()
        // Zoom operations
        .expect_command(
            patterns::zoom::STOP,
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Delayed(
                    patterns::responses::COMPLETE_1.to_vec(),
                    Duration::from_millis(50),
                ),
            ],
        )
        // Preset recall
        .expect_preset_recall(1)
        // Focus operations from fixtures
        .expect_command(
            &CommandFixtures::focus_commands()[0].1, // Auto focus
            vec![
                MockResponse::Immediate(patterns::responses::ACK_2.to_vec()),
                MockResponse::Delayed(
                    patterns::responses::COMPLETE_2.to_vec(),
                    Duration::from_millis(100),
                ),
            ],
        )
        .build();

    let mut mock = MockTransport::new();
    scenario.apply_to(&mut mock).unwrap();

    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, mock.clone()).blocking();

    // Execute the sequence
    camera.power_on().unwrap();
    camera.pan_tilt_home().unwrap();
    camera.zoom_stop().unwrap();
    camera.preset_recall(PresetNumber::new(1).unwrap()).unwrap();
    camera.focus_auto().unwrap();

    // MockTransportBuilder automatically verifies expectations when dropped
}

// Helper function to build pan/tilt absolute command
fn build_pan_tilt_absolute_command(pan: i16, tilt: i16, pan_speed: u8, tilt_speed: u8) -> Vec<u8> {
    let pan_bytes = position_to_bytes(pan);
    let tilt_bytes = position_to_bytes(tilt);

    vec![
        0x81,
        0x01,
        0x06,
        0x02,
        pan_speed,
        tilt_speed,
        pan_bytes[0],
        pan_bytes[1],
        pan_bytes[2],
        pan_bytes[3],
        tilt_bytes[0],
        tilt_bytes[1],
        tilt_bytes[2],
        tilt_bytes[3],
        0xFF,
    ]
}

fn position_to_bytes(pos: i16) -> [u8; 4] {
    let unsigned = pos as u16;
    [
        ((unsigned >> 12) & 0x0F) as u8,
        ((unsigned >> 8) & 0x0F) as u8,
        ((unsigned >> 4) & 0x0F) as u8,
        (unsigned & 0x0F) as u8,
    ]
}
