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
    command::{response::ResponseType, Command},
    error::Error,
    timeout::CommandCategory,
    types::{BrightnessLevel, IrisLevel, ShutterSpeed},
    visca_up_down_reset,
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
#[derive(Debug, Copy, Clone)]
pub struct ExposureCommand {
    /// The exposure mode to set.
    pub mode: ExposureMode,
}

impl Command for ExposureCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(vec![0x81, 0x01, 0x04, 0x39, self.mode as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
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
    /// Returns `Error::InvalidParameter` if value is outside -7 to +7 range.
    pub fn new(value: i8) -> Result<Self, Error> {
        if (Self::MIN..=Self::MAX).contains(&value) {
            Ok(Self(value))
        } else {
            Err(Error::InvalidParameter(format!(
                "Exposure compensation level must be between {} and {}",
                Self::MIN,
                Self::MAX
            )))
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> i8 {
        self.0
    }

    /// Convert to protocol value (0x0 to 0xE).
    #[allow(clippy::cast_sign_loss)]
    #[must_use]
    pub const fn to_protocol_value(self) -> u8 {
        (self.0 + 7) as u8
    }
}

impl TryFrom<i8> for ExposureCompensationLevel {
    type Error = Error;

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Exposure compensation commands.
///
/// # Example
/// ```no_run
/// use grafton_visca::command::{ExposureCompensationCommand, exposure::ExposureCompensationLevel};
/// use grafton_visca::Command;
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

impl Command for ExposureCompensationCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
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

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

crate::visca_bounded_param! {
    /// Dynamic range level.
    ///
    /// Valid range: 0 to 8.
    DynamicRangeLevel: u8 {
        min: 0,
        max: 8,
        error_msg: "Dynamic range level must be between 0 and 8"
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

impl Command for DynamicRangeCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
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

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Commands for controlling iris/aperture values.
///
/// Provides standard VISCA control operations:
/// - Reset to default value
/// - Increment/decrement by one step
/// - Set to a specific value
#[derive(Debug, Copy, Clone)]
pub enum IrisCommand {
    /// Reset to default value.
    Reset,
    /// Increase value by one step.
    Up,
    /// Decrease value by one step.
    Down,
    /// Set to specific value.
    Direct(IrisLevel),
}

// Manual implementation to add model validation
impl Command for IrisCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(match self {
            Self::Reset => vec![0x81, 0x01, 0x04, 0x0B, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x0B, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x0B, 0x03, 0xFF],
            Self::Direct(level) => {
                let val = level.value();
                let high = (val >> 4) & 0x0F;
                let low = val & 0x0F;
                vec![0x81, 0x01, 0x04, 0x4B, 0x00, 0x00, high, low, 0xFF]
            }
        })
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }

    fn validate_for_model(&self, model: crate::constants::CameraModel) -> Result<(), Error> {
        match self {
            Self::Direct(level) => {
                if matches!(model, crate::constants::CameraModel::PTZOpticsG2)
                    && !IrisLevel::G2_VALID_VALUES.contains(&level.value())
                {
                    return Err(Error::ModelValidation {
                        model,
                        command: "IrisLevel".to_string(),
                        reason: format!(
                            "Iris level value {:#02X} is not valid for G2. Valid values: 0x00-0x0C",
                            level.value()
                        ),
                    });
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Commands for controlling shutter speed values.
///
/// Provides standard VISCA control operations:
/// - Reset to default value
/// - Increment/decrement by one step
/// - Set to a specific value
#[derive(Debug, Copy, Clone)]
pub enum ShutterCommand {
    /// Reset to default value.
    Reset,
    /// Increase value by one step.
    Up,
    /// Decrease value by one step.
    Down,
    /// Set to specific value.
    Direct(ShutterSpeed),
}

// Manual implementation to add model validation
impl Command for ShutterCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(match self {
            Self::Reset => vec![0x81, 0x01, 0x04, 0x0A, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x0A, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x0A, 0x03, 0xFF],
            Self::Direct(value) => {
                let val = value.value();
                #[allow(clippy::cast_possible_truncation)]
                let byte_val = val as u8;
                let high = (byte_val >> 4) & 0x0F;
                let low = byte_val & 0x0F;
                vec![0x81, 0x01, 0x04, 0x4A, 0x00, 0x00, high, low, 0xFF]
            }
        })
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }

    fn validate_for_model(&self, model: crate::constants::CameraModel) -> Result<(), Error> {
        match self {
            Self::Direct(speed) => {
                if matches!(model, crate::constants::CameraModel::PTZOpticsG2)
                    && !ShutterSpeed::G2_VALID_VALUES.contains(&speed.value())
                {
                    return Err(Error::ModelValidation {
                        model,
                        command: "ShutterSpeed".to_string(),
                        reason: format!(
                            "Shutter speed value {:#04X} is not valid for G2. Valid values: 0x01-0x11",
                            speed.value()
                        ),
                    });
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

visca_up_down_reset! {
    #[category = "Quick"]
    enum BrightCommand {
        command_byte: 0x0D,
        Direct(value: BrightnessLevel) => |high, low| [0x81, 0x01, 0x04, 0x4D, 0x00, 0x00, high, low, 0xFF]
    }
}
