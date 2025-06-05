//! White balance control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera white balance settings,
//! including auto, manual, and preset modes like indoor/outdoor/one-push/color temperature.

// Standard library imports
use std::convert::TryFrom;

// Crate imports
use crate::{
    command::{ViscaCommand, ViscaResponseType},
    error::ViscaError,
    timeout::CommandCategory,
};

#[derive(Debug, Copy, Clone)]
pub enum WhiteBalanceMode {
    Auto = 0x00,
    Indoor = 0x01,
    Outdoor = 0x02,
    OnePush = 0x03,
    Manual = 0x05,
    ColorTemperature = 0x20,
}

pub struct WhiteBalanceCommand {
    pub mode: WhiteBalanceMode,
}

impl ViscaCommand for WhiteBalanceCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(vec![0x81, 0x01, 0x04, 0x35, self.mode as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

impl TryFrom<u8> for WhiteBalanceMode {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0x00 => Ok(WhiteBalanceMode::Auto),
            0x01 => Ok(WhiteBalanceMode::Indoor),
            0x02 => Ok(WhiteBalanceMode::Outdoor),
            0x03 => Ok(WhiteBalanceMode::OnePush),
            0x05 => Ok(WhiteBalanceMode::Manual),
            0x20 => Ok(WhiteBalanceMode::ColorTemperature),
            _ => Err(()),
        }
    }
}
