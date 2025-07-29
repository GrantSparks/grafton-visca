//! Variable speed mode commands for Sony FR7.
//!
//! The FR7 supports switching between 24-step and 50-step speed modes for pan/tilt control.
//! When in 50-step mode, pan/tilt speed values can range from 1-50 for finer control.

use crate::{
    capabilities::{CameraFeature, CommandFeatures},
    command::{encode_visca::EncodeVisca, ResponseType},
    error::Error,
    timeout::CommandCategory,
};

/// Variable speed mode setting for Sony FR7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub struct VariableSpeedModeCommand {
    /// The speed mode to set.
    pub mode: VariableSpeedMode,
}

impl VariableSpeedModeCommand {
    /// Create a new variable speed mode command.
    pub fn new(mode: VariableSpeedMode) -> Self {
        Self { mode }
    }
}

impl EncodeVisca for VariableSpeedModeCommand {
    type Response = ();
    const MAX_SIZE: usize = 7;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        if buffer.len() < 7 {
            return Err(Error::BufferTooSmall {
                required: 7,
                actual: buffer.len(),
            });
        }

        let mode_byte = match self.mode {
            VariableSpeedMode::Standard24 => 0x01,
            VariableSpeedMode::Fine50 => 0x02,
        };

        buffer[0] = camera_id.to_address_byte();
        buffer[1] = 0x01;
        buffer[2] = 0x7E;
        buffer[3] = 0x04;
        buffer[4] = 0x1B;
        buffer[5] = mode_byte;
        buffer[6] = 0xFF;

        Ok(7)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

impl CommandFeatures for VariableSpeedModeCommand {
    fn required_features(&self) -> &[CameraFeature] {
        &[CameraFeature::VariableSpeedMode]
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_speed_mode_standard24() {
        let cmd = VariableSpeedModeCommand::new(VariableSpeedMode::Standard24);
        let mut buffer = [0u8; 10];
        let camera_id = crate::camera_id::CameraId::default();
        let len = cmd
            .encode_into(camera_id, &mut buffer)
            .expect("Failed to encode VariableSpeedModeCommand");
        assert_eq!(len, 7);
        assert_eq!(&buffer[..7], &[0x81, 0x01, 0x7E, 0x04, 0x1B, 0x01, 0xFF]);
    }

    #[test]
    fn test_variable_speed_mode_fine50() {
        let cmd = VariableSpeedModeCommand::new(VariableSpeedMode::Fine50);
        let mut buffer = [0u8; 10];
        let camera_id = crate::camera_id::CameraId::default();
        let len = cmd
            .encode_into(camera_id, &mut buffer)
            .expect("Failed to encode VariableSpeedModeCommand");
        assert_eq!(len, 7);
        assert_eq!(&buffer[..7], &[0x81, 0x01, 0x7E, 0x04, 0x1B, 0x02, 0xFF]);
    }
}
