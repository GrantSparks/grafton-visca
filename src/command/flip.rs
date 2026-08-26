//! Image flip commands for VISCA cameras.
//!
//! This module provides commands for controlling image orientation.

use crate::visca_command;

/// Image flip state.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum Flip {
    /// Enable image flip.
    On = 0x02,
    /// Disable image flip.
    Off = 0x03,
}

visca_command! {
        /// Command to control image flip.
    ///
    /// This command flips the image vertically (upside down).
    pub struct ImageFlip { flip: Flip };
    prefix = [0x01, 0x04, 0x66];
    param = match flip { Flip::On => 0x02, Flip::Off => 0x03 };
    max_param_size = 1;
}

visca_command! {
        /// Command to control horizontal flip (mirror).
    ///
    /// This command flips the image horizontally (left-right mirror).
    pub struct HorizontalFlip { on: bool };
    prefix = [0x01, 0x04, 0x61];
    param = if *on { 0x02 } else { 0x03 };
    max_param_size = 1;
}

visca_command! {
        /// Command to control image freeze.
    ///
    /// This command freezes the current image frame.
    pub struct ImageFreeze { on: bool };
    prefix = [0x01, 0x04, 0x62];
    param = if *on { 0x02 } else { 0x03 };
    max_param_size = 1;
}

impl ImageFreeze {
    /// Create an image-freeze command.
    #[must_use]
    pub const fn new(on: bool) -> Self {
        Self { on }
    }

    /// Create a command that freezes the current image frame.
    #[must_use]
    pub const fn on() -> Self {
        Self::new(true)
    }

    /// Create a command that resumes live image output.
    #[must_use]
    pub const fn off() -> Self {
        Self::new(false)
    }

    /// Returns whether image freezing is enabled.
    #[must_use]
    pub const fn enabled(self) -> bool {
        self.on
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::clone_on_copy,
    clippy::uninlined_format_args
)]
mod tests {
    use crate::{
        camera_id::CameraId, command::bytes::VISCA_TERMINATOR, macros::test_utils::visca_test,
    };

    use super::*;

    visca_test!(
        ImageFlip,
        test_flip_on_command,
        ImageFlip { flip: Flip::On },
        &[0x81, 0x01, 0x04, 0x66, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        ImageFlip,
        test_flip_off_command,
        ImageFlip { flip: Flip::Off },
        &[0x81, 0x01, 0x04, 0x66, 0x03, VISCA_TERMINATOR]
    );

    #[test]
    fn test_flip_enum_values() {
        assert_eq!(Flip::On as u8, 0x02);
        assert_eq!(Flip::Off as u8, 0x03);
    }

    #[test]
    fn test_flip_enum_equality() {
        assert_eq!(Flip::On, Flip::On);
        assert_eq!(Flip::Off, Flip::Off);
        assert_ne!(Flip::On, Flip::Off);
    }

    #[test]
    fn test_flip_enum_debug() {
        let on = Flip::On;
        assert_eq!(format!("{on:?}"), "On");
        let off = Flip::Off;
        assert_eq!(format!("{off:?}"), "Off");
    }

    #[test]
    fn test_flip_enum_clone() {
        let flip1 = Flip::On;
        let flip2 = flip1.clone();
        assert_eq!(flip1, flip2);

        let flip1 = Flip::Off;
        let flip2 = flip1; // Copy trait
        assert_eq!(flip2, Flip::Off);
    }

    #[test]
    fn test_image_flip_command_debug() {
        let cmd = ImageFlip { flip: Flip::On };
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("On"));

        let cmd = ImageFlip { flip: Flip::Off };
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("Off"));
    }

    #[test]
    fn test_image_flip_command_clone() {
        let cmd1 = ImageFlip { flip: Flip::On };
        let cmd2 = cmd1.clone();
        // Verify commands produce same bytes
        assert_eq!(
            crate::command::test_wire_bytes(&cmd1, CameraId::CAMERA_1).unwrap(),
            crate::command::test_wire_bytes(&cmd2, CameraId::CAMERA_1).unwrap()
        );

        let cmd1 = ImageFlip { flip: Flip::Off };
        let cmd2 = cmd1; // Copy trait
        assert_eq!(
            crate::command::test_wire_bytes(&cmd2, CameraId::CAMERA_1).unwrap(),
            vec![0x81, 0x01, 0x04, 0x66, 0x03, VISCA_TERMINATOR]
        );
    }

    #[test]
    fn test_response_type_none() {
        // Flip commands don't expect a response beyond ACK/completion
        let cmd_on = ImageFlip { flip: Flip::On };
        assert!(crate::command::test_wire_bytes(&cmd_on, CameraId::CAMERA_1).is_ok());

        let cmd_off = ImageFlip { flip: Flip::Off };
        assert!(crate::command::test_wire_bytes(&cmd_off, CameraId::CAMERA_1).is_ok());
    }

    #[test]
    fn test_command_trait_impl() {
        // Verify ImageFlip implements the private wire encoder.
        let cmd = ImageFlip { flip: Flip::On };
        assert!(crate::command::test_wire_bytes(&cmd, CameraId::CAMERA_1).is_ok());
    }

    #[test]
    fn test_byte_sequence_correctness() {
        // Verify the exact byte sequences match VISCA protocol
        let on_cmd = ImageFlip { flip: Flip::On };
        let on_bytes = crate::command::test_wire_bytes(&on_cmd, CameraId::CAMERA_1).unwrap();
        assert_eq!(on_bytes[0], 0x81); // Command header
        assert_eq!(on_bytes[1], 0x01); // Command type
        assert_eq!(on_bytes[2], 0x04); // Category
        assert_eq!(on_bytes[3], 0x66); // Image flip command
        assert_eq!(on_bytes[4], 0x02); // On value
        assert_eq!(on_bytes[5], 0xFF); // Terminator

        let off_cmd = ImageFlip { flip: Flip::Off };
        let off_bytes = crate::command::test_wire_bytes(&off_cmd, CameraId::CAMERA_1).unwrap();
        assert_eq!(off_bytes[0], 0x81); // Command header
        assert_eq!(off_bytes[1], 0x01); // Command type
        assert_eq!(off_bytes[2], 0x04); // Category
        assert_eq!(off_bytes[3], 0x66); // Image flip command
        assert_eq!(off_bytes[4], 0x03); // Off value
        assert_eq!(off_bytes[5], 0xFF); // Terminator
    }

    #[test]
    fn test_command_consistency() {
        // Test that creating commands with the same flip state produces identical bytes
        let cmd1 = ImageFlip { flip: Flip::On };
        let cmd2 = ImageFlip { flip: Flip::On };
        assert_eq!(
            crate::command::test_wire_bytes(&cmd1, CameraId::CAMERA_1).unwrap(),
            crate::command::test_wire_bytes(&cmd2, CameraId::CAMERA_1).unwrap()
        );

        let cmd1 = ImageFlip { flip: Flip::Off };
        let cmd2 = ImageFlip { flip: Flip::Off };
        assert_eq!(
            crate::command::test_wire_bytes(&cmd1, CameraId::CAMERA_1).unwrap(),
            crate::command::test_wire_bytes(&cmd2, CameraId::CAMERA_1).unwrap()
        );
    }
}
