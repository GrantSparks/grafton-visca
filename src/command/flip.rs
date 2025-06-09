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
    error::Error as ViscaError,
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
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(vec![0x81, 0x01, 0x04, 0x66, self.flip as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}
