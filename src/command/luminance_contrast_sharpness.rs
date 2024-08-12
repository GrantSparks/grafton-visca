use crate::command::ViscaCommand;
use crate::error::ViscaError;

use super::ViscaResponseType;

pub struct LuminanceCommand {
    pub value: u8,
}

impl ViscaCommand for LuminanceCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        if self.value <= 14 {
            Ok(vec![
                0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00, self.value, 0xFF,
            ])
        } else {
            Err(ViscaError::InvalidParameter(
                "Luminance value must be in the range 0..=14".into(),
            ))
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

pub struct ContrastCommand {
    pub value: u8,
}

impl ViscaCommand for ContrastCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        if self.value <= 14 {
            Ok(vec![
                0x81, 0x01, 0x04, 0xA2, 0x00, 0x00, 0x00, self.value, 0xFF,
            ])
        } else {
            Err(ViscaError::InvalidParameter(
                "Contrast value must be in the range 0..=14".into(),
            ))
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}

pub struct SharpnessCommand {
    pub value: u8,
}

impl ViscaCommand for SharpnessCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        if self.value <= 11 {
            Ok(vec![
                0x81, 0x01, 0x04, 0x42, 0x00, 0x00, 0x00, self.value, 0xFF,
            ])
        } else {
            Err(ViscaError::InvalidParameter(
                "Sharpness value must be in the range 0..=11".into(),
            ))
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}
