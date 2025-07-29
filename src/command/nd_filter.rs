//! Neutral Density (ND) filter control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera ND filter functionality,
//! including mode selection, value setting, and auto ND control.
//!
//! # VISCA Compliance
//! ND filter commands are vendor-specific extensions to the VISCA protocol.
//!
//! ## Vendor-Specific Commands
//! - All ND filter commands - Sony FR7 specific
//! - The FR7 supports variable ND filter (2 to 7 stops, continuously variable)

use crate::{
    capabilities::{CameraFeature, CommandFeatures},
    command::{encode_visca::EncodeVisca, ResponseType},
    error::Error,
    timeout::CommandCategory,
};

/// ND filter mode for Sony FR7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NDFilterMode {
    /// Preset mode - ND is set to discrete levels (user-defined presets).
    Preset,
    /// Variable mode - ND can be adjusted in fine steps.
    Variable,
}

/// Set the ND filter mode (preset or variable).
///
/// # Sony FR7 Specific
/// Command: `8x 01 7E 04 52 0p FF`
/// - p = 0 (Preset mode)
/// - p = 1 (Variable mode)
#[derive(Debug, Clone)]
pub struct NDFilterModeCommand {
    mode: NDFilterMode,
}

impl NDFilterModeCommand {
    /// Create a new ND filter mode command.
    pub fn new(mode: NDFilterMode) -> Self {
        Self { mode }
    }
}

impl EncodeVisca for NDFilterModeCommand {
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
            NDFilterMode::Preset => 0x00,
            NDFilterMode::Variable => 0x01,
        };

        buffer[0] = camera_id.to_address_byte();
        buffer[1] = 0x01;
        buffer[2] = 0x7E;
        buffer[3] = 0x04;
        buffer[4] = 0x52;
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

impl CommandFeatures for NDFilterModeCommand {
    fn required_features(&self) -> &[CameraFeature] {
        &[CameraFeature::NDFilter]
    }
}

/// Direct ND filter value command for variable mode.
///
/// # Sony FR7 Specific
/// Command: `8x 01 7E 04 42 00 0p 0q FF`
/// - Value 0x0000 = ND 1/4 (2 stops, minimum ND)
/// - Value 0x0014 = ND 1/128 (7 stops, maximum density)
/// - Linear scale for optical density (each increment ~0.5 stop)
#[derive(Debug, Clone)]
pub struct NDFilterValueCommand {
    value: u16,
}

impl NDFilterValueCommand {
    /// Create a new ND filter value command.
    ///
    /// # Arguments
    /// * `value` - ND filter value (0x0000 to 0x0014)
    pub fn new(value: u16) -> Result<Self, Error> {
        if value > 0x0014 {
            return Err(Error::ParameterOutOfRange {
                parameter: "ND filter value",
                value: value as i32,
                min: 0,
                max: 20,
            });
        }
        Ok(Self { value })
    }

    /// Create from a stop value (2.0 to 7.0 stops).
    pub fn from_stops(stops: f32) -> Result<Self, Error> {
        if !(2.0..=7.0).contains(&stops) {
            return Err(Error::ParameterOutOfRange {
                parameter: "ND filter stops",
                value: stops as i32,
                min: 2,
                max: 7,
            });
        }
        // Convert stops to value: 2 stops = 0x0000, 7 stops = 0x0014
        // Linear mapping: (stops - 2) * 4 = value
        let value = ((stops - 2.0) * 4.0) as u16;
        Ok(Self { value })
    }
}

impl EncodeVisca for NDFilterValueCommand {
    type Response = ();
    const MAX_SIZE: usize = 9;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        if buffer.len() < 9 {
            return Err(Error::BufferTooSmall {
                required: 9,
                actual: buffer.len(),
            });
        }

        let high = ((self.value >> 4) & 0x0F) as u8;
        let low = (self.value & 0x0F) as u8;

        buffer[0] = camera_id.to_address_byte();
        buffer[1] = 0x01;
        buffer[2] = 0x7E;
        buffer[3] = 0x04;
        buffer[4] = 0x42;
        buffer[5] = 0x00;
        buffer[6] = high;
        buffer[7] = low;
        buffer[8] = 0xFF;

        Ok(9)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

impl CommandFeatures for NDFilterValueCommand {
    fn required_features(&self) -> &[CameraFeature] {
        &[CameraFeature::NDFilter]
    }
}

/// ND filter step adjustment direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NDFilterStep {
    /// Increase ND by one step (more density).
    Up,
    /// Decrease ND by one step (less density).
    Down,
}

/// ND filter step adjustment command.
///
/// # Sony FR7 Specific
/// Command: `8x 01 7E 04 12 0p FF`
/// - p = 02 (ND Filter Up - increase ND one step)
/// - p = 03 (ND Filter Down - decrease ND one step)
///   Works in Variable mode to bump ND in small increments.
#[derive(Debug, Clone)]
pub struct NDFilterStepCommand {
    direction: NDFilterStep,
}

impl NDFilterStepCommand {
    /// Create a new ND filter step command.
    pub fn new(direction: NDFilterStep) -> Self {
        Self { direction }
    }
}

impl EncodeVisca for NDFilterStepCommand {
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

        let direction_byte = match self.direction {
            NDFilterStep::Up => 0x02,
            NDFilterStep::Down => 0x03,
        };

        buffer[0] = camera_id.to_address_byte();
        buffer[1] = 0x01;
        buffer[2] = 0x7E;
        buffer[3] = 0x04;
        buffer[4] = 0x12;
        buffer[5] = direction_byte;
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

impl CommandFeatures for NDFilterStepCommand {
    fn required_features(&self) -> &[CameraFeature] {
        &[CameraFeature::NDFilter]
    }
}

/// Auto ND filter control.
///
/// # Sony FR7 Specific
/// Command: `8x 01 7E 04 53 0p FF`
/// - p = 02 (Auto ND On)
/// - p = 03 (Auto ND Off)
///   When Auto ND is On, the camera automatically engages the ND filter
///   to maintain exposure (like auto-iris, but using ND).
#[derive(Debug, Clone)]
pub struct AutoNDCommand {
    enabled: bool,
}

impl AutoNDCommand {
    /// Create a new auto ND command.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

impl EncodeVisca for AutoNDCommand {
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

        let state_byte = if self.enabled { 0x02 } else { 0x03 };

        buffer[0] = camera_id.to_address_byte();
        buffer[1] = 0x01;
        buffer[2] = 0x7E;
        buffer[3] = 0x04;
        buffer[4] = 0x53;
        buffer[5] = state_byte;
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

impl CommandFeatures for AutoNDCommand {
    fn required_features(&self) -> &[CameraFeature] {
        &[CameraFeature::NDFilter]
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_nd_filter_mode_command() {
        let cmd = NDFilterModeCommand::new(NDFilterMode::Preset);
        let mut buffer = [0u8; 10];
        let len = cmd
            .encode_into(crate::camera_id::CameraId::default(), &mut buffer)
            .expect("Failed to encode NDFilterModeCommand");
        assert_eq!(&buffer[..len], &[0x81, 0x01, 0x7E, 0x04, 0x52, 0x00, 0xFF]);

        let cmd = NDFilterModeCommand::new(NDFilterMode::Variable);
        let len = cmd
            .encode_into(crate::camera_id::CameraId::default(), &mut buffer)
            .expect("Failed to encode NDFilterModeCommand");
        assert_eq!(&buffer[..len], &[0x81, 0x01, 0x7E, 0x04, 0x52, 0x01, 0xFF]);
    }

    #[test]
    fn test_nd_filter_value_command() {
        let cmd = NDFilterValueCommand::new(0x0000)
            .expect("Failed to create NDFilterValueCommand with valid value");
        let mut buffer = [0u8; 10];
        let len = cmd
            .encode_into(crate::camera_id::CameraId::default(), &mut buffer)
            .expect("Failed to encode NDFilterValueCommand");
        assert_eq!(
            &buffer[..len],
            &[0x81, 0x01, 0x7E, 0x04, 0x42, 0x00, 0x00, 0x00, 0xFF]
        );

        let cmd = NDFilterValueCommand::new(0x0014)
            .expect("Failed to create NDFilterValueCommand with valid value");
        let len = cmd
            .encode_into(crate::camera_id::CameraId::default(), &mut buffer)
            .expect("Failed to encode NDFilterValueCommand");
        assert_eq!(
            &buffer[..len],
            &[0x81, 0x01, 0x7E, 0x04, 0x42, 0x00, 0x01, 0x04, 0xFF]
        );

        // Test out of range
        assert!(NDFilterValueCommand::new(0x0015).is_err());
    }

    #[test]
    fn test_nd_filter_from_stops() {
        let cmd = NDFilterValueCommand::from_stops(2.0)
            .expect("Failed to create NDFilterValueCommand from valid stops");
        assert_eq!(cmd.value, 0x0000);

        let cmd = NDFilterValueCommand::from_stops(7.0)
            .expect("Failed to create NDFilterValueCommand from valid stops");
        assert_eq!(cmd.value, 0x0014);

        let cmd = NDFilterValueCommand::from_stops(4.5)
            .expect("Failed to create NDFilterValueCommand from valid stops");
        assert_eq!(cmd.value, 0x000A); // (4.5 - 2) * 4 = 10

        // Test out of range
        assert!(NDFilterValueCommand::from_stops(1.5).is_err());
        assert!(NDFilterValueCommand::from_stops(8.0).is_err());
    }

    #[test]
    fn test_nd_filter_step_command() {
        let cmd = NDFilterStepCommand::new(NDFilterStep::Up);
        let mut buffer = [0u8; 10];
        let len = cmd
            .encode_into(crate::camera_id::CameraId::default(), &mut buffer)
            .expect("Failed to encode NDFilterStepCommand");
        assert_eq!(&buffer[..len], &[0x81, 0x01, 0x7E, 0x04, 0x12, 0x02, 0xFF]);

        let cmd = NDFilterStepCommand::new(NDFilterStep::Down);
        let len = cmd
            .encode_into(crate::camera_id::CameraId::default(), &mut buffer)
            .expect("Failed to encode NDFilterStepCommand");
        assert_eq!(&buffer[..len], &[0x81, 0x01, 0x7E, 0x04, 0x12, 0x03, 0xFF]);
    }

    #[test]
    fn test_auto_nd_command() {
        let cmd = AutoNDCommand::new(true);
        let mut buffer = [0u8; 10];
        let len = cmd
            .encode_into(crate::camera_id::CameraId::default(), &mut buffer)
            .expect("Failed to encode AutoNDCommand");
        assert_eq!(&buffer[..len], &[0x81, 0x01, 0x7E, 0x04, 0x53, 0x02, 0xFF]);

        let cmd = AutoNDCommand::new(false);
        let len = cmd
            .encode_into(crate::camera_id::CameraId::default(), &mut buffer)
            .expect("Failed to encode AutoNDCommand");
        assert_eq!(&buffer[..len], &[0x81, 0x01, 0x7E, 0x04, 0x53, 0x03, 0xFF]);
    }
}
