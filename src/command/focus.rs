use crate::command::ViscaCommand;
use crate::error::ViscaError;

use super::ViscaResponseType;

#[derive(Debug)]
pub enum FocusCommand {
    Stop,
    FarStandard,
    NearStandard,
    FarVariable(u8),
    NearVariable(u8),
    Direct(u16),
    Auto,
    Manual,
    OnePushTrigger,
    Infinity,
}

impl ViscaCommand for FocusCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        match self {
            FocusCommand::Stop => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x00, 0xFF]),
            FocusCommand::FarStandard => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x02, 0xFF]),
            FocusCommand::NearStandard => Ok(vec![0x81, 0x01, 0x04, 0x08, 0x03, 0xFF]),
            FocusCommand::FarVariable(speed) => {
                if *speed <= 7 {
                    Ok(vec![0x81, 0x01, 0x04, 0x08, 0x20 | speed, 0xFF])
                } else {
                    Err(ViscaError::InvalidParameter(
                        "Focus speed must be in the range 0..=7".into(),
                    ))
                }
            }
            FocusCommand::NearVariable(speed) => {
                if *speed <= 7 {
                    Ok(vec![0x81, 0x01, 0x04, 0x08, 0x30 | speed, 0xFF])
                } else {
                    Err(ViscaError::InvalidParameter(
                        "Focus speed must be in the range 0..=7".into(),
                    ))
                }
            }
            FocusCommand::Direct(position) => {
                let p = (*position >> 12) as u8;
                let q = (*position >> 8) as u8;
                let r = (*position >> 4) as u8;
                let s = (*position & 0x0F) as u8;
                Ok(vec![0x81, 0x01, 0x04, 0x48, p, q, r, s, 0xFF])
            }
            FocusCommand::Auto => Ok(vec![0x81, 0x01, 0x04, 0x38, 0x02, 0xFF]),
            FocusCommand::Manual => Ok(vec![0x81, 0x01, 0x04, 0x38, 0x03, 0xFF]),
            FocusCommand::OnePushTrigger => Ok(vec![0x81, 0x01, 0x04, 0x18, 0x01, 0xFF]),
            FocusCommand::Infinity => Ok(vec![0x81, 0x01, 0x04, 0x18, 0x02, 0xFF]),
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}
