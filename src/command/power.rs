use crate::command::ViscaCommand;
use crate::error::ViscaError;

use super::ViscaResponseType;

#[derive(Debug, Copy, Clone)]
pub enum Power {
    On = 0x02,
    Standby = 0x03,
}

pub struct PowerCommand {
    pub power: Power,
}

impl ViscaCommand for PowerCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(vec![0x81, 0x01, 0x04, 0x00, self.power as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}
