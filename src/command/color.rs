//! Color control commands for VISCA cameras.
//!
//! This module provides commands for controlling color-related settings
//! including white balance tuning, saturation, and hue adjustments.

// Standard library imports
use std::convert::TryFrom;

// Crate imports
use crate::{
    command::{response::ViscaResponseType, ViscaCommand},
    error::ViscaError,
    timeout::CommandCategory,
};

/// One-Push White Balance Trigger command.
///
/// Performs a one-time automatic white balance adjustment based on
/// the current scene. The camera will analyze the image and set the
/// white balance to achieve neutral colors.
#[derive(Debug, Copy, Clone)]
pub struct OnePushTriggerCommand;

impl ViscaCommand for OnePushTriggerCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(vec![0x81, 0x01, 0x04, 0x10, 0x05, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Red Gain Tuning command.
///
/// Fine-tunes the red channel gain for white balance adjustment.
/// This is typically used after setting a base white balance mode
/// to make small corrections.
#[derive(Debug, Copy, Clone)]
pub struct RedTuningCommand {
    /// Red tuning level (-10 to +10, where 0 is neutral).
    pub level: i8,
}

impl ViscaCommand for RedTuningCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        if self.level < -10 || self.level > 10 {
            return Err(ViscaError::InvalidParameter(
                "Red tuning level must be between -10 and +10".into(),
            ));
        }
        let value = u8::try_from(self.level + 10).expect("level already validated to be in range"); // Convert -10..+10 to 0x00..0x14
        Ok(vec![0x81, 0x0A, 0x01, 0x12, value, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Blue Gain Tuning command.
///
/// Fine-tunes the blue channel gain for white balance adjustment.
/// This is typically used after setting a base white balance mode
/// to make small corrections.
#[derive(Debug, Copy, Clone)]
pub struct BlueTuningCommand {
    /// Blue tuning level (-10 to +10, where 0 is neutral).
    pub level: i8,
}

impl ViscaCommand for BlueTuningCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        if self.level < -10 || self.level > 10 {
            return Err(ViscaError::InvalidParameter(
                "Blue tuning level must be between -10 and +10".into(),
            ));
        }
        let value = u8::try_from(self.level + 10).expect("level already validated to be in range"); // Convert -10..+10 to 0x00..0x14
        Ok(vec![0x81, 0x0A, 0x01, 0x13, value, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Saturation control command.
///
/// Adjusts the color saturation level of the image.
/// Lower values produce more muted colors, while higher values
/// produce more vivid colors.
#[derive(Debug, Copy, Clone)]
pub struct SaturationCommand {
    /// Saturation level (0x0 = 60%, 0xE = 200%).
    pub level: u8,
}

impl ViscaCommand for SaturationCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        if self.level > 0x0E {
            return Err(ViscaError::InvalidParameter(
                "Saturation level must be between 0x0 (60%) and 0xE (200%)".into(),
            ));
        }
        Ok(vec![
            0x81, 0x01, 0x04, 0x49, 0x00, 0x00, 0x00, self.level, 0xFF,
        ])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Hue adjustment command.
///
/// Adjusts the hue (color phase) of the image, shifting all colors
/// around the color wheel. This can be used to correct color casts
/// or create artistic effects.
#[derive(Debug, Copy, Clone)]
pub struct HueCommand {
    /// Hue level (0x0 to 0xE, representing 0 to 14 degrees of rotation).
    pub level: u8,
}

impl ViscaCommand for HueCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        if self.level > 0x0E {
            return Err(ViscaError::InvalidParameter(
                "Hue level must be between 0x0 (0) and 0xE (14)".into(),
            ));
        }
        Ok(vec![
            0x81, 0x01, 0x04, 0x4F, 0x00, 0x00, 0x00, self.level, 0xFF,
        ])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Color Temperature command
#[derive(Debug, Copy, Clone)]
pub enum ColorTemperatureCommand {
    /// Reset color temperature to default.
    Reset,
    /// Increase color temperature.
    Up,
    /// Decrease color temperature.
    Down,
    /// Set color temperature directly (0x00=2500K to 0x37=8000K).
    Direct(u16),
}

impl ViscaCommand for ColorTemperatureCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            Self::Reset => vec![0x81, 0x01, 0x04, 0x20, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x20, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x20, 0x03, 0xFF],
            Self::Direct(temp) => {
                if *temp > 0x37 {
                    return Err(ViscaError::InvalidParameter(
                        "Color temperature must be between 0x00 (2500K) and 0x37 (8000K)".into(),
                    ));
                }
                let high = ((*temp >> 4) & 0x0F) as u8;
                let low = (*temp & 0x0F) as u8;
                vec![0x81, 0x01, 0x04, 0x20, 0x00, 0x00, high, low, 0xFF]
            }
        })
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Red Gain Direct command (different from tuning)
#[derive(Debug, Copy, Clone)]
pub enum RedGainCommand {
    /// Reset red gain to default value
    Reset,
    /// Increment red gain value
    Up,
    /// Decrement red gain value
    Down,
    /// Set red gain to a specific value (0x00 to 0xFF)
    Direct(u8),
}

impl ViscaCommand for RedGainCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            Self::Reset => vec![0x81, 0x01, 0x04, 0x03, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x03, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x03, 0x03, 0xFF],
            Self::Direct(gain) => {
                let high = (*gain >> 4) & 0x0F;
                let low = *gain & 0x0F;
                vec![0x81, 0x01, 0x04, 0x43, 0x00, 0x00, high, low, 0xFF]
            }
        })
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Blue Gain Direct command (different from tuning)
#[derive(Debug, Copy, Clone)]
pub enum BlueGainCommand {
    /// Reset blue gain to default value
    Reset,
    /// Increment blue gain value
    Up,
    /// Decrement blue gain value
    Down,
    /// Set blue gain to a specific value (0x00 to 0xFF)
    Direct(u8),
}

impl ViscaCommand for BlueGainCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            Self::Reset => vec![0x81, 0x01, 0x04, 0x04, 0x00, 0xFF],
            Self::Up => vec![0x81, 0x01, 0x04, 0x04, 0x02, 0xFF],
            Self::Down => vec![0x81, 0x01, 0x04, 0x04, 0x03, 0xFF],
            Self::Direct(gain) => {
                let high = (*gain >> 4) & 0x0F;
                let low = *gain & 0x0F;
                vec![0x81, 0x01, 0x04, 0x44, 0x00, 0x00, high, low, 0xFF]
            }
        })
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}
