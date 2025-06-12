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
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_gain_command_reset() {
        let cmd = GainCommand::Reset;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0C, 0x00, 0xFF]
        );
    }

    #[test]
    fn test_gain_command_up() {
        let cmd = GainCommand::Up;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0C, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_gain_command_down() {
        let cmd = GainCommand::Down;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x0C, 0x03, 0xFF]
        );
    }

    #[test]
    fn test_gain_command_direct() {
        // Test various gain values
        let test_values = vec![0x00, 0x01, 0x03, 0x05, 0x07];
        for value in test_values {
            let gain =
                GainValue::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = GainCommand::Direct(gain);
            let high = (value >> 4) & 0x0F;
            let low = value & 0x0F;
            assert_eq!(
                cmd.to_bytes()
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
                GainValue::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = GainCommand::Direct(gain);
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // GainValue itself is limited to 0x00-0x07, which are all valid for G2
        // So all valid GainValue instances should pass G2 validation

        // Non-direct commands should always be valid
        assert!(GainCommand::Reset
            .validate_for_model(CameraModel::PTZOpticsG2)
            .is_ok());
        assert!(GainCommand::Up
            .validate_for_model(CameraModel::PTZOpticsG2)
            .is_ok());
        assert!(GainCommand::Down
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
            let cmd = GainLimitCommand { limit };
            assert_eq!(
                cmd.to_bytes()
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
            let cmd = GainLimitCommand { limit };
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Test that non-G2 values might fail (depends on what G2_VALID_VALUES contains)
        // Check if value 0x08 is not in G2_VALID_VALUES
        if !GainLimit::G2_VALID_VALUES.contains(&0x08) {
            if let Ok(limit) = GainLimit::new(0x08) {
                let cmd = GainLimitCommand { limit };
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
        let cmd = AntiFlickerCommand {
            mode: AntiFlickerMode::Off,
        };
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x23, 0x00, 0xFF]
        );

        // Test 50Hz mode
        let cmd = AntiFlickerCommand {
            mode: AntiFlickerMode::Hz50,
        };
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x23, 0x01, 0xFF]
        );

        // Test 60Hz mode
        let cmd = AntiFlickerCommand {
            mode: AntiFlickerMode::Hz60,
        };
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x23, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_command_categories() {
        // All gain commands should be Quick category
        assert_eq!(
            GainCommand::Reset.command_category(),
            CommandCategory::Quick
        );
        assert_eq!(GainCommand::Up.command_category(), CommandCategory::Quick);
        assert_eq!(GainCommand::Down.command_category(), CommandCategory::Quick);
        assert_eq!(
            GainCommand::Direct(
                GainValue::new(0x05).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
            )
            .command_category(),
            CommandCategory::Quick
        );
        assert_eq!(
            GainLimitCommand {
                limit: GainLimit::new(0x03)
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
            }
            .command_category(),
            CommandCategory::Quick
        );
        assert_eq!(
            AntiFlickerCommand {
                mode: AntiFlickerMode::Hz50
            }
            .command_category(),
            CommandCategory::Quick
        );
    }

    #[test]
    fn test_response_types() {
        // All gain commands should return None for response_type
        assert!(GainCommand::Reset.response_type().is_none());
        assert!(GainCommand::Up.response_type().is_none());
        assert!(GainCommand::Down.response_type().is_none());
        assert!(GainCommand::Direct(
            GainValue::new(0x05).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        )
        .response_type()
        .is_none());
        assert!(GainLimitCommand {
            limit: GainLimit::new(0x03).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        }
        .response_type()
        .is_none());
        assert!(AntiFlickerCommand {
            mode: AntiFlickerMode::Hz50
        }
        .response_type()
        .is_none());
    }

    #[test]
    fn test_gain_command_debug() {
        // Test Debug trait implementation
        let cmd = GainCommand::Reset;
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("Reset"));

        let cmd = GainCommand::Direct(
            GainValue::new(0x05).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
        );
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("Direct"));
    }

    #[test]
    fn test_anti_flicker_clone() {
        // Test Copy/Clone traits
        let cmd1 = AntiFlickerCommand {
            mode: AntiFlickerMode::Hz50,
        };
        let cmd2 = cmd1; // Copy
        let cmd3 = cmd1; // Copy (clone() not needed for Copy types)

        assert_eq!(
            cmd1.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            cmd2.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        );
        assert_eq!(
            cmd1.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            cmd3.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        );
    }
}
