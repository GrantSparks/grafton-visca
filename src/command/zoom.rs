use super::ViscaResponseType;
use crate::command::ViscaCommand;
use crate::error::ViscaError;

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
            // Stop command
            ZoomCommand::Stop => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]),

            // Tele standard zoom
            ZoomCommand::TeleStandard => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]),

            // Wide standard zoom
            ZoomCommand::WideStandard => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x03, 0xFF]),

            // Tele variable zoom with valid speed (0..=7)
            ZoomCommand::TeleVariable(speed) if *speed <= 7 => {
                Ok(vec![0x81, 0x01, 0x04, 0x07, 0x20 | speed, 0xFF])
            }

            // Wide variable zoom with valid speed (0..=7)
            ZoomCommand::WideVariable(speed) if *speed <= 7 => {
                Ok(vec![0x81, 0x01, 0x04, 0x07, 0x30 | speed, 0xFF])
            }

            // Handle invalid speed values for variable zoom commands
            ZoomCommand::TeleVariable(_) | ZoomCommand::WideVariable(_) => Err(
                ViscaError::InvalidParameter("Zoom speed must be in the range 0..=7".into()),
            ),

            // Direct zoom to a specific position
            ZoomCommand::Direct(position) => {
                // Extract individual nibbles from the position
                let p = ((*position >> 12) & 0x0F) as u8;
                let q = ((*position >> 8) & 0x0F) as u8;
                let r = ((*position >> 4) & 0x0F) as u8;
                let s = (*position & 0x0F) as u8;

                Ok(vec![0x81, 0x01, 0x04, 0x47, p, q, r, s, 0xFF])
            }
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        match self {
            ZoomCommand::TeleStandard => Some(ViscaResponseType::ZoomTeleStandard),
            ZoomCommand::WideStandard => Some(ViscaResponseType::ZoomWideStandard),
            _ => None,
        }
    }
}
