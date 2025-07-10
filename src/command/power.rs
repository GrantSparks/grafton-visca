//! Power control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera power state.

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

/// Power state for the camera.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Power {
    /// Power on state.
    On,
    /// Standby state (power off).
    Standby,
}

/// Command to control camera power state.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PowerCommand {
    /// The desired power state.
    pub power: Power,
}

impl Command for PowerCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let power_byte = match self.power {
            Power::On => 0x02,
            Power::Standby => 0x03,
        };
        Ok(vec![0x81, 0x01, 0x04, 0x00, power_byte, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}