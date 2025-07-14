//! Comprehensive preset command tests.
//!
//! Tests for preset position commands including set, recall, and reset operations.

#![cfg(test)]

mod common;

use crate::common::{
    patterns, test_fixtures::CommandFixtures, MockResponse, MockTransport, MockTransportBuilder,
    ProtocolValidator, ResponseBuilder, ScenarioBuilder, ValidationMode,
};
use grafton_visca::{
    blocking::*,
    camera::{Camera, CameraModel},
    command::preset::PresetNumber,
    Error,
};
use std::time::Duration;

#[test]
fn test_preset_commands_with_fixtures() {
    let preset_cmds = CommandFixtures::preset_commands();
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // Validate all preset commands
    for (name, cmd_bytes) in &preset_cmds {
        assert!(
            validator.validate_command(cmd_bytes).is_ok(),
            "Preset command '{}' should be protocol compliant",
            name
        );
    }

    // Use MockTransportBuilder for preset command expectations
    let mut builder = MockTransportBuilder::new().connected(true);

    // Set up expectations in the order they will be executed
    let commands_in_order = vec![
        vec![0x81, 0x01, 0x04, 0x3F, 0x01, 0x01, 0xFF], // preset_set_1
        vec![0x81, 0x01, 0x04, 0x3F, 0x02, 0x01, 0xFF], // preset_recall_1
        vec![0x81, 0x01, 0x04, 0x3F, 0x01, 0x02, 0xFF], // preset_set_2
        vec![0x81, 0x01, 0x04, 0x3F, 0x02, 0x02, 0xFF], // preset_recall_2
    ];

    for cmd_bytes in commands_in_order {
        builder = builder.expect(
            &cmd_bytes,
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Delayed(
                    patterns::responses::COMPLETE_1.to_vec(),
                    Duration::from_millis(100),
                ),
            ],
        );
    }

    let mock = builder.build();
    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, mock.clone()).blocking();

    // Execute preset operations
    camera.preset_set(PresetNumber::new(1).unwrap()).unwrap();
    camera.preset_recall(PresetNumber::new(1).unwrap()).unwrap();
    camera.preset_set(PresetNumber::new(2).unwrap()).unwrap();
    camera.preset_recall(PresetNumber::new(2).unwrap()).unwrap();
}

#[test]
fn test_preset_recall_with_scenario_builder() {
    // Test using the ScenarioBuilder's expect_preset_recall method
    let scenario = ScenarioBuilder::new("Preset Recall Test")
        .description("Test preset recall with settling time")
        .expect_power_on()
        .expect_home()
        .expect_preset_recall(1)
        .expect_preset_recall(5)
        .expect_preset_recall(10)
        .build();

    let mut mock = MockTransport::new();
    scenario.apply_to(&mut mock).unwrap();

    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, mock.clone()).blocking();

    // Execute the sequence
    camera.power_on().unwrap();
    camera.pan_tilt_home().unwrap();
    camera.preset_recall(PresetNumber::new(1).unwrap()).unwrap();
    camera.preset_recall(PresetNumber::new(5).unwrap()).unwrap();
    camera
        .preset_recall(PresetNumber::new(10).unwrap())
        .unwrap();
}

#[test]
fn test_preset_number_validation() {
    // Valid preset numbers (0-89)
    assert!(PresetNumber::new(0).is_ok());
    assert!(PresetNumber::new(45).is_ok());
    assert!(PresetNumber::new(89).is_ok());

    // Invalid preset numbers
    assert!(matches!(
        PresetNumber::new(90),
        Err(Error::InvalidParameter { .. })
    ));
    assert!(matches!(
        PresetNumber::new(255),
        Err(Error::InvalidParameter { .. })
    ));
}

#[test]
fn test_preset_set_command() {
    let mut mock = MockTransport::new();

    // Test preset set for various preset numbers
    let preset_numbers = vec![0, 1, 10, 45, 89];

    for preset_num in preset_numbers {
        let expected_cmd = vec![0x81, 0x01, 0x04, 0x3F, 0x01, preset_num, 0xFF];

        mock.expect_command(&expected_cmd)
            .described_as(&format!("Set preset {}", preset_num))
            .will_respond(MockResponse::Immediate(ResponseBuilder::ack(1)))
            .will_respond(MockResponse::Delayed(
                ResponseBuilder::completion(1),
                Duration::from_millis(50),
            ));
    }

    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, mock.clone()).blocking();

    // Execute preset set commands
    camera.preset_set(PresetNumber::new(0).unwrap()).unwrap();
    camera.preset_set(PresetNumber::new(1).unwrap()).unwrap();
    camera.preset_set(PresetNumber::new(10).unwrap()).unwrap();
    camera.preset_set(PresetNumber::new(45).unwrap()).unwrap();
    camera.preset_set(PresetNumber::new(89).unwrap()).unwrap();
}

#[test]
fn test_preset_recall_command() {
    let mut mock = MockTransport::new();

    // Test preset recall for various preset numbers
    let preset_numbers = vec![0, 1, 10, 45, 89];

    for preset_num in preset_numbers {
        let expected_cmd = vec![0x81, 0x01, 0x04, 0x3F, 0x02, preset_num, 0xFF];

        mock.expect_command(&expected_cmd)
            .described_as(&format!("Recall preset {}", preset_num))
            .will_respond(MockResponse::Immediate(ResponseBuilder::ack(1)))
            .will_respond(MockResponse::Delayed(
                ResponseBuilder::completion(1),
                Duration::from_millis(2000), // Longer delay for movement
            ));
    }

    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, mock.clone()).blocking();

    // Execute preset recall commands
    camera.preset_recall(PresetNumber::new(0).unwrap()).unwrap();
    camera.preset_recall(PresetNumber::new(1).unwrap()).unwrap();
    camera
        .preset_recall(PresetNumber::new(10).unwrap())
        .unwrap();
    camera
        .preset_recall(PresetNumber::new(45).unwrap())
        .unwrap();
    camera
        .preset_recall(PresetNumber::new(89).unwrap())
        .unwrap();
}

// Note: preset_reset is not exposed in the public API, only set and recall are available
// The command exists in the protocol but is not surfaced in the camera methods

#[test]
fn test_preset_command_sequence() {
    // Test a realistic sequence of preset operations
    let scenario = ScenarioBuilder::new("Preset Command Sequence")
        .description("Store positions and recall them")
        .expect_power_on()
        .expect_home()
        // Store preset 1 at home position
        .expect_command(
            &[0x81, 0x01, 0x04, 0x3F, 0x01, 0x01, 0xFF],
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Delayed(
                    patterns::responses::COMPLETE_1.to_vec(),
                    Duration::from_millis(50),
                ),
            ],
        )
        // Move to a new position (pan/tilt)
        .expect_command(
            &[
                0x81, 0x01, 0x06, 0x02, 0x18, 0x14, 0x00, 0x02, 0x0D, 0x00, 0x00, 0x01, 0x02, 0x00,
                0xFF,
            ],
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Delayed(
                    patterns::responses::COMPLETE_1.to_vec(),
                    Duration::from_millis(1000),
                ),
            ],
        )
        // Store preset 2 at new position
        .expect_command(
            &[0x81, 0x01, 0x04, 0x3F, 0x01, 0x02, 0xFF],
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Delayed(
                    patterns::responses::COMPLETE_1.to_vec(),
                    Duration::from_millis(50),
                ),
            ],
        )
        // Recall preset 1
        .expect_preset_recall(1)
        // Recall preset 2
        .expect_preset_recall(2)
        .build();

    let mut mock = MockTransport::new();
    scenario.apply_to(&mut mock).unwrap();

    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, mock.clone()).blocking();

    // Execute the sequence
    camera.power_on().unwrap();
    camera.pan_tilt_home().unwrap();
    camera.preset_set(PresetNumber::new(1).unwrap()).unwrap();

    // Move to a new position
    camera
        .pan_tilt_absolute(
            grafton_visca::units::Degrees::new(50.0),
            grafton_visca::units::Degrees::new(20.0),
            grafton_visca::types::SpeedLevel::from(0x18),
        )
        .unwrap();

    camera.preset_set(PresetNumber::new(2).unwrap()).unwrap();
    camera.preset_recall(PresetNumber::new(1).unwrap()).unwrap();
    camera.preset_recall(PresetNumber::new(2).unwrap()).unwrap();
}

#[test]
fn test_preset_error_handling() {
    let mut mock = MockTransport::new();

    // Test error response for preset recall
    mock.expect_command(&[0x81, 0x01, 0x04, 0x3F, 0x02, 0x05, 0xFF])
        .described_as("Recall preset 5 with error")
        .will_respond(MockResponse::Immediate(ResponseBuilder::error(
            patterns::responses::NOT_EXECUTABLE[2],
        )));

    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, mock.clone()).blocking();

    // Should get an error
    let result = camera.preset_recall(PresetNumber::new(5).unwrap());
    assert!(result.is_err());
}

#[test]
fn test_preset_with_timeout() {
    let mut mock = MockTransport::new();

    // Test timeout during preset recall
    mock.expect_command(&[0x81, 0x01, 0x04, 0x3F, 0x02, 0x03, 0xFF])
        .described_as("Recall preset 3 with timeout")
        .will_respond(MockResponse::Immediate(ResponseBuilder::ack(1)));
    // No completion response - will timeout

    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, mock.clone()).blocking();

    // Should timeout
    let result = camera.preset_recall(PresetNumber::new(3).unwrap());
    assert!(matches!(result, Err(Error::Timeout)));
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_preset_commands_async() {
    use grafton_visca::r#async::PresetsOps;
    let mut mock = MockTransport::new();

    // Set up expectations for async preset operations
    mock.expect_command(&[0x81, 0x01, 0x04, 0x3F, 0x01, 0x01, 0xFF])
        .described_as("Async preset set 1")
        .will_respond(MockResponse::Immediate(ResponseBuilder::ack(1)))
        .will_respond(MockResponse::Delayed(
            ResponseBuilder::completion(1),
            Duration::from_millis(50),
        ));

    mock.expect_command(&[0x81, 0x01, 0x04, 0x3F, 0x02, 0x01, 0xFF])
        .described_as("Async preset recall 1")
        .will_respond(MockResponse::Immediate(ResponseBuilder::ack(1)))
        .will_respond(MockResponse::Delayed(
            ResponseBuilder::completion(1),
            Duration::from_millis(2000),
        ));

    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, mock.clone()).r#async();

    // Execute async operations
    camera
        .preset_set(PresetNumber::new(1).unwrap())
        .await
        .unwrap();
    camera
        .preset_recall(PresetNumber::new(1).unwrap())
        .await
        .unwrap();
}
