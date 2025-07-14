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
    command::{encode_visca::EncodeVisca, response::ResponseType},
    error::Error,
    timeout::CommandCategory,
    types::{BrightnessLevel, DynamicRangeLevel, IrisLevel, ShutterSpeed},
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

impl TryFrom<u8> for ExposureMode {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(ExposureMode::Auto),
            0x03 => Ok(ExposureMode::Manual),
            0x0A => Ok(ExposureMode::Shutter),
            0x0B => Ok(ExposureMode::Iris),
            0x0D => Ok(ExposureMode::Bright),
            _ => Err(Error::InvalidResponse {
                expected:
                    "0x00 (Auto), 0x03 (Manual), 0x0A (Shutter), 0x0B (Iris), or 0x0D (Bright)"
                        .to_string(),
                actual: vec![value],
            }),
        }
    }
}

crate::visca_param_command! {
    /// Command to set the camera's exposure mode.
    ///
    /// This command allows switching between different exposure modes such as
    /// auto, manual, shutter priority, iris priority, or brightness priority.
    pub(crate) struct ExposureCommand {
        mode: ExposureMode,
    }
    prefix = [0x81, 0x01, 0x04, 0x39];
    param_byte = *mode as u8;
    timeout = Quick;
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
            Err(Error::InvalidParameter {
                parameter: "value",
                value: value.to_string(),
                reason: format!(
                    "Exposure compensation level must be between {} and {}",
                    Self::MIN,
                    Self::MAX
                ),
            })
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
/// use grafton_visca::command::{ExposureCompensation, exposure::ExposureCompensationLevel};
/// use grafton_visca::EncodeVisca;
///
/// // Enable exposure compensation
/// let enable = ExposureCompensation::On;
///
/// // Set exposure compensation to +3
/// let set_value = ExposureCompensation::SetLevel(ExposureCompensationLevel::new(3).unwrap());
/// ```
#[derive(Debug, Copy, Clone)]
pub enum ExposureCompensation {
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

impl EncodeVisca for ExposureCompensation {
    type Response = ();
    const MAX_SIZE: usize = 9;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        match self {
            Self::On | Self::Off => {
                if buffer.len() < 6 {
                    return Err(Error::BufferTooSmall {
                        required: 6,
                        actual: buffer.len(),
                    });
                }
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x3E;
                buffer[4] = match self {
                    Self::On => 0x02,
                    Self::Off => 0x03,
                    _ => unreachable!(),
                };
                buffer[5] = 0xFF;
                Ok(6)
            }
            Self::Reset | Self::Up | Self::Down => {
                if buffer.len() < 6 {
                    return Err(Error::BufferTooSmall {
                        required: 6,
                        actual: buffer.len(),
                    });
                }
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x0E;
                buffer[4] = match self {
                    Self::Reset => 0x00,
                    Self::Up => 0x02,
                    Self::Down => 0x03,
                    _ => unreachable!(),
                };
                buffer[5] = 0xFF;
                Ok(6)
            }
            Self::SetLevel(level) => {
                if buffer.len() < Self::MAX_SIZE {
                    return Err(Error::BufferTooSmall {
                        required: Self::MAX_SIZE,
                        actual: buffer.len(),
                    });
                }
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x4E;
                buffer[4] = 0x00;
                buffer[5] = 0x00;
                buffer[6] = 0x00;
                buffer[7] = level.to_protocol_value();
                buffer[8] = 0xFF;
                Ok(Self::MAX_SIZE)
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Commands for controlling the camera's dynamic range.
///
/// Dynamic range control adjusts the camera's ability to capture detail
/// in both bright and dark areas of a scene simultaneously. Higher values
/// increase the dynamic range, allowing better detail retention in scenes
/// with high contrast.
#[derive(Debug, Copy, Clone)]
pub enum DynamicRange {
    /// Set dynamic range to a specific level (0-8).
    SetLevel(DynamicRangeLevel),
}

impl EncodeVisca for DynamicRange {
    type Response = ();
    const MAX_SIZE: usize = 9;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        let level = match self {
            Self::SetLevel(level) => level,
        };

        buffer[0] = 0x81;
        buffer[1] = 0x01;
        buffer[2] = 0x04;
        buffer[3] = 0x25;
        buffer[4] = 0x00;
        buffer[5] = 0x00;
        buffer[6] = 0x00;
        buffer[7] = level.value();
        buffer[8] = 0xFF;

        Ok(Self::MAX_SIZE)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
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
pub enum Iris {
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
impl EncodeVisca for Iris {
    type Response = ();
    const MAX_SIZE: usize = 9;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        match self {
            Self::Reset | Self::Up | Self::Down => {
                if buffer.len() < 6 {
                    return Err(Error::BufferTooSmall {
                        required: 6,
                        actual: buffer.len(),
                    });
                }
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x0B;
                buffer[4] = match self {
                    Self::Reset => 0x00,
                    Self::Up => 0x02,
                    Self::Down => 0x03,
                    _ => unreachable!(),
                };
                buffer[5] = 0xFF;
                Ok(6)
            }
            Self::SetAperture(level) => {
                if buffer.len() < Self::MAX_SIZE {
                    return Err(Error::BufferTooSmall {
                        required: Self::MAX_SIZE,
                        actual: buffer.len(),
                    });
                }
                let value = level.value();
                let high = (value >> 4) & 0x0F;
                let low = value & 0x0F;

                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x4B;
                buffer[4] = 0x00;
                buffer[5] = 0x00;
                buffer[6] = high;
                buffer[7] = low;
                buffer[8] = 0xFF;
                Ok(Self::MAX_SIZE)
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Commands for controlling shutter speed values.
///
/// Provides standard VISCA control operations:
/// - Reset to default value
/// - Increment/decrement by one step
/// - Set to a specific value
#[derive(Debug, Copy, Clone)]
pub enum Shutter {
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
impl EncodeVisca for Shutter {
    type Response = ();
    const MAX_SIZE: usize = 9;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        match self {
            Self::Reset | Self::Up | Self::Down => {
                if buffer.len() < 6 {
                    return Err(Error::BufferTooSmall {
                        required: 6,
                        actual: buffer.len(),
                    });
                }
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x0A;
                buffer[4] = match self {
                    Self::Reset => 0x00,
                    Self::Up => 0x02,
                    Self::Down => 0x03,
                    _ => unreachable!(),
                };
                buffer[5] = 0xFF;
                Ok(6)
            }
            Self::SetSpeed(speed) => {
                if buffer.len() < Self::MAX_SIZE {
                    return Err(Error::BufferTooSmall {
                        required: Self::MAX_SIZE,
                        actual: buffer.len(),
                    });
                }
                let value = speed.value();
                let high = ((value >> 4) & 0x0F) as u8;
                let low = (value & 0x0F) as u8;

                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x4A;
                buffer[4] = 0x00;
                buffer[5] = 0x00;
                buffer[6] = high;
                buffer[7] = low;
                buffer[8] = 0xFF;
                Ok(Self::MAX_SIZE)
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Brightness control command.
#[derive(Debug, Clone, Copy)]
pub enum Bright {
    /// Reset brightness to default.
    Reset,
    /// Increase brightness.
    Up,
    /// Decrease brightness.
    Down,
    /// Set brightness to specific level.
    SetLevel(BrightnessLevel),
}

impl EncodeVisca for Bright {
    type Response = ();
    const MAX_SIZE: usize = 9;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        match self {
            Self::Reset | Self::Up | Self::Down => {
                if buffer.len() < 6 {
                    return Err(Error::BufferTooSmall {
                        required: 6,
                        actual: buffer.len(),
                    });
                }
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x0D;
                buffer[4] = match self {
                    Self::Reset => 0x00,
                    Self::Up => 0x02,
                    Self::Down => 0x03,
                    _ => unreachable!(),
                };
                buffer[5] = 0xFF;
                Ok(6)
            }
            Self::SetLevel(level) => {
                if buffer.len() < Self::MAX_SIZE {
                    return Err(Error::BufferTooSmall {
                        required: Self::MAX_SIZE,
                        actual: buffer.len(),
                    });
                }
                let value = level.value();
                let high = ((value >> 4) & 0x0F) as u8;
                let low = (value & 0x0F) as u8;

                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x4D;
                buffer[4] = 0x00;
                buffer[5] = 0x00;
                buffer[6] = high;
                buffer[7] = low;
                buffer[8] = 0xFF;
                Ok(Self::MAX_SIZE)
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

visca_command! {
    /// Spotlight command (Sony models).
    ///
    /// Controls the spotlight feature which enhances exposure for specific subjects.
    category = "Quick",
    enum Spotlight {
        /// Turn spotlight on
        On => {
            let cmd = crate::command::const_encoding::CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::exposure::SPOTLIGHT_PREFIX)
                .append(&[0x02])
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Turn spotlight off
        Off => {
            let cmd = crate::command::const_encoding::CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::exposure::SPOTLIGHT_PREFIX)
                .append(&[0x03])
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
    }
}

use crate::visca_command;

visca_command! {
    /// Auto Slow Shutter command.
    ///
    /// Controls whether the camera can use slower shutter speeds automatically
    /// in low light conditions.
    category = "Quick",
    enum AutoSlowShutter {
        /// Enable auto slow shutter
        On => {
            let cmd = crate::command::const_encoding::CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::exposure::AUTO_SLOW_SHUTTER_PREFIX)
                .append(&[0x02])
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Disable auto slow shutter
        Off => {
            let cmd = crate::command::const_encoding::CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::exposure::AUTO_SLOW_SHUTTER_PREFIX)
                .append(&[0x03])
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use crate::{constants::CameraModel, EncodeVisca};

    #[test]
    fn test_exposure_mode_command() {
        // Test Auto mode
        let cmd = ExposureCommand {
            mode: ExposureMode::Auto,
        };
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x39, 0x00, 0xFF]
        );

        // Test Manual mode
        let cmd = ExposureCommand {
            mode: ExposureMode::Manual,
        };
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x39, 0x03, 0xFF]
        );

        // Test Shutter Priority mode
        let cmd = ExposureCommand {
            mode: ExposureMode::Shutter,
        };
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x39, 0x0A, 0xFF]
        );

        // Test Iris Priority mode
        let cmd = ExposureCommand {
            mode: ExposureMode::Iris,
        };
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x39, 0x0B, 0xFF]
        );

        // Test Brightness Priority mode
        let cmd = ExposureCommand {
            mode: ExposureMode::Bright,
        };
        assert_eq!(
            cmd.try_into_vec()
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
        let cmd = ExposureCompensation::On;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x3E, 0x02, 0xFF]
        );

        // Test Off command
        let cmd = ExposureCompensation::Off;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x3E, 0x03, 0xFF]
        );

        // Test Reset command
        let cmd = ExposureCompensation::Reset;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0E, 0x00, 0xFF]
        );

        // Test Up command
        let cmd = ExposureCompensation::Up;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0E, 0x02, 0xFF]
        );

        // Test Down command
        let cmd = ExposureCompensation::Down;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0E, 0x03, 0xFF]
        );

        // Test SetLevel command with various values
        for value in -7..=7 {
            let level = ExposureCompensationLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = ExposureCompensation::SetLevel(level);
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
                cmd.try_into_vec()
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
            let cmd = ExposureCompensation::SetLevel(level);
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Other command types should always be valid
        assert!(ExposureCompensation::On
            .validate_for_model(CameraModel::PTZOpticsG2)
            .is_ok());
        assert!(ExposureCompensation::Off
            .validate_for_model(CameraModel::PTZOpticsG2)
            .is_ok());
    }

    #[test]
    fn test_dynamic_range_command() {
        // Test all valid dynamic range levels
        for value in 0..=8 {
            let level = DynamicRangeLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = DynamicRange::SetLevel(level);
            assert_eq!(
                cmd.try_into_vec()
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
            let cmd = DynamicRange::SetLevel(level);
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }
    }

    #[test]
    fn test_iris_commands() {
        // Test Reset command
        let cmd = Iris::Reset;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0B, 0x00, 0xFF]
        );

        // Test Up command
        let cmd = Iris::Up;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0B, 0x02, 0xFF]
        );

        // Test Down command
        let cmd = Iris::Down;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0B, 0x03, 0xFF]
        );

        // Test SetAperture command with valid values
        let test_values = vec![0x00, 0x05, 0x0A, 0x0C];
        for value in test_values {
            let level =
                IrisLevel::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = Iris::SetAperture(level);
            let high = (value >> 4) & 0x0F;
            let low = value & 0x0F;
            assert_eq!(
                cmd.try_into_vec()
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
            let cmd = Iris::SetAperture(level);
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Test that non-G2 valid value still passes validation (G2 is more restrictive)
        // The general IrisLevel accepts values up to 0x0C, which are all valid for G2

        // Non-direct commands should always be valid
        assert!(Iris::Reset
            .validate_for_model(CameraModel::PTZOpticsG2)
            .is_ok());
    }

    #[test]
    fn test_shutter_commands() {
        // Test Reset command
        let cmd = Shutter::Reset;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0A, 0x00, 0xFF]
        );

        // Test Up command
        let cmd = Shutter::Up;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0A, 0x02, 0xFF]
        );

        // Test Down command
        let cmd = Shutter::Down;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0A, 0x03, 0xFF]
        );

        // Test SetSpeed command with valid values
        let test_values = vec![0x01u16, 0x05, 0x0A, 0x10, 0x11];
        for value in test_values {
            let speed =
                ShutterSpeed::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = Shutter::SetSpeed(speed);
            let high = ((value >> 4) & 0x0F) as u8;
            let low = (value & 0x0F) as u8;
            assert_eq!(
                cmd.try_into_vec()
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
            let cmd = Shutter::SetSpeed(speed);
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Test that non-G2 valid value still passes validation (G2 is more restrictive)
        // The general ShutterSpeed accepts values up to 0x11, which are all valid for G2

        // Non-direct commands should always be valid
        assert!(Shutter::Reset
            .validate_for_model(CameraModel::PTZOpticsG2)
            .is_ok());
    }

    #[test]
    fn test_bright_commands() {
        // Test Reset command
        let cmd = Bright::Reset;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0D, 0x00, 0xFF]
        );

        // Test Up command
        let cmd = Bright::Up;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0D, 0x02, 0xFF]
        );

        // Test Down command
        let cmd = Bright::Down;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0D, 0x03, 0xFF]
        );

        // Test SetLevel command with valid values
        let test_values = vec![0x00u16, 0x08, 0x0F, 0x10, 0x11];
        for value in test_values {
            let level = BrightnessLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = Bright::SetLevel(level);
            let high = ((value >> 4) & 0x0F) as u8;
            let low = (value & 0x0F) as u8;
            assert_eq!(
                cmd.try_into_vec()
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
            let cmd = Bright::SetLevel(level);
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Test that non-G2 valid value still passes validation (G2 is more restrictive)
        // The general BrightnessLevel accepts values up to 0x11, which are all valid for G2

        // Non-direct commands should always be valid
        assert!(Bright::Reset
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
            .timeout_kind(),
            CommandCategory::Quick
        );
        assert_eq!(
            ExposureCompensation::On.timeout_kind(),
            CommandCategory::Quick
        );
        assert_eq!(
            DynamicRange::SetLevel(
                DynamicRangeLevel::new(5)
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
            )
            .timeout_kind(),
            CommandCategory::Quick
        );
        assert_eq!(Iris::Reset.timeout_kind(), CommandCategory::Quick);
        assert_eq!(Shutter::Reset.timeout_kind(), CommandCategory::Quick);
        assert_eq!(Bright::Reset.timeout_kind(), CommandCategory::Quick);
    }

    #[test]
    fn test_response_types() {
        // All exposure commands should return None for response_type
        assert!(ExposureCommand {
            mode: ExposureMode::Auto
        }
        .response_type()
        .is_none());
        assert!(ExposureCompensation::On.response_type().is_none());
        assert!(DynamicRange::SetLevel(
            DynamicRangeLevel::new(5).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        )
        .response_type()
        .is_none());
        assert!(Iris::Reset.response_type().is_none());
        assert!(Shutter::Reset.response_type().is_none());
        assert!(Bright::Reset.response_type().is_none());
    }
}
