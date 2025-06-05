//! Exposure control commands for VISCA cameras.
//!
//! This module provides commands for controlling various exposure-related settings
//! including exposure mode, exposure compensation, iris, shutter, and brightness.

// Standard library imports
use std::convert::TryFrom;

// Third-party crate imports
// (none)

// Workspace / local-crate imports
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
            0x00 => Ok(Self::Auto),
            0x03 => Ok(Self::Manual),
            0x0A => Ok(Self::Shutter),
            0x0B => Ok(Self::Iris),
            0x0D => Ok(Self::Bright),
            _ => Err(()),
        }
    }
}

/// Exposure compensation level.
///
/// Valid range: -7 to +7.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExposureCompensationLevel(i8);

impl ExposureCompensationLevel {
    /// Minimum exposure compensation level.
    pub const MIN: i8 = -7;
    /// Maximum exposure compensation level.
    pub const MAX: i8 = 7;

    /// Creates a new ExposureCompensationLevel with validation.
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if value is outside -7 to +7 range.
    pub fn new(value: i8) -> Result<Self, ViscaError> {
        if (Self::MIN..=Self::MAX).contains(&value) {
            Ok(ExposureCompensationLevel(value))
        } else {
            Err(ViscaError::InvalidParameter(format!(
                "Exposure compensation level must be between {} and {}",
                Self::MIN,
                Self::MAX
            )))
        }
    }

    /// Get the raw value.
    pub fn value(self) -> i8 {
        self.0
    }

    /// Convert to protocol value (0x0 to 0xE).
    pub fn to_protocol_value(self) -> u8 {
        (self.0 + 7) as u8
    }
}

impl TryFrom<i8> for ExposureCompensationLevel {
    type Error = ViscaError;

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        ExposureCompensationLevel::new(value)
    }
}

/// Exposure compensation commands.
///
/// # Example
/// ```no_run
/// use grafton_visca::command::{ExposureCompensationCommand, exposure::ExposureCompensationLevel};
/// use grafton_visca::ViscaCommand;
///
/// // Enable exposure compensation
/// let enable = ExposureCompensationCommand::On;
///
/// // Set exposure compensation to +3
/// let set_value = ExposureCompensationCommand::Direct(ExposureCompensationLevel::new(3).unwrap());
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
    Direct(ExposureCompensationLevel),
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
                let value = level.to_protocol_value();
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

/// Dynamic range level.
///
/// Valid range: 0 to 8.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DynamicRangeLevel(u8);

impl DynamicRangeLevel {
    /// Maximum allowed dynamic range level.
    pub const MAX: u8 = 8;

    /// Creates a new DynamicRangeLevel with validation.
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if value > 8.
    pub fn new(value: u8) -> Result<Self, ViscaError> {
        if value <= Self::MAX {
            Ok(DynamicRangeLevel(value))
        } else {
            Err(ViscaError::InvalidParameter(format!(
                "Dynamic range level must be between 0 and {}",
                Self::MAX
            )))
        }
    }

    /// Get the raw value.
    pub fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for DynamicRangeLevel {
    type Error = ViscaError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        DynamicRangeLevel::new(value)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum DynamicRangeCommand {
    Direct(DynamicRangeLevel),
}

impl ViscaCommand for DynamicRangeCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        match self {
            DynamicRangeCommand::Direct(level) => Ok(vec![
                0x81,
                0x01,
                0x04,
                0x25,
                0x00,
                0x00,
                0x00,
                level.value(),
                0xFF,
            ]),
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
