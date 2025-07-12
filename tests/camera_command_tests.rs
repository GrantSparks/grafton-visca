//! Tests for Camera API with command execution using MockTransport.
//!
//! These tests verify that the Camera API correctly sends commands
//! and handles responses through the transport layer.

mod common;

use crate::common::{
    patterns, MockResponse, MockTransport, MockTransportBuilder, ProtocolValidator,
    ResponseBuilder, ScenarioBuilder, ValidationMode,
};
use std::time::Duration;

#[cfg(not(feature = "tokio"))]
mod blocking_tests {
    use super::*;
    use grafton_visca::{
        camera::{
            methods::{PanTiltOps, PowerOps, PresetsOps, ZoomOps},
            ProfileId, Camera,
        },
        Error,
    };

    #[test]
    fn test_camera_power_command() {
        // Create a mock transport with expectations
        let mut mock = MockTransport::new();

        // Set up expectation for power on command
        mock.expect_command(&patterns::power::ON)
            .described_as("power on command")
            .will_ack(1)
            .then_complete(1);

        // Create camera with mock transport
        let mut camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone());

        // Send power on command
        let result = camera.power_on_blocking();
        assert!(result.is_ok(), "Power on command should succeed");

        // Verify all expectations were met
        mock.verify().unwrap();

        // Check command history
        let history = mock.sent_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], patterns::power::ON);
    }

    #[test]
    fn test_camera_home_command() {
        let mut mock = MockTransport::new();

        // Set up expectation for home command
        mock.expect_command(&patterns::pan_tilt::HOME)
            .described_as("pan/tilt home")
            .will_ack(1)
            .then_complete(1);

        let mut camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone());

        // Send home command
        let result = camera.pan_tilt_home_blocking();
        assert!(result.is_ok(), "Home command should succeed");

        // Verify expectations
        mock.verify().unwrap();

        // Verify command was sent
        let history = mock.sent_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], patterns::pan_tilt::HOME);
    }

    #[test]
    fn test_camera_zoom_commands() {
        // Use MockTransportBuilder for multiple command expectations
        let mock = MockTransportBuilder::new()
            .connected(true)
            .expect(
                &patterns::zoom::STOP,
                vec![
                    MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                    MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
                ],
            )
            .expect(
                &patterns::zoom::TELE_STD,
                vec![
                    MockResponse::Immediate(patterns::responses::ACK_2.to_vec()),
                    MockResponse::Immediate(patterns::responses::COMPLETE_2.to_vec()),
                ],
            )
            .expect(
                &patterns::zoom::WIDE_STD,
                vec![
                    MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                    MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
                ],
            )
            .build();

        let mut camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone());

        // Test zoom commands
        assert!(camera.zoom_stop_blocking().is_ok());
        assert!(camera.zoom_in_blocking().is_ok());
        assert!(camera.zoom_out_blocking().is_ok());

        // MockTransportBuilder automatically verifies expectations when dropped

        // Verify all commands were sent
        let history = mock.sent_history();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0], patterns::zoom::STOP);
        assert_eq!(history[1], patterns::zoom::TELE_STD);
        assert_eq!(history[2], patterns::zoom::WIDE_STD);
    }

    #[test]
    fn test_camera_preset_operations() {
        // Use MockTransportBuilder for preset operation expectations
        let mock = MockTransportBuilder::new()
            .connected(true)
            .expect(
                &[0x81, 0x01, 0x04, 0x3F, 0x01, 0x05, 0xFF],
                vec![
                    MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                    MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
                ],
            )
            .expect(
                &[0x81, 0x01, 0x04, 0x3F, 0x02, 0x05, 0xFF],
                vec![
                    MockResponse::Immediate(patterns::responses::ACK_2.to_vec()),
                    MockResponse::Immediate(patterns::responses::COMPLETE_2.to_vec()),
                ],
            )
            .build();

        let mut camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone());

        // Set and recall preset 5
        use grafton_visca::command::preset::PresetNumber;
        let preset_id = PresetNumber::new(5).unwrap();
        assert!(camera.preset_set_blocking(preset_id).is_ok());
        assert!(camera.preset_recall_blocking(preset_id).is_ok());

        // MockTransportBuilder automatically verifies expectations when dropped
    }

    #[test]
    fn test_camera_error_handling() {
        // Use MockTransportBuilder for error response
        let mock = MockTransportBuilder::new()
            .connected(true)
            .expect(
                &patterns::power::ON,
                vec![MockResponse::ErrorCode(0x02)], // Syntax error
            )
            .build();

        let mut camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone());

        // Send a command that will get an error response
        let result = camera.power_on_blocking();
        assert!(result.is_err(), "Should get an error");

        match result {
            Err(Error::SyntaxError) => {
                // Expected error type
            }
            _ => panic!("Expected SyntaxError, got {:?}", result),
        }

        // MockTransportBuilder automatically verifies expectations when dropped
    }

    #[test]
    fn test_camera_timeout() {
        // Create a mock that will timeout
        let mock = MockTransport::new();
        // Don't set up any expectations - this will cause a timeout

        let mut camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock);

        let result = camera.pan_tilt_home_blocking();
        assert!(result.is_err(), "Should timeout");

        match result {
            Err(Error::Timeout) => {
                // Expected timeout
            }
            _ => panic!("Expected Timeout error, got {:?}", result),
        }
    }

    #[test]
    fn test_camera_command_sequence_with_scenario() {
        // Use ScenarioBuilder for complex sequences
        let scenario = ScenarioBuilder::new("Command Sequence Test")
            .description("Test home, zoom stop, and power off sequence")
            .expect_home()
            .expect_command(
                &patterns::zoom::STOP,
                vec![
                    MockResponse::Immediate(patterns::responses::ACK_2.to_vec()),
                    MockResponse::Delayed(
                        patterns::responses::COMPLETE_2.to_vec(),
                        Duration::from_millis(50),
                    ),
                ],
            )
            .expect_command(
                &patterns::power::STANDBY,
                vec![
                    MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                    MockResponse::Delayed(
                        patterns::responses::COMPLETE_1.to_vec(),
                        Duration::from_millis(50),
                    ),
                ],
            )
            .build();

        let mut mock = MockTransport::new();
        scenario.apply_to(&mut mock).unwrap();

        let mut camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone());

        // Execute the sequence
        assert!(camera.pan_tilt_home_blocking().is_ok());
        assert!(camera.zoom_stop_blocking().is_ok());
        assert!(camera.power_off_blocking().is_ok());

        // Verify all expectations were met
        mock.verify().unwrap();

        // Verify command history
        let history = mock.sent_history();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0], patterns::pan_tilt::HOME);
        assert_eq!(history[1], patterns::zoom::STOP);
        assert_eq!(history[2], patterns::power::STANDBY);
    }

    #[test]
    fn test_camera_with_protocol_validation() {
        // Use MockTransportBuilder with protocol validation
        let mock = MockTransportBuilder::new()
            .connected(true)
            .expect(
                &patterns::power::ON,
                vec![
                    MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                    MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
                ],
            )
            .build();

        let mut validator = ProtocolValidator::new(ValidationMode::Strict);
        let mut camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone());

        // Validate command before sending
        validator.validate_command(&patterns::power::ON).unwrap();

        // Send command
        camera.power_on_blocking().unwrap();

        // Get responses for validation
        let history = mock.response_history();
        for response in &history {
            validator.validate_response(response).unwrap();
        }

        // Check protocol state
        assert!(validator.all_sockets_free());

        let summary = validator.get_summary();
        assert_eq!(summary.commands_sent, 1);
        assert_eq!(summary.responses_received, 2); // ACK + Completion
    }

    #[test]
    fn test_camera_with_mock_builder() {
        // Use MockTransportBuilder for fluent configuration
        let mock = MockTransportBuilder::new()
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
            .expect(
                &patterns::pan_tilt::HOME,
                vec![
                    MockResponse::Immediate(patterns::responses::ACK_2.to_vec()),
                    MockResponse::Delayed(
                        patterns::responses::COMPLETE_2.to_vec(),
                        Duration::from_millis(100),
                    ),
                ],
            )
            .build();

        let mut camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock);

        // Execute commands
        assert!(camera.power_on_blocking().is_ok());
        assert!(camera.pan_tilt_home_blocking().is_ok());
    }
}

// Note: Async tests would need similar updates but are omitted for brevity
// The pattern would be similar - use the new test utilities instead of the old mock
