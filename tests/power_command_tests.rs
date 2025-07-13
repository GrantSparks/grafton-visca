//! Tests for power control commands.

mod common;

use crate::common::{patterns, MockTransport, ProtocolValidator, ResponseBuilder, ValidationMode};
use grafton_visca::{
    blocking::PowerOps,
    camera::{Camera, ProfileId},
};

#[test]
fn test_power_on_command_bytes() {
    // Since PowerCommand is not public, we test the byte pattern
    let bytes = patterns::power::ON;

    // Validate protocol compliance
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);
    assert!(validator.validate_command(&bytes).is_ok());

    // Verify the expected command bytes
    assert_eq!(bytes, vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);
}

#[test]
fn test_power_standby_command_bytes() {
    // Test the standby command byte pattern
    let bytes = patterns::power::STANDBY;

    // Validate with different modes
    let mut lenient_validator = ProtocolValidator::new(ValidationMode::Lenient);
    assert!(lenient_validator.validate_command(&bytes).is_ok());

    // Verify the expected command bytes
    assert_eq!(bytes, vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]);
}

#[test]
fn test_power_command_patterns() {
    // Test that power command patterns are valid
    assert_eq!(patterns::power::ON.len(), 6);
    assert_eq!(patterns::power::STANDBY.len(), 6);

    // Check header and terminator
    assert_eq!(patterns::power::ON[0], 0x81); // Header
    assert_eq!(patterns::power::ON[5], 0xFF); // Terminator
    assert_eq!(patterns::power::STANDBY[0], 0x81); // Header
    assert_eq!(patterns::power::STANDBY[5], 0xFF); // Terminator
}

#[test]
fn test_power_patterns_distinct() {
    // Ensure power commands have distinct patterns
    assert_ne!(patterns::power::ON, patterns::power::STANDBY);

    // Check the specific command bytes that differ
    assert_eq!(patterns::power::ON[4], 0x02); // ON command
    assert_eq!(patterns::power::STANDBY[4], 0x03); // STANDBY command
}

#[test]
fn test_byte_sequence_correctness() {
    // Verify the exact byte sequences match VISCA protocol
    let on_bytes = patterns::power::ON;

    // Detailed byte-by-byte verification
    assert_eq!(on_bytes[0], 0x81); // Command header
    assert_eq!(on_bytes[1], 0x01); // Command type
    assert_eq!(on_bytes[2], 0x04); // Category
    assert_eq!(on_bytes[3], 0x00); // Power command
    assert_eq!(on_bytes[4], 0x02); // On value
    assert_eq!(on_bytes[5], 0xFF); // Terminator

    let standby_bytes = patterns::power::STANDBY;

    // Detailed byte-by-byte verification
    assert_eq!(standby_bytes[0], 0x81); // Command header
    assert_eq!(standby_bytes[1], 0x01); // Command type
    assert_eq!(standby_bytes[2], 0x04); // Category
    assert_eq!(standby_bytes[3], 0x00); // Power command
    assert_eq!(standby_bytes[4], 0x03); // Standby value
    assert_eq!(standby_bytes[5], 0xFF); // Terminator
}

#[test]
fn test_power_command_with_mock_response() {
    // Test with ResponseBuilder for simulating responses
    let ack = ResponseBuilder::ack(1);
    assert_eq!(ack, patterns::responses::ACK_1);

    let completion = ResponseBuilder::completion(1);
    assert_eq!(completion, patterns::responses::COMPLETE_1);

    // Test error response
    let error = ResponseBuilder::error(0x02);
    assert_eq!(error, patterns::responses::SYNTAX_ERROR);
}

#[test]
fn test_power_commands_with_camera() {
    let mut mock = MockTransport::new();

    // Set up expectation for power on
    mock.expect_command(patterns::power::ON)
        .described_as("power on command")
        .will_ack(1)
        .then_complete(1);

    // Set up expectation for power off
    mock.expect_command(patterns::power::STANDBY)
        .described_as("power off command")
        .will_ack(1)
        .then_complete(1);

    let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

    // Test power on
    assert!(camera.power_on().is_ok());

    // Test power off
    assert!(camera.power_off().is_ok());

    // Verify all expectations were met
    mock.verify().unwrap();
}
