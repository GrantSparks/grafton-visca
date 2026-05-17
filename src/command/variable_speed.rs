//! Variable speed mode commands for Sony FR7.
//!
//! The FR7 supports switching between 24-step and 50-step speed modes for pan/tilt control.
//! When in 50-step mode, pan/tilt speed values can range from 1-50 for finer control.

use crate::{
    command::{bytes::builder::ConstCommandBuilder, encode::ViscaCommand},
    error::Error,
    timeout::CommandCategory,
};

/// Variable speed mode setting for Sony FR7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum VariableSpeedMode {
    /// Standard 24-step speed mode (1-24 speeds).
    Standard24,
    /// Fine 50-step speed mode (1-50 speeds).
    Fine50,
}

/// Command to set variable speed mode on Sony FR7.
///
/// # Sony FR7 Specific
/// Command: `81 01 7E 04 1B 0p FF`
/// - p = 1 (24-step mode)
/// - p = 2 (50-step mode)
#[derive(Debug, Clone, Copy)]
pub struct SetVariableSpeedMode {
    /// The speed mode to set.
    pub mode: VariableSpeedMode,
}

impl SetVariableSpeedMode {
    /// Create a new variable speed mode command.
    pub fn new(mode: VariableSpeedMode) -> Self {
        Self { mode }
    }
}

impl ViscaCommand for SetVariableSpeedMode {
    const MAX_SIZE: usize = 7;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::bytes::constants;

        let mode_byte = match self.mode {
            VariableSpeedMode::Standard24 => 0x01,
            VariableSpeedMode::Fine50 => 0x02,
        };

        ConstCommandBuilder::<7>::from_prefix(constants::variable_speed::CONTROL_PREFIX)
            .with_camera_id(camera_id)
            .push(mode_byte)
            .terminate()
            .build_into(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::macros::test_utils::visca_test;

    visca_test!(
        SetVariableSpeedMode,
        test_variable_speed_mode_standard24,
        SetVariableSpeedMode::new(VariableSpeedMode::Standard24),
        &[0x81, 0x01, 0x7E, 0x04, 0x1B, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        SetVariableSpeedMode,
        test_variable_speed_mode_fine50,
        SetVariableSpeedMode::new(VariableSpeedMode::Fine50),
        &[0x81, 0x01, 0x7E, 0x04, 0x1B, 0x02, VISCA_TERMINATOR]
    );
}
