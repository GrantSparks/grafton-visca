//! Pan/tilt speed-step commands for Sony FR7.
//!
//! The FR7 supports switching between normal 24-step and extended 50-step
//! pan/tilt speed ranges.

use crate::{
    command::{bytes::builder::ConstCommandBuilder, encode::WireEncode},
    error::Error,
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

/// Command to set the pan/tilt speed-step range on Sony FR7.
///
/// # Sony FR7 Specific
/// Command: `81 01 06 45 pp FF`
/// - pp = `08` (normal 24-step range)
/// - pp = `18` (extended 50-step range)
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

impl WireEncode for SetVariableSpeedMode {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::bytes::constants;

        let mode_byte = match self.mode {
            VariableSpeedMode::Standard24 => 0x08,
            VariableSpeedMode::Fine50 => 0x18,
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
        &[0x81, 0x01, 0x06, 0x45, 0x08, VISCA_TERMINATOR]
    );

    visca_test!(
        SetVariableSpeedMode,
        test_variable_speed_mode_fine50,
        SetVariableSpeedMode::new(VariableSpeedMode::Fine50),
        &[0x81, 0x01, 0x06, 0x45, 0x18, VISCA_TERMINATOR]
    );
}
