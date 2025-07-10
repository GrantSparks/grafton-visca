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
    command::{Command, ResponseType},
    constants::CameraConstants,
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
        max: 89,
        error_msg: "Preset number must be between 0 and 89"
    }
}

/// Command to manage camera presets.
#[derive(Debug, Copy, Clone)]
pub struct PresetCommand {
    /// The action to perform.
    pub action: PresetAction,
    /// The preset number to operate on.
    pub preset_number: PresetNumber,
}

impl PresetCommand {
    // Legacy method - removed in new API
    // pub fn new<P: crate::camera::CameraProfile>(...) { ... }
}

impl Command for PresetCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(vec![
            0x81,
            0x01,
            0x04,
            0x3F,
            self.action as u8,
            self.preset_number.value(),
            0xFF,
        ])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Preset
    }

    fn validate_for_model(&self, model: crate::constants::CameraModel) -> Result<(), Error> {
        let max_preset = model.max_preset_id();
        if self.preset_number.value() > max_preset {
            return Err(Error::ModelValidation {
                model,
                command: "Preset".to_string(),
                reason: format!(
                    "Preset number {} exceeds maximum {max_preset} for {model:?}",
                    self.preset_number.value()
                ),
            });
        }
        Ok(())
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
            Err(Error::InvalidParameter(_))
        ));
        assert!(matches!(
            PresetNumber::new(255),
            Err(Error::InvalidParameter(_))
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
            cmd.to_bytes()
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
            cmd.to_bytes()
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
            cmd.to_bytes()
                .unwrap_or_else(|e| panic!("Valid command: {e:?}")),
            vec![0x81, 0x01, 0x04, 0x3F, 0x02, 0x59, 0xFF]
        );
    }

    #[test]
    fn test_preset_command_category() {
        let cmd = PresetCommand {
            action: PresetAction::Recall,
            preset_number: PresetNumber::new(0)
                .unwrap_or_else(|e| panic!("Valid preset number: {e:?}")),
        };
        assert_eq!(cmd.command_category(), CommandCategory::Preset);
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
