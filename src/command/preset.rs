//! Preset position commands for VISCA cameras.
//!
//! This module provides commands for storing and recalling camera positions.
//! `PtzOptics` G2 cameras support up to 128 presets (0-127).

use crate::{
    command::{bytes::builder::ConstCommandBuilder, encode::ViscaCommand},
    error::Error,
    timeout::CommandCategory,
};

/// Action to perform on a preset.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum PresetAction {
    /// Reset/clear the preset.
    Reset = 0x00,
    /// Store current position to preset.
    Set = 0x01,
    /// Move camera to preset position.
    Recall = 0x02,
}

crate::visca_range_type! {
    /// Preset number with validation.
    ///
    /// Valid range: 0 to 255 (0x00 to 0xFF).
    /// Note: Actual valid range depends on camera model:
    /// - PtzOptics G2: 0-127
    /// - PtzOptics G3: 0-127 for raw VISCA until values above 0x7F are target-tested
    /// - Sony FR7: 0-255
    /// - Sony EVI-H100: 0-6
    /// Camera-specific validation is performed when sending commands.
    PresetNumber: u8 {
        min: 0,
        max: 255
    }
}

crate::visca_range_type! {
    /// Preset recall speed.
    ///
    /// Valid range: 1 to 24 (0x01 to 0x18).
    /// Controls the speed at which the camera moves when recalling a preset position.
    /// **Vendor-Specific**: PTZOptics cameras only.
    PresetRecallSpeed: u8 {
        min: 1,
        max: 24
    }
}

/// Command to set the preset recall movement speed.
///
/// This controls how fast the camera moves when recalling a preset position.
/// **Vendor-Specific**: PTZOptics cameras only.
#[derive(Debug, Copy, Clone)]
pub(crate) struct PresetRecallSpeedCommand {
    /// The recall speed to set.
    pub speed: PresetRecallSpeed,
}

impl ViscaCommand for PresetRecallSpeedCommand {
    const MAX_SIZE: usize = 6;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::bytes::constants::preset;

        ConstCommandBuilder::<6>::from_prefix(preset::RECALL_SPEED_PREFIX)
            .with_camera_id(camera_id)
            .push(self.speed.value())
            .terminate()
            .build_into(buffer)
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

impl PresetCommand {}

impl ViscaCommand for PresetCommand {
    const MAX_SIZE: usize = 7;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Preset;

    fn operation_metadata(&self) -> Option<crate::camera::OperationMetadata> {
        use crate::camera::{Axes, OperationMetadata};

        Some(match self.action {
            PresetAction::Recall => OperationMetadata::targeted(Axes::ALL),
            PresetAction::Reset | PresetAction::Set => OperationMetadata::applied_only(Axes::NONE),
        })
    }

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::bytes::constants::preset;

        ConstCommandBuilder::<7>::from_prefix(preset::CONTROL_PREFIX)
            .with_camera_id(camera_id)
            .push(self.action as u8)
            .push(self.preset_number.value())
            .terminate()
            .build_into(buffer)
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use crate::{
        command::bytes::VISCA_TERMINATOR, macros::test_utils::visca_test, timeout::CommandTimeout,
    };

    use super::*;

    #[test]
    fn test_preset_number_new() {
        // Valid preset numbers
        assert!(PresetNumber::new(0).is_ok());
        assert!(PresetNumber::new(45).is_ok());
        assert!(PresetNumber::new(89).is_ok());
        assert!(PresetNumber::new(90).is_ok());
        assert!(PresetNumber::new(255).is_ok());
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
        &[0x81, 0x01, 0x04, 0x3F, 0x00, 0x0A, VISCA_TERMINATOR]
    );

    visca_test!(
        PresetCommand,
        test_preset_command_set,
        PresetCommand {
            action: PresetAction::Set,
            preset_number: PresetNumber::new(45)
                .unwrap_or_else(|e| panic!("Valid preset number: {e:?}")),
        },
        &[0x81, 0x01, 0x04, 0x3F, 0x01, 0x2D, VISCA_TERMINATOR]
    );

    visca_test!(
        PresetCommand,
        test_preset_command_recall,
        PresetCommand {
            action: PresetAction::Recall,
            preset_number: PresetNumber::new(89)
                .unwrap_or_else(|e| panic!("Valid preset number: {e:?}")),
        },
        &[0x81, 0x01, 0x04, 0x3F, 0x02, 0x59, VISCA_TERMINATOR]
    );

    #[test]
    fn preset_255_keeps_data_and_terminator_bytes() {
        for (action, action_byte) in [
            (PresetAction::Reset, 0x00),
            (PresetAction::Set, 0x01),
            (PresetAction::Recall, 0x02),
        ] {
            let command = PresetCommand {
                action,
                preset_number: PresetNumber::new(255)
                    .unwrap_or_else(|error| panic!("valid preset number: {error:?}")),
            };
            let bytes = command
                .to_bytes(crate::CameraId::CAMERA_1)
                .unwrap_or_else(|error| panic!("preset 255 must encode: {error:?}"));
            assert_eq!(
                bytes.as_ref(),
                &[
                    0x81,
                    0x01,
                    0x04,
                    0x3F,
                    action_byte,
                    u8::MAX,
                    VISCA_TERMINATOR,
                ]
            );
        }
    }

    #[test]
    fn test_preset_timeout_kind() {
        let cmd = PresetCommand {
            action: PresetAction::Recall,
            preset_number: PresetNumber::new(0)
                .unwrap_or_else(|e| panic!("Valid preset number: {e:?}")),
        };
        assert_eq!(cmd.timeout_class(), CommandCategory::Preset);
    }

    #[test]
    fn test_preset_command_response_type() {
        let cmd = PresetCommand {
            action: PresetAction::Set,
            preset_number: PresetNumber::new(0)
                .unwrap_or_else(|e| panic!("Valid preset number: {e:?}")),
        };
        assert!(cmd.behavior().command_kind() == crate::command::CommandKind::Command);
    }

    #[test]
    fn test_preset_recall_speed_valid_range() {
        assert!(PresetRecallSpeed::new(1).is_ok());
        assert!(PresetRecallSpeed::new(24).is_ok());
        assert!(PresetRecallSpeed::new(0).is_err());
        assert!(PresetRecallSpeed::new(25).is_err());
    }

    visca_test!(
        PresetRecallSpeedCommand,
        test_preset_recall_speed_min,
        PresetRecallSpeedCommand {
            speed: PresetRecallSpeed::new(1).unwrap_or_else(|e| panic!("Valid speed: {e:?}")),
        },
        &[0x81, 0x01, 0x06, 0x01, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        PresetRecallSpeedCommand,
        test_preset_recall_speed_max,
        PresetRecallSpeedCommand {
            speed: PresetRecallSpeed::new(24).unwrap_or_else(|e| panic!("Valid speed: {e:?}")),
        },
        &[0x81, 0x01, 0x06, 0x01, 0x18, VISCA_TERMINATOR]
    );
}
