//! Gain control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera gain settings,
//! including manual gain adjustment, gain limit control, and anti-flicker settings.

// Crate imports
use crate::{
    command::{const_encoding::CommandBuilder, encode_visca::EncodeVisca, response::ResponseType},
    error::Error,
    timeout::CommandCategory,
    types::{GainLevel, GainLimit},
    visca_command,
};

/// Commands for controlling gain values.
///
/// Provides standard VISCA control operations:
/// - Reset to default value
/// - Increment/decrement by one step
/// - Set to a specific value
#[derive(Debug, Copy, Clone)]
pub enum Gain {
    /// Reset to default value.
    Reset,
    /// Increase value by one step.
    Up,
    /// Decrease value by one step.
    Down,
    /// Set gain to specific value.
    SetValue(GainLevel),
}

impl Gain {
    // Legacy method - removed in new API
    // pub fn direct<P: crate::camera::CameraProfile>(gain: P::Gain) -> Result<Self, Error> {
    //     ...
    // }
}

// Manual implementation to add model validation
impl EncodeVisca for Gain {
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
                buffer[3] = 0x0C;
                buffer[4] = match self {
                    Self::Reset => 0x00,
                    Self::Up => 0x02,
                    Self::Down => 0x03,
                    _ => unreachable!(),
                };
                buffer[5] = 0xFF;
                Ok(6)
            }
            Self::SetValue(level) => {
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
                buffer[3] = 0x4C;
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

crate::visca_builder! {
    /// Command to set the automatic gain control limit.
    pub(crate) struct GainLimitCommand {
        /// The gain limit to set.
        limit: GainLimit,
    }
    builder<6> => |builder, limit| {
        let _ = builder.append(crate::command::const_encoding::constants::gain::GAIN_LIMIT_PREFIX);
        let _ = builder.push(limit.value());
    }
    timeout = Quick;
}

impl GainLimitCommand {
    /// Create a new gain limit command.
    pub fn new(limit: GainLimit) -> Self {
        Self { limit }
    }
}

/// Anti-flicker mode settings.
///
/// Reduces flicker caused by artificial lighting that operates at
/// different frequencies than the camera's frame rate.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AntiFlickerMode {
    /// Disable anti-flicker processing.
    Off = 0x00,
    /// Enable 50Hz anti-flicker (for regions with 50Hz AC power).
    Hz50 = 0x01,
    /// Enable 60Hz anti-flicker (for regions with 60Hz AC power).
    Hz60 = 0x02,
}

impl TryFrom<u8> for AntiFlickerMode {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(AntiFlickerMode::Off),
            0x01 => Ok(AntiFlickerMode::Hz50),
            0x02 => Ok(AntiFlickerMode::Hz60),
            _ => Err(Error::InvalidResponse {
                expected: "0x00 (Off), 0x01 (50Hz), or 0x02 (60Hz)".to_string(),
                actual: vec![value],
            }),
        }
    }
}

visca_command! {
    /// Command to set anti-flicker mode.
    category = "Quick",
    enum AntiFlickerCommand {
        /// Disable anti-flicker processing.
        Off => {
            let cmd = CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::gain::ANTI_FLICKER_PREFIX)
                .push(0x00)
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Enable 50Hz anti-flicker (for regions with 50Hz AC power).
        Hz50 => {
            let cmd = CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::gain::ANTI_FLICKER_PREFIX)
                .push(0x01)
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Enable 60Hz anti-flicker (for regions with 60Hz AC power).
        Hz60 => {
            let cmd = CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::gain::ANTI_FLICKER_PREFIX)
                .push(0x02)
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
    }
}

impl AntiFlickerCommand {
    /// Create a new anti-flicker command.
    pub fn new(mode: AntiFlickerMode) -> Self {
        match mode {
            AntiFlickerMode::Off => Self::Off,
            AntiFlickerMode::Hz50 => Self::Hz50,
            AntiFlickerMode::Hz60 => Self::Hz60,
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use crate::{constants::CameraModel, EncodeVisca, visca_test};

    visca_test!(Gain, test_gain_command_reset, Gain::Reset, &[0x81, 0x01, 0x04, 0x0C, 0x00, 0xFF]);

    visca_test!(Gain, test_gain_command_up, Gain::Up, &[0x81, 0x01, 0x04, 0x0C, 0x02, 0xFF]);

    visca_test!(Gain, test_gain_command_down, Gain::Down, &[0x81, 0x01, 0x04, 0x0C, 0x03, 0xFF]);

    #[test]
    fn test_gain_command_set_value() {
        // Test various gain values
        let test_values = vec![0x00, 0x01, 0x03, 0x05, 0x07];
        for value in test_values {
            let gain =
                GainLevel::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = Gain::SetValue(gain);
            let high = (value >> 4) & 0x0F;
            let low = value & 0x0F;
            assert_eq!(
                cmd.try_into_vec()
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
                vec![0x81, 0x01, 0x04, 0x4C, 0x00, 0x00, high, low, 0xFF]
            );
        }
    }

    #[test]
    fn test_gain_command_g2_validation() {
        // Test valid G2 gain values (0x00-0x07)
        for value in 0x00..=0x07 {
            let gain =
                GainLevel::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = Gain::SetValue(gain);
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Gain itself is limited to 0x00-0x07, which are all valid for G2
        // So all valid Gain instances should pass G2 validation

        // Non-direct commands should always be valid
        assert!(Gain::Reset
            .validate_for_model(CameraModel::PTZOpticsG2)
            .is_ok());
        assert!(Gain::Up
            .validate_for_model(CameraModel::PTZOpticsG2)
            .is_ok());
        assert!(Gain::Down
            .validate_for_model(CameraModel::PTZOpticsG2)
            .is_ok());
    }

    #[test]
    fn test_gain_limit_command() {
        // Test various gain limit values
        let test_values = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
        for value in test_values {
            let limit =
                GainLimit::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = GainLimitCommand::new(limit);
            assert_eq!(
                cmd.try_into_vec()
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
                vec![0x81, 0x01, 0x04, 0x2C, value, 0xFF]
            );
        }
    }

    #[test]
    fn test_gain_limit_g2_validation() {
        // Test valid G2 gain limit values
        for value in GainLimit::G2_VALID_VALUES {
            let limit =
                GainLimit::new(*value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = GainLimitCommand::new(limit);
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Test that non-G2 values might fail (depends on what G2_VALID_VALUES contains)
        // Check if value 0x08 is not in G2_VALID_VALUES
        if !GainLimit::G2_VALID_VALUES.contains(&0x08) {
            if let Ok(limit) = GainLimit::new(0x08) {
                let cmd = GainLimitCommand::new(limit);
                assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_err());
            }
        }
    }

    #[test]
    fn test_anti_flicker_mode_values() {
        assert_eq!(AntiFlickerMode::Off as u8, 0x00);
        assert_eq!(AntiFlickerMode::Hz50 as u8, 0x01);
        assert_eq!(AntiFlickerMode::Hz60 as u8, 0x02);
    }

    #[test]
    fn test_anti_flicker_command() {
        // Test Off mode
        let cmd = AntiFlickerCommand::new(AntiFlickerMode::Off);
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x23, 0x00, 0xFF]
        );

        // Test 50Hz mode
        let cmd = AntiFlickerCommand::new(AntiFlickerMode::Hz50);
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x23, 0x01, 0xFF]
        );

        // Test 60Hz mode
        let cmd = AntiFlickerCommand::new(AntiFlickerMode::Hz60);
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x23, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_command_categories() {
        // All gain commands should be Quick category
        assert_eq!(Gain::Reset.timeout_kind(), CommandCategory::Quick);
        assert_eq!(Gain::Up.timeout_kind(), CommandCategory::Quick);
        assert_eq!(Gain::Down.timeout_kind(), CommandCategory::Quick);
        assert_eq!(
            Gain::SetValue(
                GainLevel::new(0x05).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
            )
            .timeout_kind(),
            CommandCategory::Quick
        );
        assert_eq!(
            GainLimitCommand::new(
                GainLimit::new(0x03).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
            )
            .timeout_kind(),
            CommandCategory::Quick
        );
        assert_eq!(
            AntiFlickerCommand::new(AntiFlickerMode::Hz50).timeout_kind(),
            CommandCategory::Quick
        );
    }

    #[test]
    fn test_response_types() {
        // All gain commands should return None for response_type
        assert!(Gain::Reset.response_type().is_none());
        assert!(Gain::Up.response_type().is_none());
        assert!(Gain::Down.response_type().is_none());
        assert!(Gain::SetValue(
            GainLevel::new(0x05).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        )
        .response_type()
        .is_none());
        assert!(GainLimitCommand::new(
            GainLimit::new(0x03).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        )
        .response_type()
        .is_none());
        assert!(AntiFlickerCommand::new(AntiFlickerMode::Hz50)
            .response_type()
            .is_none());
    }

    #[test]
    fn test_gain_command_debug() {
        // Test Debug trait implementation
        let cmd = Gain::Reset;
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("Reset"));

        let cmd = Gain::SetValue(
            GainLevel::new(0x05).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
        );
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("SetValue"));
    }

    #[test]
    fn test_anti_flicker_clone() {
        // Test Copy/Clone traits
        let cmd1 = AntiFlickerCommand::new(AntiFlickerMode::Hz50);
        let cmd2 = cmd1; // Copy
        let cmd3 = cmd1; // Copy (clone() not needed for Copy types)

        assert_eq!(
            cmd1.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            cmd2.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        );
        assert_eq!(
            cmd1.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            cmd3.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        );
    }
}
