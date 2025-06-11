//! Gain control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera gain settings,
//! including manual gain adjustment, gain limit control, and anti-flicker settings.

// Crate imports
use crate::{
    command::{response::ResponseType, Command},
    constants::CameraModel,
    error::Error,
    timeout::CommandCategory,
    types::{GainLimit, GainValue},
};

/// Commands for controlling gain values.
///
/// Provides standard VISCA control operations:
/// - Reset to default value
/// - Increment/decrement by one step
/// - Set to a specific value
#[derive(Debug, Copy, Clone)]
pub enum GainCommand {
    /// Reset to default value.
    Reset,
    /// Increase value by one step.
    Up,
    /// Decrease value by one step.
    Down,
    /// Set to specific value.
    Direct(GainValue),
}

// Manual implementation to add model validation
impl Command for GainCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(match self {
            Self::Reset => vec![0x81, 0x01, 0x04, 0x0C, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x0C, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x0C, 0x03, 0xFF],
            Self::Direct(value) => {
                let val = value.value();
                let high = (val >> 4) & 0x0F;
                let low = val & 0x0F;
                vec![0x81, 0x01, 0x04, 0x4C, 0x00, 0x00, high, low, 0xFF]
            }
        })
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        match self {
            Self::Direct(gain) => {
                if matches!(model, CameraModel::PTZOpticsG2)
                    && !GainValue::G2_VALID_VALUES.contains(&gain.value())
                {
                    return Err(Error::ModelValidation {
                        model,
                        command: "GainDirect".to_string(),
                        reason: format!(
                            "Gain value {:#02X} is not valid for G2. Valid values: 0x00-0x07",
                            gain.value()
                        ),
                    });
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Command to set the automatic gain control limit.
#[derive(Debug, Clone, Copy)]
pub struct GainLimitCommand {
    /// The maximum gain level allowed in auto mode.
    pub limit: GainLimit,
}

impl Command for GainLimitCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(vec![0x81, 0x01, 0x04, 0x2C, self.limit.value(), 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        match model {
            CameraModel::PTZOpticsG2 => {
                let value = self.limit.value();
                if !GainLimit::G2_VALID_VALUES.contains(&value) {
                    return Err(Error::ModelValidation {
                        model,
                        command: "GainLimit".to_string(),
                        reason: format!("Value {value:#02X} not supported on G2 cameras"),
                    });
                }
                Ok(())
            }
            _ => Ok(()),
        }
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
