//! Image adjustment commands for VISCA cameras.
//!
//! This module provides commands for adjusting image quality parameters such as
//! sharpness, brightness (luminance), and contrast levels.

// Standard library imports
use std::borrow::Cow;

// Crate imports
use crate::macros::internal::*;

use crate::{
    command::{encode_visca::EncodeVisca, ResponseType},
    error::Error,
    timeout::CommandCategory,
    types::{ContrastLevel, LuminanceLevel},
};
use grafton_visca_macros::ViscaEnum;

/// Sharpness control modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
pub enum SharpnessMode {
    /// Automatic sharpness adjustment based on scene content.
    Auto = 0x02,
    /// Manual sharpness control.
    Manual = 0x03,
}

/// Noise reduction modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
pub enum NrMode {
    /// Noise reduction disabled.
    Off = 0x02,
    /// Noise reduction enabled.
    On = 0x03,
}

/// Noise reduction speed settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
pub enum NrSpeed {
    /// Slow noise reduction processing.
    Slow = 0x00,
    /// Normal noise reduction processing.
    Normal = 0x01,
    /// Fast noise reduction processing.
    Fast = 0x02,
}

/// Black and white mode settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
pub enum BlackWhiteMode {
    /// Color mode (normal operation).
    Color = 0x02,
    /// Black and white mode.
    BlackWhite = 0x03,
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
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Custom;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::const_encoding::{constants, CommandBuilder};

        match self {
            Self::Mode(mode) => {
                let mode_byte = match mode {
                    SharpnessMode::Auto => 0x02,
                    SharpnessMode::Manual => 0x03,
                };
                let mut builder = CommandBuilder::<6>::new();
                builder.append_mut(constants::image::SHARPNESS_MODE_PREFIX);
                builder.push_mut(mode_byte);
                builder.with_camera_id_mut(camera_id);
                builder.finalize();
                builder.copy_to(buffer)
            }
            Self::Reset => {
                let mut builder = CommandBuilder::<6>::new();
                builder.append_mut(constants::image::SHARPNESS_CONTROL_PREFIX);
                builder.push_mut(0x00);
                builder.with_camera_id_mut(camera_id);
                builder.finalize();
                builder.copy_to(buffer)
            }
            Self::Up => {
                let mut builder = CommandBuilder::<6>::new();
                builder.append_mut(constants::image::SHARPNESS_CONTROL_PREFIX);
                builder.push_mut(0x02);
                builder.with_camera_id_mut(camera_id);
                builder.finalize();
                builder.copy_to(buffer)
            }
            Self::Down => {
                let mut builder = CommandBuilder::<6>::new();
                builder.append_mut(constants::image::SHARPNESS_CONTROL_PREFIX);
                builder.push_mut(0x03);
                builder.with_camera_id_mut(camera_id);
                builder.finalize();
                builder.copy_to(buffer)
            }
            Self::SetLevel { value } => {
                if *value > 11 {
                    return Err(Error::InvalidParameter {
                        parameter: "value",
                        value: Cow::Owned(value.to_string()),
                        reason: Cow::Borrowed("Sharpness value must be in the range 0..=11"),
                    });
                }
                let mut builder = CommandBuilder::<9>::new();
                builder.append_mut(constants::image::SHARPNESS_LEVEL_PREFIX);
                builder.push_nibble_pair_mut(*value as u16);
                builder.with_camera_id_mut(camera_id);
                builder.finalize();
                builder.copy_to(buffer)
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }
}

visca_builder! {
    /// Command to set the luminance (brightness) level.
    pub struct LuminanceCommand {
        /// The luminance level to set.
        value: LuminanceLevel,
    }
    builder<9> => |builder, value| {
        builder
            .append(crate::command::const_encoding::constants::image::LUMINANCE_PREFIX)
            .push(value.value())
    }
    timeout = Quick;
}

impl LuminanceCommand {
    /// Create a new luminance command.
    pub fn new(value: LuminanceLevel) -> Self {
        Self { value }
    }
}

visca_builder! {
    /// Command to set the contrast level.
    pub struct ContrastCommand {
        /// The contrast level to set.
        value: ContrastLevel,
    }
    builder<9> => |builder, value| {
        builder
            .append(crate::command::const_encoding::constants::image::CONTRAST_PREFIX)
            .push(value.value())
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
#[allow(clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::command::const_encoding::VISCA_TERMINATOR;
    use crate::command::encode_visca::EncodeVisca;
    use crate::constants::CameraVariant;
    use crate::macros::test_utils::visca_test;
    use crate::types::SharpnessLevel;

    // Test Auto mode
    visca_test!(
        Sharpness,
        test_sharpness_mode_auto,
        Sharpness::Mode(SharpnessMode::Auto),
        &[0x81, 0x01, 0x04, 0x05, 0x02, VISCA_TERMINATOR]
    );

    // Test Manual mode
    visca_test!(
        Sharpness,
        test_sharpness_mode_manual,
        Sharpness::Mode(SharpnessMode::Manual),
        &[0x81, 0x01, 0x04, 0x05, 0x03, VISCA_TERMINATOR]
    );

    #[test]
    fn test_sharpness_properties() {
        let cmd = Sharpness::Mode(SharpnessMode::Auto);
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Custom));
    }

    visca_test!(
        Sharpness,
        test_sharpness_reset,
        Sharpness::Reset,
        &[0x81, 0x01, 0x04, 0x02, 0x00, VISCA_TERMINATOR]
    );

    // Test Up
    visca_test!(
        Sharpness,
        test_sharpness_up,
        Sharpness::Up,
        &[0x81, 0x01, 0x04, 0x02, 0x02, VISCA_TERMINATOR]
    );

    // Test Down
    visca_test!(
        Sharpness,
        test_sharpness_down,
        Sharpness::Down,
        &[0x81, 0x01, 0x04, 0x02, 0x03, VISCA_TERMINATOR]
    );

    // Test sharpness level 0
    visca_test!(
        Sharpness,
        test_sharpness_level_0,
        Sharpness::SetLevel { value: 0 },
        &[
            0x81,
            0x01,
            0x04,
            0x42,
            0x00,
            0x00,
            0x00,
            0x00,
            VISCA_TERMINATOR
        ]
    );

    // Test sharpness level 5
    visca_test!(
        Sharpness,
        test_sharpness_level_5,
        Sharpness::SetLevel { value: 5 },
        &[
            0x81,
            0x01,
            0x04,
            0x42,
            0x00,
            0x00,
            0x00,
            0x05,
            VISCA_TERMINATOR
        ]
    );

    // Test sharpness level 11
    visca_test!(
        Sharpness,
        test_sharpness_level_11,
        Sharpness::SetLevel { value: 11 },
        &[
            0x81,
            0x01,
            0x04,
            0x42,
            0x00,
            0x00,
            0x00,
            0x0B,
            VISCA_TERMINATOR
        ]
    );

    #[test]
    fn test_sharpness_g2_validation() {
        // Test valid G2 values
        for value in 0..=11 {
            let cmd = Sharpness::SetLevel { value };
            assert!(cmd.validate_for_model(CameraVariant::PTZOpticsG2).is_ok());
        }

        // Test invalid G2 value
        // SharpnessLevel enforces valid range 0-11, so we can't create value 12
        // The validation is done at the type level
        let result = SharpnessLevel::new(12);
        assert!(result.is_err());

        // Test that non-SetLevel commands pass validation
        let cmd = Sharpness::Reset;
        assert!(cmd.validate_for_model(CameraVariant::PTZOpticsG2).is_ok());

        let cmd = Sharpness::Mode(SharpnessMode::Auto);
        assert!(cmd.validate_for_model(CameraVariant::PTZOpticsG2).is_ok());
    }

    // Test luminance level 0
    visca_test!(
        LuminanceCommand,
        test_luminance_level_0,
        LuminanceCommand::new(LuminanceLevel::new(0).unwrap()),
        &[
            0x81,
            0x01,
            0x04,
            0xA1,
            0x00,
            0x00,
            0x00,
            0x00,
            VISCA_TERMINATOR
        ]
    );

    // Test luminance level 7
    visca_test!(
        LuminanceCommand,
        test_luminance_level_7,
        LuminanceCommand::new(LuminanceLevel::new(7).unwrap()),
        &[
            0x81,
            0x01,
            0x04,
            0xA1,
            0x00,
            0x00,
            0x00,
            0x07,
            VISCA_TERMINATOR
        ]
    );

    // Test luminance level 14
    visca_test!(
        LuminanceCommand,
        test_luminance_level_14,
        LuminanceCommand::new(LuminanceLevel::new(14).unwrap()),
        &[
            0x81,
            0x01,
            0x04,
            0xA1,
            0x00,
            0x00,
            0x00,
            0x0E,
            VISCA_TERMINATOR
        ]
    );

    #[test]
    fn test_luminance_properties() {
        let cmd = LuminanceCommand::new(LuminanceLevel::new(7).unwrap());
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));
    }

    #[test]
    fn test_luminance_g2_validation() {
        // Test valid G2 values
        for value in 0..=14 {
            let level = LuminanceLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = LuminanceCommand::new(level);
            assert!(cmd.validate_for_model(CameraVariant::PTZOpticsG2).is_ok());
        }

        // G2 supports all values 0-14, so no invalid values to test
        // The LuminanceLevel type itself enforces the valid range
    }

    // Test contrast level 0
    visca_test!(
        ContrastCommand,
        test_contrast_level_0,
        ContrastCommand::new(ContrastLevel::new(0).unwrap()),
        &[
            0x81,
            0x01,
            0x04,
            0xA2,
            0x00,
            0x00,
            0x00,
            0x00,
            VISCA_TERMINATOR
        ]
    );

    // Test contrast level 7
    visca_test!(
        ContrastCommand,
        test_contrast_level_7,
        ContrastCommand::new(ContrastLevel::new(7).unwrap()),
        &[
            0x81,
            0x01,
            0x04,
            0xA2,
            0x00,
            0x00,
            0x00,
            0x07,
            VISCA_TERMINATOR
        ]
    );

    // Test contrast level 14
    visca_test!(
        ContrastCommand,
        test_contrast_level_14,
        ContrastCommand::new(ContrastLevel::new(14).unwrap()),
        &[
            0x81,
            0x01,
            0x04,
            0xA2,
            0x00,
            0x00,
            0x00,
            0x0E,
            VISCA_TERMINATOR
        ]
    );

    #[test]
    fn test_contrast_properties() {
        let cmd = ContrastCommand::new(ContrastLevel::new(7).unwrap());
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));
    }

    #[test]
    fn test_contrast_g2_validation() {
        // Test valid G2 values
        for value in 0..=14 {
            let level = ContrastLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = ContrastCommand::new(level);
            assert!(cmd.validate_for_model(CameraVariant::PTZOpticsG2).is_ok());
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
        assert!(cmd
            .try_into_vec(crate::camera_id::CameraId::CAMERA_1)
            .is_ok());

        let cmd = Sharpness::SetLevel { value: 11 };
        assert!(cmd
            .try_into_vec(crate::camera_id::CameraId::CAMERA_1)
            .is_ok());

        // Test boundary values for luminance
        let level =
            LuminanceLevel::new(0).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = LuminanceCommand { value: level };
        assert!(cmd
            .try_into_vec(crate::camera_id::CameraId::CAMERA_1)
            .is_ok());

        let level =
            LuminanceLevel::new(14).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = LuminanceCommand { value: level };
        assert!(cmd
            .try_into_vec(crate::camera_id::CameraId::CAMERA_1)
            .is_ok());

        // Test boundary values for contrast
        let level =
            ContrastLevel::new(0).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = ContrastCommand { value: level };
        assert!(cmd
            .try_into_vec(crate::camera_id::CameraId::CAMERA_1)
            .is_ok());

        let level =
            ContrastLevel::new(14).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = ContrastCommand { value: level };
        assert!(cmd
            .try_into_vec(crate::camera_id::CameraId::CAMERA_1)
            .is_ok());
    }

    #[test]
    fn test_nibble_encoding_sharpness() {
        // Test that SetLevel command properly encodes value as nibbles
        let cmd = Sharpness::SetLevel { value: 0x0B };
        let bytes = cmd
            .try_into_vec(crate::camera_id::CameraId::CAMERA_1)
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        assert_eq!(bytes[6], 0x00); // High nibble
        assert_eq!(bytes[7], 0x0B); // Low nibble

        let cmd = Sharpness::SetLevel { value: 0x05 };
        let bytes = cmd
            .try_into_vec(crate::camera_id::CameraId::CAMERA_1)
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
