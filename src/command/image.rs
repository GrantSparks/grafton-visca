//! Image adjustment commands for VISCA cameras.
//!
//! This module provides commands for controlling various image quality settings,
//! including brightness, contrast, sharpness, saturation, and hue adjustments.

// Crate imports
use crate::{
    command::{Command, ResponseType},
    error::Error,
    timeout::CommandCategory,
    types::{NoiseReduction2DLevel, NoiseReduction3DLevel},
};

// Use the visca_bool_command! macro for BacklightCommand
crate::visca_bool_command! {
    /// Backlight compensation command.
    ///
    /// Enables or disables backlight compensation, which helps properly expose
    /// subjects that are backlit (have a bright light source behind them).
    struct BacklightCommand {
        /// Enable (true) or disable (false) backlight compensation.
        status: bool => |v| if v { 0x02 } else { 0x03 }
    }
    bytes = [0x81, 0x01, 0x04, 0x33, {status}, 0xFF]
}

// Use the visca_command! macro for NoiseReduction2DCommand
crate::visca_command! {
    /// 2D Noise Reduction command.
    ///
    /// Reduces spatial noise in individual frames by analyzing and smoothing
    /// pixel variations. Higher levels provide more noise reduction but may
    /// reduce fine detail.
    category = "Custom",
    enum NoiseReduction2DCommand {
        /// Disable 2D noise reduction.
        Off => [0x81, 0x01, 0x04, 0x53, 0x00, 0xFF],
        /// Set 2D noise reduction level.
        Level(level: NoiseReduction2DLevel) => {
            Ok(vec![0x81, 0x01, 0x04, 0x53, level.value(), 0xFF])
        }
    }
}

// Use the visca_command! macro for NoiseReduction3DCommand
crate::visca_command! {
    /// 3D Noise Reduction command.
    ///
    /// Reduces temporal noise by analyzing multiple frames over time.
    /// This is effective for reducing noise in video streams while preserving
    /// motion detail. Higher levels provide more noise reduction.
    category = "Custom",
    enum NoiseReduction3DCommand {
        /// Disable 3D noise reduction.
        Off => [0x81, 0x01, 0x04, 0x54, 0x00, 0xFF],
        /// Set 3D noise reduction level.
        Level(level: NoiseReduction3DLevel) => {
            Ok(vec![0x81, 0x01, 0x04, 0x54, level.value(), 0xFF])
        }
    }
}

// Use the visca_bool_command! macro for BlackWhiteCommand
crate::visca_bool_command! {
    /// Black and White Mode command.
    ///
    /// Switches the camera output between color and monochrome (black and white) modes.
    struct BlackWhiteCommand {
        /// Enable (true) for black and white mode, disable (false) for color mode.
        on: bool => |v| if v { 0x04 } else { 0x00 }
    }
    bytes = [0x81, 0x01, 0x04, 0x01, {on}, 0xFF]
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

/// Command to set the combined image flip mode.
#[derive(Debug, Copy, Clone)]
pub struct ImageFlipCombinedCommand {
    /// The flip mode to apply.
    pub mode: ImageFlipMode,
}

impl Command for ImageFlipCombinedCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mode_byte = match self.mode {
            ImageFlipMode::Off => 0x00,
            ImageFlipMode::Horizontal => 0x01,
            ImageFlipMode::Vertical => 0x02,
            ImageFlipMode::Both => 0x03,
        };
        Ok(vec![0x81, 0x01, 0x04, 0x61, mode_byte, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Custom
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::uninlined_format_args, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_backlight_command() {
        // Test backlight on
        let cmd = BacklightCommand { status: true };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x33, 0x02, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.command_category(), CommandCategory::Quick));

        // Test backlight off
        let cmd = BacklightCommand { status: false };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x33, 0x03, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.command_category(), CommandCategory::Quick));
    }

    #[test]
    fn test_noise_reduction_2d_command() {
        // Test off
        let cmd = NoiseReduction2DCommand::Off;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x53, 0x00, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.command_category(), CommandCategory::Custom));

        // Test valid levels (1-5)
        for level in 1..=5 {
            let nr_level = NoiseReduction2DLevel::new(level).unwrap();
            let cmd = NoiseReduction2DCommand::Level(nr_level);
            assert_eq!(
                cmd.to_bytes().unwrap(),
                vec![0x81, 0x01, 0x04, 0x53, level, 0xFF]
            );
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.command_category(), CommandCategory::Custom));
        }
    }

    #[test]
    fn test_noise_reduction_3d_command() {
        // Test off
        let cmd = NoiseReduction3DCommand::Off;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x54, 0x00, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.command_category(), CommandCategory::Custom));

        // Test valid levels (1-8)
        for level in 1..=8 {
            let nr_level = NoiseReduction3DLevel::new(level).unwrap();
            let cmd = NoiseReduction3DCommand::Level(nr_level);
            assert_eq!(
                cmd.to_bytes().unwrap(),
                vec![0x81, 0x01, 0x04, 0x54, level, 0xFF]
            );
            assert!(cmd.response_type().is_none());
            assert!(matches!(cmd.command_category(), CommandCategory::Custom));
        }
    }

    #[test]
    fn test_black_white_command() {
        // Test black and white on
        let cmd = BlackWhiteCommand { on: true };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x01, 0x04, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.command_category(), CommandCategory::Quick));

        // Test black and white off (color mode)
        let cmd = BlackWhiteCommand { on: false };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x01, 0x00, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.command_category(), CommandCategory::Quick));
    }

    #[test]
    fn test_image_flip_combined_command() {
        // Test Off
        let cmd = ImageFlipCombinedCommand {
            mode: ImageFlipMode::Off,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x61, 0x00, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.command_category(), CommandCategory::Custom));

        // Test Horizontal
        let cmd = ImageFlipCombinedCommand {
            mode: ImageFlipMode::Horizontal,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x61, 0x01, 0xFF]
        );

        // Test Vertical
        let cmd = ImageFlipCombinedCommand {
            mode: ImageFlipMode::Vertical,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x61, 0x02, 0xFF]
        );

        // Test Both
        let cmd = ImageFlipCombinedCommand {
            mode: ImageFlipMode::Both,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x61, 0x03, 0xFF]
        );
    }

    #[test]
    fn test_command_traits() {
        // Test Debug trait
        let cmds: Vec<Box<dyn std::fmt::Debug>> = vec![
            Box::new(BacklightCommand { status: true }),
            Box::new(NoiseReduction2DCommand::Off),
            Box::new(NoiseReduction3DCommand::Off),
            Box::new(BlackWhiteCommand { on: true }),
            Box::new(ImageFlipCombinedCommand {
                mode: ImageFlipMode::Off,
            }),
        ];

        for cmd in cmds {
            let _ = format!("{:?}", cmd);
        }

        // Test Clone
        let backlight_cmd1 = BacklightCommand { status: true };
        let backlight_cmd2 = backlight_cmd1;
        assert_eq!(backlight_cmd1.status, backlight_cmd2.status);

        let flip_cmd1 = ImageFlipCombinedCommand {
            mode: ImageFlipMode::Horizontal,
        };
        let flip_cmd2 = flip_cmd1;
        match (flip_cmd1.mode, flip_cmd2.mode) {
            (ImageFlipMode::Horizontal, ImageFlipMode::Horizontal) => {}
            _ => panic!("Clone didn't preserve mode"),
        }
    }

    #[test]
    fn test_response_type_none() {
        // Verify all commands return None for response_type
        let cmds: Vec<Box<dyn Command>> = vec![
            Box::new(BacklightCommand { status: true }),
            Box::new(NoiseReduction2DCommand::Off),
            Box::new(NoiseReduction3DCommand::Off),
            Box::new(BlackWhiteCommand { on: true }),
            Box::new(ImageFlipCombinedCommand {
                mode: ImageFlipMode::Off,
            }),
        ];

        for cmd in cmds {
            assert!(cmd.response_type().is_none());
        }
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
        let cmd1 = BacklightCommand { status: true };
        let cmd2 = BacklightCommand { status: true };
        assert_eq!(cmd1.to_bytes().unwrap(), cmd2.to_bytes().unwrap());

        let level = NoiseReduction2DLevel::new(3).unwrap();
        let cmd1 = NoiseReduction2DCommand::Level(level);
        let cmd2 = NoiseReduction2DCommand::Level(level);
        assert_eq!(cmd1.to_bytes().unwrap(), cmd2.to_bytes().unwrap());
    }

    #[test]
    fn test_noise_reduction_2d_enum_variants() {
        // Test that enum variants work correctly
        match NoiseReduction2DCommand::Off {
            NoiseReduction2DCommand::Off => {}
            _ => panic!("Expected Off variant"),
        }

        let level = NoiseReduction2DLevel::new(3).unwrap();
        match NoiseReduction2DCommand::Level(level) {
            NoiseReduction2DCommand::Level(l) => assert_eq!(l.value(), 3),
            _ => panic!("Expected Level variant"),
        }
    }

    #[test]
    fn test_noise_reduction_3d_enum_variants() {
        // Test that enum variants work correctly
        match NoiseReduction3DCommand::Off {
            NoiseReduction3DCommand::Off => {}
            _ => panic!("Expected Off variant"),
        }

        let level = NoiseReduction3DLevel::new(5).unwrap();
        match NoiseReduction3DCommand::Level(level) {
            NoiseReduction3DCommand::Level(l) => assert_eq!(l.value(), 5),
            _ => panic!("Expected Level variant"),
        }
    }

    #[test]
    fn test_command_categories() {
        // Test that BacklightCommand and BlackWhiteCommand use Quick category
        let cmd = BacklightCommand { status: true };
        assert!(matches!(cmd.command_category(), CommandCategory::Quick));

        let cmd = BlackWhiteCommand { on: true };
        assert!(matches!(cmd.command_category(), CommandCategory::Quick));

        // Test that noise reduction and flip commands use Custom category
        let cmd = NoiseReduction2DCommand::Off;
        assert!(matches!(cmd.command_category(), CommandCategory::Custom));

        let cmd = NoiseReduction3DCommand::Off;
        assert!(matches!(cmd.command_category(), CommandCategory::Custom));

        let cmd = ImageFlipCombinedCommand {
            mode: ImageFlipMode::Off,
        };
        assert!(matches!(cmd.command_category(), CommandCategory::Custom));
    }

    #[test]
    fn test_backlight_command_debug() {
        let cmd = BacklightCommand { status: true };
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("BacklightCommand"));
        assert!(debug_str.contains("true"));

        let cmd = BacklightCommand { status: false };
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("false"));
    }

    #[test]
    fn test_black_white_command_debug() {
        let cmd = BlackWhiteCommand { on: true };
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("BlackWhiteCommand"));
        assert!(debug_str.contains("true"));

        let cmd = BlackWhiteCommand { on: false };
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("false"));
    }
}
