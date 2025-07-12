//! Tests for power control commands.

mod common;

use crate::common::{patterns, MockTransport, ProtocolValidator, ResponseBuilder, ValidationMode};
use grafton_visca::{
    camera::methods::PowerOps,
    command::{
        power::{Power, PowerCommand},
        EncodeVisca,
    },
    profiles::PTZOpticsG2,
    timeout::CommandCategory,
    Camera,
};

#[test]
fn test_power_on_command() {
    let cmd = PowerCommand { power: Power::On };
    let bytes = cmd.try_into_vec().unwrap();

    // Verify using pattern constants
    assert_eq!(bytes, patterns::power::ON);
    assert!(cmd.response_type().is_none());
    assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));

    // Validate protocol compliance
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);
    assert!(validator.validate_command(&bytes).is_ok());
}

#[test]
fn test_power_standby_command() {
    let cmd = PowerCommand {
        power: Power::Standby,
    };
    let bytes = cmd.try_into_vec().unwrap();

    assert_eq!(bytes, patterns::power::STANDBY);

    // Validate with different modes
    let mut lenient_validator = ProtocolValidator::new(ValidationMode::Lenient);
    assert!(lenient_validator.validate_command(&bytes).is_ok());
}

#[test]
fn test_power_response_type() {
    let cmd_on = PowerCommand { power: Power::On };
    assert!(cmd_on.response_type().is_none());

    let cmd_standby = PowerCommand {
        power: Power::Standby,
    };
    assert!(cmd_standby.response_type().is_none());
}

#[test]
fn test_command_trait_impl() {
    // Verify PowerCommand implements Command trait
    let cmd: Box<dyn EncodeVisca<Response = ()>> = Box::new(PowerCommand { power: Power::On });
    assert!(cmd.try_into_vec().is_ok());
    assert!(cmd.response_type().is_none());
    assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));
}

#[test]
fn test_byte_sequence_correctness() {
    // Verify the exact byte sequences match VISCA protocol
    let on_cmd = PowerCommand { power: Power::On };
    let on_bytes = on_cmd.try_into_vec().unwrap();

    // Use pattern constants for verification
    assert_eq!(on_bytes, patterns::power::ON);

    // Detailed byte-by-byte verification
    assert_eq!(on_bytes[0], 0x81); // Command header
    assert_eq!(on_bytes[1], 0x01); // Command type
    assert_eq!(on_bytes[2], 0x04); // Category
    assert_eq!(on_bytes[3], 0x00); // Power command
    assert_eq!(on_bytes[4], 0x02); // On value
    assert_eq!(on_bytes[5], 0xFF); // Terminator

    let standby_cmd = PowerCommand {
        power: Power::Standby,
    };
    let standby_bytes = standby_cmd.try_into_vec().unwrap();

    // Use pattern constants for verification
    assert_eq!(standby_bytes, patterns::power::STANDBY);

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
    mock.expect_command(&patterns::power::ON)
        .described_as("power on command")
        .will_ack(1)
        .then_complete(1);

    // Set up expectation for power off
    mock.expect_command(&patterns::power::STANDBY)
        .described_as("power off command")
        .will_ack(1)
        .then_complete(1);

    let mut camera = Camera::<PTZOpticsG2>::new(mock.clone());

    // Test power on
    assert!(camera.power_on().is_ok());

    // Test power off
    assert!(camera.power_off().is_ok());

    // Verify all expectations were met
    mock.verify().unwrap();
}
