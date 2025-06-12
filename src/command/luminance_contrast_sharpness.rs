//! Luminance, contrast, and sharpness control commands for VISCA cameras.
//!
//! This module provides commands for adjusting image quality parameters such as
//! luminance (brightness), contrast levels, and sharpness settings.

// Crate imports
use crate::{
    command::{Command, ResponseType},
    constants::CameraModel,
    error::Error,
    timeout::CommandCategory,
    types::{ContrastLevel, LuminanceLevel},
};

/// Sharpness control modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharpnessMode {
    /// Automatic sharpness adjustment based on scene content.
    Auto,
    /// Manual sharpness control.
    Manual,
}

/// Sharpness control commands.
///
/// Controls edge enhancement to make images appear more or less sharp.
/// Higher sharpness values enhance edges but may introduce artifacts.
#[derive(Debug, Copy, Clone)]
pub enum SharpnessCommand {
    /// Set sharpness mode (auto or manual).
    Mode(SharpnessMode),
    /// Reset sharpness to default value.
    Reset,
    /// Increase sharpness by one step.
    Up,
    /// Decrease sharpness by one step.
    Down,
    /// Set sharpness to specific value (0-11).
    Direct {
        /// Sharpness value (0 = minimum, 11 = maximum).
        value: u8,
    },
}

impl Command for SharpnessCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(match self {
            Self::Mode(mode) => {
                let mode_byte = match mode {
                    SharpnessMode::Auto => 0x02,
                    SharpnessMode::Manual => 0x03,
                };
                vec![0x81, 0x01, 0x04, 0x05, mode_byte, 0xFF]
            }
            Self::Reset => vec![0x81, 0x01, 0x04, 0x02, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x02, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x02, 0x03, 0xFF],
            Self::Direct { value } => {
                if *value > 11 {
                    return Err(Error::InvalidParameter(
                        "Sharpness value must be in the range 0..=11".into(),
                    ));
                }
                let high = (*value >> 4) & 0x0F;
                let low = *value & 0x0F;
                vec![0x81, 0x01, 0x04, 0x42, 0x00, 0x00, high, low, 0xFF]
            }
        })
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Custom
    }

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        use crate::types::SharpnessLevel;

        match self {
            Self::Direct { value } => {
                if matches!(model, CameraModel::PTZOpticsG2)
                    && !SharpnessLevel::G2_VALID_VALUES.contains(value)
                {
                    return Err(Error::ModelValidation {
                        model,
                        command: "SharpnessDirect".to_string(),
                        reason: format!(
                            "Sharpness value {value} is not valid for G2. Valid values: 0-11"
                        ),
                    });
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Command to set the luminance level.
#[derive(Debug, Clone, Copy)]
pub struct LuminanceCommand {
    /// The luminance level.
    pub value: LuminanceLevel,
}

impl Command for LuminanceCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(vec![
            0x81,
            0x01,
            0x04,
            0xA1,
            0x00,
            0x00,
            0x00,
            self.value.value(),
            0xFF,
        ])
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
                let value = self.value.value();
                if !LuminanceLevel::G2_VALID_VALUES.contains(&value) {
                    return Err(Error::ModelValidation {
                        model,
                        command: "Luminance".to_string(),
                        reason: format!("Value {value:#02X} not supported on G2 cameras"),
                    });
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Command to set the contrast level.
#[derive(Debug, Clone, Copy)]
pub struct ContrastCommand {
    /// The contrast level.
    pub value: ContrastLevel,
}

impl Command for ContrastCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(vec![
            0x81,
            0x01,
            0x04,
            0xA2,
            0x00,
            0x00,
            0x00,
            self.value.value(),
            0xFF,
        ])
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
                let value = self.value.value();
                if !ContrastLevel::G2_VALID_VALUES.contains(&value) {
                    return Err(Error::ModelValidation {
                        model,
                        command: "Contrast".to_string(),
                        reason: format!("Value {value:#02X} not supported on G2 cameras"),
                    });
                }
                Ok(())
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
    fn test_sharpness_mode() {
        // Test Auto mode
        let cmd = SharpnessCommand::Mode(SharpnessMode::Auto);
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x05, 0x02, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.command_category(), CommandCategory::Custom));

        // Test Manual mode
        let cmd = SharpnessCommand::Mode(SharpnessMode::Manual);
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x05, 0x03, 0xFF]
        );
    }

    #[test]
    fn test_sharpness_reset() {
        let cmd = SharpnessCommand::Reset;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x02, 0x00, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.command_category(), CommandCategory::Custom));
    }

    #[test]
    fn test_sharpness_up_down() {
        // Test Up
        let cmd = SharpnessCommand::Up;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x02, 0x02, 0xFF]
        );

        // Test Down
        let cmd = SharpnessCommand::Down;
        assert_eq!(
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x02, 0x03, 0xFF]
        );
    }

    #[test]
    fn test_sharpness_direct() {
        // Test valid values 0-11
        for value in 0..=11 {
            let cmd = SharpnessCommand::Direct { value };
            let bytes = cmd
                .to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            assert_eq!(bytes.len(), 9);
            assert_eq!(bytes[0..6], [0x81, 0x01, 0x04, 0x42, 0x00, 0x00]);
            assert_eq!(bytes[6], (value >> 4) & 0x0F);
            assert_eq!(bytes[7], value & 0x0F);
            assert_eq!(bytes[8], 0xFF);
        }

        // Test invalid value
        let cmd = SharpnessCommand::Direct { value: 12 };
        assert!(cmd.to_bytes().is_err());
    }

    #[test]
    fn test_sharpness_g2_validation() {
        // Test valid G2 values
        for value in 0..=11 {
            let cmd = SharpnessCommand::Direct { value };
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Test invalid G2 value
        let cmd = SharpnessCommand::Direct { value: 12 };
        let result = cmd.validate_for_model(CameraModel::PTZOpticsG2);
        assert!(matches!(result, Err(Error::ModelValidation { .. })));

        // Test that non-Direct commands pass validation
        let cmd = SharpnessCommand::Reset;
        assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());

        let cmd = SharpnessCommand::Mode(SharpnessMode::Auto);
        assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
    }

    #[test]
    fn test_luminance_command() {
        // Test valid values
        for value in 0..=14 {
            let level = LuminanceLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = LuminanceCommand { value: level };
            let bytes = cmd
                .to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            assert_eq!(
                bytes,
                vec![0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00, value, 0xFF]
            );
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.command_category(), CommandCategory::Quick));
        }
    }

    #[test]
    fn test_luminance_g2_validation() {
        // Test valid G2 values
        for value in 0..=14 {
            let level = LuminanceLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = LuminanceCommand { value: level };
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // G2 supports all values 0-14, so no invalid values to test
        // The LuminanceLevel type itself enforces the valid range
    }

    #[test]
    fn test_contrast_command() {
        // Test valid values
        for value in 0..=14 {
            let level = ContrastLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = ContrastCommand { value: level };
            let bytes = cmd
                .to_bytes()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            assert_eq!(
                bytes,
                vec![0x81, 0x01, 0x04, 0xA2, 0x00, 0x00, 0x00, value, 0xFF]
            );
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.command_category(), CommandCategory::Quick));
        }
    }

    #[test]
    fn test_contrast_g2_validation() {
        // Test valid G2 values
        for value in 0..=14 {
            let level = ContrastLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = ContrastCommand { value: level };
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // G2 supports all values 0-14, so no invalid values to test
        // The ContrastLevel type itself enforces the valid range
    }

    #[test]
    fn test_sharpness_mode_equality() {
        assert_eq!(SharpnessMode::Auto, SharpnessMode::Auto);
        assert_eq!(SharpnessMode::Manual, SharpnessMode::Manual);
        assert_ne!(SharpnessMode::Auto, SharpnessMode::Manual);
    }

    #[test]
    fn test_command_traits() {
        // Test Debug trait
        let cmds: Vec<Box<dyn std::fmt::Debug>> = vec![
            Box::new(SharpnessCommand::Reset),
            Box::new(SharpnessCommand::Mode(SharpnessMode::Auto)),
            Box::new(SharpnessCommand::Direct { value: 5 }),
            Box::new(LuminanceCommand {
                value: LuminanceLevel::new(7)
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            }),
            Box::new(ContrastCommand {
                value: ContrastLevel::new(7)
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            }),
        ];

        for cmd in cmds {
            let _ = format!("{cmd:?}");
        }

        // Test Clone
        let sharp_cmd1 = SharpnessCommand::Direct { value: 5 };
        let sharp_cmd2 = sharp_cmd1;
        match (sharp_cmd1, sharp_cmd2) {
            (SharpnessCommand::Direct { value: v1 }, SharpnessCommand::Direct { value: v2 }) => {
                assert_eq!(v1, v2);
            }
            _ => panic!("Clone didn't preserve variant"),
        }
    }

    #[test]
    fn test_edge_cases() {
        // Test boundary values for sharpness
        let cmd = SharpnessCommand::Direct { value: 0 };
        assert!(cmd.to_bytes().is_ok());

        let cmd = SharpnessCommand::Direct { value: 11 };
        assert!(cmd.to_bytes().is_ok());

        // Test boundary values for luminance
        let level =
            LuminanceLevel::new(0).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = LuminanceCommand { value: level };
        assert!(cmd.to_bytes().is_ok());

        let level =
            LuminanceLevel::new(14).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = LuminanceCommand { value: level };
        assert!(cmd.to_bytes().is_ok());

        // Test boundary values for contrast
        let level =
            ContrastLevel::new(0).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = ContrastCommand { value: level };
        assert!(cmd.to_bytes().is_ok());

        let level =
            ContrastLevel::new(14).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = ContrastCommand { value: level };
        assert!(cmd.to_bytes().is_ok());
    }

    #[test]
    fn test_nibble_encoding_sharpness() {
        // Test that Direct command properly encodes value as nibbles
        let cmd = SharpnessCommand::Direct { value: 0x0B };
        let bytes = cmd
            .to_bytes()
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        assert_eq!(bytes[6], 0x00); // High nibble
        assert_eq!(bytes[7], 0x0B); // Low nibble

        let cmd = SharpnessCommand::Direct { value: 0x05 };
        let bytes = cmd
            .to_bytes()
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        assert_eq!(bytes[6], 0x00); // High nibble
        assert_eq!(bytes[7], 0x05); // Low nibble
    }

    #[test]
    fn test_response_type_none() {
        // Verify all commands return None for response_type
        let cmds: Vec<Box<dyn Command>> = vec![
            Box::new(SharpnessCommand::Reset),
            Box::new(SharpnessCommand::Mode(SharpnessMode::Auto)),
            Box::new(SharpnessCommand::Up),
            Box::new(SharpnessCommand::Down),
            Box::new(SharpnessCommand::Direct { value: 5 }),
            Box::new(LuminanceCommand {
                value: LuminanceLevel::new(7)
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            }),
            Box::new(ContrastCommand {
                value: ContrastLevel::new(7)
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            }),
        ];

        for cmd in cmds {
            assert!(cmd.response_type().is_none());
        }
    }

    #[test]
    fn test_command_categories() {
        // Test SharpnessCommand uses Custom category
        let sharpness_cmds = vec![
            SharpnessCommand::Reset,
            SharpnessCommand::Mode(SharpnessMode::Auto),
            SharpnessCommand::Up,
            SharpnessCommand::Down,
            SharpnessCommand::Direct { value: 5 },
        ];

        for cmd in sharpness_cmds {
            assert!(matches!(cmd.command_category(), CommandCategory::Custom));
        }

        // Test LuminanceCommand and ContrastCommand use Quick category
        let level =
            LuminanceLevel::new(7).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = LuminanceCommand { value: level };
        assert!(matches!(cmd.command_category(), CommandCategory::Quick));

        let level =
            ContrastLevel::new(7).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = ContrastCommand { value: level };
        assert!(matches!(cmd.command_category(), CommandCategory::Quick));
    }
}
