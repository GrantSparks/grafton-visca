//! Image adjustment commands for VISCA cameras.
//!
//! This module provides commands for controlling various image quality settings,
//! including brightness, contrast, sharpness, saturation, and hue adjustments.

// Crate imports
use crate::macros::internal::*;

use crate::{
    command::const_encoding::constants,
    types::{NoiseReduction2DLevel, NoiseReduction3DLevel},
};

visca_bool_command! {
    /// Backlight compensation command.
    ///
    /// Enables or disables backlight compensation, which helps properly expose
    /// subjects that are backlit (have a bright light source behind them).
    struct BacklightCommand {
        prefix: constants::image::BACKLIGHT_PREFIX,
        on: 0x02,
        off: 0x03,
    }
}

visca_command! {
    /// 2D Noise Reduction command.
    ///
    /// Reduces spatial noise in individual frames by analyzing and smoothing
    /// pixel variations. Higher levels provide more noise reduction but may
    /// reduce fine detail.
    category = "Custom",
    enum NoiseReduction2D {
        /// Disable 2D noise reduction.
        Off => {
            Ok(CommandBuilder::<16>::new()
                .append(constants::image::NOISE_REDUCTION_2D_PREFIX)
                .push(0x00))
        },
        /// Set 2D noise reduction level.
        Level(level: NoiseReduction2DLevel) => {
            Ok(CommandBuilder::<16>::new()
                .append(constants::image::NOISE_REDUCTION_2D_PREFIX)
                .push(level.value()))
        }
    }
}

visca_command! {
    /// 3D Noise Reduction command.
    ///
    /// Reduces temporal noise by analyzing multiple frames over time.
    /// This is effective for reducing noise in video streams while preserving
    /// motion detail. Higher levels provide more noise reduction.
    category = "Custom",
    enum NoiseReduction3D {
        /// Disable 3D noise reduction.
        Off => {
            Ok(CommandBuilder::<16>::new()
                .append(constants::image::NOISE_REDUCTION_3D_PREFIX)
                .push(0x00))
        },
        /// Set 3D noise reduction level.
        Level(level: NoiseReduction3DLevel) => {
            Ok(CommandBuilder::<16>::new()
                .append(constants::image::NOISE_REDUCTION_3D_PREFIX)
                .push(level.value()))
        }
    }
}

visca_bool_command! {
    /// Black and White Mode command.
    ///
    /// Switches the camera output between color and monochrome (black and white) modes.
    struct BlackWhiteCommand {
        prefix: constants::image::BLACK_WHITE_PREFIX,
        on: 0x04,
        off: 0x00,
    }
}

/// Combined Image Flip modes.
///
/// Allows flipping the image horizontally, vertically, or both.
/// Useful for when cameras are mounted upside down or need mirror effects.
#[derive(Debug, Copy, Clone)]
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

visca_param_command! {
    /// Command to set the combined image flip mode.
    ///
    /// TODO: Connect to camera API for image flip functionality
    #[allow(dead_code)]
    pub(crate) struct ImageFlipCombinedCommand {
        mode: ImageFlipMode,
    }
    prefix = constants::image::FLIP_COMBINED_PREFIX;
    param_byte = match mode {
        ImageFlipMode::Off => 0x00,
        ImageFlipMode::Horizontal => 0x01,
        ImageFlipMode::Vertical => 0x02,
        ImageFlipMode::Both => 0x03,
    };
    timeout = Custom;
}

impl ImageFlipCombinedCommand {
    /// Create a new image flip combined command.
    #[allow(dead_code)]
    pub fn new(mode: ImageFlipMode) -> Self {
        Self { mode }
    }
}

// Import PictureEffectMode from resolution module
use crate::command::resolution::PictureEffectMode;

visca_param_command! {
    /// Command to set picture effect mode.
    ///
    /// Controls various artistic effects like negative, sepia, sketch, etc.
    /// Note that not all effects are supported on all camera models.
    ///
    /// TODO: Connect to camera API for picture effects functionality
    #[allow(dead_code)]
    pub(crate) struct PictureEffectCommand {
        mode: PictureEffectMode,
    }
    prefix = constants::image::PICTURE_EFFECT_PREFIX;
    param_byte = mode.to_byte();
    timeout = Quick;
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
    use crate::command::const_encoding::VISCA_TERMINATOR;
    use crate::command::encode_visca::EncodeVisca;
    use crate::macros::test_utils::visca_test;
    use crate::timeout::CommandCategory;

    // Test backlight on
    visca_test!(
        BacklightCommand,
        test_backlight_on,
        BacklightCommand::new(true),
        &[0x81, 0x01, 0x04, 0x33, 0x02, VISCA_TERMINATOR]
    );

    // Test backlight off
    visca_test!(
        BacklightCommand,
        test_backlight_off,
        BacklightCommand::new(false),
        &[0x81, 0x01, 0x04, 0x33, 0x03, VISCA_TERMINATOR]
    );

    #[test]
    fn test_backlight_command_properties() {
        let cmd = BacklightCommand::new(true);
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));
    }

    // Test off
    visca_test!(
        NoiseReduction2D,
        test_noise_reduction_2d_off,
        NoiseReduction2D::Off,
        &[0x81, 0x01, 0x04, 0x53, 0x00, VISCA_TERMINATOR]
    );

    // Test level 1
    visca_test!(
        NoiseReduction2D,
        test_noise_reduction_2d_level_1,
        NoiseReduction2D::Level(NoiseReduction2DLevel::new(1).unwrap()),
        &[0x81, 0x01, 0x04, 0x53, 0x01, VISCA_TERMINATOR]
    );

    // Test level 3
    visca_test!(
        NoiseReduction2D,
        test_noise_reduction_2d_level_3,
        NoiseReduction2D::Level(NoiseReduction2DLevel::new(3).unwrap()),
        &[0x81, 0x01, 0x04, 0x53, 0x03, VISCA_TERMINATOR]
    );

    // Test level 5
    visca_test!(
        NoiseReduction2D,
        test_noise_reduction_2d_level_5,
        NoiseReduction2D::Level(NoiseReduction2DLevel::new(5).unwrap()),
        &[0x81, 0x01, 0x04, 0x53, 0x05, VISCA_TERMINATOR]
    );

    #[test]
    fn test_noise_reduction_2d_properties() {
        let cmd = NoiseReduction2D::Off;
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Custom));
    }

    // Test off
    visca_test!(
        NoiseReduction3D,
        test_noise_reduction_3d_off,
        NoiseReduction3D::Off,
        &[0x81, 0x01, 0x04, 0x54, 0x00, VISCA_TERMINATOR]
    );

    // Test level 1
    visca_test!(
        NoiseReduction3D,
        test_noise_reduction_3d_level_1,
        NoiseReduction3D::Level(NoiseReduction3DLevel::new(1).unwrap()),
        &[0x81, 0x01, 0x04, 0x54, 0x01, VISCA_TERMINATOR]
    );

    // Test level 4
    visca_test!(
        NoiseReduction3D,
        test_noise_reduction_3d_level_4,
        NoiseReduction3D::Level(NoiseReduction3DLevel::new(4).unwrap()),
        &[0x81, 0x01, 0x04, 0x54, 0x04, VISCA_TERMINATOR]
    );

    // Test level 8
    visca_test!(
        NoiseReduction3D,
        test_noise_reduction_3d_level_8,
        NoiseReduction3D::Level(NoiseReduction3DLevel::new(8).unwrap()),
        &[0x81, 0x01, 0x04, 0x54, 0x08, VISCA_TERMINATOR]
    );

    #[test]
    fn test_noise_reduction_3d_properties() {
        let cmd = NoiseReduction3D::Off;
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Custom));
    }

    // Test black and white on
    visca_test!(
        BlackWhiteCommand,
        test_black_white_on,
        BlackWhiteCommand::new(true),
        &[0x81, 0x01, 0x04, 0x01, 0x04, VISCA_TERMINATOR]
    );

    // Test black and white off (color mode)
    visca_test!(
        BlackWhiteCommand,
        test_black_white_off,
        BlackWhiteCommand::new(false),
        &[0x81, 0x01, 0x04, 0x01, 0x00, VISCA_TERMINATOR]
    );

    #[test]
    fn test_black_white_properties() {
        let cmd = BlackWhiteCommand::new(true);
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));
    }

    // Test Off
    visca_test!(
        ImageFlipCombinedCommand,
        test_image_flip_off,
        ImageFlipCombinedCommand::new(ImageFlipMode::Off),
        &[0x81, 0x01, 0x04, 0x61, 0x00, VISCA_TERMINATOR]
    );

    // Test Horizontal
    visca_test!(
        ImageFlipCombinedCommand,
        test_image_flip_horizontal,
        ImageFlipCombinedCommand::new(ImageFlipMode::Horizontal),
        &[0x81, 0x01, 0x04, 0x61, 0x01, VISCA_TERMINATOR]
    );

    // Test Vertical
    visca_test!(
        ImageFlipCombinedCommand,
        test_image_flip_vertical,
        ImageFlipCombinedCommand::new(ImageFlipMode::Vertical),
        &[0x81, 0x01, 0x04, 0x61, 0x02, VISCA_TERMINATOR]
    );

    // Test Both
    visca_test!(
        ImageFlipCombinedCommand,
        test_image_flip_both,
        ImageFlipCombinedCommand::new(ImageFlipMode::Both),
        &[0x81, 0x01, 0x04, 0x61, 0x03, VISCA_TERMINATOR]
    );

    #[test]
    fn test_image_flip_properties() {
        let cmd = ImageFlipCombinedCommand::new(ImageFlipMode::Off);
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Custom));
    }

    #[test]
    fn test_command_traits() {
        // Test Debug trait
        let cmds: Vec<Box<dyn std::fmt::Debug>> = vec![
            Box::new(BacklightCommand::new(true)),
            Box::new(NoiseReduction2D::Off),
            Box::new(NoiseReduction3D::Off),
            Box::new(BlackWhiteCommand::new(true)),
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
                .try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap(),
            backlight_cmd2
                .try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap()
        );

        let flip_cmd1 = ImageFlipCombinedCommand::new(ImageFlipMode::Horizontal);
        let flip_cmd2 = flip_cmd1;
        // Verify the command was copied correctly
        assert_eq!(
            flip_cmd2
                .try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap(),
            vec![0x81, 0x01, 0x04, 0x61, 0x01, VISCA_TERMINATOR]
        );
    }

    #[test]
    fn test_response_type_none() {
        // Verify all commands return None for response_type
        assert!(BacklightCommand::new(true).response_type().is_none());
        assert!(NoiseReduction2D::Off.response_type().is_none());
        assert!(NoiseReduction3D::Off.response_type().is_none());
        assert!(BlackWhiteCommand::new(true).response_type().is_none());
        assert!(ImageFlipCombinedCommand::new(ImageFlipMode::Off)
            .response_type()
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
            cmd1.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap(),
            cmd2.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap()
        );

        let level = NoiseReduction2DLevel::new(3).unwrap();
        let cmd1 = NoiseReduction2D::Level(level);
        let cmd2 = NoiseReduction2D::Level(level);
        assert_eq!(
            cmd1.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap(),
            cmd2.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap()
        );
    }

    #[test]
    fn test_noise_reduction_2d_enum_variants() {
        // Test that enum variants work correctly
        match NoiseReduction2D::Off {
            NoiseReduction2D::Off => {}
            NoiseReduction2D::Level(_) => panic!("Expected Off variant"),
        }

        let level = NoiseReduction2DLevel::new(3).unwrap();
        match NoiseReduction2D::Level(level) {
            NoiseReduction2D::Level(l) => assert_eq!(l.value(), 3),
            NoiseReduction2D::Off => panic!("Expected Level variant"),
        }
    }

    #[test]
    fn test_noise_reduction_3d_enum_variants() {
        // Test that enum variants work correctly
        match NoiseReduction3D::Off {
            NoiseReduction3D::Off => {}
            NoiseReduction3D::Level(_) => panic!("Expected Off variant"),
        }

        let level = NoiseReduction3DLevel::new(5).unwrap();
        match NoiseReduction3D::Level(level) {
            NoiseReduction3D::Level(l) => assert_eq!(l.value(), 5),
            NoiseReduction3D::Off => panic!("Expected Level variant"),
        }
    }

    #[test]
    fn test_command_categories() {
        use crate::command::encode_visca::EncodeVisca;

        // Test that BacklightCommand and BlackWhiteCommand use Quick category
        let cmd = BacklightCommand::new(true);
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));

        let cmd = BlackWhiteCommand::new(true);
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));

        // Test that noise reduction and flip commands use Custom category
        let cmd = NoiseReduction2D::Off;
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Custom));

        let cmd = NoiseReduction3D::Off;
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Custom));

        let cmd = ImageFlipCombinedCommand::new(ImageFlipMode::Off);
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Custom));
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

    #[test]
    fn test_black_white_command_debug() {
        let cmd = BlackWhiteCommand::new(true);
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("BlackWhiteCommand"));
        // The debug output will show the field name
        assert!(debug_str.contains("enabled"));

        let cmd = BlackWhiteCommand::new(false);
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("BlackWhiteCommand"));
    }

    // Test Off (normal) mode
    visca_test!(
        PictureEffectCommand,
        test_picture_effect_off,
        PictureEffectCommand {
            mode: PictureEffectMode::Off
        },
        &[0x81, 0x01, 0x04, 0x63, 0x00, VISCA_TERMINATOR]
    );

    // Test Negative effect
    visca_test!(
        PictureEffectCommand,
        test_picture_effect_negative,
        PictureEffectCommand {
            mode: PictureEffectMode::Negative
        },
        &[0x81, 0x01, 0x04, 0x63, 0x01, VISCA_TERMINATOR]
    );

    // Test Black and White effect
    visca_test!(
        PictureEffectCommand,
        test_picture_effect_black_white,
        PictureEffectCommand {
            mode: PictureEffectMode::BlackAndWhite
        },
        &[0x81, 0x01, 0x04, 0x63, 0x02, VISCA_TERMINATOR]
    );

    // Test Sepia effect
    visca_test!(
        PictureEffectCommand,
        test_picture_effect_sepia,
        PictureEffectCommand {
            mode: PictureEffectMode::Sepia
        },
        &[0x81, 0x01, 0x04, 0x63, 0x03, VISCA_TERMINATOR]
    );

    // Test Sketch effect
    visca_test!(
        PictureEffectCommand,
        test_picture_effect_sketch,
        PictureEffectCommand {
            mode: PictureEffectMode::Sketch
        },
        &[0x81, 0x01, 0x04, 0x63, 0x04, VISCA_TERMINATOR]
    );

    // Test Emboss effect
    visca_test!(
        PictureEffectCommand,
        test_picture_effect_emboss,
        PictureEffectCommand {
            mode: PictureEffectMode::Emboss
        },
        &[0x81, 0x01, 0x04, 0x63, 0x05, VISCA_TERMINATOR]
    );

    // Test Mosaic effect
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
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));
    }
}
