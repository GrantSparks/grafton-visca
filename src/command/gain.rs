//! Gain control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera gain settings,
//! including manual gain adjustment, gain limit control, and anti-flicker settings.

// Crate imports
use crate::{
    command::{response::ResponseType, Command},
    error::Error,
    timeout::CommandCategory,
    types::{GainLimit, GainValue},
    visca_param_command, visca_up_down_reset,
};

visca_up_down_reset! {
    #[category = "Quick"]
    enum GainCommand {
        command_byte: 0x0C,
        Direct(value: GainValue) => |high, low| [0x81, 0x01, 0x04, 0x4C, 0x00, 0x00, high, low, 0xFF]
    }
}

crate::visca_param_command! {
    /// Command to set the automatic gain control limit.
    struct GainLimitCommand {
        /// The maximum gain level allowed in auto mode.
        limit: GainLimit => direct
    }
    bytes = [0x81, 0x01, 0x04, 0x2C, {limit}, 0xFF]
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

impl Command for AntiFlickerCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(vec![0x81, 0x01, 0x04, 0x23, self.mode as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}
