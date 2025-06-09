//! White balance control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera white balance settings,
//! including auto, manual, and preset modes like indoor/outdoor/one-push/color temperature.

// Standard library imports
use std::convert::TryFrom;

// Crate imports
use crate::{
    command::{ViscaCommand, ViscaResponseType},
    error::Error as ViscaError,
    timeout::CommandCategory,
};

/// White balance modes.
///
/// Controls how the camera adjusts color temperature to ensure
/// white objects appear white under different lighting conditions.
#[derive(Debug, Copy, Clone)]
pub enum WhiteBalanceMode {
    /// Automatic white balance adjustment.
    Auto = 0x00,
    /// Indoor preset (optimized for incandescent/tungsten lighting).
    Indoor = 0x01,
    /// Outdoor preset (optimized for daylight).
    Outdoor = 0x02,
    /// One-push white balance (calibrate once based on current scene).
    OnePush = 0x03,
    /// Manual white balance control.
    Manual = 0x05,
    /// Color temperature mode (specify exact color temperature).
    ColorTemperature = 0x20,
}

/// Command to set the white balance mode.
#[derive(Debug, Copy, Clone)]
pub struct WhiteBalanceCommand {
    /// The white balance mode to set.
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
            0x00 => Ok(Self::Auto),
            0x01 => Ok(Self::Indoor),
            0x02 => Ok(Self::Outdoor),
            0x03 => Ok(Self::OnePush),
            0x05 => Ok(Self::Manual),
            0x20 => Ok(Self::ColorTemperature),
            _ => Err(()),
        }
    }
}
