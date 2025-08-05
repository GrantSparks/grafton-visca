//! Image flip commands for VISCA cameras.
//!
//! This module provides commands for controlling image orientation.

// Standard library imports
// (none)

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::macros::internal::*;

use crate::{command::const_encoding::CommandBuilder, error::Error};

/// Image flip state.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
    category = "Quick",
    enum ImageFlipCommand {
        /// Enable image flip.
        On => {
            let cmd = CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::flip::PREFIX)
                .push(0x02)
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Disable image flip.
        Off => {
            let cmd = CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::flip::PREFIX)
                .push(0x03)
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
    }
}

impl ImageFlipCommand {
    /// Create a new image flip command.
    pub fn new(flip: Flip) -> Self {
        match flip {
            Flip::On => Self::On,
            Flip::Off => Self::Off,
        }
    }
}

/// Horizontal flip (mirror) state.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HorizontalFlip {
    /// Enable horizontal flip (mirror).
    On = 0x02,
    /// Disable horizontal flip (mirror).
    Off = 0x03,
}

visca_command! {
    /// Command to control horizontal flip (mirror).
    ///
    /// This command flips the image horizontally (left-right mirror).
    category = "Quick",
    enum HorizontalFlipCommand {
        /// Enable horizontal flip (mirror).
        On => {
            let cmd = CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::flip::HFLIP_PREFIX)
                .push(0x02)
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Disable horizontal flip (mirror).
        Off => {
            let cmd = CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::flip::HFLIP_PREFIX)
                .push(0x03)
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
    }
}

impl HorizontalFlipCommand {
    /// Create a new horizontal flip command.
    pub fn new(flip: HorizontalFlip) -> Self {
        match flip {
            HorizontalFlip::On => Self::On,
            HorizontalFlip::Off => Self::Off,
        }
    }
}

/// Image freeze state.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Freeze {
    /// Enable image freeze.
    On = 0x02,
    /// Disable image freeze.
    Off = 0x03,
}

visca_command! {
    /// Command to control image freeze.
    ///
    /// This command freezes the current image frame.
    category = "Quick",
    enum ImageFreezeCommand {
        /// Enable image freeze.
        On => {
            let cmd = CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::flip::FREEZE_PREFIX)
                .push(0x02)
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Disable image freeze.
        Off => {
            let cmd = CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::flip::FREEZE_PREFIX)
                .push(0x03)
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
    }
}

impl ImageFreezeCommand {
    /// Create a new image freeze command.
    pub fn new(freeze: Freeze) -> Self {
        match freeze {
            Freeze::On => Self::On,
            Freeze::Off => Self::Off,
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::clone_on_copy,
    clippy::uninlined_format_args
)]
mod tests {
    use super::*;
    use crate::macros::test_utils::visca_test;
    use crate::{command::encode_visca::EncodeVisca, timeout::CommandCategory};

    visca_test!(
        ImageFlipCommand,
        test_flip_on_command,
        ImageFlipCommand::new(Flip::On),
        &[0x81, 0x01, 0x04, 0x66, 0x02, 0xFF]
    );

    visca_test!(
        ImageFlipCommand,
        test_flip_off_command,
        ImageFlipCommand::new(Flip::Off),
        &[0x81, 0x01, 0x04, 0x66, 0x03, 0xFF]
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
        let cmd = ImageFlipCommand::new(Flip::On);
        let debug_str = format!("{cmd:?}");
        // With the macro-generated enum, debug output will be "On" or "Off"
        assert!(debug_str == "On" || debug_str.contains("On"));

        let cmd = ImageFlipCommand::new(Flip::Off);
        let debug_str = format!("{cmd:?}");
        assert!(debug_str == "Off" || debug_str.contains("Off"));
    }

    #[test]
    fn test_image_flip_command_clone() {
        let cmd1 = ImageFlipCommand::new(Flip::On);
        let cmd2 = cmd1.clone();
        // Verify commands produce same bytes
        assert_eq!(
            cmd1.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap(),
            cmd2.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap()
        );

        let cmd1 = ImageFlipCommand::new(Flip::Off);
        let cmd2 = cmd1; // Copy trait
                         // Verify the command was copied correctly
        assert_eq!(
            cmd2.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap(),
            vec![0x81, 0x01, 0x04, 0x66, 0x03, 0xFF]
        );
    }

    #[test]
    fn test_response_type_none() {
        // Flip commands don't expect a response beyond ACK/completion
        let cmd_on = ImageFlipCommand::new(Flip::On);
        assert!(cmd_on.response_type().is_none());

        let cmd_off = ImageFlipCommand::new(Flip::Off);
        assert!(cmd_off.response_type().is_none());
    }

    #[test]
    fn test_command_trait_impl() {
        // Verify ImageFlipCommand implements EncodeVisca trait
        let cmd = ImageFlipCommand::new(Flip::On);
        assert!(cmd
            .try_into_vec(crate::camera_id::CameraId::CAMERA_1)
            .is_ok());
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));
    }

    #[test]
    fn test_byte_sequence_correctness() {
        // Verify the exact byte sequences match VISCA protocol
        let on_cmd = ImageFlipCommand::new(Flip::On);
        let on_bytes = on_cmd
            .try_into_vec(crate::camera_id::CameraId::CAMERA_1)
            .unwrap();
        assert_eq!(on_bytes[0], 0x81); // Command header
        assert_eq!(on_bytes[1], 0x01); // Command type
        assert_eq!(on_bytes[2], 0x04); // Category
        assert_eq!(on_bytes[3], 0x66); // Image flip command
        assert_eq!(on_bytes[4], 0x02); // On value
        assert_eq!(on_bytes[5], 0xFF); // Terminator

        let off_cmd = ImageFlipCommand::new(Flip::Off);
        let off_bytes = off_cmd
            .try_into_vec(crate::camera_id::CameraId::CAMERA_1)
            .unwrap();
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
        let cmd1 = ImageFlipCommand::new(Flip::On);
        let cmd2 = ImageFlipCommand::new(Flip::On);
        assert_eq!(
            cmd1.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap(),
            cmd2.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap()
        );

        let cmd1 = ImageFlipCommand::new(Flip::Off);
        let cmd2 = ImageFlipCommand::new(Flip::Off);
        assert_eq!(
            cmd1.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap(),
            cmd2.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap()
        );
    }
}
