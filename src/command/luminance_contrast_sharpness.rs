//! Luminance, contrast, and sharpness control commands for VISCA cameras.
//!
//! This module provides commands for adjusting image quality parameters such as
//! luminance (brightness), contrast levels, and sharpness settings.

// Crate imports
use crate::{
    command::{ViscaCommand, ViscaResponseType},
    error::Error as ViscaError,
    timeout::CommandCategory,
    types::{ContrastLevel, LuminanceLevel},
    visca_param_command,
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

impl ViscaCommand for SharpnessCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
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
                    return Err(ViscaError::InvalidParameter(
                        "Sharpness value must be in the range 0..=11".into(),
                    ));
                }
                let high = (*value >> 4) & 0x0F;
                let low = *value & 0x0F;
                vec![0x81, 0x01, 0x04, 0x42, 0x00, 0x00, high, low, 0xFF]
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

crate::visca_param_command! {
    /// Command to set the luminance level.
    struct LuminanceCommand {
        /// The luminance level.
        value: LuminanceLevel => direct
    }
    bytes = [0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00, {value}, 0xFF]
}

crate::visca_param_command! {
    /// Command to set the contrast level.
    struct ContrastCommand {
        /// The contrast level.
        value: ContrastLevel => direct
    }
    bytes = [0x81, 0x01, 0x04, 0xA2, 0x00, 0x00, 0x00, {value}, 0xFF]
}
