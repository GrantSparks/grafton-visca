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
    constants::CameraModel,
    error::Error,
    timeout::CommandCategory,
    types::{BrightnessLevel, IrisLevel, ShutterSpeed},
};

/// Camera exposure control modes.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
/// let set_value = ExposureCompensationCommand::SetLevel(ExposureCompensationLevel::new(3).unwrap());
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
    /// Set exposure compensation to a specific level (-7 to +7)
    SetLevel(ExposureCompensationLevel),
}

impl Command for ExposureCompensationCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(match self {
            Self::On => vec![0x81, 0x01, 0x04, 0x3E, 0x02, 0xFF],
            Self::Off => vec![0x81, 0x01, 0x04, 0x3E, 0x03, 0xFF],
            Self::Reset => vec![0x81, 0x01, 0x04, 0x0E, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x0E, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x0E, 0x03, 0xFF],
            Self::SetLevel(level) => {
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

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        match (self, model) {
            (Self::SetLevel(level), CameraModel::PTZOpticsG2) => {
                // G2 supports values -7 to +7 (protocol values 0x0 to 0xE)
                let value = level.value();
                if !(-7..=7).contains(&value) {
                    return Err(Error::ModelValidation {
                        model,
                        command: "ExposureCompensationDirect".to_string(),
                        reason: format!(
                            "Value {value} not supported on G2 cameras (range is -7 to +7)"
                        ),
                    });
                }
                Ok(())
            }
            _ => Ok(()),
        }
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
    SetLevel(DynamicRangeLevel),
}

impl Command for DynamicRangeCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        match self {
            Self::SetLevel(level) => Ok(vec![
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

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        match (self, model) {
            (Self::SetLevel(level), CameraModel::PTZOpticsG2) => {
                // G2 supports values 0-8
                let value = level.value();
                if value > 8 {
                    return Err(Error::ModelValidation {
                        model,
                        command: "DynamicRangeDirect".to_string(),
                        reason: format!("Value {value} not supported on G2 cameras (max is 8)"),
                    });
                }
                Ok(())
            }
            _ => Ok(()),
        }
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
    /// Set iris to specific aperture value.
    SetAperture(IrisLevel),
}

// Manual implementation to add model validation
impl Command for IrisCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(match self {
            Self::Reset => vec![0x81, 0x01, 0x04, 0x0B, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x0B, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x0B, 0x03, 0xFF],
            Self::SetAperture(level) => {
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

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        match self {
            Self::SetAperture(level) => {
                level
                    .validate_for_model(model)
                    .map_err(|_| Error::ModelValidation {
                        model,
                        command: "IrisLevel".to_string(),
                        reason: format!(
                            "Iris level value {:#02X} is not valid for {}",
                            level.value(),
                            match model {
                                CameraModel::PTZOpticsG2 => "G2 (valid values: 0x00-0x0C)",
                                _ => "this camera model",
                            }
                        ),
                    })
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
    /// Set shutter to specific speed value.
    SetSpeed(ShutterSpeed),
}

// Manual implementation to add model validation
impl Command for ShutterCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(match self {
            Self::Reset => vec![0x81, 0x01, 0x04, 0x0A, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x0A, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x0A, 0x03, 0xFF],
            Self::SetSpeed(value) => {
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

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        match self {
            Self::SetSpeed(speed) => {
                speed
                    .validate_for_model(model)
                    .map_err(|_| Error::ModelValidation {
                        model,
                        command: "ShutterSpeed".to_string(),
                        reason: format!(
                            "Shutter speed value {:#04X} is not valid for {}",
                            speed.value(),
                            match model {
                                CameraModel::PTZOpticsG2 => "G2 (valid values: 0x01-0x11)",
                                _ => "this camera model",
                            }
                        ),
                    })
            }
            _ => Ok(()),
        }
    }
}

/// Brightness control command.
#[derive(Debug, Clone, Copy)]
pub enum BrightCommand {
    /// Reset brightness to default.
    Reset,
    /// Increase brightness.
    Up,
    /// Decrease brightness.
    Down,
    /// Set brightness to specific level.
    SetLevel(BrightnessLevel),
}

impl Command for BrightCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(match self {
            Self::Reset => vec![0x81, 0x01, 0x04, 0x0D, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x0D, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x0D, 0x03, 0xFF],
            Self::SetLevel(level) => {
                let value = level.value();
                let high = ((value >> 4) & 0x0F) as u8;
                let low = (value & 0x0F) as u8;
                vec![0x81, 0x01, 0x04, 0x4D, 0x00, 0x00, high, low, 0xFF]
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
            Self::SetLevel(level) => {
                level
                    .validate_for_model(model)
                    .map_err(|_| Error::ModelValidation {
                        model,
                        command: "BrightDirect".to_string(),
                        reason: format!(
                            "Brightness value {:#04X} is not valid for {}",
                            level.value(),
                            match model {
                                CameraModel::PTZOpticsG2 => "G2 (valid values: 0x00-0x11)",
                                _ => "this camera model",
                            }
                        ),
                    })
            }
            _ => Ok(()),
        }
    }
}
#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_exposure_mode_command() {
        // Test Auto mode
        let cmd = ExposureCommand {
            mode: ExposureMode::Auto,
        };
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x39, 0x00, 0xFF]
        );

        // Test Manual mode
        let cmd = ExposureCommand {
            mode: ExposureMode::Manual,
        };
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x39, 0x03, 0xFF]
        );

        // Test Shutter Priority mode
        let cmd = ExposureCommand {
            mode: ExposureMode::Shutter,
        };
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x39, 0x0A, 0xFF]
        );

        // Test Iris Priority mode
        let cmd = ExposureCommand {
            mode: ExposureMode::Iris,
        };
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x39, 0x0B, 0xFF]
        );

        // Test Brightness Priority mode
        let cmd = ExposureCommand {
            mode: ExposureMode::Bright,
        };
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x39, 0x0D, 0xFF]
        );
    }

    #[test]
    fn test_exposure_mode_try_from() {
        assert!(matches!(
            ExposureMode::try_from(0x00),
            Ok(ExposureMode::Auto)
        ));
        assert!(matches!(
            ExposureMode::try_from(0x03),
            Ok(ExposureMode::Manual)
        ));
        assert!(matches!(
            ExposureMode::try_from(0x0A),
            Ok(ExposureMode::Shutter)
        ));
        assert!(matches!(
            ExposureMode::try_from(0x0B),
            Ok(ExposureMode::Iris)
        ));
        assert!(matches!(
            ExposureMode::try_from(0x0D),
            Ok(ExposureMode::Bright)
        ));
        assert!(ExposureMode::try_from(0xFF).is_err());
    }

    #[test]
    fn test_exposure_compensation_level() {
        // Test valid values
        for value in -7..=7 {
            let level = ExposureCompensationLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            assert_eq!(level.value(), value);
            assert_eq!(
                level.to_protocol_value(),
                u8::try_from(value + 7).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
            );
        }

        // Test invalid values
        assert!(ExposureCompensationLevel::new(-8).is_err());
        assert!(ExposureCompensationLevel::new(8).is_err());
    }

    #[test]
    fn test_exposure_compensation_commands() {
        // Test On command
        let cmd = ExposureCompensationCommand::On;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x3E, 0x02, 0xFF]
        );

        // Test Off command
        let cmd = ExposureCompensationCommand::Off;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x3E, 0x03, 0xFF]
        );

        // Test Reset command
        let cmd = ExposureCompensationCommand::Reset;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0E, 0x00, 0xFF]
        );

        // Test Up command
        let cmd = ExposureCompensationCommand::Up;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0E, 0x02, 0xFF]
        );

        // Test Down command
        let cmd = ExposureCompensationCommand::Down;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0E, 0x03, 0xFF]
        );

        // Test Direct command with various values
        for value in -7..=7 {
            let level = ExposureCompensationLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = ExposureCompensationCommand::SetLevel(level);
            let expected = vec![
                0x81,
                0x01,
                0x04,
                0x4E,
                0x00,
                0x00,
                0x00,
                u8::try_from(value + 7).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
                0xFF,
            ];
            assert_eq!(
                cmd.to_bytes()
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
                expected
            );
        }
    }

    #[test]
    fn test_exposure_compensation_g2_validation() {
        // Test valid G2 values
        for value in -7..=7 {
            let level = ExposureCompensationLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = ExposureCompensationCommand::SetLevel(level);
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Other command types should always be valid
        assert!(ExposureCompensationCommand::On
            .validate_for_model(CameraModel::PTZOpticsG2)
            .is_ok());
        assert!(ExposureCompensationCommand::Off
            .validate_for_model(CameraModel::PTZOpticsG2)
            .is_ok());
    }

    #[test]
    fn test_dynamic_range_level() {
        // Test valid values
        for value in 0..=8 {
            let level = DynamicRangeLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            assert_eq!(level.value(), value);
        }

        // Test invalid values
        assert!(DynamicRangeLevel::new(9).is_err());
    }

    #[test]
    fn test_dynamic_range_command() {
        // Test all valid dynamic range levels
        for value in 0..=8 {
            let level = DynamicRangeLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = DynamicRangeCommand::SetLevel(level);
            assert_eq!(
                cmd.to_bytes()
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
                vec![0x81, 0x01, 0x04, 0x25, 0x00, 0x00, 0x00, value, 0xFF]
            );
        }
    }

    #[test]
    fn test_dynamic_range_g2_validation() {
        // Test valid G2 values
        for value in 0..=8 {
            let level = DynamicRangeLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = DynamicRangeCommand::SetLevel(level);
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }
    }

    #[test]
    fn test_iris_commands() {
        // Test Reset command
        let cmd = IrisCommand::Reset;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0B, 0x00, 0xFF]
        );

        // Test Up command
        let cmd = IrisCommand::Up;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0B, 0x02, 0xFF]
        );

        // Test Down command
        let cmd = IrisCommand::Down;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0B, 0x03, 0xFF]
        );

        // Test Direct command with valid values
        let test_values = vec![0x00, 0x05, 0x0A, 0x0C];
        for value in test_values {
            let level =
                IrisLevel::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = IrisCommand::SetAperture(level);
            let high = (value >> 4) & 0x0F;
            let low = value & 0x0F;
            assert_eq!(
                cmd.to_bytes()
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
                vec![0x81, 0x01, 0x04, 0x4B, 0x00, 0x00, high, low, 0xFF]
            );
        }
    }

    #[test]
    fn test_iris_g2_validation() {
        // Test valid G2 iris values
        for value in 0x00..=0x0C {
            let level =
                IrisLevel::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = IrisCommand::SetAperture(level);
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Test that non-G2 valid value still passes validation (G2 is more restrictive)
        // The general IrisLevel accepts values up to 0x0C, which are all valid for G2

        // Non-direct commands should always be valid
        assert!(IrisCommand::Reset
            .validate_for_model(CameraModel::PTZOpticsG2)
            .is_ok());
    }

    #[test]
    fn test_shutter_commands() {
        // Test Reset command
        let cmd = ShutterCommand::Reset;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0A, 0x00, 0xFF]
        );

        // Test Up command
        let cmd = ShutterCommand::Up;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0A, 0x02, 0xFF]
        );

        // Test Down command
        let cmd = ShutterCommand::Down;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0A, 0x03, 0xFF]
        );

        // Test Direct command with valid values
        let test_values = vec![0x01u16, 0x05, 0x0A, 0x10, 0x11];
        for value in test_values {
            let speed =
                ShutterSpeed::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = ShutterCommand::SetSpeed(speed);
            let high = ((value >> 4) & 0x0F) as u8;
            let low = (value & 0x0F) as u8;
            assert_eq!(
                cmd.to_bytes()
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
                vec![0x81, 0x01, 0x04, 0x4A, 0x00, 0x00, high, low, 0xFF]
            );
        }
    }

    #[test]
    fn test_shutter_g2_validation() {
        // Test valid G2 shutter values
        for value in 0x01..=0x11 {
            let speed =
                ShutterSpeed::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = ShutterCommand::SetSpeed(speed);
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Test that non-G2 valid value still passes validation (G2 is more restrictive)
        // The general ShutterSpeed accepts values up to 0x11, which are all valid for G2

        // Non-direct commands should always be valid
        assert!(ShutterCommand::Reset
            .validate_for_model(CameraModel::PTZOpticsG2)
            .is_ok());
    }

    #[test]
    fn test_bright_commands() {
        // Test Reset command
        let cmd = BrightCommand::Reset;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0D, 0x00, 0xFF]
        );

        // Test Up command
        let cmd = BrightCommand::Up;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0D, 0x02, 0xFF]
        );

        // Test Down command
        let cmd = BrightCommand::Down;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0D, 0x03, 0xFF]
        );

        // Test Direct command with valid values
        let test_values = vec![0x00u16, 0x08, 0x0F, 0x10, 0x11];
        for value in test_values {
            let level = BrightnessLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = BrightCommand::SetLevel(level);
            let high = ((value >> 4) & 0x0F) as u8;
            let low = (value & 0x0F) as u8;
            assert_eq!(
                cmd.to_bytes()
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
                vec![0x81, 0x01, 0x04, 0x4D, 0x00, 0x00, high, low, 0xFF]
            );
        }
    }

    #[test]
    fn test_bright_g2_validation() {
        // Test valid G2 brightness values
        for value in 0x00..=0x11 {
            let level = BrightnessLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = BrightCommand::SetLevel(level);
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Test that non-G2 valid value still passes validation (G2 is more restrictive)
        // The general BrightnessLevel accepts values up to 0x11, which are all valid for G2

        // Non-direct commands should always be valid
        assert!(BrightCommand::Reset
            .validate_for_model(CameraModel::PTZOpticsG2)
            .is_ok());
    }

    #[test]
    fn test_command_categories() {
        // All exposure commands should be Quick category
        assert_eq!(
            ExposureCommand {
                mode: ExposureMode::Auto
            }
            .command_category(),
            CommandCategory::Quick
        );
        assert_eq!(
            ExposureCompensationCommand::On.command_category(),
            CommandCategory::Quick
        );
        assert_eq!(
            DynamicRangeCommand::SetLevel(
                DynamicRangeLevel::new(5)
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
            )
            .command_category(),
            CommandCategory::Quick
        );
        assert_eq!(
            IrisCommand::Reset.command_category(),
            CommandCategory::Quick
        );
        assert_eq!(
            ShutterCommand::Reset.command_category(),
            CommandCategory::Quick
        );
        assert_eq!(
            BrightCommand::Reset.command_category(),
            CommandCategory::Quick
        );
    }

    #[test]
    fn test_response_types() {
        // All exposure commands should return None for response_type
        assert!(ExposureCommand {
            mode: ExposureMode::Auto
        }
        .response_type()
        .is_none());
        assert!(ExposureCompensationCommand::On.response_type().is_none());
        assert!(DynamicRangeCommand::SetLevel(
            DynamicRangeLevel::new(5).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        )
        .response_type()
        .is_none());
        assert!(IrisCommand::Reset.response_type().is_none());
        assert!(ShutterCommand::Reset.response_type().is_none());
        assert!(BrightCommand::Reset.response_type().is_none());
    }
}
