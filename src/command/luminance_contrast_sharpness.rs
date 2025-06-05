//! Luminance, contrast, and sharpness control commands for VISCA cameras.
//!
//! This module provides commands for adjusting image quality parameters such as
//! luminance (brightness), contrast levels, and sharpness settings.

// Crate imports
use crate::{
    command::{ViscaCommand, ViscaResponseType},
    error::ViscaError,
    timeout::CommandCategory,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharpnessMode {
    Auto,
    Manual,
}

pub enum SharpnessCommand {
    Mode(SharpnessMode),
    Reset,
    Up,
    Down,
    Direct { value: u8 },
}

impl ViscaCommand for SharpnessCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            SharpnessCommand::Mode(mode) => {
                let mode_byte = match mode {
                    SharpnessMode::Auto => 0x02,
                    SharpnessMode::Manual => 0x03,
                };
                vec![0x81, 0x01, 0x04, 0x05, mode_byte, 0xFF]
            }
            SharpnessCommand::Reset => vec![0x81, 0x01, 0x04, 0x02, 0x00, 0xFF],
            SharpnessCommand::Up => vec![0x81, 0x01, 0x04, 0x02, 0x02, 0xFF],
            SharpnessCommand::Down => vec![0x81, 0x01, 0x04, 0x02, 0x03, 0xFF],
            SharpnessCommand::Direct { value } => {
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

pub struct LuminanceCommand {
    pub value: u8,
}

impl ViscaCommand for LuminanceCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        if self.value <= 14 {
            Ok(vec![
                0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00, self.value, 0xFF,
            ])
        } else {
            Err(ViscaError::InvalidParameter(
                "Luminance value must be in the range 0..=14".into(),
            ))
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Custom
    }
}

pub struct ContrastCommand {
    pub value: u8,
}

impl ViscaCommand for ContrastCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        if self.value <= 14 {
            Ok(vec![
                0x81, 0x01, 0x04, 0xA2, 0x00, 0x00, 0x00, self.value, 0xFF,
            ])
        } else {
            Err(ViscaError::InvalidParameter(
                "Contrast value must be in the range 0..=14".into(),
            ))
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Custom
    }
}
