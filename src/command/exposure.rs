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

/// Camera exposure control modes.
#[derive(Debug, Copy, Clone)]
pub enum ExposureMode {
    /// Automatic exposure control - camera adjusts all exposure parameters automatically
    Auto = 0x00,
    /// Manual exposure control - user has full control over exposure parameters
    Manual = 0x03,
    /// Shutter priority mode - user controls shutter speed, camera adjusts other parameters
    Shutter = 0x0A,
    /// Iris priority mode - user controls iris/aperture, camera adjusts other parameters
    Iris = 0x0B,
    /// Brightness priority mode - user controls brightness level, camera adjusts other parameters
    Bright = 0x0D,
}

/// Command to set the camera's exposure mode.
///
/// This command allows switching between different exposure modes such as
/// auto, manual, shutter priority, iris priority, or brightness priority.
pub struct ExposureCommand {
    /// The exposure mode to set.
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

    /// Creates a new `ExposureCompensationLevel` with validation.
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if value is outside -7 to +7 range.
    pub fn new(value: i8) -> Result<Self, ViscaError> {
        if (Self::MIN..=Self::MAX).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ViscaError::InvalidParameter(format!(
                "Exposure compensation level must be between {} and {}",
                Self::MIN,
                Self::MAX
            )))
        }
    }

    /// Get the raw value.
    pub const fn value(self) -> i8 {
        self.0
    }

    /// Convert to protocol value (0x0 to 0xE).
    #[allow(clippy::cast_sign_loss)]
    pub const fn to_protocol_value(self) -> u8 {
        (self.0 + 7) as u8
    }
}

impl TryFrom<i8> for ExposureCompensationLevel {
    type Error = ViscaError;

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        Self::new(value)
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
            Self::On => vec![0x81, 0x01, 0x04, 0x3E, 0x02, 0xFF],
            Self::Off => vec![0x81, 0x01, 0x04, 0x3E, 0x03, 0xFF],
            Self::Reset => vec![0x81, 0x01, 0x04, 0x0E, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x0E, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x0E, 0x03, 0xFF],
            Self::Direct(level) => {
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

    /// Creates a new `DynamicRangeLevel` with validation.
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if value > 8.
    pub fn new(value: u8) -> Result<Self, ViscaError> {
        if value <= Self::MAX {
            Ok(Self(value))
        } else {
            Err(ViscaError::InvalidParameter(format!(
                "Dynamic range level must be between 0 and {}",
                Self::MAX
            )))
        }
    }

    /// Get the raw value.
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for DynamicRangeLevel {
    type Error = ViscaError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Commands for controlling the camera's dynamic range.
///
/// Dynamic range control adjusts the camera's ability to capture detail
/// in both bright and dark areas of a scene simultaneously. Higher values
/// increase the dynamic range, allowing better detail retention in scenes
/// with high contrast.
#[derive(Debug, Copy, Clone)]
pub enum DynamicRangeCommand {
    /// Set dynamic range to a specific level (0-8).
    Direct(DynamicRangeLevel),
}

impl ViscaCommand for DynamicRangeCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        match self {
            Self::Direct(level) => Ok(vec![
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

/// Commands for controlling the camera iris (aperture).
///
/// The iris controls the amount of light entering the camera by adjusting
/// the aperture size. Smaller aperture values mean less light but greater
/// depth of field.
#[derive(Debug, Copy, Clone)]
pub enum IrisCommand {
    /// Reset iris to default position.
    Reset,
    /// Open iris to increase aperture (let in more light).
    Up,
    /// Close iris to decrease aperture (let in less light).
    Down,
    /// Set iris to specific aperture value.
    ///
    /// Valid range: 0x00 (fully closed) to 0x0C (F1.8 - fully open).
    Direct(u8),
}

impl ViscaCommand for IrisCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            Self::Reset => vec![0x81, 0x01, 0x04, 0x0B, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x0B, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x0B, 0x03, 0xFF],
            Self::Direct(level) => {
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

#[allow(missing_docs)] // TODO: Add documentation for shutter control
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
            Self::Reset => vec![0x81, 0x01, 0x04, 0x0A, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x0A, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x0A, 0x03, 0xFF],
            Self::Direct(value) => {
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
            Self::Reset => vec![0x81, 0x01, 0x04, 0x0D, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x0D, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x0D, 0x03, 0xFF],
            Self::Direct(value) => {
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
