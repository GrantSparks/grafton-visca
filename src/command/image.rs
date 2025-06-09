//! Image adjustment commands for VISCA cameras.
//!
//! This module provides commands for controlling various image quality settings,
//! including brightness, contrast, sharpness, saturation, and hue adjustments.

// Crate imports
use crate::{
    command::{Command, ResponseType},
    error::Error as ViscaError,
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
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
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
