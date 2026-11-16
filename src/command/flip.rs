//! Image flip commands for VISCA cameras.
//!
//! This module provides commands for controlling image orientation.

use crate::command::bytes::constants::flip;

crate::macros::consolidated::on_off_command! {
    /// Command to control image flip.
    ///
    /// This command flips the image vertically (upside down).
    #[derive(Debug, Copy, Clone)]
    pub struct ImageFlip { pub flip: bool };
    prefix = flip::VERTICAL;
    convention = crate::wire::BoolConvention::OnIs02;
}

crate::macros::consolidated::on_off_command! {
    /// Command to control horizontal flip (mirror).
    ///
    /// This command flips the image horizontally (left-right mirror).
    #[derive(Debug, Copy, Clone)]
    pub struct HorizontalFlip { pub on: bool };
    prefix = flip::HORIZONTAL;
    convention = crate::wire::BoolConvention::OnIs02;
}

crate::macros::consolidated::on_off_command! {
    /// Command to control image freeze.
    ///
    /// This command freezes the current image frame.
    #[derive(Debug, Copy, Clone)]
    pub struct ImageFreeze { pub on: bool };
    prefix = flip::FREEZE;
    convention = crate::wire::BoolConvention::OnIs02;
}

impl ImageFreeze {
    /// Create an image-freeze command.
    #[must_use]
    pub const fn new(on: bool) -> Self {
        Self { on }
    }

    /// Create a command that freezes the current image frame.
    #[must_use]
    #[cfg(all(test, any(feature = "blocking", feature = "async")))]
    pub const fn on() -> Self {
        Self::new(true)
    }

    /// Create a command that resumes live image output.
    #[must_use]
    #[cfg(all(test, any(feature = "blocking", feature = "async")))]
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
        ImageFlip { flip: true },
        &[0x81, 0x01, 0x04, 0x66, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        ImageFlip,
        test_flip_off_command,
        ImageFlip { flip: false },
        &[0x81, 0x01, 0x04, 0x66, 0x03, VISCA_TERMINATOR]
    );

    #[test]
    fn test_image_flip_command_debug() {
        let cmd = ImageFlip { flip: true };
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("true"));

        let cmd = ImageFlip { flip: false };
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("false"));
    }

    #[test]
    fn test_image_flip_command_clone() {
        let cmd1 = ImageFlip { flip: true };
        let cmd2 = cmd1.clone();
        // Verify commands produce same bytes
        assert_eq!(
            crate::command::test_wire_bytes(&cmd1, CameraId::CAMERA_1).unwrap(),
            crate::command::test_wire_bytes(&cmd2, CameraId::CAMERA_1).unwrap()
        );

        let cmd1 = ImageFlip { flip: false };
        let cmd2 = cmd1; // Copy trait
        assert_eq!(
            crate::command::test_wire_bytes(&cmd2, CameraId::CAMERA_1).unwrap(),
            vec![0x81, 0x01, 0x04, 0x66, 0x03, VISCA_TERMINATOR]
        );
    }

    #[test]
    fn test_response_type_none() {
        // Flip commands don't expect a response beyond ACK/completion
        let cmd_on = ImageFlip { flip: true };
        assert!(crate::command::test_wire_bytes(&cmd_on, CameraId::CAMERA_1).is_ok());

        let cmd_off = ImageFlip { flip: false };
        assert!(crate::command::test_wire_bytes(&cmd_off, CameraId::CAMERA_1).is_ok());
    }

    #[test]
    fn test_command_trait_impl() {
        // Verify ImageFlip implements the private wire encoder.
        let cmd = ImageFlip { flip: true };
        assert!(crate::command::test_wire_bytes(&cmd, CameraId::CAMERA_1).is_ok());
    }

    #[test]
    fn test_byte_sequence_correctness() {
        // Verify the exact byte sequences match VISCA protocol
        let on_cmd = ImageFlip { flip: true };
        let on_bytes = crate::command::test_wire_bytes(&on_cmd, CameraId::CAMERA_1).unwrap();
        assert_eq!(on_bytes[0], 0x81); // Command header
        assert_eq!(on_bytes[1], 0x01); // Command type
        assert_eq!(on_bytes[2], 0x04); // Category
        assert_eq!(on_bytes[3], 0x66); // Image flip command
        assert_eq!(on_bytes[4], 0x02); // On value
        assert_eq!(on_bytes[5], 0xFF); // Terminator

        let off_cmd = ImageFlip { flip: false };
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
        let cmd1 = ImageFlip { flip: true };
        let cmd2 = ImageFlip { flip: true };
        assert_eq!(
            crate::command::test_wire_bytes(&cmd1, CameraId::CAMERA_1).unwrap(),
            crate::command::test_wire_bytes(&cmd2, CameraId::CAMERA_1).unwrap()
        );

        let cmd1 = ImageFlip { flip: false };
        let cmd2 = ImageFlip { flip: false };
        assert_eq!(
            crate::command::test_wire_bytes(&cmd1, CameraId::CAMERA_1).unwrap(),
            crate::command::test_wire_bytes(&cmd2, CameraId::CAMERA_1).unwrap()
        );
    }
}
