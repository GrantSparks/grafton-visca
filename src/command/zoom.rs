use crate::command::ViscaCommand;
use crate::error::ViscaError;

use super::ViscaResponseType;

#[derive(Debug)]
pub enum ZoomCommand {
    Stop,
    TeleStandard,
    WideStandard,
    TeleVariable(u8),
    WideVariable(u8),
    Direct(u16),
}

impl ViscaCommand for ZoomCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        match self {
            ZoomCommand::Stop => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]),
            ZoomCommand::TeleStandard => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]),
            ZoomCommand::WideStandard => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x03, 0xFF]),
            ZoomCommand::TeleVariable(speed) => {
                if *speed <= 7 {
                    Ok(vec![0x81, 0x01, 0x04, 0x07, 0x20 | speed, 0xFF])
                } else {
                    Err(ViscaError::InvalidParameter(
                        "Zoom speed must be in the range 0..=7".into(),
                    ))
                }
            }
            ZoomCommand::WideVariable(speed) => {
                if *speed <= 7 {
                    Ok(vec![0x81, 0x01, 0x04, 0x07, 0x30 | speed, 0xFF])
                } else {
                    Err(ViscaError::InvalidParameter(
                        "Zoom speed must be in the range 0..=7".into(),
                    ))
                }
            }
            ZoomCommand::Direct(position) => {
                let p = (*position >> 12) as u8;
                let q = (*position >> 8) as u8;
                let r = (*position >> 4) as u8;
                let s = (*position & 0x0F) as u8;
                Ok(vec![0x81, 0x01, 0x04, 0x47, p, q, r, s, 0xFF])
            }
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        match self {
            ZoomCommand::WideStandard => Some(ViscaResponseType::ZoomWideStandard),
            ZoomCommand::TeleStandard => Some(ViscaResponseType::ZoomTeleStandard),
            _ => None,
        }
    }
}
