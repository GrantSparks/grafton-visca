//! Image adjustment commands for VISCA cameras.
//!
//! This module provides commands for controlling various image quality settings,
//! including brightness, contrast, sharpness, saturation, and hue adjustments.

// Crate imports
use crate::{
    command::{ViscaCommand, ViscaResponseType},
    error::ViscaError,
    timeout::CommandCategory,
};

/// Backlight compensation command.
///
/// Enables or disables backlight compensation, which helps properly expose
/// subjects that are backlit (have a bright light source behind them).
#[derive(Debug, Copy, Clone)]
pub struct BacklightCommand {
    /// Enable (true) or disable (false) backlight compensation.
    pub status: bool,
}

impl ViscaCommand for BacklightCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let status_byte = if self.status { 0x02 } else { 0x03 };
        Ok(vec![0x81, 0x01, 0x04, 0x33, status_byte, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Custom
    }
}

/// 2D Noise Reduction command.
///
/// Reduces spatial noise in individual frames by analyzing and smoothing
/// pixel variations. Higher levels provide more noise reduction but may
/// reduce fine detail.
#[derive(Debug, Copy, Clone)]
pub enum NoiseReduction2DCommand {
    /// Disable 2D noise reduction.
    Off,
    /// Set 2D noise reduction level (1 = minimal, 5 = maximum).
    Level(u8),
}

impl ViscaCommand for NoiseReduction2DCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            Self::Off => vec![0x81, 0x01, 0x04, 0x53, 0x00, 0xFF],
            Self::Level(level) => {
                if *level < 1 || *level > 5 {
                    return Err(ViscaError::InvalidParameter(
                        "2D Noise Reduction level must be between 1 and 5".into(),
                    ));
                }
                vec![0x81, 0x01, 0x04, 0x53, *level, 0xFF]
            }
        })
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Custom
    }
}

/// 3D Noise Reduction command.
///
/// Reduces temporal noise by analyzing multiple frames over time.
/// This is effective for reducing noise in video streams while preserving
/// motion detail. Higher levels provide more noise reduction.
#[derive(Debug, Copy, Clone)]
pub enum NoiseReduction3DCommand {
    /// Disable 3D noise reduction.
    Off,
    /// Set 3D noise reduction level (1 = minimal, 8 = maximum).
    Level(u8),
}

impl ViscaCommand for NoiseReduction3DCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            Self::Off => vec![0x81, 0x01, 0x04, 0x54, 0x00, 0xFF],
            Self::Level(level) => {
                if *level < 1 || *level > 8 {
                    return Err(ViscaError::InvalidParameter(
                        "3D Noise Reduction level must be between 1 and 8".into(),
                    ));
                }
                vec![0x81, 0x01, 0x04, 0x54, *level, 0xFF]
            }
        })
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Custom
    }
}

/// Black and White Mode command.
///
/// Switches the camera output between color and monochrome (black and white) modes.
#[derive(Debug, Copy, Clone)]
pub struct BlackWhiteCommand {
    /// Enable (true) for black and white mode, disable (false) for color mode.
    pub on: bool,
}

impl ViscaCommand for BlackWhiteCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let mode = if self.on { 0x04 } else { 0x00 };
        Ok(vec![0x81, 0x01, 0x04, 0x01, mode, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Custom
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

/// Command to set the combined image flip mode.
#[derive(Debug, Copy, Clone)]
pub struct ImageFlipCombinedCommand {
    /// The flip mode to apply.
    pub mode: ImageFlipMode,
}

impl ViscaCommand for ImageFlipCombinedCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let mode_byte = match self.mode {
            ImageFlipMode::Off => 0x00,
            ImageFlipMode::Horizontal => 0x01,
            ImageFlipMode::Vertical => 0x02,
            ImageFlipMode::Both => 0x03,
        };
        Ok(vec![0x81, 0x01, 0x04, 0x61, mode_byte, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Custom
    }
}
