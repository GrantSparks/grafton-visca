//! Color control commands for VISCA cameras.
//!
//! This module provides commands for controlling color-related settings
//! including white balance tuning, saturation, and hue adjustments.

use crate::error::ViscaError;

use super::{response::ViscaResponseType, ViscaCommand};

/// One-Push White Balance Trigger command
#[derive(Debug, Copy, Clone)]
pub struct OnePushTriggerCommand;

impl ViscaCommand for OnePushTriggerCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(vec![0x81, 0x01, 0x04, 0x10, 0x05, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

/// Red Gain Tuning command
#[derive(Debug, Copy, Clone)]
pub struct RedTuningCommand {
    pub level: i8, // -10 to +10
}

impl ViscaCommand for RedTuningCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        if self.level < -10 || self.level > 10 {
            return Err(ViscaError::InvalidParameter(
                "Red tuning level must be between -10 and +10".into(),
            ));
        }
        let value = (self.level + 10) as u8; // Convert -10..+10 to 0x00..0x14
        Ok(vec![0x81, 0x0A, 0x01, 0x12, value, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

/// Blue Gain Tuning command
#[derive(Debug, Copy, Clone)]
pub struct BlueTuningCommand {
    pub level: i8, // -10 to +10
}

impl ViscaCommand for BlueTuningCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        if self.level < -10 || self.level > 10 {
            return Err(ViscaError::InvalidParameter(
                "Blue tuning level must be between -10 and +10".into(),
            ));
        }
        let value = (self.level + 10) as u8; // Convert -10..+10 to 0x00..0x14
        Ok(vec![0x81, 0x0A, 0x01, 0x13, value, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

/// Saturation command
#[derive(Debug, Copy, Clone)]
pub struct SaturationCommand {
    pub level: u8, // 0x0=60% to 0xE=200%
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
}

/// Hue command
#[derive(Debug, Copy, Clone)]
pub struct HueCommand {
    pub level: u8, // 0x0=0 to 0xE=14
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
}

/// Color Temperature command
#[derive(Debug, Copy, Clone)]
pub enum ColorTemperatureCommand {
    Reset,
    Up,
    Down,
    Direct(u16), // 0x00=2500K to 0x37=8000K
}

impl ViscaCommand for ColorTemperatureCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            ColorTemperatureCommand::Reset => vec![0x81, 0x01, 0x04, 0x20, 0x00, 0xFF],
            ColorTemperatureCommand::Up => vec![0x81, 0x01, 0x04, 0x20, 0x02, 0xFF],
            ColorTemperatureCommand::Down => vec![0x81, 0x01, 0x04, 0x20, 0x03, 0xFF],
            ColorTemperatureCommand::Direct(temp) => {
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
}

/// Red Gain Direct command (different from tuning)
#[derive(Debug, Copy, Clone)]
pub enum RedGainCommand {
    Reset,
    Up,
    Down,
    Direct(u8), // 0x00 to 0xFF
}

impl ViscaCommand for RedGainCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            RedGainCommand::Reset => vec![0x81, 0x01, 0x04, 0x03, 0x00, 0xFF],
            RedGainCommand::Up => vec![0x81, 0x01, 0x04, 0x03, 0x02, 0xFF],
            RedGainCommand::Down => vec![0x81, 0x01, 0x04, 0x03, 0x03, 0xFF],
            RedGainCommand::Direct(gain) => {
                let high = (*gain >> 4) & 0x0F;
                let low = *gain & 0x0F;
                vec![0x81, 0x01, 0x04, 0x43, 0x00, 0x00, high, low, 0xFF]
            }
        })
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

/// Blue Gain Direct command (different from tuning)
#[derive(Debug, Copy, Clone)]
pub enum BlueGainCommand {
    Reset,
    Up,
    Down,
    Direct(u8), // 0x00 to 0xFF
}

impl ViscaCommand for BlueGainCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(match self {
            BlueGainCommand::Reset => vec![0x81, 0x01, 0x04, 0x04, 0x00, 0xFF],
            BlueGainCommand::Up => vec![0x81, 0x01, 0x04, 0x04, 0x02, 0xFF],
            BlueGainCommand::Down => vec![0x81, 0x01, 0x04, 0x04, 0x03, 0xFF],
            BlueGainCommand::Direct(gain) => {
                let high = (*gain >> 4) & 0x0F;
                let low = *gain & 0x0F;
                vec![0x81, 0x01, 0x04, 0x44, 0x00, 0x00, high, low, 0xFF]
            }
        })
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}
