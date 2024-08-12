use crate::command::ViscaCommand;
use crate::error::ViscaError;

use super::ViscaResponseType;

#[derive(Debug, Copy, Clone)]
pub enum Flip {
    On = 0x02,
    Off = 0x03,
}

pub struct ImageFlipCommand {
    pub flip: Flip,
}

impl ViscaCommand for ImageFlipCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        Ok(vec![0x81, 0x01, 0x04, 0x66, self.flip as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}
