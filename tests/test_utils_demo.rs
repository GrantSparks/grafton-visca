//! Demonstration of the new test utilities.
//!
//! This test file shows how to use the comprehensive test library
//! for testing VISCA implementations.

#![cfg(test)]

mod common;

use crate::common::*;
use grafton_visca::{camera::{methods::*, ProfileId, Camera}, Error, Result};
use std::time::Duration;

#[test]
fn test_with_mock_transport_basic() {
    // Create a mock transport
    let mut mock = MockTransport::new();

    // Set up expectations for power on command
    mock.expect_command(&patterns::power::ON)
        .described_as("power on command")
        .will_ack(1)
        .then_complete(1);

    // Create camera with mock transport
    let mut camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone());

    // Execute the command
    camera.power_on_blocking().unwrap();

    // Verify all expectations were met
    mock.verify().unwrap();

    // Check command history
    let history = mock.sent_history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0], patterns::power::ON);
}

#[test]
fn test_with_protocol_validator() {
    let mut mock = MockTransport::new();
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // Set up mock responses
    mock.queue_response(MockResponse::Immediate(patterns::responses::ACK_1.to_vec()));
    mock.queue_response(MockResponse::Delayed(
        patterns::responses::COMPLETE_1.to_vec(),
        Duration::from_millis(50),
    ));

    // Send command
    let command = patterns::power::ON;
    validator.validate_command(command).unwrap();
    mock.send(command).unwrap();

    // Receive responses
    let ack = mock.receive(Duration::from_millis(100)).unwrap();
    validator.validate_response(&ack).unwrap();

    let completion = mock.receive(Duration::from_millis(100)).unwrap();
    validator.validate_response(&completion).unwrap();

    // Check protocol state
    assert!(validator.all_sockets_free());

    let summary = validator.get_summary();
    assert_eq!(summary.commands_sent, 1);
    assert_eq!(summary.responses_received, 2);
}

#[test]
fn test_response_builder() {
    // Build various responses
    let ack = ResponseBuilder::ack(1);
    assert_eq!(ack, vec![0x90, 0x41, 0xFF]);

    let zoom_pos = ResponseBuilder::inquiry().add_u16_nibbles(0x4000).build();
    assert_eq!(zoom_pos, vec![0x90, 0x50, 0x04, 0x00, 0x00, 0x00, 0xFF]);

    let pan_tilt_pos = response_patterns::pan_tilt_position_response(1000, 500); // Note: changed to u16
    assert_eq!(pan_tilt_pos.len(), 11); // 0x90 0x50 + 8 nibbles + 0xFF
}

// TODO: These macros don't exist yet
/*
#[test]
fn test_command_macros() {
    // Test command creation macros
    let power_on = cmd!(0x01, 0x04, 0x00, 0x02);
    assert_eq!(power_on, patterns::power::ON);

    let power_inq = inq!(0x04, 0x00);
    assert_eq!(power_inq, vec![0x81, 0x09, 0x04, 0x00, 0xFF]);

    let ack = resp!(ack 2);
    assert_eq!(ack, patterns::responses::ACK_2);
}
*/

#[test]
fn test_scenario_builder() -> Result<()> {
    let mut mock = MockTransport::new();

    // Create a test scenario
    let scenario = ScenarioBuilder::new("Power Cycle Test")
        .description("Test power on, home, and power off sequence")
        .expect_power_on()
        .expect_home()
        .expect_power_standby()
        .build();

    // Apply scenario to mock
    scenario.apply_to(&mut mock).unwrap();

    // Create camera and execute commands
    let mut camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone());

    camera.power_on_blocking()?;
    camera.pan_tilt_home_blocking()?;
    camera.power_off_blocking()?;

    // Verify all expectations were met
    mock.verify()?;

    Ok(())
}

// TODO: Buffer full scenario simulation not yet implemented
/*
#[test]
fn test_buffer_full_scenario() {
    let mut mock = MockTransport::new();

    // Create buffer full scenario
    let mut builder = ScenarioBuilder::new("Buffer Full Test");
    builder.simulate_buffer_full();
    let scenario = builder.build();

    scenario.apply_to(&mut mock).unwrap();

    // Send three commands quickly
    mock.send(&[0x81, 0x01, 0x06, 0x04, 0xFF]).unwrap(); // Home
    let ack1 = mock.receive(Duration::from_millis(100)).unwrap();
    assert_eq!(ack1, patterns::responses::ACK_1);

    mock.send(&[0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]).unwrap(); // Zoom in
    let ack2 = mock.receive(Duration::from_millis(100)).unwrap();
    assert_eq!(ack2, patterns::responses::ACK_2);

    mock.send(&[0x81, 0x01, 0x04, 0x08, 0x02, 0xFF]).unwrap(); // Focus far
    let error = mock.receive(Duration::from_millis(100)).unwrap();
    assert_eq!(error, patterns::responses::BUFFER_FULL);
}
*/

#[test]
fn test_command_fixtures() {
    // Test fixture data
    let power_cmds = CommandFixtures::power_commands();
    assert_eq!(power_cmds.len(), 2);

    let zoom_cmds = CommandFixtures::zoom_commands();
    assert!(zoom_cmds.len() > 5);

    // Test edge cases
    let edge_cases = CommandFixtures::edge_case_commands();
    for (name, cmd) in edge_cases {
        println!("Testing edge case: {}", name);
        if name != "Invalid terminator" && name != "Invalid address" {
            // Validate basic frame structure
            assert!(cmd.len() >= 3, "Command too short: {}", name);
            assert_eq!(cmd[0] & 0xF0, 0x80, "Invalid address byte in: {}", name);
            assert_eq!(cmd[cmd.len() - 1], 0xFF, "Missing terminator in: {}", name);
        }
    }
}

#[test]
fn test_mock_transport_builder() {
    let transport = MockTransportBuilder::new()
        .connected(true)
        .with_latency(Duration::from_millis(10))
        .expect(
            &patterns::power::ON,
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Delayed(
                    patterns::responses::COMPLETE_1.to_vec(),
                    Duration::from_millis(50),
                ),
            ],
        )
        .build();

    assert!(transport.is_connected());
}

#[test]
fn test_inquiry_responses() {
    let mut mock = MockTransport::new();

    // Set up zoom position inquiry
    mock.expect_command(&[0x81, 0x09, 0x04, 0x47, 0xFF])
        .will_return_data(&[0x04, 0x00, 0x00, 0x00]); // Position 0x4000

    // Send inquiry
    mock.send(&[0x81, 0x09, 0x04, 0x47, 0xFF]).unwrap();

    // Get response
    let response = mock.receive(Duration::from_millis(100)).unwrap();
    assert_eq!(response, vec![0x90, 0x50, 0x04, 0x00, 0x00, 0x00, 0xFF]);
}

// TODO: ResponseSequence not yet implemented
/*
#[test]
fn test_response_sequence() {
    let mut seq = ResponseSequence::new();
    seq.ack_and_complete(1)
        .then_after(
            patterns::power_on_response(),
            Duration::from_millis(100),
        );

    let responses = seq.build();
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[0].0, patterns::responses::ACK_1);
    assert_eq!(responses[1].0, patterns::responses::COMPLETE_1);
    assert!(responses[2].1.is_some()); // Has delay
}
*/

#[test]
fn test_network_failure_recovery() {
    let mut mock = MockTransport::new();

    // Simulate network failure
    mock.queue_response(MockResponse::Immediate(patterns::responses::ACK_1.to_vec()));
    mock.queue_response(MockResponse::Immediate(
        patterns::responses::COMPLETE_1.to_vec(),
    ));

    // Send command successfully
    mock.send(patterns::power::ON).unwrap();
    assert!(mock.receive(Duration::from_millis(100)).is_ok());
    assert!(mock.receive(Duration::from_millis(100)).is_ok());

    // Disconnect
    mock.disconnect();

    // Try to send - should fail
    assert!(matches!(
        mock.send(patterns::power::STANDBY),
        Err(Error::ConnectionLost { .. })
    ));

    // Reconnect
    mock = MockTransport::builder().connected(true).build();

    // Should work again
    mock.queue_response(MockResponse::Immediate(patterns::responses::ACK_1.to_vec()));
    assert!(mock.send(patterns::power::ON).is_ok());
}

// TODO: Pre-built scenarios not yet implemented
/*
#[test]
fn test_common_scenarios() {
    // Test pre-built scenarios
    let scenarios = vec![
        scenarios::basic_initialization(),
        scenarios::preset_tour(),
        scenarios::error_recovery(),
        scenarios::network_failure(),
    ];

    for scenario in scenarios {
        println!("Testing scenario: {}", scenario.name);
        let mut mock = MockTransport::new();
        scenario.apply_to(&mut mock).unwrap();
    }
}
*/

#[test]
fn test_data_generators() {
    // Test zoom positions
    let zoom_positions = generators::zoom_positions();
    assert!(zoom_positions.contains(&0x0000)); // Wide end
    assert!(zoom_positions.contains(&0x4000)); // 20x optical

    // Test pan/tilt positions
    let positions = generators::pan_tilt_positions();
    for (pan, tilt) in positions {
        println!("Testing position: pan={}, tilt={}", pan, tilt);
        assert!(pan >= -170 && pan <= 170);
        assert!(tilt >= -30 && tilt <= 90);
    }

    // Test preset numbers
    let presets = generators::preset_numbers();
    for preset in presets {
        assert!(preset <= 127);
    }
}
