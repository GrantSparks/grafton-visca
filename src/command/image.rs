//! Image adjustment commands for VISCA cameras.
//!
//! This module provides commands for controlling various image quality settings,
//! including brightness, contrast, sharpness, saturation, and hue adjustments.

// Crate imports
use crate::{
    command::const_encoding::CommandBuilder,
    error::Error,
    types::{NoiseReduction2DLevel, NoiseReduction3DLevel},
    visca_bool_command, visca_command,
};

visca_bool_command! {
    /// Backlight compensation command.
    ///
    /// Enables or disables backlight compensation, which helps properly expose
    /// subjects that are backlit (have a bright light source behind them).
    struct BacklightCommand {
        prefix: [0x81, 0x01, 0x04, 0x33],
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
            let cmd = CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::image::NOISE_REDUCTION_2D_PREFIX)
                .push(0x00)
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Set 2D noise reduction level.
        Level(level: NoiseReduction2DLevel) => {
            let cmd = CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::image::NOISE_REDUCTION_2D_PREFIX)
                .push(level.value())
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
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
            let cmd = CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::image::NOISE_REDUCTION_3D_PREFIX)
                .push(0x00)
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Set 3D noise reduction level.
        Level(level: NoiseReduction3DLevel) => {
            let cmd = CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::image::NOISE_REDUCTION_3D_PREFIX)
                .push(level.value())
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        }
    }
}

visca_bool_command! {
    /// Black and White Mode command.
    ///
    /// Switches the camera output between color and monochrome (black and white) modes.
    struct BlackWhiteCommand {
        prefix: [0x81, 0x01, 0x04, 0x01],
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

crate::visca_param_command! {
    /// Command to set the combined image flip mode.
    pub(crate) struct ImageFlipCombinedCommand {
        mode: ImageFlipMode,
    }
    prefix = [0x81, 0x01, 0x04, 0x61];
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
    pub fn new(mode: ImageFlipMode) -> Self {
        Self { mode }
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
    use crate::{timeout::CommandCategory, EncodeVisca};

    #[test]
    fn test_backlight_command() {
        // Test backlight on
        let cmd = BacklightCommand::new(true);
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x33, 0x02, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));

        // Test backlight off
        let cmd = BacklightCommand::new(false);
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x33, 0x03, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));
    }

    #[test]
    fn test_noise_reduction_2d_command() {
        // Test off
        let cmd = NoiseReduction2D::Off;
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x53, 0x00, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Custom));

        // Test valid levels (1-5)
        for level in 1..=5 {
            let nr_level = NoiseReduction2DLevel::new(level).unwrap();
            let cmd = NoiseReduction2D::Level(nr_level);
            assert_eq!(
                cmd.try_into_vec().unwrap(),
                vec![0x81, 0x01, 0x04, 0x53, level, 0xFF]
            );
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.timeout_kind(), CommandCategory::Custom));
        }
    }

    #[test]
    fn test_noise_reduction_3d_command() {
        // Test off
        let cmd = NoiseReduction3D::Off;
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x54, 0x00, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Custom));

        // Test valid levels (1-8)
        for level in 1..=8 {
            let nr_level = NoiseReduction3DLevel::new(level).unwrap();
            let cmd = NoiseReduction3D::Level(nr_level);
            assert_eq!(
                cmd.try_into_vec().unwrap(),
                vec![0x81, 0x01, 0x04, 0x54, level, 0xFF]
            );
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.timeout_kind(), CommandCategory::Custom));
        }
    }

    #[test]
    fn test_black_white_command() {
        // Test black and white on
        let cmd = BlackWhiteCommand::new(true);
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x01, 0x04, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));

        // Test black and white off (color mode)
        let cmd = BlackWhiteCommand::new(false);
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x01, 0x00, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Quick));
    }

    #[test]
    fn test_image_flip_combined_command() {
        // Test Off
        let cmd = ImageFlipCombinedCommand::new(ImageFlipMode::Off);
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x61, 0x00, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.timeout_kind(), CommandCategory::Custom));

        // Test Horizontal
        let cmd = ImageFlipCombinedCommand::new(ImageFlipMode::Horizontal);
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x61, 0x01, 0xFF]
        );

        // Test Vertical
        let cmd = ImageFlipCombinedCommand::new(ImageFlipMode::Vertical);
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x61, 0x02, 0xFF]
        );

        // Test Both
        let cmd = ImageFlipCombinedCommand::new(ImageFlipMode::Both);
        assert_eq!(
            cmd.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x61, 0x03, 0xFF]
        );
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
            let _ = format!("{:?}", cmd);
        }

        // Test Clone
        let backlight_cmd1 = BacklightCommand::new(true);
        let backlight_cmd2 = backlight_cmd1;
        // Verify commands produce same bytes
        assert_eq!(
            backlight_cmd1.try_into_vec().unwrap(),
            backlight_cmd2.try_into_vec().unwrap()
        );

        let flip_cmd1 = ImageFlipCombinedCommand::new(ImageFlipMode::Horizontal);
        let flip_cmd2 = flip_cmd1;
        // Verify the command was copied correctly
        assert_eq!(
            flip_cmd2.try_into_vec().unwrap(),
            vec![0x81, 0x01, 0x04, 0x61, 0x01, 0xFF]
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
        assert!(format!("{:?}", ImageFlipMode::Off).contains("Off"));
        assert!(format!("{:?}", ImageFlipMode::Horizontal).contains("Horizontal"));
        assert!(format!("{:?}", ImageFlipMode::Vertical).contains("Vertical"));
        assert!(format!("{:?}", ImageFlipMode::Both).contains("Both"));
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
        assert_eq!(cmd1.try_into_vec().unwrap(), cmd2.try_into_vec().unwrap());

        let level = NoiseReduction2DLevel::new(3).unwrap();
        let cmd1 = NoiseReduction2D::Level(level);
        let cmd2 = NoiseReduction2D::Level(level);
        assert_eq!(cmd1.try_into_vec().unwrap(), cmd2.try_into_vec().unwrap());
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
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("BacklightCommand"));
        // The debug output will show the field name
        assert!(debug_str.contains("enabled"));

        let cmd = BacklightCommand::new(false);
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("BacklightCommand"));
    }

    #[test]
    fn test_black_white_command_debug() {
        let cmd = BlackWhiteCommand::new(true);
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("BlackWhiteCommand"));
        // The debug output will show the field name
        assert!(debug_str.contains("enabled"));

        let cmd = BlackWhiteCommand::new(false);
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("BlackWhiteCommand"));
    }
}
