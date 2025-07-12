#![allow(missing_docs)]
//! Demonstration of the new test infrastructure
//!
//! This test file demonstrates how to use the improved test helpers
//! and patterns to write tests without needing #[allow(...)] directives.

#![cfg(not(feature = "async"))]

#[path = "common/mod.rs"]
mod common;

// Import test helpers
use common::builders::*;
use common::helpers::*;

// Import needed types
use grafton_visca::{Error};

#[cfg(not(feature = "tokio"))]
#[test]
fn test_with_helpers() {
    // Use test speeds helper
    let (_pan_speed, _tilt_speed) = test_speeds();

    // Create commands with builders
    let cmd = TestPanTiltBuilder::new()
        .with_position(100, 200)
        .with_speeds(15, 20)
        .build_absolute();

    // Assert with context
    use grafton_visca::command::EncodeVisca;
    let mut buffer = [0u8; 64];
    let size = assert_ok(cmd.encode_into(&mut buffer), "PanTilt command should encode");
    let bytes = buffer[..size].to_vec();
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

#[cfg(not(feature = "tokio"))]
#[test]
fn test_with_mock_transport() {
    use common::MockTransport;
    use grafton_visca::camera::methods::PowerOps;
    use grafton_visca::camera::{ProfileId, Camera};

    // Create a mock that returns specific responses
    let mut mock = MockTransport::new();

    // Set up expectation for power on command
    mock.expect_command(&[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF])
        .described_as("power on")
        .will_ack(1)
        .then_complete(1);

    let mut camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone());

    // Send command and verify response
    assert_ok(camera.power_on_blocking(), "Power on command should succeed");

    // Verify expectations were met
    mock.verify().unwrap();

    // Verify command was sent
    let history = mock.sent_history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0], vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);
}

#[cfg(not(feature = "tokio"))]
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

#[cfg(not(feature = "tokio"))]
#[test]
fn test_preset_commands() {
    // Use builders for complex test data
    let _preset_cmd = TestPresetBuilder::new().with_number(5).build_recall();

    // Parameter types are no longer part of public API
    // Tests should use Camera<P> API instead
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

    #[cfg(not(feature = "tokio"))]
    #[test]
    fn test_command_sequence() {
        use common::MockTransport;
        use grafton_visca::camera::methods::{PanTiltOps, ZoomOps};
        use grafton_visca::camera::{ProfileId, Camera};

        // Create mock with expected responses
        let mut mock = MockTransport::new();

        // Set up expectations for home command
        mock.expect_command(&[0x81, 0x01, 0x06, 0x04, 0xFF])
            .described_as("pan/tilt home")
            .will_ack(1)
            .then_complete(1);

        // Set up expectations for zoom stop command
        mock.expect_command(&[0x81, 0x01, 0x04, 0x07, 0x00, 0xFF])
            .described_as("zoom stop")
            .will_ack(2)
            .then_complete(2);

        let mut camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone());

        // Test multiple commands
        assert_ok(camera.pan_tilt_home_blocking(), "Home command should succeed");

        assert_ok(camera.zoom_stop_blocking(), "Zoom stop command should succeed");

        // Verify expectations were met
        mock.verify().unwrap();

        // Verify commands were sent
        let history = mock.sent_history();
        assert_eq!(history.len(), 2, "Should have sent 2 commands");

        // Check specific command bytes
        assert_bytes_start_with(
            &history[0],
            &[0x81, 0x01, 0x06, 0x04],
            "First command should be PTZ Home",
        );
        assert_bytes_eq(
            &history[1],
            &[0x81, 0x01, 0x04, 0x07, 0x00, 0xFF],
            "Second command should be Zoom Stop",
        );
    }
}
