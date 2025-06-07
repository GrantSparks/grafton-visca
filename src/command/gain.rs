//! Gain control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera gain settings,
//! including manual gain adjustment, gain limit control, and anti-flicker settings.

// Crate imports
use crate::{
    command::{response::ViscaResponseType, ViscaCommand},
    error::ViscaError,
    timeout::CommandCategory,
};

/// Commands for controlling camera gain.
///
/// Gain amplifies the camera sensor's signal, allowing for brighter images
/// in low light conditions at the cost of increased noise.
#[derive(Debug, Copy, Clone)]
pub enum GainCommand {
    /// Reset gain to default value.
    Reset,
    /// Increase gain by one step.
    Up,
    /// Decrease gain by one step.
    Down,
    /// Set gain to specific value (0x00 to 0x07, representing gain levels 0-7).
    Direct(u16),
}

impl ViscaCommand for GainCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            GainCommand::Reset => vec![0x81, 0x01, 0x04, 0x0C, 0x00, 0xFF],
            GainCommand::Up => vec![0x81, 0x01, 0x04, 0x0C, 0x02, 0xFF],
            GainCommand::Down => vec![0x81, 0x01, 0x04, 0x0C, 0x03, 0xFF],
            GainCommand::Direct(value) => {
                if *value > 0x07 {
                    return Err(ViscaError::InvalidParameter(
                        "Gain value must be between 0x00 (0) and 0x07 (7)".into(),
                    ));
                }
                let high = (*value >> 4) as u8;
                let low = (*value & 0x0F) as u8;
                vec![0x81, 0x01, 0x04, 0x0C, 0x00, 0x00, high, low, 0xFF]
            }
        })
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Command to set the maximum gain limit.
///
/// Limits the maximum gain that can be applied when in auto exposure mode,
/// helping to control noise levels in low light conditions.
#[derive(Debug, Copy, Clone)]
pub struct GainLimitCommand {
    /// Maximum gain limit (0x0 to 0xF, representing levels 0-15).
    pub limit: u8,
}

impl ViscaCommand for GainLimitCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        if self.limit > 0x0F {
            return Err(ViscaError::InvalidParameter(
                "Gain limit must be between 0x0 (0) and 0xF (15)".into(),
            ));
        }
        Ok(vec![0x81, 0x01, 0x04, 0x2C, self.limit, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Anti-flicker mode settings.
///
/// Reduces flicker caused by artificial lighting that operates at
/// different frequencies than the camera's frame rate.
#[derive(Debug, Copy, Clone)]
pub enum AntiFlickerMode {
    /// Disable anti-flicker processing.
    Off = 0x00,
    /// Enable 50Hz anti-flicker (for regions with 50Hz AC power).
    Hz50 = 0x01,
    /// Enable 60Hz anti-flicker (for regions with 60Hz AC power).
    Hz60 = 0x02,
}

/// Command to set anti-flicker mode.
#[derive(Debug, Copy, Clone)]
pub struct AntiFlickerCommand {
    /// The anti-flicker mode to apply.
    pub mode: AntiFlickerMode,
}

impl ViscaCommand for AntiFlickerCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(vec![0x81, 0x01, 0x04, 0x23, self.mode as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}
