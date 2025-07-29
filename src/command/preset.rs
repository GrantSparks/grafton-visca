//! Preset position commands for VISCA cameras.
//!
//! This module provides commands for storing and recalling camera positions.
//! `PTZOptics` G2 cameras support up to 90 presets (0-89).

// Standard library imports
// (none)

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{
    command::{encode_visca::EncodeVisca, ResponseType},
    error::Error,
    timeout::CommandCategory,
};

/// Action to perform on a preset.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PresetAction {
    /// Reset/clear the preset.
    Reset = 0x00,
    /// Store current position to preset.
    Set = 0x01,
    /// Move camera to preset position.
    Recall = 0x02,
}

crate::visca_bounded_param! {
    /// Preset number with validation.
    ///
    /// Valid range: 0 to 89 (0x00 to 0x59).
    PresetNumber: u8 {
        min: 0,
        max: 89
    }
}

/// Command to manage camera presets.
#[derive(Debug, Copy, Clone)]
pub(crate) struct PresetCommand {
    /// The action to perform.
    pub action: PresetAction,
    /// The preset number to operate on.
    pub preset_number: PresetNumber,
}

impl PresetCommand {
    // Legacy method - removed in new API
    // pub fn new<P: crate::camera::CameraProfile>(...) { ... }
}

impl EncodeVisca for PresetCommand {
    type Response = ();
    const MAX_SIZE: usize = 7;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        buffer[0] = camera_id.to_address_byte();
        buffer[1] = 0x01;
        buffer[2] = 0x04;
        buffer[3] = 0x3F;
        buffer[4] = self.action as u8;
        buffer[5] = self.preset_number.value();
        buffer[6] = 0xFF;

        Ok(Self::MAX_SIZE)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Preset
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_number_new() {
        // Valid preset numbers
        assert!(PresetNumber::new(0).is_ok());
        assert!(PresetNumber::new(45).is_ok());
        assert!(PresetNumber::new(89).is_ok());

        // Invalid preset numbers
        assert!(matches!(
            PresetNumber::new(90),
            Err(Error::InvalidParameter { .. })
        ));
        assert!(matches!(
            PresetNumber::new(255),
            Err(Error::InvalidParameter { .. })
        ));
    }

    #[test]
    fn test_preset_number_try_from() {
        // Valid conversion
        let preset =
            PresetNumber::try_from(50).unwrap_or_else(|e| panic!("Valid preset number: {e:?}"));
        assert_eq!(preset.value(), 50);

        // Invalid conversion
        assert!(PresetNumber::try_from(90).is_err());
    }

    #[test]
    fn test_preset_action_values() {
        assert_eq!(PresetAction::Reset as u8, 0x00);
        assert_eq!(PresetAction::Set as u8, 0x01);
        assert_eq!(PresetAction::Recall as u8, 0x02);
    }

    #[test]
    fn test_preset_command_reset() {
        let cmd = PresetCommand {
            action: PresetAction::Reset,
            preset_number: PresetNumber::new(10)
                .unwrap_or_else(|e| panic!("Valid preset number: {e:?}")),
        };
        assert_eq!(
            cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap_or_else(|e| panic!("Valid command: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x3F, 0x00, 0x0A, 0xFF]
        );
    }

    #[test]
    fn test_preset_command_set() {
        let cmd = PresetCommand {
            action: PresetAction::Set,
            preset_number: PresetNumber::new(45)
                .unwrap_or_else(|e| panic!("Valid preset number: {e:?}")),
        };
        assert_eq!(
            cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap_or_else(|e| panic!("Valid command: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x3F, 0x01, 0x2D, 0xFF]
        );
    }

    #[test]
    fn test_preset_command_recall() {
        let cmd = PresetCommand {
            action: PresetAction::Recall,
            preset_number: PresetNumber::new(89)
                .unwrap_or_else(|e| panic!("Valid preset number: {e:?}")),
        };
        assert_eq!(
            cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                .unwrap_or_else(|e| panic!("Valid command: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x3F, 0x02, 0x59, 0xFF]
        );
    }

    #[test]
    fn test_preset_timeout_kind() {
        let cmd = PresetCommand {
            action: PresetAction::Recall,
            preset_number: PresetNumber::new(0)
                .unwrap_or_else(|e| panic!("Valid preset number: {e:?}")),
        };
        assert_eq!(cmd.timeout_kind(), CommandCategory::Preset);
    }

    #[test]
    fn test_preset_command_response_type() {
        let cmd = PresetCommand {
            action: PresetAction::Set,
            preset_number: PresetNumber::new(0)
                .unwrap_or_else(|e| panic!("Valid preset number: {e:?}")),
        };
        assert!(cmd.response_type().is_none());
    }
}
