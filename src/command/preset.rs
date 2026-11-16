//! Preset position commands for VISCA cameras.
//!
//! This module provides commands for storing and recalling camera positions.
//! `PtzOptics` G2 cameras support up to 128 presets (0-127).

use crate::{
    command::sony_fr7_presets::PresetSpeed,
    command::{
        bytes::{constants::preset, FrameWriter},
        encode::WireEncode,
    },
    error::Error,
};

/// Action to perform on a preset.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub(crate) enum PresetAction {
    /// Reset/clear the preset.
    Reset = 0x00,
    /// Store current position to preset.
    Set = 0x01,
    /// Move camera to preset position.
    Recall = 0x02,
}

/// Zero-based preset number over the union of supported profile domains.
///
/// Preparation validates the selected camera's limit: FR7 0..=99,
/// PTZOptics 0..=127, and Sony EVI-H100 0..=5.
#[crate::value(min = 0, max = 255, display = "Preset Number {}")]
pub struct PresetNumber(u8);

/// Command to set the preset recall movement speed.
///
/// This controls how fast the camera moves when recalling a preset position.
/// **Vendor-Specific**: PTZOptics cameras only.
#[derive(Debug, Copy, Clone)]
pub struct PresetRecallSpeedCommand {
    /// The recall speed to set.
    pub speed: PresetSpeed,
}

impl PresetRecallSpeedCommand {
    /// Create a command that sets the preset-recall speed.
    pub const fn new(speed: PresetSpeed) -> Self {
        Self { speed }
    }
}

impl WireEncode for PresetRecallSpeedCommand {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        // Profile-free encoding still checks this packet's domain. The range
        // itself is shared with profile validation and preparation.
        let range = crate::capabilities::PresetRecallSpeedEncoding::PtzOptics.speed_range();
        if self.speed.value() < range.min() || self.speed.value() > range.max() {
            return Err(Error::parameter_out_of_range(
                "preset_recall_speed",
                i32::from(self.speed.value()),
                i32::from(range.min()),
                i32::from(range.max()),
            ));
        }
        FrameWriter::new(camera_id, buffer)
            .bytes(&preset::RECALL_SPEED)
            .byte(self.speed.value())
            .finish()
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

impl WireEncode for PresetCommand {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&preset::MEMORY)
            .byte(self.action as u8)
            .byte(self.preset_number.value())
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use crate::{command::bytes::VISCA_TERMINATOR, macros::test_utils::visca_test};

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

    // Regression for #683: preset number 255 (0xFF) is a data byte. The frame
    // must still be terminated, encoding to `... 01 FF FF` (data FF then the
    // terminator FF), not truncated to `... 01 FF`.
    visca_test!(
        PresetCommand,
        test_preset_command_set_255_keeps_terminator,
        PresetCommand {
            action: PresetAction::Set,
            preset_number: PresetNumber::new(255)
                .unwrap_or_else(|e| panic!("Valid preset number: {e:?}")),
        },
        &[0x81, 0x01, 0x04, 0x3F, 0x01, 0xFF, VISCA_TERMINATOR]
    );

    /// Exhaustive value-domain sweep for #683: every preset number and action
    /// must encode to a frame whose length matches, that ends in the terminator,
    /// and whose byte *before* the terminator is the preset number itself (so a
    /// trailing 0xFF data byte is never swallowed).
    #[test]
    fn every_preset_number_and_action_terminates() {
        for action in [PresetAction::Reset, PresetAction::Set, PresetAction::Recall] {
            for number in 0..=u8::MAX {
                let command = PresetCommand {
                    action,
                    preset_number: PresetNumber::new(number)
                        .unwrap_or_else(|e| panic!("preset {number} valid: {e:?}")),
                };
                let mut buffer = [0u8; 32];
                let len = command
                    .write_into(crate::camera_id::CameraId::CAMERA_1, &mut buffer)
                    .unwrap_or_else(|e| panic!("preset {number} action {action:?}: {e:?}"));
                assert_eq!(
                    len, 7,
                    "preset {number} action {action:?} must be a 7-byte frame"
                );
                assert_eq!(
                    buffer[..len],
                    [
                        0x81,
                        0x01,
                        0x04,
                        0x3F,
                        action as u8,
                        number,
                        VISCA_TERMINATOR
                    ],
                    "preset {number} action {action:?} wire bytes"
                );
            }
        }
    }

    #[test]
    fn test_preset_recall_speed_valid_range() {
        assert!(PresetSpeed::new(1).is_ok());
        assert!(PresetSpeed::new(24).is_ok());
        assert!(PresetSpeed::new(0).is_err());
        assert!(PresetSpeed::new(50).is_ok());
        assert!(PresetSpeed::new(51).is_err());
    }

    #[test]
    fn ptzoptics_encoder_refuses_fr7_only_union_speeds() -> crate::Result<()> {
        let command = PresetRecallSpeedCommand::new(PresetSpeed::new(25)?);
        let mut buffer = [0; 6];
        assert!(matches!(
            command.write_into(crate::CameraId::CAMERA_1, &mut buffer),
            Err(Error::ParameterOutOfRange { max: 24, .. })
        ));
        Ok(())
    }

    visca_test!(
        PresetRecallSpeedCommand,
        test_preset_recall_speed_min,
        PresetRecallSpeedCommand {
            speed: PresetSpeed::new(1).unwrap_or_else(|e| panic!("Valid speed: {e:?}")),
        },
        &[0x81, 0x01, 0x06, 0x01, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        PresetRecallSpeedCommand,
        test_preset_recall_speed_max,
        PresetRecallSpeedCommand {
            speed: PresetSpeed::new(24).unwrap_or_else(|e| panic!("Valid speed: {e:?}")),
        },
        &[0x81, 0x01, 0x06, 0x01, 0x18, VISCA_TERMINATOR]
    );
}
