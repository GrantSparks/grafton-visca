use crate::command::ViscaCommand;
use crate::error::ViscaError;
use std::convert::TryFrom;

use super::ViscaResponseType;

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
