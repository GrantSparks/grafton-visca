#![allow(missing_docs)]
//! Demonstration of the new test infrastructure
//!
//! This test file demonstrates how to use the improved test helpers
//! and patterns to write tests without needing #[allow(...)] directives.

#![cfg(feature = "blocking-client")]

#[path = "common/mod.rs"]
mod common;

// Import test helpers
use common::builders::*;
use common::helpers::*;

// Import the macros - they're exported at crate root
use grafton_visca::*;

#[cfg(feature = "blocking-client")]
#[test]
fn test_with_helpers() {
    // Use helper functions instead of unwrap()
    let _client = create_test_udp_client("127.0.0.1:1234");

    // Use test speeds helper
    let (_pan_speed, _tilt_speed) = test_speeds();

    // Create commands with builders
    let cmd = TestPanTiltBuilder::new()
        .with_position(100, 200)
        .with_speeds(15, 20)
        .build_absolute();

    // Assert with context
    let bytes = assert_ok(cmd.to_bytes(), "PanTilt command should convert to bytes");
    // The expected bytes:
    // - Header: 0x81, 0x01, 0x06, 0x02
    // - Pan speed: 0x0F (15)
    // - Tilt speed: 0x14 (20)
    // - Pan position 100 (0x0064) as nibbles: 0x00, 0x00, 0x06, 0x04
    // - Tilt position 200 (0x00C8) as nibbles: 0x00, 0x00, 0x0C, 0x08
    // - Terminator: 0xFF
    assert_bytes_eq(
        &bytes,
        &[
            0x81, 0x01, 0x06, 0x02, 0x0F, 0x14, 0x00, 0x00, 0x06, 0x04, 0x00, 0x00, 0x0C, 0x08,
            0xFF,
        ],
        "PanTilt absolute position command",
    );
}

#[cfg(feature = "blocking-client")]
#[test]
fn test_with_mock_transport() {
    use common::{MockDevice, MockTransport};
    use grafton_visca::command::{power::Power, PowerCommand};

    // Create a mock that returns specific responses
    let mock = MockTransport::with_ack_completion();
    let mut device = MockDevice::from_transport(mock);

    // Send command and verify response
    let response = assert_ok(
        device.execute_command(&PowerCommand { power: Power::On }),
        "Power on command should succeed",
    );

    // Response doesn't implement PartialEq, so check the variant
    match response {
        Response::Completion => {}
        _ => panic!("Expected Completion response, got {:?}", response),
    }
}

#[cfg(feature = "blocking-client")]
#[test]
fn test_error_handling() {
    use grafton_visca::command::zoom::ZoomSpeed;

    // Test invalid parameter with context
    let result = ZoomSpeed::new(10);
    let error = assert_err(result, "ZoomSpeed 10 should be invalid");

    // Error::InvalidParameter doesn't have named fields in this version
    match error {
        Error::InvalidParameter(_) => {
            // Error validated - the specific format may vary
        }
        _ => panic!("Expected InvalidParameter error, got {:?}", error),
    }
}

#[cfg(feature = "blocking-client")]
#[test]
fn test_preset_commands() {
    // Use builders for complex test data
    let _preset_cmd = TestPresetBuilder::new().with_number(5).build_recall();

    // Use parameter helpers - note: brightness takes u16
    let brightness = TestParameters::brightness(10);
    let contrast = TestParameters::contrast(14); // Max is 14

    assert_eq!(brightness.value(), 10);
    assert_eq!(contrast.value(), 14);
}

#[test]
fn test_response_helpers() {
    // Use response creation helpers
    let ack = create_ack_response(0);
    let completion = create_completion_response(1);
    let error = create_error_response(0x02);

    assert_bytes_eq(&ack, &[0x90, 0x40, 0xFF], "ACK response");
    assert_bytes_eq(&completion, &[0x90, 0x51, 0xFF], "Completion response");
    assert_bytes_eq(&error, &[0x90, 0x60, 0x02, 0xFF], "Error response");
}

#[cfg(test)]
mod integration_style_tests {
    use super::*;

    #[cfg(feature = "blocking-client")]
    #[test]
    fn test_command_sequence() {
        use common::MockDevice;
        use grafton_visca::command::{pan_tilt::PanTiltCommand, zoom::ZoomCommand};

        // Create mock device with expected responses
        let mut device = MockDevice::with_completion();

        // Queue additional responses for multiple commands
        device.add_response(create_ack_response(0));
        device.add_response(create_completion_response(0));

        // Test multiple commands
        let home_response = assert_ok(
            device.execute_command(&PanTiltCommand::Home),
            "Home command should succeed",
        );
        match home_response {
            Response::Completion => {}
            _ => panic!("Expected Completion response, got {:?}", home_response),
        }

        let zoom_response = assert_ok(
            device.execute_command(&ZoomCommand::Stop),
            "Zoom stop command should succeed",
        );
        match zoom_response {
            Response::Completion => {}
            _ => panic!("Expected Completion response, got {:?}", zoom_response),
        }

        // Verify commands were sent
        let commands = device.commands_sent();
        assert_eq!(commands.len(), 2, "Should have sent 2 commands");

        // Check specific command bytes
        assert_bytes_start_with(
            &commands[0],
            &[0x81, 0x01, 0x06, 0x04],
            "First command should be PTZ Home",
        );
    }
}
