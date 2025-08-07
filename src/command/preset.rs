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
    command::{const_encoding::builder::CommandBuilder, encode_visca::EncodeVisca, ResponseType},
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
    /// Valid range: 0 to 255 (0x00 to 0xFF).
    /// Note: Actual valid range depends on camera model:
    /// - PTZOptics G2: 0-89
    /// - PTZOptics G3: 0-255
    /// - Sony FR7: 0-255
    /// - Sony EVI-H100: 0-6
    /// Camera-specific validation is performed when sending commands.
    PresetNumber: u8 {
        min: 0,
        max: 255
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
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Preset;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::const_encoding::constants::preset;

        let command = CommandBuilder::<7>::from_prefix(preset::CONTROL_PREFIX)
            .with_camera_id(camera_id)
            .push(self.action as u8)
            .push(self.preset_number.value())
            .build();

        buffer[..7].copy_from_slice(&command);
        Ok(7)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use crate::command::const_encoding::VISCA_TERMINATOR;
    use crate::macros::test_utils::visca_test;

    #[test]
    fn test_preset_number_new() {
        // Valid preset numbers
        assert!(PresetNumber::new(0).is_ok());
        assert!(PresetNumber::new(45).is_ok());
        assert!(PresetNumber::new(89).is_ok());
        assert!(PresetNumber::new(90).is_ok());
        assert!(PresetNumber::new(255).is_ok());

        // No longer has invalid numbers since max is now 255
    }

    #[test]
    fn test_preset_number_try_from() {
        // Valid conversion
        let preset =
            PresetNumber::try_from(50).unwrap_or_else(|e| panic!("Valid preset number: {e:?}"));
        assert_eq!(preset.value(), 50);

        // Test higher values are now valid
        let preset =
            PresetNumber::try_from(90).unwrap_or_else(|e| panic!("Valid preset number: {e:?}"));
        assert_eq!(preset.value(), 90);

        let preset =
            PresetNumber::try_from(255).unwrap_or_else(|e| panic!("Valid preset number: {e:?}"));
        assert_eq!(preset.value(), 255);
    }

    #[test]
    fn test_preset_action_values() {
        assert_eq!(PresetAction::Reset as u8, 0x00);
        assert_eq!(PresetAction::Set as u8, 0x01);
        assert_eq!(PresetAction::Recall as u8, 0x02);
    }

    visca_test!(
        PresetCommand,
        test_preset_command_reset,
        PresetCommand {
            action: PresetAction::Reset,
            preset_number: PresetNumber::new(10)
                .unwrap_or_else(|e| panic!("Valid preset number: {e:?}")),
        },
        &[0x81, 0x01, 0x04, 0x3F, 0x00, 0x0A,  VISCA_TERMINATOR]
    );

    visca_test!(
        PresetCommand,
        test_preset_command_set,
        PresetCommand {
            action: PresetAction::Set,
            preset_number: PresetNumber::new(45)
                .unwrap_or_else(|e| panic!("Valid preset number: {e:?}")),
        },
        &[0x81, 0x01, 0x04, 0x3F, 0x01, 0x2D,  VISCA_TERMINATOR]
    );

    visca_test!(
        PresetCommand,
        test_preset_command_recall,
        PresetCommand {
            action: PresetAction::Recall,
            preset_number: PresetNumber::new(89)
                .unwrap_or_else(|e| panic!("Valid preset number: {e:?}")),
        },
        &[0x81, 0x01, 0x04, 0x3F, 0x02, 0x59,  VISCA_TERMINATOR]
    );

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
