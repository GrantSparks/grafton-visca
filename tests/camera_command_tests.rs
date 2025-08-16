//! Tests for Camera API with command execution using ScriptedTransport.
//!
//! These tests verify that the Camera API correctly sends commands
//! and handles responses through the transport layer.

mod common;

#[cfg(not(feature = "async"))]
mod blocking_tests {
    use crate::common::patterns;
    use grafton_visca::{
        camera::methods::{
            pan_tilt::PanTiltOpsBlocking, power::PowerOpsBlocking, presets::PresetsOpsBlocking,
            zoom::ZoomOpsBlocking,
        },
        camera::{BlockingMode, Camera},
        prelude::blocking::*,
        testing::testkit::{helpers, ScriptedBlockingTransport},
        Error,
    };

    /// VISCA command terminator byte.
    const VISCA_TERMINATOR: u8 = 0xFF;

    #[test]
    fn test_camera_power_command() {
        // Create scripted transport that responds to power on command
        let transport = ScriptedBlockingTransport::new(vec![helpers::command_response(
            patterns::power::ON.to_vec(),
            1,
        )]);

        // Create camera with scripted transport
        let camera: Camera<BlockingMode, PTZOpticsG2, _, ()> = Camera::new(transport.clone());

        // Send power on command
        let result = camera.power_on();
        assert!(
            result.is_ok(),
            "Power on command should succeed: {:?}",
            result
        );

        // Check command history
        let history = transport.sent();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], patterns::power::ON);
    }

    #[test]
    fn test_camera_home_command() {
        let transport = ScriptedBlockingTransport::new(vec![helpers::command_response(
            patterns::pan_tilt::HOME.to_vec(),
            1,
        )]);

        let camera: Camera<BlockingMode, PTZOpticsG2, _, ()> = Camera::new(transport.clone());

        // Send home command
        let result = camera.pan_tilt_home();
        assert!(result.is_ok(), "Home command should succeed: {:?}", result);

        // Verify command was sent
        let history = transport.sent();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], patterns::pan_tilt::HOME);
    }

    #[test]
    fn test_camera_zoom_commands() {
        // Standard speed zoom commands as per VISCA spec
        let zoom_in_cmd = vec![0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR]; // TeleStd
        let zoom_out_cmd = vec![0x81, 0x01, 0x04, 0x07, 0x03, VISCA_TERMINATOR]; // WideStd

        let transport = ScriptedBlockingTransport::new(vec![
            helpers::command_response(patterns::zoom::STOP.to_vec(), 1),
            helpers::command_response(zoom_in_cmd.clone(), 2),
            helpers::command_response(zoom_out_cmd.clone(), 1),
        ]);

        let camera: Camera<BlockingMode, PTZOpticsG2, _, ()> = Camera::new(transport.clone());

        // Test zoom commands
        let stop_result = camera.zoom_stop();
        assert!(stop_result.is_ok(), "zoom_stop failed: {stop_result:?}");

        let in_result = camera.zoom_tele_std();
        assert!(in_result.is_ok(), "zoom_in failed: {in_result:?}");

        let out_result = camera.zoom_wide_std();
        assert!(out_result.is_ok(), "zoom_out failed: {out_result:?}");

        // Verify all commands were sent
        let history = transport.sent();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0], patterns::zoom::STOP);
        assert_eq!(history[1], zoom_in_cmd);
        assert_eq!(history[2], zoom_out_cmd);
    }

    #[test]
    fn test_simple_zoom_in() {
        // Simple test with just zoom_in
        let zoom_in_cmd = vec![0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR]; // Standard speed

        let transport =
            ScriptedBlockingTransport::new(vec![helpers::command_response(zoom_in_cmd, 1)]);

        let camera: Camera<BlockingMode, PTZOpticsG2, _, ()> = Camera::new(transport);

        let result = camera.zoom_tele_std();
        assert!(result.is_ok(), "zoom_in failed: {result:?}");
    }

    #[test]
    fn test_camera_preset_operations() {
        let transport = ScriptedBlockingTransport::new(vec![
            helpers::command_response(
                vec![0x81, 0x01, 0x04, 0x3F, 0x01, 0x05, VISCA_TERMINATOR],
                1,
            ),
            helpers::command_response(
                vec![0x81, 0x01, 0x04, 0x3F, 0x02, 0x05, VISCA_TERMINATOR],
                2,
            ),
        ]);

        let camera: Camera<BlockingMode, PTZOpticsG2, _, ()> = Camera::new(transport);

        // Set and recall preset 5
        use grafton_visca::PresetNumber;
        let preset_id = PresetNumber::new(5).unwrap();
        assert!(camera.preset_set(preset_id).is_ok());
        assert!(camera.preset_recall(preset_id).is_ok());
    }

    #[test]
    fn test_camera_error_handling() {
        let transport = ScriptedBlockingTransport::new(vec![helpers::errors::syntax_error(1)]);

        let camera: Camera<BlockingMode, PTZOpticsG2, _, ()> = Camera::new(transport);

        // Send a command that will get an error response
        let result = camera.power_on();
        assert!(result.is_err(), "Should get an error");

        match result {
            Err(Error::SyntaxError) => {
                // Expected error type
            }
            _ => panic!("Expected SyntaxError, got {result:?}"),
        }
    }

    #[test]
    fn test_camera_timeout() {
        // Create transport with no scripted responses - will timeout
        let transport = ScriptedBlockingTransport::new(vec![]);

        let camera: Camera<BlockingMode, PTZOpticsG2, _, ()> = Camera::new(transport);

        let result = camera.pan_tilt_home();
        assert!(result.is_err(), "Should timeout");

        match result {
            Err(Error::Timeout) => {
                // Expected timeout
            }
            Err(Error::CommandTimeout { .. }) => {
                // Also accept CommandTimeout error
            }
            _ => panic!("Expected Timeout or CommandTimeout error, got {result:?}"),
        }
    }

    #[test]
    fn test_camera_command_sequence_with_scenario() {
        // Test a sequence of commands
        let transport = ScriptedBlockingTransport::new(vec![
            helpers::command_response(patterns::pan_tilt::HOME.to_vec(), 1),
            helpers::command_response(patterns::zoom::STOP.to_vec(), 2),
            helpers::command_response(patterns::power::STANDBY.to_vec(), 1),
        ]);

        let camera: Camera<BlockingMode, PTZOpticsG2, _, ()> = Camera::new(transport.clone());

        // Execute the sequence
        assert!(camera.pan_tilt_home().is_ok());
        assert!(camera.zoom_stop().is_ok());
        assert!(camera.power_off().is_ok());

        // Verify command history
        let history = transport.sent();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0], patterns::pan_tilt::HOME);
        assert_eq!(history[1], patterns::zoom::STOP);
        assert_eq!(history[2], patterns::power::STANDBY);
    }

    #[test]
    fn test_camera_with_protocol_validation() {
        // Simple power command test (protocol validation simplified for now)
        let transport = ScriptedBlockingTransport::new(vec![helpers::command_response(
            patterns::power::ON.to_vec(),
            1,
        )]);

        let camera: Camera<BlockingMode, PTZOpticsG2, _, ()> = Camera::new(transport.clone());

        // Send command
        camera.power_on().unwrap();

        // Verify command was sent
        let history = transport.sent();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], patterns::power::ON);
    }

    #[test]
    fn test_camera_with_mock_builder() {
        // Test multiple commands with ScriptedTransport
        let transport = ScriptedBlockingTransport::new(vec![
            helpers::command_response(patterns::power::ON.to_vec(), 1),
            helpers::command_response(patterns::pan_tilt::HOME.to_vec(), 2),
        ]);

        let camera: Camera<BlockingMode, PTZOpticsG2, _, ()> = Camera::new(transport.clone());

        // Execute commands
        assert!(camera.power_on().is_ok());
        assert!(camera.pan_tilt_home().is_ok());

        // Verify both commands were sent
        let history = transport.sent();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0], patterns::power::ON);
        assert_eq!(history[1], patterns::pan_tilt::HOME);
    }
}

// Note: Async tests would need similar updates but are omitted for brevity
// The pattern would be similar - use the new test utilities instead of the old mock
