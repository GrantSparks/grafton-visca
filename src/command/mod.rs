//! VISCA command definitions and traits.
//!
//! This module provides all command types for controlling VISCA cameras,
//! organized by functionality.

// Command modules
pub mod color;
pub mod exposure;
pub mod flip;
pub mod focus;
pub mod gain;
pub mod image;
pub mod inquiry;
pub(crate) mod inquiry_registry;
pub(crate) mod inquiry_structs; // Internal module for macro-generated inquiry commands
pub mod inquiry_types;
pub mod menu;
pub mod motion_sync;
pub mod nd_filter;
pub mod pan_tilt;
pub mod power;
pub mod preset;
pub mod resolution;
pub mod response;
pub mod streaming;
pub mod system;
pub mod tally;
pub mod typed;
pub mod variable_speed;
pub mod white_balance;
pub mod zoom;

// Command encoding module
pub mod bytes;

// New unified ViscaCommand trait
pub mod encode;

// Re-export command types
pub use self::{
    bytes::FixedCommandBytes,
    color::*,
    encode::{CommandKind, ViscaCommand},
    exposure::*,
    focus::*,
    image::*,
    inquiry_registry::{InquiryData, InquiryKind},
    inquiry_types::{FlipState, IrisControl, NightDayMode, TallyStatus, Version},
    menu::*,
    nd_filter::*,
    pan_tilt::*,
    power::*,
    preset::*,
    response::Response,
    system::{MotionSyncMode, MotionSyncPreset, SettingsSaveCommand},
    variable_speed::*,
    white_balance::*,
};

#[cfg(test)]
mod tests {
    use crate::camera_id::CameraId;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::command::encode::ViscaCommand;

    /// Helper to encode a command and verify it has a terminator
    #[allow(clippy::expect_used, clippy::unwrap_used)]
    fn assert_command_has_terminator<C: ViscaCommand>(command: C, name: &str) {
        let mut buffer = [0u8; 256];
        let result = command.write_into(CameraId::CAMERA_1, &mut buffer);

        assert!(
            result.is_ok(),
            "Command {name} failed to encode: {result:?}"
        );

        let len = result.unwrap();
        assert!(len > 0, "Command {name} encoded to empty buffer");

        // Check that the command ends with VISCA_TERMINATOR
        assert_eq!(
            buffer[len - 1],
            VISCA_TERMINATOR,
            "Command {name} does not end with VISCA_TERMINATOR (0xFF). Last byte: 0x{last_byte:02X}",
            last_byte = buffer[len - 1]
        );

        // Validate no double terminators
        if len > 1 {
            let mut terminator_count = 0;
            for &byte in buffer.iter().take(len) {
                if byte == VISCA_TERMINATOR {
                    terminator_count += 1;
                }
            }
            assert_eq!(
                terminator_count, 1,
                "Command {name} has {terminator_count} terminators, expected exactly 1"
            );
        }
    }

    #[test]
    fn test_power_commands_have_terminator() {
        use crate::command::power::{PowerOn, PowerStandby};
        assert_command_has_terminator(PowerOn::new(), "PowerOn");
        assert_command_has_terminator(PowerStandby::new(), "PowerStandby");
    }

    #[test]
    fn test_zoom_commands_have_terminator() {
        use crate::command::zoom::Zoom;
        assert_command_has_terminator(Zoom::Stop, "Zoom::Stop");
        assert_command_has_terminator(Zoom::TeleStd, "Zoom::TeleStd");
        assert_command_has_terminator(Zoom::WideStd, "Zoom::WideStd");
    }

    #[test]
    fn test_focus_commands_have_terminator() {
        use crate::command::focus::Focus;
        assert_command_has_terminator(Focus::Stop, "Focus::Stop");
        assert_command_has_terminator(Focus::Auto, "Focus::Auto");
        assert_command_has_terminator(Focus::Manual, "Focus::Manual");
        assert_command_has_terminator(Focus::Far, "Focus::Far");
        assert_command_has_terminator(Focus::Near, "Focus::Near");
    }

    #[test]
    #[allow(clippy::expect_used, clippy::unwrap_used)]
    fn test_pan_tilt_commands_have_terminator() {
        use crate::command::pan_tilt::PanTilt;
        assert_command_has_terminator(PanTilt::Home, "PanTilt::Home");
        assert_command_has_terminator(PanTilt::Reset, "PanTilt::Reset");
        // Test Move variant with Stop direction
        use crate::command::pan_tilt::PanTiltDirection;
        use crate::types::{PanSpeed, TiltSpeed};
        assert_command_has_terminator(
            PanTilt::Move {
                direction: PanTiltDirection::Stop,
                pan_speed: PanSpeed::new(0).unwrap(),
                tilt_speed: TiltSpeed::new(0).unwrap(),
            },
            "PanTilt::Stop",
        );
    }

    #[test]
    fn test_gain_commands_have_terminator() {
        use crate::command::gain::Gain;
        assert_command_has_terminator(Gain::Reset, "Gain::Reset");
        assert_command_has_terminator(Gain::Up, "Gain::Up");
        assert_command_has_terminator(Gain::Down, "Gain::Down");
    }

    #[test]
    fn test_exposure_commands_have_terminator() {
        use crate::command::exposure::{ExposureCommand, ExposureMode};
        assert_command_has_terminator(
            ExposureCommand {
                mode: ExposureMode::Auto,
            },
            "Exposure::Auto",
        );
        assert_command_has_terminator(
            ExposureCommand {
                mode: ExposureMode::Manual,
            },
            "Exposure::Manual",
        );
    }

    #[test]
    fn test_preset_commands_have_terminator() {
        use crate::command::preset::PresetCommand;
        use crate::command::preset::PresetNumber;

        if let Ok(preset) = PresetNumber::new(1) {
            assert_command_has_terminator(
                PresetCommand {
                    action: crate::command::preset::PresetAction::Recall,
                    preset_number: preset,
                },
                "Preset::Recall(1)",
            );
            assert_command_has_terminator(
                PresetCommand {
                    action: crate::command::preset::PresetAction::Set,
                    preset_number: preset,
                },
                "Preset::Set(1)",
            );
        }
    }

    #[test]
    #[allow(clippy::expect_used, clippy::unwrap_used)]
    fn test_white_balance_commands_have_terminator() {
        use crate::command::white_balance::{WhiteBalanceCommand, WhiteBalanceMode};

        assert_command_has_terminator(
            WhiteBalanceCommand {
                mode: WhiteBalanceMode::Auto,
            },
            "WhiteBalance::Auto",
        );
        assert_command_has_terminator(
            WhiteBalanceCommand {
                mode: WhiteBalanceMode::Manual,
            },
            "WhiteBalance::Manual",
        );
    }

    #[test]
    fn test_type_state_prevents_unterminated_commands() {
        use crate::command::bytes::ConstCommandBuilder;

        // Create a builder and terminate it
        let builder = ConstCommandBuilder::<8>::new()
            .push(0x81)
            .push(0x01)
            .push(0x04)
            .push(0x00)
            .push(0x02)
            .terminate();

        // Verify the terminated command has the terminator
        let bytes = builder.as_bytes();
        assert_eq!(bytes[bytes.len() - 1], VISCA_TERMINATOR);

        // Verify we can't access bytes without terminating (compile-time check)
        // The following would not compile:
        // let unterminated = ConstCommandBuilder::<8>::new().push(0x81);
        // let bytes = unterminated.as_bytes(); // ERROR: method not found
    }

    #[test]
    #[allow(clippy::expect_used, clippy::unwrap_used)]
    fn test_all_command_categories_terminate() {
        use crate::command::focus::Focus;
        use crate::command::pan_tilt::PanTilt;
        use crate::command::power::PowerOn;
        use crate::command::zoom::Zoom;

        // Test commands from different categories to ensure
        // the macros are correctly applying termination

        type EncodeFunction = Box<dyn Fn(&mut [u8]) -> Result<usize, crate::Error>>;

        struct TestCase {
            name: &'static str,
            encode: EncodeFunction,
        }

        let test_cases = vec![
            TestCase {
                name: "Quick command (Power)",
                encode: Box::new(|buf| PowerOn::new().write_into(CameraId::CAMERA_1, buf)),
            },
            TestCase {
                name: "Movement command (PanTilt)",
                encode: Box::new(|buf| PanTilt::Home.write_into(CameraId::CAMERA_1, buf)),
            },
            TestCase {
                name: "Movement command (Zoom)",
                encode: Box::new(|buf| Zoom::Stop.write_into(CameraId::CAMERA_1, buf)),
            },
            TestCase {
                name: "Movement command (Focus)",
                encode: Box::new(|buf| Focus::Near.write_into(CameraId::CAMERA_1, buf)),
            },
        ];

        for test_case in test_cases {
            let mut buffer = [0u8; 256];
            let result = (test_case.encode)(&mut buffer);

            assert!(
                result.is_ok(),
                "{test_case_name} failed: {result:?}",
                test_case_name = test_case.name
            );
            let len = result.unwrap();

            assert_eq!(
                buffer[len - 1],
                VISCA_TERMINATOR,
                "{test_case_name} missing terminator",
                test_case_name = test_case.name
            );
        }
    }
}
