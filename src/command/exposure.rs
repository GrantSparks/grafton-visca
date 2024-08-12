use crate::error::ViscaError;
use std::convert::TryFrom;

use super::{response::ViscaResponseType, ViscaCommand};

#[derive(Debug, Copy, Clone)]
pub enum ExposureMode {
    Auto = 0x00,
    Manual = 0x03,
    Shutter = 0x0A,
    Iris = 0x0B,
    Bright = 0x0D,
}

pub struct ExposureCommand {
    pub mode: ExposureMode,
}

impl ViscaCommand for ExposureCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(vec![0x81, 0x01, 0x04, 0x39, self.mode as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

impl TryFrom<u8> for ExposureMode {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0x00 => Ok(ExposureMode::Auto),
            0x03 => Ok(ExposureMode::Manual),
            0x0A => Ok(ExposureMode::Shutter),
            0x0B => Ok(ExposureMode::Iris),
            0x0D => Ok(ExposureMode::Bright),
            _ => Err(()),
        }
    }
}
