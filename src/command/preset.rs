use crate::command::ViscaCommand;
use crate::error::ViscaError;

use super::ViscaResponseType;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PresetAction {
    Reset = 0x00,
    Set = 0x01,
    Recall = 0x02,
}

pub struct PresetCommand {
    pub action: PresetAction,
    pub preset_number: u8, // 0x00 to 0x59 (0 to 89)
}

impl ViscaCommand for PresetCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        if self.preset_number <= 0x59 {
            Ok(vec![
                0x81,
                0x01,
                0x04,
                0x3F,
                self.action as u8,
                self.preset_number,
                0xFF,
            ])
        } else {
            Err(ViscaError::InvalidParameter(
                "Preset number must be in the range 0x00..=0x59 (0-89)".into(),
            ))
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }
}
