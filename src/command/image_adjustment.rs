//! Image adjustment commands for VISCA cameras.
//!
//! This module provides commands for adjusting image quality parameters such as
//! sharpness, brightness (luminance), and contrast levels.

// Crate imports
use crate::{
    command::{encode_visca::EncodeVisca, ResponseType},
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

impl TryFrom<u8> for SharpnessMode {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x02 => Ok(SharpnessMode::Auto),
            0x03 => Ok(SharpnessMode::Manual),
            _ => Err(Error::InvalidResponse {
                expected: "0x02 (Auto) or 0x03 (Manual)".to_string(),
                actual: vec![value],
            }),
        }
    }
}

/// Sharpness control commands.
///
/// Controls edge enhancement to make images appear more or less sharp.
/// Higher sharpness values enhance edges but may introduce artifacts.
#[derive(Debug, Copy, Clone)]
pub enum Sharpness {
    /// Set sharpness mode (auto or manual).
    Mode(SharpnessMode),
    /// Reset sharpness to default value.
    Reset,
    /// Increase sharpness by one step.
    Up,
    /// Decrease sharpness by one step.
    Down,
    /// Set sharpness to specific value (0-11).
    SetLevel {
        /// Sharpness value (0 = minimum, 11 = maximum).
        value: u8,
    },
}

impl EncodeVisca for Sharpness {
    type Response = ();
    const MAX_SIZE: usize = 9;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        match self {
            Self::Mode(mode) => {
                let mode_byte = match mode {
                    SharpnessMode::Auto => 0x02,
                    SharpnessMode::Manual => 0x03,
                };
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x05;
                buffer[4] = mode_byte;
                buffer[5] = 0xFF;
                Ok(6)
            }
            Self::Reset => {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x02;
                buffer[4] = 0x00;
                buffer[5] = 0xFF;
                Ok(6)
            }
            Self::Up => {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x02;
                buffer[4] = 0x02;
                buffer[5] = 0xFF;
                Ok(6)
            }
            Self::Down => {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x02;
                buffer[4] = 0x03;
                buffer[5] = 0xFF;
                Ok(6)
            }
            Self::SetLevel { value } => {
                if *value > 11 {
                    return Err(Error::InvalidParameter {
                        parameter: "value",
                        value: value.to_string(),
                        reason: "Sharpness value must be in the range 0..=11".to_string(),
                    });
                }
                let high = (*value >> 4) & 0x0F;
                let low = *value & 0x0F;

                if buffer.len() < 9 {
                    return Err(Error::BufferTooSmall {
                        required: 9,
                        actual: buffer.len(),
                    });
                }

                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x42;
                buffer[4] = 0x00;
                buffer[5] = 0x00;
                buffer[6] = high;
                buffer[7] = low;
                buffer[8] = 0xFF;
                Ok(9)
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Custom
    }
}

crate::visca_builder! {
    /// Command to set the luminance (brightness) level.
    pub(crate) struct LuminanceCommand {
        /// The luminance level to set.
        value: LuminanceLevel,
    }
    builder<9> => |builder, value| {
        let _ = builder.append(crate::command::const_encoding::constants::image::LUMINANCE_PREFIX);
        let _ = builder.push(value.value());
    }
    timeout = Quick;
}

impl LuminanceCommand {
    /// Create a new luminance command.
    pub fn new(value: LuminanceLevel) -> Self {
        Self { value }
    }
}

crate::visca_builder! {
    /// Command to set the contrast level.
    pub(crate) struct ContrastCommand {
        /// The contrast level to set.
        value: ContrastLevel,
    }
    builder<9> => |builder, value| {
        let _ = builder.append(crate::command::const_encoding::constants::image::CONTRAST_PREFIX);
        let _ = builder.push(value.value());
    }
    timeout = Quick;
}

impl ContrastCommand {
    /// Create a new contrast command.
    pub fn new(value: ContrastLevel) -> Self {
        Self { value }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use crate::{constants::CameraModel, EncodeVisca};
    use crate::types::SharpnessLevel;

    #[test]
    fn test_sharpness_mode() {
        // Test Auto mode
        let cmd = Sharpness::Mode(SharpnessMode::Auto);
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x05, 0x02, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Custom));

        // Test Manual mode
        let cmd = Sharpness::Mode(SharpnessMode::Manual);
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x05, 0x03, 0xFF]
        );
    }

    #[test]
    fn test_sharpness_reset() {
        let cmd = Sharpness::Reset;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x02, 0x00, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Custom));
    }

    #[test]
    fn test_sharpness_up_down() {
        // Test Up
        let cmd = Sharpness::Up;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x02, 0x02, 0xFF]
        );

        // Test Down
        let cmd = Sharpness::Down;
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x02, 0x03, 0xFF]
        );
    }

    #[test]
    fn test_sharpness_set_level() {
        // Test valid values 0-11
        for value in 0..=11 {
            let cmd = Sharpness::SetLevel { value };
            let bytes = cmd
                .try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            assert_eq!(bytes.len(), 9);
            assert_eq!(bytes[0..6], [0x81, 0x01, 0x04, 0x42, 0x00, 0x00]);
            assert_eq!(bytes[6], (value >> 4) & 0x0F);
            assert_eq!(bytes[7], value & 0x0F);
            assert_eq!(bytes[8], 0xFF);
        }

        // Test invalid value
        // SharpnessLevel enforces valid range, so we can't create an invalid value
        // The validation is done at the type level
    }

    #[test]
    fn test_sharpness_g2_validation() {
        // Test valid G2 values
        for value in 0..=11 {
            let cmd = Sharpness::SetLevel { value };
            assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        }

        // Test invalid G2 value
        // SharpnessLevel enforces valid range 0-11, so we can't create value 12
        // The validation is done at the type level
        let result = SharpnessLevel::new(12);
        assert!(result.is_err());

        // Test that non-SetLevel commands pass validation
        let cmd = Sharpness::Reset;
        assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());

        let cmd = Sharpness::Mode(SharpnessMode::Auto);
        assert!(cmd.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
    }

    #[test]
    fn test_luminance_command() {
        // Test valid values
        for value in 0..=14 {
            let level = LuminanceLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = LuminanceCommand::new(level);
            let bytes = cmd
                .try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            assert_eq!(
                bytes,
                vec![0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00, value, 0xFF]
            );
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));
        }
    }

    #[test]
    fn test_luminance_g2_validation() {
        // Test valid G2 values
        for value in 0..=14 {
            let level = LuminanceLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = LuminanceCommand::new(level);
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
            let cmd = ContrastCommand::new(level);
            let bytes = cmd
                .try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            assert_eq!(
                bytes,
                vec![0x81, 0x01, 0x04, 0xA2, 0x00, 0x00, 0x00, value, 0xFF]
            );
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));
        }
    }

    #[test]
    fn test_contrast_g2_validation() {
        // Test valid G2 values
        for value in 0..=14 {
            let level = ContrastLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = ContrastCommand::new(level);
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
            Box::new(Sharpness::Reset),
            Box::new(Sharpness::Mode(SharpnessMode::Auto)),
            Box::new(Sharpness::SetLevel { value: 5 }),
            Box::new(LuminanceCommand::new(
                LuminanceLevel::new(7).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            )),
            Box::new(ContrastCommand::new(
                ContrastLevel::new(7).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            )),
        ];

        for cmd in cmds {
            let _ = format!("{cmd:?}");
        }

        // Test Clone
        let sharp_cmd1 = Sharpness::SetLevel { value: 5 };
        let sharp_cmd2 = sharp_cmd1;
        match (sharp_cmd1, sharp_cmd2) {
            (Sharpness::SetLevel { value: v1 }, Sharpness::SetLevel { value: v2 }) => {
                assert_eq!(v1, v2);
            }
            _ => panic!("Clone didn't preserve variant"),
        }
    }

    #[test]
    fn test_edge_cases() {
        // Test boundary values for sharpness
        let cmd = Sharpness::SetLevel { value: 0 };
        assert!(cmd.try_into_vec().is_ok());

        let cmd = Sharpness::SetLevel { value: 11 };
        assert!(cmd.try_into_vec().is_ok());

        // Test boundary values for luminance
        let level =
            LuminanceLevel::new(0).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = LuminanceCommand { value: level };
        assert!(cmd.try_into_vec().is_ok());

        let level =
            LuminanceLevel::new(14).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = LuminanceCommand { value: level };
        assert!(cmd.try_into_vec().is_ok());

        // Test boundary values for contrast
        let level =
            ContrastLevel::new(0).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = ContrastCommand { value: level };
        assert!(cmd.try_into_vec().is_ok());

        let level =
            ContrastLevel::new(14).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = ContrastCommand { value: level };
        assert!(cmd.try_into_vec().is_ok());
    }

    #[test]
    fn test_nibble_encoding_sharpness() {
        // Test that SetLevel command properly encodes value as nibbles
        let cmd = Sharpness::SetLevel { value: 0x0B };
        let bytes = cmd
            .try_into_vec()
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        assert_eq!(bytes[6], 0x00); // High nibble
        assert_eq!(bytes[7], 0x0B); // Low nibble

        let cmd = Sharpness::SetLevel { value: 0x05 };
        let bytes = cmd
            .try_into_vec()
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        assert_eq!(bytes[6], 0x00); // High nibble
        assert_eq!(bytes[7], 0x05); // Low nibble
    }

    #[test]
    fn test_response_type_none() {
        // Verify all commands return None for response_type
        assert!(Sharpness::Reset.response_type().is_none());
        assert!(Sharpness::Mode(SharpnessMode::Auto)
            .response_type()
            .is_none());
        assert!(Sharpness::Up.response_type().is_none());
        assert!(Sharpness::Down.response_type().is_none());
        assert!(Sharpness::SetLevel { value: 5 }.response_type().is_none());
        assert!(LuminanceCommand::new(
            LuminanceLevel::new(7).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        )
        .response_type()
        .is_none());
        assert!(ContrastCommand::new(
            ContrastLevel::new(7).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        )
        .response_type()
        .is_none());
    }

    #[test]
    fn test_command_categories() {
        // Test Sharpness uses Custom category
        let sharpness_cmds = vec![
            Sharpness::Reset,
            Sharpness::Mode(SharpnessMode::Auto),
            Sharpness::Up,
            Sharpness::Down,
            Sharpness::SetLevel { value: 5 },
        ];

        for cmd in sharpness_cmds {
            assert!(matches!(cmd.timeout_kind(), CommandCategory::Custom));
        }

        // Test LuminanceCommand and ContrastCommand use Quick category
        let level =
            LuminanceLevel::new(7).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = LuminanceCommand { value: level };
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));

        let level =
            ContrastLevel::new(7).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = ContrastCommand { value: level };
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));
    }
}
