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
    error::Error as ViscaError,
    timeout::CommandCategory,
};

/// Power state for the camera.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Power {
    /// Camera is powered on and operational.
    On = 0x02,
    /// Camera is in standby mode.
    Standby = 0x03,
}

/// Command to set camera power state.
#[derive(Debug, Copy, Clone)]
pub struct PowerCommand {
    /// The desired power state.
    pub power: Power,
}

impl Command for PowerCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(vec![0x81, 0x01, 0x04, 0x00, self.power as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}
