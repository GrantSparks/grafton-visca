//! Image adjustment commands for VISCA cameras.
//!
//! This module provides commands for controlling various image quality settings,
//! including backlight compensation, noise reduction, image flip, picture effects,
//! brightness (luminance), contrast, gamma, and sharpness adjustments.

use grafton_visca_macros::ViscaEnum;

use std::borrow::Cow;

use crate::{
    command::{encode::WireEncode, resolution::PictureEffectMode},
    error::Error,
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

/// 2D noise reduction modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum NoiseReduction2DMode {
    /// The camera automatically selects the 2D noise-reduction level.
    Auto = 0x02,
    /// The camera uses its manual 2D noise-reduction setting.
    Manual = 0x03,
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
    /// Set sharpness to specific value (0-15).
    SetLevel {
        /// Sharpness value (0 = minimum, 15 = maximum).
        value: u8,
    },
}

impl WireEncode for Sharpness {
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
                if *value > 15 {
                    return Err(Error::InvalidParameter {
                        parameter: "value",
                        value: Cow::Owned(value.to_string()),
                        reason: Cow::Borrowed("Sharpness value must be in the range 0..=15"),
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
}

visca_command! {
        /// Command to set the luminance (brightness) level.
    pub struct Luminance { value: LuminanceLevel };
    prefix = [0x01, 0x04, 0xA1, 0x00, 0x00, 0x00];
    param = value.value();
    max_param_size = 1;
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
    /// Use the image accessor's `gamma` inquiry to query the current value.
    pub struct GammaCommand { level: GammaLevel };
    prefix = [0x01, 0x04, 0x5B];
    param = level.value();
    max_param_size = 1;
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
}

impl BacklightCommand {
    /// Create a new backlight compensation command.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

visca_command! {
    /// Command to select the 2D noise-reduction mode.
    pub struct NoiseReduction2DModeCommand { mode: NoiseReduction2DMode };
    prefix = [0x01, 0x04, 0x50];
    param = *mode as u8;
    max_param_size = 1;
}

impl NoiseReduction2DModeCommand {
    /// Creates a 2D noise-reduction mode command.
    #[must_use]
    pub const fn new(mode: NoiseReduction2DMode) -> Self {
        Self { mode }
    }
}

visca_command! {
    /// Command to set or disable 2D noise reduction.
    pub struct NoiseReduction2D { level: Option<NoiseReduction2DLevel> };
    prefix = [0x01, 0x04, 0x53];
    param = match level { None => 0x00, Some(level) => level.value() };
    max_param_size = 1;
}

impl NoiseReduction2D {
    /// Disables 2D noise reduction.
    #[must_use]
    pub const fn off() -> Self {
        Self { level: None }
    }

    /// Sets 2D noise reduction to a validated level.
    ///
    /// Level zero is the protocol's Off value and is intentionally valid.
    #[must_use]
    pub const fn with_level(level: NoiseReduction2DLevel) -> Self {
        Self { level: Some(level) }
    }
}

visca_command! {
    /// Command to set or disable 3D noise reduction.
    pub struct NoiseReduction3D { level: Option<NoiseReduction3DLevel> };
    prefix = [0x01, 0x04, 0x54];
    param = match level { None => 0x00, Some(level) => level.value() };
    max_param_size = 1;
}

impl NoiseReduction3D {
    /// Disables 3D noise reduction.
    #[must_use]
    pub const fn off() -> Self {
        Self { level: None }
    }

    /// Sets 3D noise reduction to a validated level.
    ///
    /// Level zero is the protocol's Off value and is intentionally valid.
    #[must_use]
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
}

impl ImageFlipCombinedCommand {
    /// Create a new image flip combined command.
    pub fn new(mode: ImageFlipMode) -> Self {
        Self { mode }
    }
}

/// Command to set picture effect mode.
///
/// The built-in reference validates the standard off command and PTZOptics
/// black-and-white command. Use `PictureEffectMode::Unknown(value)` for raw
/// model-specific picture-effect values outside that source-backed surface.
#[derive(Debug, Copy, Clone)]
pub struct PictureEffectCommand {
    /// Picture-effect mode to send.
    pub mode: PictureEffectMode,
}

impl PictureEffectCommand {
    /// Create a command that sets the picture-effect mode.
    pub const fn new(mode: PictureEffectMode) -> Self {
        Self { mode }
    }
}

impl WireEncode for PictureEffectCommand {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::bytes::ConstCommandBuilder;

        let effect = self.mode.as_byte();

        let mut builder = ConstCommandBuilder::<6>::new();
        builder.push_mut(camera_id.to_address_byte());
        builder.append_mut(&[0x01, 0x04, 0x63]);
        builder.push_mut(effect);
        builder.terminate().build_into(buffer)
    }
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
    use crate::{command::bytes::VISCA_TERMINATOR, macros::test_utils::visca_test};

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
    fn test_command_traits() {
        // Test Debug trait
        let cmds: Vec<Box<dyn std::fmt::Debug>> = vec![
            Box::new(BacklightCommand::new(true)),
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
            crate::command::test_wire_bytes(&backlight_cmd1, crate::camera_id::CameraId::CAMERA_1)
                .unwrap(),
            crate::command::test_wire_bytes(&backlight_cmd2, crate::camera_id::CameraId::CAMERA_1)
                .unwrap()
        );

        let flip_cmd1 = ImageFlipCombinedCommand::new(ImageFlipMode::Horizontal);
        let flip_cmd2 = flip_cmd1;
        // Verify the command was copied correctly
        assert_eq!(
            crate::command::test_wire_bytes(&flip_cmd2, crate::camera_id::CameraId::CAMERA_1)
                .unwrap(),
            vec![0x81, 0x01, 0x04, 0xA4, 0x01, VISCA_TERMINATOR]
        );
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
            crate::command::test_wire_bytes(&cmd1, crate::camera_id::CameraId::CAMERA_1).unwrap(),
            crate::command::test_wire_bytes(&cmd2, crate::camera_id::CameraId::CAMERA_1).unwrap()
        );
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
        test_picture_effect_black_white,
        PictureEffectCommand {
            mode: PictureEffectMode::BlackAndWhite
        },
        &[0x81, 0x01, 0x04, 0x63, 0x04, VISCA_TERMINATOR]
    );

    visca_test!(
        PictureEffectCommand,
        test_picture_effect_unknown_raw_value,
        PictureEffectCommand {
            mode: PictureEffectMode::Unknown(0x05)
        },
        &[0x81, 0x01, 0x04, 0x63, 0x05, VISCA_TERMINATOR]
    );

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
    visca_test!(
        Sharpness,
        test_sharpness_level_15,
        Sharpness::SetLevel { value: 15 },
        &[
            0x81,
            0x01,
            0x04,
            0x42,
            0x00,
            0x00,
            0x00,
            0x0F,
            VISCA_TERMINATOR
        ]
    );

    #[test]
    fn test_sharpness_valid_values() {
        use crate::types::SharpnessLevel;

        // Test valid values (hardware-validated: PTZOptics G2 accepts 0-15)
        for value in 0..=15 {
            let _cmd = Sharpness::SetLevel { value };
        }

        // Test invalid value
        // SharpnessLevel enforces valid range 0-15, so we can't create value 16
        let result = SharpnessLevel::new(16);
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
        assert!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).is_ok()
        );

        let cmd = Sharpness::SetLevel { value: 15 };
        assert!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).is_ok()
        );

        // Test boundary values for luminance
        let level =
            LuminanceLevel::new(0).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = Luminance { value: level };
        assert!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).is_ok()
        );

        let level =
            LuminanceLevel::new(14).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = Luminance { value: level };
        assert!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).is_ok()
        );

        // Test boundary values for contrast
        let level =
            ContrastLevel::new(0).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = Contrast { value: level };
        assert!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).is_ok()
        );

        let level =
            ContrastLevel::new(14).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        let cmd = Contrast { value: level };
        assert!(
            crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1).is_ok()
        );
    }

    #[test]
    fn test_nibble_encoding_sharpness() {
        // Test that SetLevel command properly encodes value as nibbles
        let cmd = Sharpness::SetLevel { value: 0x0B };
        let bytes = crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1)
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        assert_eq!(bytes[6], 0x00); // High nibble
        assert_eq!(bytes[7], 0x0B); // Low nibble

        let cmd = Sharpness::SetLevel { value: 0x05 };
        let bytes = crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1)
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        assert_eq!(bytes[6], 0x00); // High nibble
        assert_eq!(bytes[7], 0x05); // Low nibble

        let cmd = Sharpness::SetLevel { value: 0x0F };
        let bytes = crate::command::test_wire_bytes(&cmd, crate::camera_id::CameraId::CAMERA_1)
            .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
        assert_eq!(bytes[6], 0x00);
        assert_eq!(bytes[7], 0x0F);
    }
}
