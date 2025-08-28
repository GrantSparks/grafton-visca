//! Tests for Camera API with command execution using ScriptedTransport.
//!
//! These tests verify that the Camera API correctly sends commands
//! and handles responses through the transport layer.

mod common;

#[cfg(all(
    not(feature = "async"),
    any(feature = "rt-tokio", feature = "test-utils")
))]
mod blocking_tests {
    use grafton_visca::{
        camera::{
            methods::{
                pan_tilt::PanTiltControl, power::PowerControl, presets::PresetsControl,
                zoom::ZoomControl,
            },
            BlockingCamera,
        },
        prelude::blocking::*,
        testing::testkit::{helpers, ScriptedBlockingTransport},
        Error,
    };

    use crate::common::patterns;

    /// VISCA command terminator byte.
    const VISCA_TERMINATOR: u8 = 0xFF;

    #[test]
    fn test_camera_power_command() {
        let transport = ScriptedBlockingTransport::new(vec![helpers::command_response(
            patterns::power::ON.to_vec(),
            1,
        )]);

        let mut camera: BlockingCamera<PtzOpticsG2, _> = BlockingCamera::new(transport.clone());

        let result = camera.power_on();
        assert!(
            result.is_ok(),
            "Power on command should succeed: {result:?}"
        );

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

        let mut camera: BlockingCamera<PtzOpticsG2, _> = BlockingCamera::new(transport.clone());

        let result = camera.pan_tilt_home();
        assert!(result.is_ok(), "Home command should succeed: {result:?}");

        let history = transport.sent();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], patterns::pan_tilt::HOME);
    }

    #[test]
    fn test_camera_zoom_commands() {
        let zoom_in_cmd = vec![0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR];
        let zoom_out_cmd = vec![0x81, 0x01, 0x04, 0x07, 0x03, VISCA_TERMINATOR];

        let transport = ScriptedBlockingTransport::new(vec![
            helpers::command_response(patterns::zoom::STOP.to_vec(), 1),
            helpers::command_response(zoom_in_cmd.clone(), 2),
            helpers::command_response(zoom_out_cmd.clone(), 1),
        ]);

        let mut camera: BlockingCamera<PtzOpticsG2, _> = BlockingCamera::new(transport.clone());

        let stop_result = camera.zoom_stop();
        assert!(stop_result.is_ok(), "zoom_stop failed: {stop_result:?}");

        let in_result = camera.zoom_tele_std();
        assert!(in_result.is_ok(), "zoom_in failed: {in_result:?}");

        let out_result = camera.zoom_wide_std();
        assert!(out_result.is_ok(), "zoom_out failed: {out_result:?}");

        let history = transport.sent();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0], patterns::zoom::STOP);
        assert_eq!(history[1], zoom_in_cmd);
        assert_eq!(history[2], zoom_out_cmd);
    }

    #[test]
    fn test_simple_zoom_in() {
        let zoom_in_cmd = vec![0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR];

        let transport =
            ScriptedBlockingTransport::new(vec![helpers::command_response(zoom_in_cmd, 1)]);

        let mut camera: BlockingCamera<PtzOpticsG2, _> = BlockingCamera::new(transport);

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

        let mut camera: BlockingCamera<PtzOpticsG2, _> = BlockingCamera::new(transport);

        use grafton_visca::PresetNumber;

        let preset_id = PresetNumber::new(5).unwrap();
        assert!(camera.preset_set(preset_id).is_ok());
        assert!(camera.preset_recall(preset_id).is_ok());
    }

    #[test]
    fn test_camera_error_handling() {
        let transport = ScriptedBlockingTransport::new(vec![helpers::errors::syntax_error(1)]);

        let mut camera: BlockingCamera<PtzOpticsG2, _> = BlockingCamera::new(transport);

        let result = camera.power_on();
        assert!(result.is_err(), "Should get an error");

        match result {
            Err(Error::SyntaxError) => {}
            _ => panic!("Expected SyntaxError, got {result:?}"),
        }
    }

    #[test]
    fn test_camera_timeout() {
        let transport = ScriptedBlockingTransport::new(vec![]);

        let mut camera: BlockingCamera<PtzOpticsG2, _> = BlockingCamera::new(transport);

        let result = camera.pan_tilt_home();
        assert!(result.is_err(), "Should timeout");

        match result {
            Err(Error::Timeout) | Err(Error::CommandTimeout { .. }) => {}
            _ => panic!("Expected Timeout or CommandTimeout error, got {result:?}"),
        }
    }

    #[test]
    fn test_camera_command_sequence_with_scenario() {
        let transport = ScriptedBlockingTransport::new(vec![
            helpers::command_response(patterns::pan_tilt::HOME.to_vec(), 1),
            helpers::command_response(patterns::zoom::STOP.to_vec(), 2),
            helpers::command_response(patterns::power::STANDBY.to_vec(), 1),
        ]);

        let mut camera: BlockingCamera<PtzOpticsG2, _> = BlockingCamera::new(transport.clone());

        assert!(camera.pan_tilt_home().is_ok());
        assert!(camera.zoom_stop().is_ok());
        assert!(camera.power_off().is_ok());

        let history = transport.sent();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0], patterns::pan_tilt::HOME);
        assert_eq!(history[1], patterns::zoom::STOP);
        assert_eq!(history[2], patterns::power::STANDBY);
    }

    #[test]
    fn test_camera_with_protocol_validation() {
        let transport = ScriptedBlockingTransport::new(vec![helpers::command_response(
            patterns::power::ON.to_vec(),
            1,
        )]);

        let mut camera: BlockingCamera<PtzOpticsG2, _> = BlockingCamera::new(transport.clone());

        camera.power_on().unwrap();

        let history = transport.sent();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], patterns::power::ON);
    }

    #[test]
    fn test_camera_with_mock_builder() {
        let transport = ScriptedBlockingTransport::new(vec![
            helpers::command_response(patterns::power::ON.to_vec(), 1),
            helpers::command_response(patterns::pan_tilt::HOME.to_vec(), 2),
        ]);

        let mut camera: BlockingCamera<PtzOpticsG2, _> = BlockingCamera::new(transport.clone());

        assert!(camera.power_on().is_ok());
        assert!(camera.pan_tilt_home().is_ok());

        let history = transport.sent();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0], patterns::power::ON);
        assert_eq!(history[1], patterns::pan_tilt::HOME);
    }
}
