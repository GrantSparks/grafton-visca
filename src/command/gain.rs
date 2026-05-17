//! Gain control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera gain settings,
//! including manual gain adjustment, gain limit control, and anti-flicker settings.

use crate::{
    command::{bytes::builder::ConstCommandBuilder, encode::ViscaCommand},
    error::Error,
    timeout::CommandCategory,
    types::{GainLevel, GainLimit},
    visca_command,
};

/// Commands for controlling gain values.
///
/// Provides standard VISCA control operations:
/// - Reset to default value
/// - Increment/decrement by one step
/// - Set to a specific value
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum Gain {
    /// Reset to default value.
    Reset,
    /// Increase value by one step.
    Up,
    /// Decrease value by one step.
    Down,
    /// Set gain to specific value.
    SetValue(GainLevel),
}

// Manual implementation to add model validation
impl ViscaCommand for Gain {
    const MAX_SIZE: usize = 9;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        match self {
            Self::Reset | Self::Up | Self::Down => {
                let control_byte = match self {
                    Self::Reset => 0x00,
                    Self::Up => 0x02,
                    Self::Down => 0x03,
                    _ => unreachable!(),
                };

                ConstCommandBuilder::<6>::from_prefix(
                    crate::command::bytes::constants::gain::CONTROL_PREFIX,
                )
                .with_camera_id(camera_id)
                .push(control_byte)
                .terminate()
                .build_into(buffer)
            }
            Self::SetValue(level) => {
                let value = level.value();
                let high = (value >> 4) & 0x0F;
                let low = value & 0x0F;

                ConstCommandBuilder::<9>::from_prefix(
                    crate::command::bytes::constants::gain::DIRECT_PREFIX,
                )
                .with_camera_id(camera_id)
                .push(high)
                .push(low)
                .terminate()
                .build_into(buffer)
            }
        }
    }
}

visca_command! {
        /// Command to set the automatic gain control limit.
    pub struct GainLimitCommand { limit: GainLimit };
    prefix = [0x01, 0x04, 0x2C];
    param = limit.value();
    max_param_size = 1;
    category = CommandCategory::Quick;
}

impl GainLimitCommand {
    /// Create a new gain limit command.
    pub fn new(limit: GainLimit) -> Self {
        Self { limit }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    use crate::{
        command::{bytes::VISCA_TERMINATOR, encode::ViscaCommand},
        macros::test_utils::visca_test,
        timeout::CommandTimeout,
    };

    visca_test!(
        Gain,
        test_gain_command_reset,
        Gain::Reset,
        &[0x81, 0x01, 0x04, 0x0C, 0x00, VISCA_TERMINATOR]
    );

    visca_test!(
        Gain,
        test_gain_command_up,
        Gain::Up,
        &[0x81, 0x01, 0x04, 0x0C, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        Gain,
        test_gain_command_down,
        Gain::Down,
        &[0x81, 0x01, 0x04, 0x0C, 0x03, VISCA_TERMINATOR]
    );

    #[test]
    fn test_gain_command_set_value() {
        let test_values = vec![0x00, 0x01, 0x03, 0x05, 0x07];
        for value in test_values {
            let gain =
                GainLevel::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = Gain::SetValue(gain);
            let high = (value >> 4) & 0x0F;
            let low = value & 0x0F;
            assert_eq!(
                cmd.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                    .map(|b| b.to_vec())
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
                vec![
                    0x81,
                    0x01,
                    0x04,
                    0x4C,
                    0x00,
                    0x00,
                    high,
                    low,
                    VISCA_TERMINATOR
                ]
            );
        }
    }

    #[test]
    fn test_gain_valid_values() {
        for value in 0x00..=0x07 {
            let gain =
                GainLevel::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let _cmd = Gain::SetValue(gain);
        }
    }

    #[test]
    fn test_gain_limit_command() {
        let test_values = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
        for value in test_values {
            let limit =
                GainLimit::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = GainLimitCommand::new(limit);
            assert_eq!(
                cmd.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                    .map(|b| b.to_vec())
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
                vec![0x81, 0x01, 0x04, 0x2C, value, VISCA_TERMINATOR]
            );
        }
    }

    #[test]
    fn test_command_categories() {
        assert_eq!(Gain::Reset.timeout_class(), CommandCategory::Quick);
        assert_eq!(Gain::Up.timeout_class(), CommandCategory::Quick);
        assert_eq!(Gain::Down.timeout_class(), CommandCategory::Quick);
        assert_eq!(
            Gain::SetValue(
                GainLevel::new(0x05).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
            )
            .timeout_class(),
            CommandCategory::Quick
        );
        assert_eq!(
            GainLimitCommand::new(
                GainLimit::new(0x03).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
            )
            .timeout_class(),
            CommandCategory::Quick
        );
    }

    #[test]
    fn test_response_types() {
        assert!(Gain::Reset.behavior().command_kind() == crate::command::CommandKind::Command);
        assert!(Gain::Up.behavior().command_kind() == crate::command::CommandKind::Command);
        assert!(Gain::Down.behavior().command_kind() == crate::command::CommandKind::Command);
        assert!(
            Gain::SetValue(
                GainLevel::new(0x05).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
            )
            .behavior()
            .command_kind()
                == crate::command::CommandKind::Command
        );
        assert!(
            GainLimitCommand::new(
                GainLimit::new(0x03).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
            )
            .behavior()
            .command_kind()
                == crate::command::CommandKind::Command
        );
    }

    #[test]
    fn test_gain_command_debug() {
        let cmd = Gain::Reset;
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("Reset"));

        let cmd = Gain::SetValue(
            GainLevel::new(0x05).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
        );
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("SetValue"));
    }
}
