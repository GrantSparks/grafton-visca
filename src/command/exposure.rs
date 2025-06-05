//! Exposure control commands for VISCA cameras.
//!
//! This module provides commands for controlling various exposure-related settings
//! including exposure mode, exposure compensation, iris, shutter, and brightness.

// Standard library imports
use std::convert::TryFrom;

// Crate imports
use crate::{
    command::{response::ViscaResponseType, ViscaCommand},
    error::ViscaError,
    timeout::CommandCategory,
};

#[derive(Debug, Copy, Clone)]
pub enum ExposureMode {
    Auto = 0x00,
    Manual = 0x03,
    Shutter = 0x0A,
    Iris = 0x0B,
    Bright = 0x0D,
}

pub struct ExposureCommand {
    pub mode: ExposureMode,
}

impl ViscaCommand for ExposureCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(vec![0x81, 0x01, 0x04, 0x39, self.mode as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

impl TryFrom<u8> for ExposureMode {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0x00 => Ok(ExposureMode::Auto),
            0x03 => Ok(ExposureMode::Manual),
            0x0A => Ok(ExposureMode::Shutter),
            0x0B => Ok(ExposureMode::Iris),
            0x0D => Ok(ExposureMode::Bright),
            _ => Err(()),
        }
    }
}

/// Exposure compensation commands.
///
/// # Example
/// ```no_run
/// use grafton_visca::command::ExposureCompensationCommand;
/// use grafton_visca::ViscaCommand;
///
/// // Enable exposure compensation
/// let enable = ExposureCompensationCommand::On;
///
/// // Set exposure compensation to +3
/// let set_value = ExposureCompensationCommand::Direct(3);
/// ```
#[derive(Debug, Copy, Clone)]
pub enum ExposureCompensationCommand {
    /// Enable exposure compensation
    On,
    /// Disable exposure compensation
    Off,
    /// Reset exposure compensation to 0
    Reset,
    /// Increase exposure compensation by one step
    Up,
    /// Decrease exposure compensation by one step
    Down,
    /// Set exposure compensation directly (-7 to +7)
    Direct(i8),
}

impl ViscaCommand for ExposureCompensationCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            ExposureCompensationCommand::On => vec![0x81, 0x01, 0x04, 0x3E, 0x02, 0xFF],
            ExposureCompensationCommand::Off => vec![0x81, 0x01, 0x04, 0x3E, 0x03, 0xFF],
            ExposureCompensationCommand::Reset => vec![0x81, 0x01, 0x04, 0x0E, 0x00, 0xFF],
            ExposureCompensationCommand::Up => vec![0x81, 0x01, 0x04, 0x0E, 0x02, 0xFF],
            ExposureCompensationCommand::Down => vec![0x81, 0x01, 0x04, 0x0E, 0x03, 0xFF],
            ExposureCompensationCommand::Direct(level) => {
                if *level < -7 || *level > 7 {
                    return Err(ViscaError::InvalidParameter(
                        "Exposure compensation level must be between -7 and +7".into(),
                    ));
                }
                let value = (*level + 7) as u8; // Convert -7..+7 to 0x0..0xE
                vec![0x81, 0x01, 0x04, 0x4E, 0x00, 0x00, 0x00, value, 0xFF]
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

#[derive(Debug, Copy, Clone)]
pub enum DynamicRangeCommand {
    Direct(u8), // 0 to 8
}

impl ViscaCommand for DynamicRangeCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        match self {
            DynamicRangeCommand::Direct(level) => {
                if *level > 8 {
                    return Err(ViscaError::InvalidParameter(
                        "Dynamic range level must be between 0 and 8".into(),
                    ));
                }
                Ok(vec![0x81, 0x01, 0x04, 0x25, 0x00, 0x00, 0x00, *level, 0xFF])
            }
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

#[derive(Debug, Copy, Clone)]
pub enum IrisCommand {
    Reset,
    Up,
    Down,
    Direct(u8), // 0x00=Close to 0x0C=F1.8
}

impl ViscaCommand for IrisCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            IrisCommand::Reset => vec![0x81, 0x01, 0x04, 0x0B, 0x00, 0xFF],
            IrisCommand::Up => vec![0x81, 0x01, 0x04, 0x0B, 0x02, 0xFF],
            IrisCommand::Down => vec![0x81, 0x01, 0x04, 0x0B, 0x03, 0xFF],
            IrisCommand::Direct(level) => {
                if *level > 0x0C {
                    return Err(ViscaError::InvalidParameter(
                        "Iris level must be between 0x00 (Close) and 0x0C (F1.8)".into(),
                    ));
                }
                vec![0x81, 0x01, 0x04, 0x4B, 0x00, 0x00, 0x00, *level, 0xFF]
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

#[derive(Debug, Copy, Clone)]
pub enum ShutterCommand {
    Reset,
    Up,
    Down,
    Direct(u16), // 0x01=1/30 to 0x11=1/10000
}

impl ViscaCommand for ShutterCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            ShutterCommand::Reset => vec![0x81, 0x01, 0x04, 0x0A, 0x00, 0xFF],
            ShutterCommand::Up => vec![0x81, 0x01, 0x04, 0x0A, 0x02, 0xFF],
            ShutterCommand::Down => vec![0x81, 0x01, 0x04, 0x0A, 0x03, 0xFF],
            ShutterCommand::Direct(value) => {
                if *value < 0x01 || *value > 0x11 {
                    return Err(ViscaError::InvalidParameter(
                        "Shutter value must be between 0x01 (1/30) and 0x11 (1/10000)".into(),
                    ));
                }
                let high = (*value >> 4) as u8;
                let low = (*value & 0x0F) as u8;
                vec![0x81, 0x01, 0x04, 0x4A, 0x00, 0x00, high, low, 0xFF]
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

#[derive(Debug, Copy, Clone)]
pub enum BrightCommand {
    Reset,
    Up,
    Down,
    Direct(u16), // 0x00=0 to 0x11=17
}

impl ViscaCommand for BrightCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            BrightCommand::Reset => vec![0x81, 0x01, 0x04, 0x0D, 0x00, 0xFF],
            BrightCommand::Up => vec![0x81, 0x01, 0x04, 0x0D, 0x02, 0xFF],
            BrightCommand::Down => vec![0x81, 0x01, 0x04, 0x0D, 0x03, 0xFF],
            BrightCommand::Direct(value) => {
                if *value > 0x11 {
                    return Err(ViscaError::InvalidParameter(
                        "Bright value must be between 0x00 (0) and 0x11 (17)".into(),
                    ));
                }
                let high = (*value >> 4) as u8;
                let low = (*value & 0x0F) as u8;
                vec![0x81, 0x01, 0x04, 0x0D, 0x00, 0x00, high, low, 0xFF]
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
