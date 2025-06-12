//! Image flip commands for VISCA cameras.
//!
//! This module provides commands for controlling image orientation.

// Standard library imports
// (none)

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{
    command::{Command, ResponseType},
    error::Error,
    timeout::CommandCategory,
};

/// Image flip state.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Flip {
    /// Enable image flip.
    On = 0x02,
    /// Disable image flip.
    Off = 0x03,
}

/// Command to control image flip.
///
/// This command flips the image vertically (upside down).
#[derive(Debug, Copy, Clone)]
pub struct ImageFlipCommand {
    /// The desired flip state.
    pub flip: Flip,
}

impl Command for ImageFlipCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(vec![0x81, 0x01, 0x04, 0x66, self.flip as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flip_on_command() {
        let cmd = ImageFlipCommand { flip: Flip::On };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x66, 0x02, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.command_category(), CommandCategory::Quick));
    }

    #[test]
    fn test_flip_off_command() {
        let cmd = ImageFlipCommand { flip: Flip::Off };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x66, 0x03, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.command_category(), CommandCategory::Quick));
    }

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
        assert_eq!(format!("{:?}", Flip::On), "On");
        assert_eq!(format!("{:?}", Flip::Off), "Off");
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
        let cmd = ImageFlipCommand { flip: Flip::On };
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("ImageFlipCommand"));
        assert!(debug_str.contains("On"));

        let cmd = ImageFlipCommand { flip: Flip::Off };
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("Off"));
    }

    #[test]
    fn test_image_flip_command_clone() {
        let cmd1 = ImageFlipCommand { flip: Flip::On };
        let cmd2 = cmd1.clone();
        assert_eq!(cmd1.flip, cmd2.flip);

        let cmd1 = ImageFlipCommand { flip: Flip::Off };
        let cmd2 = cmd1; // Copy trait
        assert_eq!(cmd2.flip, Flip::Off);
    }

    #[test]
    fn test_response_type_none() {
        // Flip commands don't expect a response beyond ACK/completion
        let cmd_on = ImageFlipCommand { flip: Flip::On };
        assert!(cmd_on.response_type().is_none());

        let cmd_off = ImageFlipCommand { flip: Flip::Off };
        assert!(cmd_off.response_type().is_none());
    }

    #[test]
    fn test_command_trait_impl() {
        // Verify ImageFlipCommand implements Command trait
        let cmd: Box<dyn Command> = Box::new(ImageFlipCommand { flip: Flip::On });
        assert!(cmd.to_bytes().is_ok());
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.command_category(), CommandCategory::Quick));
    }

    #[test]
    fn test_byte_sequence_correctness() {
        // Verify the exact byte sequences match VISCA protocol
        let on_cmd = ImageFlipCommand { flip: Flip::On };
        let on_bytes = on_cmd.to_bytes().unwrap();
        assert_eq!(on_bytes[0], 0x81); // Command header
        assert_eq!(on_bytes[1], 0x01); // Command type
        assert_eq!(on_bytes[2], 0x04); // Category
        assert_eq!(on_bytes[3], 0x66); // Image flip command
        assert_eq!(on_bytes[4], 0x02); // On value
        assert_eq!(on_bytes[5], 0xFF); // Terminator

        let off_cmd = ImageFlipCommand { flip: Flip::Off };
        let off_bytes = off_cmd.to_bytes().unwrap();
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
        let cmd1 = ImageFlipCommand { flip: Flip::On };
        let cmd2 = ImageFlipCommand { flip: Flip::On };
        assert_eq!(cmd1.to_bytes().unwrap(), cmd2.to_bytes().unwrap());

        let cmd1 = ImageFlipCommand { flip: Flip::Off };
        let cmd2 = ImageFlipCommand { flip: Flip::Off };
        assert_eq!(cmd1.to_bytes().unwrap(), cmd2.to_bytes().unwrap());
    }
}
