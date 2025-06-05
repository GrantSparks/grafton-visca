//! Gain control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera gain settings,
//! including manual gain adjustment, gain limit control, and anti-flicker settings.

// Crate imports
use crate::{
    command::{response::ViscaResponseType, ViscaCommand},
    error::ViscaError,
    timeout::CommandCategory,
};

#[derive(Debug, Copy, Clone)]
pub enum GainCommand {
    Reset,
    Up,
    Down,
    Direct(u16), // 0x00=0 to 0x07=7
}

impl ViscaCommand for GainCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            GainCommand::Reset => vec![0x81, 0x01, 0x04, 0x0C, 0x00, 0xFF],
            GainCommand::Up => vec![0x81, 0x01, 0x04, 0x0C, 0x02, 0xFF],
            GainCommand::Down => vec![0x81, 0x01, 0x04, 0x0C, 0x03, 0xFF],
            GainCommand::Direct(value) => {
                if *value > 0x07 {
                    return Err(ViscaError::InvalidParameter(
                        "Gain value must be between 0x00 (0) and 0x07 (7)".into(),
                    ));
                }
                let high = (*value >> 4) as u8;
                let low = (*value & 0x0F) as u8;
                vec![0x81, 0x01, 0x04, 0x0C, 0x00, 0x00, high, low, 0xFF]
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

#[derive(Debug, Copy, Clone)]
pub struct GainLimitCommand {
    pub limit: u8, // 0x0=0 to 0xF=15
}

impl ViscaCommand for GainLimitCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        if self.limit > 0x0F {
            return Err(ViscaError::InvalidParameter(
                "Gain limit must be between 0x0 (0) and 0xF (15)".into(),
            ));
        }
        Ok(vec![0x81, 0x01, 0x04, 0x2C, self.limit, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

#[derive(Debug, Copy, Clone)]
pub enum AntiFlickerMode {
    Off = 0x00,
    Hz50 = 0x01,
    Hz60 = 0x02,
}

#[derive(Debug, Copy, Clone)]
pub struct AntiFlickerCommand {
    pub mode: AntiFlickerMode,
}

impl ViscaCommand for AntiFlickerCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(vec![0x81, 0x01, 0x04, 0x23, self.mode as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}
