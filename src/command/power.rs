//! Power control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera power state.

// Standard library imports
// (none)

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{
    command::{encode_visca::EncodeVisca, ResponseType},
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
pub(crate) struct PowerCommand {
    /// The desired power state.
    pub power: Power,
}

impl EncodeVisca for PowerCommand {
    type Response = ();
    const MAX_SIZE: usize = 6;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        let power_byte = match self.power {
            Power::On => 0x02,
            Power::Standby => 0x03,
        };
        
        buffer[0] = 0x81;
        buffer[1] = 0x01;
        buffer[2] = 0x04;
        buffer[3] = 0x00;
        buffer[4] = power_byte;
        buffer[5] = 0xFF;
        
        Ok(Self::MAX_SIZE)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}
