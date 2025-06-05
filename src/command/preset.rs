//! Preset position commands for VISCA cameras.
//!
//! This module provides commands for storing and recalling camera positions.
//! PTZOptics G2 cameras support up to 90 presets (0-89).

// Standard library imports
use std::convert::TryFrom;

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{
    command::{ViscaCommand, ViscaResponseType},
    error::ViscaError,
    timeout::CommandCategory,
};

/// Action to perform on a preset.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PresetAction {
    /// Reset/clear the preset.
    Reset = 0x00,
    /// Store current position to preset.
    Set = 0x01,
    /// Move camera to preset position.
    Recall = 0x02,
}

/// Preset number with validation.
///
/// Valid range: 0 to 89 (0x00 to 0x59).
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PresetNumber(u8);

impl PresetNumber {
    /// Maximum allowed preset number (89).
    pub const MAX: u8 = 89;

    /// Creates a new PresetNumber with validation.
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if value > 89.
    pub fn new(value: u8) -> Result<Self, ViscaError> {
        if value <= Self::MAX {
            Ok(PresetNumber(value))
        } else {
            Err(ViscaError::InvalidParameter(format!(
                "Preset number must be between 0 and {}",
                Self::MAX
            )))
        }
    }

    /// Get the raw value.
    pub fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for PresetNumber {
    type Error = ViscaError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        PresetNumber::new(value)
    }
}

/// Command to manage camera presets.
#[derive(Debug)]
pub struct PresetCommand {
    /// The action to perform.
    pub action: PresetAction,
    /// The preset number to operate on.
    pub preset_number: PresetNumber,
}

impl ViscaCommand for PresetCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(vec![
            0x81,
            0x01,
            0x04,
            0x3F,
            self.action as u8,
            self.preset_number.value(),
            0xFF,
        ])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Preset
    }
}
