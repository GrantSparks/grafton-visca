//! Image flip commands for VISCA cameras.
//!
//! This module provides commands for controlling image orientation.

use crate::{timeout::CommandCategory, visca_command};

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
    pub struct ImageFlip { flip: Flip };
    prefix = [0x01, 0x04, 0x66];
    param = match flip { Flip::On => 0x02, Flip::Off => 0x03 };
    category = CommandCategory::Quick;
}

visca_command! {
        /// Command to control horizontal flip (mirror).
    ///
    /// This command flips the image horizontally (left-right mirror).
    pub struct HorizontalFlip { on: bool };
    prefix = [0x01, 0x04, 0x61];
    param = if *on { 0x02 } else { 0x03 };
    category = CommandCategory::Quick;
}

visca_command! {
        /// Command to control image freeze.
    ///
    /// This command freezes the current image frame.
    pub struct ImageFreeze { on: bool };
    prefix = [0x01, 0x04, 0x62];
    param = if *on { 0x02 } else { 0x03 };
    category = CommandCategory::Quick;
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::clone_on_copy,
    clippy::uninlined_format_args
)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::macros::test_utils::visca_test;
    use crate::timeout::CommandTimeout;
    use crate::{command::encode::ViscaCommand, timeout::CommandCategory};

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
            cmd1.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap(),
            cmd2.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap()
        );

        let cmd1 = ImageFlip { flip: Flip::Off };
        let cmd2 = cmd1; // Copy trait
        assert_eq!(
            cmd2.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap(),
            vec![0x81, 0x01, 0x04, 0x66, 0x03, VISCA_TERMINATOR]
        );
    }

    #[test]
    fn test_response_type_none() {
        // Flip commands don't expect a response beyond ACK/completion
        let cmd_on = ImageFlip { flip: Flip::On };
        assert!(cmd_on.response_kind().is_none());

        let cmd_off = ImageFlip { flip: Flip::Off };
        assert!(cmd_off.response_kind().is_none());
    }

    #[test]
    fn test_command_trait_impl() {
        // Verify ImageFlip implements ViscaCommand trait
        let cmd = ImageFlip { flip: Flip::On };
        assert!(cmd
            .to_bytes(crate::camera_id::CameraId::CAMERA_1)
            .map(|b| b.to_vec())
            .is_ok());
        assert!(cmd.response_kind().is_none());
        assert!(matches!(cmd.timeout_class(), CommandCategory::Quick));
    }

    #[test]
    fn test_byte_sequence_correctness() {
        // Verify the exact byte sequences match VISCA protocol
        let on_cmd = ImageFlip { flip: Flip::On };
        let on_bytes = on_cmd
            .to_bytes(crate::camera_id::CameraId::CAMERA_1)
            .map(|b| b.to_vec())
            .unwrap();
        assert_eq!(on_bytes[0], 0x81); // Command header
        assert_eq!(on_bytes[1], 0x01); // Command type
        assert_eq!(on_bytes[2], 0x04); // Category
        assert_eq!(on_bytes[3], 0x66); // Image flip command
        assert_eq!(on_bytes[4], 0x02); // On value
        assert_eq!(on_bytes[5], 0xFF); // Terminator

        let off_cmd = ImageFlip { flip: Flip::Off };
        let off_bytes = off_cmd
            .to_bytes(crate::camera_id::CameraId::CAMERA_1)
            .map(|b| b.to_vec())
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
        let cmd1 = ImageFlip { flip: Flip::On };
        let cmd2 = ImageFlip { flip: Flip::On };
        assert_eq!(
            cmd1.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap(),
            cmd2.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap()
        );

        let cmd1 = ImageFlip { flip: Flip::Off };
        let cmd2 = ImageFlip { flip: Flip::Off };
        assert_eq!(
            cmd1.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap(),
            cmd2.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap()
        );
    }
}
