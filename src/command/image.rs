//! Image adjustment commands for VISCA cameras.
//!
//! This module provides commands for controlling various image quality settings,
//! including backlight compensation, noise reduction, image flip, picture effects,
//! brightness (luminance), contrast, gamma, and sharpness adjustments.

use grafton_visca_macros::ViscaEnum;

use std::borrow::Cow;

use crate::{
    command::{encode::ViscaCommand, resolution::PictureEffectMode, InquiryKind},
    error::Error,
    timeout::CommandCategory,
    types::{
        ContrastLevel, GammaLevel, LuminanceLevel, NoiseReduction2DLevel, NoiseReduction3DLevel,
    },
    visca_command,
};

/// Sharpness control modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum SharpnessMode {
    /// Automatic sharpness adjustment based on scene content.
    Auto = 0x02,
    /// Manual sharpness control.
    Manual = 0x03,
}

/// Noise reduction modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum NoiseReductionMode {
    /// Noise reduction disabled.
    Off = 0x02,
    /// Noise reduction enabled.
    On = 0x03,
}

/// Noise reduction speed settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum NoiseReductionSpeed {
    /// Slow noise reduction processing.
    Slow = 0x00,
    /// Normal noise reduction processing.
    Normal = 0x01,
    /// Fast noise reduction processing.
    Fast = 0x02,
}

/// Black and white mode settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
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

impl ViscaCommand for Sharpness {
    type Response = ();
    const MAX_SIZE: usize = 9;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Custom;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::bytes::{constants, ConstCommandBuilder};

        match self {
            Self::Mode(mode) => {
                let mode_byte = match mode {
                    SharpnessMode::Auto => 0x02,
                    SharpnessMode::Manual => 0x03,
                };
                let mut builder = ConstCommandBuilder::<6>::new();
                builder.append_mut(constants::image::SHARPNESS_MODE_PREFIX);
                builder.push_mut(mode_byte);
                builder
                    .with_camera_id(camera_id)
                    .terminate()
                    .build_into(buffer)
            }
            Self::Reset => {
                let mut builder = ConstCommandBuilder::<6>::new();
                builder.append_mut(constants::image::SHARPNESS_CONTROL_PREFIX);
                builder.push_mut(0x00);
                builder
                    .with_camera_id(camera_id)
                    .terminate()
                    .build_into(buffer)
            }
            Self::Up => {
                let mut builder = ConstCommandBuilder::<6>::new();
                builder.append_mut(constants::image::SHARPNESS_CONTROL_PREFIX);
                builder.push_mut(0x02);
                builder
                    .with_camera_id(camera_id)
                    .terminate()
                    .build_into(buffer)
            }
            Self::Down => {
                let mut builder = ConstCommandBuilder::<6>::new();
                builder.append_mut(constants::image::SHARPNESS_CONTROL_PREFIX);
                builder.push_mut(0x03);
                builder
                    .with_camera_id(camera_id)
                    .terminate()
                    .build_into(buffer)
            }
            Self::SetLevel { value } => {
                if *value > 11 {
                    return Err(Error::InvalidParameter {
                        parameter: "value",
                        value: Cow::Owned(value.to_string()),
                        reason: Cow::Borrowed("Sharpness value must be in the range 0..=11"),
                    });
                }
                let mut builder = ConstCommandBuilder::<9>::new();
                builder.append_mut(constants::image::SHARPNESS_LEVEL_PREFIX);
                builder.push_nibble_pair_mut(*value as u16);
                builder
                    .with_camera_id(camera_id)
                    .terminate()
                    .build_into(buffer)
            }
        }
    }

    fn response_kind(&self) -> Option<InquiryKind> {
        None
    }
}

visca_command! {
        /// Command to set the luminance (brightness) level.
    pub struct Luminance { value: LuminanceLevel };
    prefix = [0x01, 0x04, 0xA1, 0x00, 0x00, 0x00];
    param = value.value();
    max_param_size = 1;
    category = CommandCategory::Quick;
}

impl Luminance {
    /// Create a new luminance command.
    pub fn new(value: LuminanceLevel) -> Self {
        Self { value }
    }
}

visca_command! {
        /// Command to set the contrast level.
    pub struct Contrast { value: ContrastLevel };
    prefix = [0x01, 0x04, 0xA2, 0x00, 0x00, 0x00];
    param = value.value();
    max_param_size = 1;
    category = CommandCategory::Quick;
}

impl Contrast {
    /// Create a new contrast command.
    pub fn new(value: ContrastLevel) -> Self {
        Self { value }
    }
}

visca_command! {
        /// Command to set the gamma curve.
    ///
    /// Selects the gamma correction curve for the camera's image output.
    /// Gamma affects the overall brightness curve and tonal response.
    /// Value 0 is typically standard gamma, while values 1-4 select
    /// different gamma curves depending on the camera model.
    ///
    /// Use [`InquiryControl::gamma`] to query the current value.
    ///
    /// [`InquiryControl::gamma`]: crate::camera::controls::inquiry::InquiryControl::gamma
    pub struct GammaCommand { level: GammaLevel };
    prefix = [0x01, 0x04, 0x5B];
    param = level.value();
    max_param_size = 1;
    category = CommandCategory::Custom;
}

impl GammaCommand {
    /// Create a new gamma command.
    pub fn new(level: GammaLevel) -> Self {
        Self { level }
    }
}

visca_command! {
        /// Backlight compensation command.
    ///
    /// Enables or disables backlight compensation, which helps properly expose
    /// subjects that are backlit (have a bright light source behind them).
    pub struct BacklightCommand { enabled: bool };
    prefix = [0x01, 0x04, 0x33];
    param = if *enabled { 0x02 } else { 0x03 };
    max_param_size = 1;
    category = CommandCategory::Quick;
}

impl BacklightCommand {
    /// Create a new backlight compensation command.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

visca_command! {
        /// 2D Noise Reduction command.
    ///
    /// Reduces spatial noise in individual frames by analyzing and smoothing
    /// pixel variations. Higher levels provide more noise reduction but may
    /// reduce fine detail.
    pub struct NoiseReduction2D { level: Option<NoiseReduction2DLevel> };
    prefix = [0x01, 0x04, 0x53];
    param = match level { None => 0x00, Some(l) => l.value() };
    max_param_size = 1;
    category = CommandCategory::Custom;
}

impl NoiseReduction2D {
    /// Disable 2D noise reduction.
    pub const fn off() -> Self {
        Self { level: None }
    }

    /// Set 2D noise reduction to a specific level.
    pub const fn with_level(level: NoiseReduction2DLevel) -> Self {
        Self { level: Some(level) }
    }
}

visca_command! {
        /// 3D Noise Reduction command.
    ///
    /// Reduces temporal noise by analyzing multiple frames over time.
    /// This is effective for reducing noise in video streams while preserving
    /// motion detail. Higher levels provide more noise reduction.
    pub struct NoiseReduction3D { level: Option<NoiseReduction3DLevel> };
    prefix = [0x01, 0x04, 0x54];
    param = match level { None => 0x00, Some(l) => l.value() };
    max_param_size = 1;
    category = CommandCategory::Custom;
}

impl NoiseReduction3D {
    /// Disable 3D noise reduction.
    pub const fn off() -> Self {
        Self { level: None }
    }

    /// Set 3D noise reduction to a specific level.
    pub const fn with_level(level: NoiseReduction3DLevel) -> Self {
        Self { level: Some(level) }
    }
}

/// Combined Image Flip modes.
///
/// Allows flipping the image horizontally, vertically, or both.
/// Useful for when cameras are mounted upside down or need mirror effects.
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum ImageFlipMode {
    /// No image flipping.
    Off,
    /// Flip image horizontally (mirror).
    Horizontal,
    /// Flip image vertically (upside down).
    Vertical,
    /// Flip image both horizontally and vertically (180° rotation).
    Both,
}

visca_command! {
        /// Command to set the combined image flip mode.
    pub struct ImageFlipCombinedCommand { mode: ImageFlipMode };
    prefix = [0x01, 0x04, 0xA4];
    param = match mode {
        ImageFlipMode::Off => 0x00,
        ImageFlipMode::Horizontal => 0x01,
        ImageFlipMode::Vertical => 0x02,
        ImageFlipMode::Both => 0x03,
    };
    max_param_size = 1;
    category = CommandCategory::Custom;
}

impl ImageFlipCombinedCommand {
    /// Create a new image flip combined command.
    pub fn new(mode: ImageFlipMode) -> Self {
        Self { mode }
    }
}

visca_command! {
        /// Command to set picture effect mode.
    ///
    /// Controls various artistic effects like negative, sepia, sketch, etc.
    /// Note that not all effects are supported on all camera models.
    pub struct PictureEffectCommand { mode: PictureEffectMode };
    prefix = [0x01, 0x04, 0x63];
    param = mode.as_byte();
    max_param_size = 1;
    category = CommandCategory::Quick;
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::uninlined_format_args,
    clippy::panic,
    clippy::match_wildcard_for_single_variants
)]
mod tests {
    use super::*;
    use crate::{
        command::{bytes::VISCA_TERMINATOR, encode::ViscaCommand},
        macros::test_utils::visca_test,
        timeout::{CommandCategory, CommandTimeout},
    };

    visca_test!(
        BacklightCommand,
        test_backlight_on,
        BacklightCommand::new(true),
        &[0x81, 0x01, 0x04, 0x33, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        BacklightCommand,
        test_backlight_off,
        BacklightCommand::new(false),
        &[0x81, 0x01, 0x04, 0x33, 0x03, VISCA_TERMINATOR]
    );

    #[test]
    fn test_backlight_command_properties() {
        let cmd = BacklightCommand::new(true);
        assert!(cmd.response_kind().is_none());
        assert!(matches!(cmd.timeout_class(), CommandCategory::Quick));
    }

    visca_test!(
        NoiseReduction2D,
        test_noise_reduction_2d_off,
        NoiseReduction2D::off(),
        &[0x81, 0x01, 0x04, 0x53, 0x00, VISCA_TERMINATOR]
    );

    visca_test!(
        NoiseReduction2D,
        test_noise_reduction_2d_level_1,
        NoiseReduction2D::with_level(NoiseReduction2DLevel::new(1).unwrap()),
        &[0x81, 0x01, 0x04, 0x53, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        NoiseReduction2D,
        test_noise_reduction_2d_level_3,
        NoiseReduction2D::with_level(NoiseReduction2DLevel::new(3).unwrap()),
        &[0x81, 0x01, 0x04, 0x53, 0x03, VISCA_TERMINATOR]
    );

    visca_test!(
        NoiseReduction2D,
        test_noise_reduction_2d_level_5,
        NoiseReduction2D::with_level(NoiseReduction2DLevel::new(5).unwrap()),
        &[0x81, 0x01, 0x04, 0x53, 0x05, VISCA_TERMINATOR]
    );

    #[test]
    fn test_noise_reduction_2d_properties() {
        let cmd = NoiseReduction2D::off();
        assert!(cmd.response_kind().is_none());
        assert!(matches!(cmd.timeout_class(), CommandCategory::Custom));
    }

    visca_test!(
        NoiseReduction3D,
        test_noise_reduction_3d_off,
        NoiseReduction3D::off(),
        &[0x81, 0x01, 0x04, 0x54, 0x00, VISCA_TERMINATOR]
    );

    visca_test!(
        NoiseReduction3D,
        test_noise_reduction_3d_level_1,
        NoiseReduction3D::with_level(NoiseReduction3DLevel::new(1).unwrap()),
        &[0x81, 0x01, 0x04, 0x54, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        NoiseReduction3D,
        test_noise_reduction_3d_level_4,
        NoiseReduction3D::with_level(NoiseReduction3DLevel::new(4).unwrap()),
        &[0x81, 0x01, 0x04, 0x54, 0x04, VISCA_TERMINATOR]
    );

    visca_test!(
        NoiseReduction3D,
        test_noise_reduction_3d_level_8,
        NoiseReduction3D::with_level(NoiseReduction3DLevel::new(8).unwrap()),
        &[0x81, 0x01, 0x04, 0x54, 0x08, VISCA_TERMINATOR]
    );

    #[test]
    fn test_noise_reduction_3d_properties() {
        let cmd = NoiseReduction3D::off();
        assert!(cmd.response_kind().is_none());
        assert!(matches!(cmd.timeout_class(), CommandCategory::Custom));
    }

    visca_test!(
        ImageFlipCombinedCommand,
        test_image_flip_off,
        ImageFlipCombinedCommand::new(ImageFlipMode::Off),
        &[0x81, 0x01, 0x04, 0xA4, 0x00, VISCA_TERMINATOR]
    );

    visca_test!(
        ImageFlipCombinedCommand,
        test_image_flip_horizontal,
        ImageFlipCombinedCommand::new(ImageFlipMode::Horizontal),
        &[0x81, 0x01, 0x04, 0xA4, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        ImageFlipCombinedCommand,
        test_image_flip_vertical,
        ImageFlipCombinedCommand::new(ImageFlipMode::Vertical),
        &[0x81, 0x01, 0x04, 0xA4, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        ImageFlipCombinedCommand,
        test_image_flip_both,
        ImageFlipCombinedCommand::new(ImageFlipMode::Both),
        &[0x81, 0x01, 0x04, 0xA4, 0x03, VISCA_TERMINATOR]
    );

    #[test]
    fn test_image_flip_properties() {
        let cmd = ImageFlipCombinedCommand::new(ImageFlipMode::Off);
        assert!(cmd.response_kind().is_none());
        assert!(matches!(cmd.timeout_class(), CommandCategory::Custom));
    }

    #[test]
    fn test_command_traits() {
        // Test Debug trait
        let cmds: Vec<Box<dyn std::fmt::Debug>> = vec![
            Box::new(BacklightCommand::new(true)),
            Box::new(NoiseReduction2D::off()),
            Box::new(NoiseReduction3D::off()),
            Box::new(ImageFlipCombinedCommand::new(ImageFlipMode::Off)),
        ];

        for cmd in cmds {
            let _ = format!("{cmd:?}");
        }

        // Test Clone
        let backlight_cmd1 = BacklightCommand::new(true);
        let backlight_cmd2 = backlight_cmd1;
        // Verify commands produce same bytes
        assert_eq!(
            backlight_cmd1
                .to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap(),
            backlight_cmd2
                .to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap()
        );

        let flip_cmd1 = ImageFlipCombinedCommand::new(ImageFlipMode::Horizontal);
        let flip_cmd2 = flip_cmd1;
        // Verify the command was copied correctly
        assert_eq!(
            flip_cmd2
                .to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap(),
            vec![0x81, 0x01, 0x04, 0xA4, 0x01, VISCA_TERMINATOR]
        );
    }

    #[test]
    fn test_response_type_none() {
        // Verify all commands return None for response_type
        assert!(BacklightCommand::new(true).response_kind().is_none());
        assert!(NoiseReduction2D::off().response_kind().is_none());
        assert!(NoiseReduction3D::off().response_kind().is_none());
        assert!(ImageFlipCombinedCommand::new(ImageFlipMode::Off)
            .response_kind()
            .is_none());
    }

    #[test]
    fn test_image_flip_mode_debug() {
        let mode = ImageFlipMode::Off;
        assert!(format!("{mode:?}").contains("Off"));
        let mode = ImageFlipMode::Horizontal;
        assert!(format!("{mode:?}").contains("Horizontal"));
        let mode = ImageFlipMode::Vertical;
        assert!(format!("{mode:?}").contains("Vertical"));
        let mode = ImageFlipMode::Both;
        assert!(format!("{mode:?}").contains("Both"));
    }

    #[test]
    fn test_image_flip_mode_clone() {
        let mode1 = ImageFlipMode::Horizontal;
        let mode2 = mode1;
        match (mode1, mode2) {
            (ImageFlipMode::Horizontal, ImageFlipMode::Horizontal) => {}
            _ => panic!("Copy didn't preserve mode"),
        }
    }

    #[test]
    fn test_command_consistency() {
        // Test that creating commands with the same parameters produces identical bytes
        let cmd1 = BacklightCommand::new(true);
        let cmd2 = BacklightCommand::new(true);
        assert_eq!(
            cmd1.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap(),
            cmd2.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap()
        );

        let level = NoiseReduction2DLevel::new(3).unwrap();
        let cmd1 = NoiseReduction2D::with_level(level);
        let cmd2 = NoiseReduction2D::with_level(level);
        assert_eq!(
            cmd1.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap(),
            cmd2.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap()
        );
    }

    #[test]
    fn test_noise_reduction_2d_struct_creation() {
        // Test off creation
        let off_cmd = NoiseReduction2D::off();
        assert!(off_cmd.level.is_none());

        // Test with level creation
        let level = NoiseReduction2DLevel::new(3).unwrap();
        let level_cmd = NoiseReduction2D::with_level(level);
        assert_eq!(level_cmd.level.unwrap().value(), 3);
    }

    #[test]
    fn test_noise_reduction_3d_struct_creation() {
        // Test off creation
        let off_cmd = NoiseReduction3D::off();
        assert!(off_cmd.level.is_none());

        // Test with level creation
        let level = NoiseReduction3DLevel::new(5).unwrap();
        let level_cmd = NoiseReduction3D::with_level(level);
        assert_eq!(level_cmd.level.unwrap().value(), 5);
    }

    #[test]
    fn test_command_categories() {
        // Test that BacklightCommand and BlackWhiteCommand use Quick category
        let cmd = BacklightCommand::new(true);
        assert!(matches!(cmd.timeout_class(), CommandCategory::Quick));

        // Test that noise reduction and flip commands use Custom category
        let cmd = NoiseReduction2D::off();
        assert!(matches!(cmd.timeout_class(), CommandCategory::Custom));

        let cmd = NoiseReduction3D::off();
        assert!(matches!(cmd.timeout_class(), CommandCategory::Custom));

        let cmd = ImageFlipCombinedCommand::new(ImageFlipMode::Off);
        assert!(matches!(cmd.timeout_class(), CommandCategory::Custom));
    }

    #[test]
    fn test_backlight_command_debug() {
        let cmd = BacklightCommand::new(true);
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("BacklightCommand"));
        // The debug output will show the field name
        assert!(debug_str.contains("enabled"));

        let cmd = BacklightCommand::new(false);
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("BacklightCommand"));
    }

    visca_test!(
        PictureEffectCommand,
        test_picture_effect_off,
        PictureEffectCommand {
            mode: PictureEffectMode::Off
        },
        &[0x81, 0x01, 0x04, 0x63, 0x00, VISCA_TERMINATOR]
    );

    visca_test!(
        PictureEffectCommand,
        test_picture_effect_negative,
        PictureEffectCommand {
            mode: PictureEffectMode::Negative
        },
        &[0x81, 0x01, 0x04, 0x63, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        PictureEffectCommand,
        test_picture_effect_black_white,
        PictureEffectCommand {
            mode: PictureEffectMode::BlackAndWhite
        },
        &[0x81, 0x01, 0x04, 0x63, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        PictureEffectCommand,
        test_picture_effect_sepia,
        PictureEffectCommand {
            mode: PictureEffectMode::Sepia
        },
        &[0x81, 0x01, 0x04, 0x63, 0x03, VISCA_TERMINATOR]
    );

    visca_test!(
        PictureEffectCommand,
        test_picture_effect_sketch,
        PictureEffectCommand {
            mode: PictureEffectMode::Sketch
        },
        &[0x81, 0x01, 0x04, 0x63, 0x04, VISCA_TERMINATOR]
    );

    visca_test!(
        PictureEffectCommand,
        test_picture_effect_emboss,
        PictureEffectCommand {
            mode: PictureEffectMode::Emboss
        },
        &[0x81, 0x01, 0x04, 0x63, 0x05, VISCA_TERMINATOR]
    );

    visca_test!(
        PictureEffectCommand,
        test_picture_effect_mosaic,
        PictureEffectCommand {
            mode: PictureEffectMode::Mosaic
        },
        &[0x81, 0x01, 0x04, 0x63, 0x06, VISCA_TERMINATOR]
    );

    #[test]
    fn test_picture_effect_properties() {
        let cmd = PictureEffectCommand {
            mode: PictureEffectMode::Off,
        };
        assert!(cmd.response_kind().is_none());
        assert!(matches!(cmd.timeout_class(), CommandCategory::Quick));
    }

    visca_test!(
        Sharpness,
        test_sharpness_mode_auto,
        Sharpness::Mode(SharpnessMode::Auto),
        &[0x81, 0x01, 0x04, 0x05, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        Sharpness,
        test_sharpness_mode_manual,
        Sharpness::Mode(SharpnessMode::Manual),
        &[0x81, 0x01, 0x04, 0x05, 0x03, VISCA_TERMINATOR]
    );

    #[test]
    fn test_sharpness_properties() {
        let cmd = Sharpness::Mode(SharpnessMode::Auto);
        assert!(cmd.response_kind().is_none());
        assert!(matches!(cmd.timeout_class(), CommandCategory::Custom));
    }

    visca_test!(
        Sharpness,
        test_sharpness_reset,
        Sharpness::Reset,
        &[0x81, 0x01, 0x04, 0x02, 0x00, VISCA_TERMINATOR]
    );

    visca_test!(
        Sharpness,
        test_sharpness_up,
        Sharpness::Up,
        &[0x81, 0x01, 0x04, 0x02, 0x02, VISCA_TERMINATOR]
    );
    visca_test!(
        Sharpness,
        test_sharpness_down,
        Sharpness::Down,
        &[0x81, 0x01, 0x04, 0x02, 0x03, VISCA_TERMINATOR]
    );
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
    fn test_sharpness_valid_values() {
        use crate::types::SharpnessLevel;

        // Test valid values
        for value in 0..=11 {
            let _cmd = Sharpness::SetLevel { value };
        }

        // Test invalid value
        // SharpnessLevel enforces valid range 0-11, so we can't create value 12
        let result = SharpnessLevel::new(12);
        assert!(result.is_err());
    }

    visca_test!(
        Luminance,
        test_luminance_level_0,
        Luminance::new(LuminanceLevel::new(0).unwrap()),
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

    visca_test!(
        Luminance,
        test_luminance_level_7,
        Luminance::new(LuminanceLevel::new(7).unwrap()),
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

    visca_test!(
        Luminance,
        test_luminance_level_14,
        Luminance::new(LuminanceLevel::new(14).unwrap()),
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
        let cmd = Luminance::new(LuminanceLevel::new(7).unwrap());
        assert!(cmd.response_kind().is_none());
        assert!(matches!(cmd.timeout_class(), CommandCategory::Quick));
    }

    #[test]
    fn test_luminance_valid_values() {
        // Test valid values
        for value in 0..=14 {
            let level = LuminanceLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let _cmd = Luminance::new(level);
        }
    }

    visca_test!(
        Contrast,
        test_contrast_level_0,
        Contrast::new(ContrastLevel::new(0).unwrap()),
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

    visca_test!(
        Contrast,
        test_contrast_level_7,
        Contrast::new(ContrastLevel::new(7).unwrap()),
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

    visca_test!(
        Contrast,
        test_contrast_level_14,
        Contrast::new(ContrastLevel::new(14).unwrap()),
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
        let cmd = Contrast::new(ContrastLevel::new(7).unwrap());
        assert!(cmd.response_kind().is_none());
        assert!(matches!(cmd.timeout_class(), CommandCategory::Quick));
    }

    #[test]
    fn test_contrast_valid_values() {
        // Test valid values
        for value in 0..=14 {
            let level = ContrastLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let _cmd = Contrast::new(level);
        }
    }

    visca_test!(
        GammaCommand,
        test_gamma_level_0,
        GammaCommand::new(GammaLevel::new(0).unwrap()),
        &[0x81, 0x01, 0x04, 0x5B, 0x00, VISCA_TERMINATOR]
    );

    visca_test!(
        GammaCommand,
        test_gamma_level_2,
        GammaCommand::new(GammaLevel::new(2).unwrap()),
        &[0x81, 0x01, 0x04, 0x5B, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        GammaCommand,
        test_gamma_level_4,
        GammaCommand::new(GammaLevel::new(4).unwrap()),
        &[0x81, 0x01, 0x04, 0x5B, 0x04, VISCA_TERMINATOR]
    );

    #[test]
    fn test_gamma_properties() {
        let cmd = GammaCommand::new(GammaLevel::new(0).unwrap());
        assert!(cmd.response_kind().is_none());
        assert!(matches!(cmd.timeout_class(), CommandCategory::Custom));
    }

    #[test]
    fn test_gamma_valid_values() {
        for value in 0..=4 {
            let level =
                GammaLevel::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let _cmd = GammaCommand::new(level);
        }
    }

    #[test]
    fn test_sharpness_mode_equality() {
        assert_eq!(SharpnessMode::Auto, SharpnessMode::Auto);
        assert_eq!(SharpnessMode::Manual, SharpnessMode::Manual);
        assert_ne!(SharpnessMode::Auto, SharpnessMode::Manual);
    }

    #[test]
    fn test_additional_command_traits() {
        // Test Debug trait
        let cmds: Vec<Box<dyn std::fmt::Debug>> = vec![
            Box::new(Sharpness::Reset),
            Box::new(Sharpness::Mode(SharpnessMode::Auto)),
            Box::new(Sharpness::SetLevel { value: 5 }),
            Box::new(Luminance::new(
                LuminanceLevel::new(7).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            )),
            Box::new(Contrast::new(
                ContrastLevel::new(7).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            )),
            Box::new(GammaCommand::new(
                GammaLevel::new(2).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
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
    fn test_edge_cases_extended() {
        // Test boundary values for sharpness
        let cmd = Sharpness::SetLevel { value: 0 };
        assert!(cmd
            .to_bytes(crate::camera_id::CameraId::CAMERA_1)
            .map(|b| b.to_vec())
            .is_ok());

        let cmd = Sharpness::SetLevel { value: 11 };
        assert!(cmd
            .to_bytes(crate::camera_id::CameraId::CAMERA_1)
            .map(|b| b.to_vec())
            .is_ok());

        // Test boundary values for luminance
        let level =
            LuminanceLevel::new(0).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = Luminance { value: level };
        assert!(cmd
            .to_bytes(crate::camera_id::CameraId::CAMERA_1)
            .map(|b| b.to_vec())
            .is_ok());

        let level =
            LuminanceLevel::new(14).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = Luminance { value: level };
        assert!(cmd
            .to_bytes(crate::camera_id::CameraId::CAMERA_1)
            .map(|b| b.to_vec())
            .is_ok());

        // Test boundary values for contrast
        let level =
            ContrastLevel::new(0).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = Contrast { value: level };
        assert!(cmd
            .to_bytes(crate::camera_id::CameraId::CAMERA_1)
            .map(|b| b.to_vec())
            .is_ok());

        let level =
            ContrastLevel::new(14).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = Contrast { value: level };
        assert!(cmd
            .to_bytes(crate::camera_id::CameraId::CAMERA_1)
            .map(|b| b.to_vec())
            .is_ok());
    }

    #[test]
    fn test_nibble_encoding_sharpness() {
        // Test that SetLevel command properly encodes value as nibbles
        let cmd = Sharpness::SetLevel { value: 0x0B };
        let bytes = cmd
            .to_bytes(crate::camera_id::CameraId::CAMERA_1)
            .map(|b| b.to_vec())
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        assert_eq!(bytes[6], 0x00); // High nibble
        assert_eq!(bytes[7], 0x0B); // Low nibble

        let cmd = Sharpness::SetLevel { value: 0x05 };
        let bytes = cmd
            .to_bytes(crate::camera_id::CameraId::CAMERA_1)
            .map(|b| b.to_vec())
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        assert_eq!(bytes[6], 0x00); // High nibble
        assert_eq!(bytes[7], 0x05); // Low nibble
    }

    #[test]
    fn test_response_type_none_extended() {
        // Verify all commands return None for response_type
        assert!(Sharpness::Reset.response_kind().is_none());
        assert!(Sharpness::Mode(SharpnessMode::Auto)
            .response_kind()
            .is_none());
        assert!(Sharpness::Up.response_kind().is_none());
        assert!(Sharpness::Down.response_kind().is_none());
        assert!(Sharpness::SetLevel { value: 5 }.response_kind().is_none());
        assert!(Luminance::new(
            LuminanceLevel::new(7).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        )
        .response_kind()
        .is_none());
        assert!(Contrast::new(
            ContrastLevel::new(7).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        )
        .response_kind()
        .is_none());
        assert!(GammaCommand::new(
            GammaLevel::new(2).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        )
        .response_kind()
        .is_none());
    }

    #[test]
    fn test_command_categories_extended() {
        // Test Sharpness uses Custom category
        let sharpness_cmds = vec![
            Sharpness::Reset,
            Sharpness::Mode(SharpnessMode::Auto),
            Sharpness::Up,
            Sharpness::Down,
            Sharpness::SetLevel { value: 5 },
        ];

        for cmd in sharpness_cmds {
            assert!(matches!(cmd.timeout_class(), CommandCategory::Custom));
        }

        // Test Luminance and Contrast use Quick category
        let level =
            LuminanceLevel::new(7).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = Luminance { value: level };
        assert!(matches!(cmd.timeout_class(), CommandCategory::Quick));

        let level =
            ContrastLevel::new(7).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = Contrast { value: level };
        assert!(matches!(cmd.timeout_class(), CommandCategory::Quick));

        // Test GammaCommand uses Custom category
        let level = GammaLevel::new(2).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = GammaCommand { level };
        assert!(matches!(cmd.timeout_class(), CommandCategory::Custom));
    }
}
