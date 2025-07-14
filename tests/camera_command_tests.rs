//! Tests for Camera API with command execution using MockTransport.
//!
//! These tests verify that the Camera API correctly sends commands
//! and handles responses through the transport layer.

mod common;

#[cfg(not(feature = "tokio"))]
mod blocking_tests {
    use crate::common::{
        patterns, MockResponse, MockTransport, MockTransportBuilder, ProtocolValidator,
        ScenarioBuilder, ValidationMode,
    };
    use grafton_visca::{
        blocking::{PanTiltOps, PowerOps, PresetsOps, ZoomOps},
        camera::{Camera, ProfileId},
        Error,
    };
    use std::time::Duration;

    #[test]
    fn test_camera_power_command() {
        // Create a mock transport with expectations
        let mut mock = MockTransport::new();

        // Set up expectation for power on command
        mock.expect_command(patterns::power::ON)
            .described_as("power on command")
            .will_ack(1)
            .then_complete(1);

        // Create camera with mock transport
        let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

        // Send power on command
        let result = camera.power_on();
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
        mock.expect_command(patterns::pan_tilt::HOME)
            .described_as("pan/tilt home")
            .will_ack(1)
            .then_complete(1);

        let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

        // Send home command
        let result = camera.pan_tilt_home();
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
        // Note: blocking mode uses variable speed commands, not standard speed
        // For PTZOpticsG2, zoom_speed_range().end is 8, so medium speed is 4
        let zoom_in_cmd = vec![0x81, 0x01, 0x04, 0x07, 0x24, 0xFF]; // TeleVariable with speed 4
        let zoom_out_cmd = vec![0x81, 0x01, 0x04, 0x07, 0x34, 0xFF]; // WideVariable with speed 4

        let mock = MockTransportBuilder::new()
            .connected(true)
            .expect(
                patterns::zoom::STOP,
                vec![
                    MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                    MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
                ],
            )
            .expect(
                &zoom_in_cmd,
                vec![
                    MockResponse::Immediate(patterns::responses::ACK_2.to_vec()),
                    MockResponse::Immediate(patterns::responses::COMPLETE_2.to_vec()),
                ],
            )
            .expect(
                &zoom_out_cmd,
                vec![
                    MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                    MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
                ],
            )
            .build();

        let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

        // Test zoom commands
        let stop_result = camera.zoom_stop();
        assert!(stop_result.is_ok(), "zoom_stop failed: {:?}", stop_result);

        let in_result = camera.zoom_in();
        assert!(in_result.is_ok(), "zoom_in failed: {:?}", in_result);

        let out_result = camera.zoom_out();
        assert!(out_result.is_ok(), "zoom_out failed: {:?}", out_result);

        // MockTransportBuilder automatically verifies expectations when dropped

        // Verify all commands were sent
        let history = mock.sent_history();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0], patterns::zoom::STOP);
        assert_eq!(history[1], zoom_in_cmd);
        assert_eq!(history[2], zoom_out_cmd);
    }

    #[test]
    fn test_simple_zoom_in() {
        // Simple test with just zoom_in to isolate the issue
        let zoom_in_cmd = vec![0x81, 0x01, 0x04, 0x07, 0x24, 0xFF]; // Speed 4

        let mock = MockTransportBuilder::new()
            .connected(true)
            .expect(
                &zoom_in_cmd,
                vec![
                    MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                    MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
                ],
            )
            .build();

        let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

        let result = camera.zoom_in();
        assert!(result.is_ok(), "zoom_in failed: {:?}", result);
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

        let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

        // Set and recall preset 5
        use grafton_visca::command::preset::PresetNumber;
        let preset_id = PresetNumber::new(5).unwrap();
        assert!(camera.preset_set(preset_id).is_ok());
        assert!(camera.preset_recall(preset_id).is_ok());

        // MockTransportBuilder automatically verifies expectations when dropped
    }

    #[test]
    fn test_camera_error_handling() {
        // Use MockTransportBuilder for error response
        let mock = MockTransportBuilder::new()
            .connected(true)
            .expect(
                patterns::power::ON,
                vec![MockResponse::ErrorCode(0x02)], // Syntax error
            )
            .build();

        let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

        // Send a command that will get an error response
        let result = camera.power_on();
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

        let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock).blocking();

        let result = camera.pan_tilt_home();
        assert!(result.is_err(), "Should timeout");

        match result {
            Err(Error::Timeout) => {
                // Expected timeout
            }
            Err(Error::Io(e)) if e.to_string().contains("timed out") => {
                // Also accept IO error with timeout message
            }
            _ => panic!("Expected Timeout or IO timeout error, got {:?}", result),
        }
    }

    #[test]
    fn test_camera_command_sequence_with_scenario() {
        // Use ScenarioBuilder for complex sequences
        let scenario = ScenarioBuilder::new("Command Sequence Test")
            .description("Test home, zoom stop, and power off sequence")
            .expect_home()
            .expect_command(
                patterns::zoom::STOP,
                vec![
                    MockResponse::Immediate(patterns::responses::ACK_2.to_vec()),
                    MockResponse::Delayed(
                        patterns::responses::COMPLETE_2.to_vec(),
                        Duration::from_millis(50),
                    ),
                ],
            )
            .expect_command(
                patterns::power::STANDBY,
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

        let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

        // Execute the sequence
        assert!(camera.pan_tilt_home().is_ok());
        assert!(camera.zoom_stop().is_ok());
        assert!(camera.power_off().is_ok());

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
                patterns::power::ON,
                vec![
                    MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                    MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
                ],
            )
            .build();

        let mut validator = ProtocolValidator::new(ValidationMode::Strict);
        let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock.clone()).blocking();

        // Validate command before sending
        validator.validate_command(patterns::power::ON).unwrap();

        // Send command
        camera.power_on().unwrap();

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
                patterns::power::ON,
                vec![
                    MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                    MockResponse::Delayed(
                        patterns::responses::COMPLETE_1.to_vec(),
                        Duration::from_millis(50),
                    ),
                ],
            )
            .expect(
                patterns::pan_tilt::HOME,
                vec![
                    MockResponse::Immediate(patterns::responses::ACK_2.to_vec()),
                    MockResponse::Delayed(
                        patterns::responses::COMPLETE_2.to_vec(),
                        Duration::from_millis(100),
                    ),
                ],
            )
            .build();

        let camera = Camera::with_profile(ProfileId::PTZOpticsG2, mock).blocking();

        // Execute commands
        assert!(camera.power_on().is_ok());
        assert!(camera.pan_tilt_home().is_ok());
    }
}

// Note: Async tests would need similar updates but are omitted for brevity
// The pattern would be similar - use the new test utilities instead of the old mock
