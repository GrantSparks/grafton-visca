//! Focus control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera focus functionality,
//! including auto/manual modes, directional focus, and direct position control.

use crate::command::ViscaCommand;
use crate::error::ViscaError;
use crate::timeout::CommandCategory;

use super::ViscaResponseType;

/// Focus control commands.
///
/// Provides various ways to control camera focus.
#[derive(Debug)]
pub enum FocusCommand {
    Stop,
    FarStandard,
    NearStandard,
    FarVariable(u8),
    NearVariable(u8),
    Direct(u16),
    Auto,
    Manual,
    OnePushTrigger,
    Infinity,
}

impl ViscaCommand for FocusCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        match self {
            FocusCommand::Stop => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x00, 0xFF]),
            FocusCommand::FarStandard => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x02, 0xFF]),
            FocusCommand::NearStandard => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x03, 0xFF]),
            FocusCommand::FarVariable(speed) => {
                if *speed <= 7 {
                    Ok(vec![0x81, 0x01, 0x04, 0x08, 0x20 | speed, 0xFF])
                } else {
                    Err(ViscaError::InvalidParameter(
                        "Focus speed must be in the range 0..=7".into(),
                    ))
                }
            }
            FocusCommand::NearVariable(speed) => {
                if *speed <= 7 {
                    Ok(vec![0x81, 0x01, 0x04, 0x08, 0x30 | speed, 0xFF])
                } else {
                    Err(ViscaError::InvalidParameter(
                        "Focus speed must be in the range 0..=7".into(),
                    ))
                }
            }
            FocusCommand::Direct(position) => {
                let p = (*position >> 12) as u8;
                let q = (*position >> 8) as u8;
                let r = (*position >> 4) as u8;
                let s = (*position & 0x0F) as u8;
                Ok(vec![0x81, 0x01, 0x04, 0x48, p, q, r, s, 0xFF])
            }
            FocusCommand::Auto => Ok(vec![0x81, 0x01, 0x04, 0x38, 0x02, 0xFF]),
            FocusCommand::Manual => Ok(vec![0x81, 0x01, 0x04, 0x38, 0x03, 0xFF]),
            FocusCommand::OnePushTrigger => Ok(vec![0x81, 0x01, 0x04, 0x18, 0x01, 0xFF]),
            FocusCommand::Infinity => Ok(vec![0x81, 0x01, 0x04, 0x18, 0x02, 0xFF]),
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Movement
    }
}

/// Focus Zone selection
#[derive(Debug, Copy, Clone)]
pub enum FocusZone {
    Top,
    Center,
    Bottom,
}

pub struct FocusZoneCommand {
    pub zone: FocusZone,
}

impl ViscaCommand for FocusZoneCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let zone_byte = match self.zone {
            FocusZone::Top => 0x00,
            FocusZone::Center => 0x01,
            FocusZone::Bottom => 0x02,
        };
        Ok(vec![0x81, 0x01, 0x04, 0x3C, zone_byte, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Auto Focus Sensitivity
#[derive(Debug, Copy, Clone)]
pub enum AFSensitivity {
    High,
    Normal,
    Low,
}

pub struct AFSensitivityCommand {
    pub sensitivity: AFSensitivity,
}

impl ViscaCommand for AFSensitivityCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let sens_byte = match self.sensitivity {
            AFSensitivity::High => 0x02,
            AFSensitivity::Normal => 0x01,
            AFSensitivity::Low => 0x00,
        };
        Ok(vec![0x81, 0x01, 0x04, 0x58, sens_byte, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Focus Near Limit
pub struct FocusNearLimitCommand {
    pub position: u16,
}

impl ViscaCommand for FocusNearLimitCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let p = (self.position >> 12) as u8;
        let q = ((self.position >> 8) & 0x0F) as u8;
        let r = ((self.position >> 4) & 0x0F) as u8;
        let s = (self.position & 0x0F) as u8;
        Ok(vec![0x81, 0x01, 0x04, 0x28, p, q, r, s, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}
