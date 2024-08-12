use crate::command::ViscaCommand;
use crate::error::ViscaError;

use super::ViscaResponseType;

pub struct BacklightCommand {
    pub status: bool,
}

impl ViscaCommand for BacklightCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let status_byte = if self.status { 0x02 } else { 0x03 };
        Ok(vec![0x81, 0x01, 0x04, 0x33, status_byte, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}
