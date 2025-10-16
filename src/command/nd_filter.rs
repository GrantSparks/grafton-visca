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

use std::borrow::Cow;

use crate::{
    command::{
        bytes::{constants, ConstCommandBuilder},
        encode::ViscaCommand,
        InquiryKind,
    },
    constants::CameraVariant,
    error::Error,
    timeout::CommandCategory,
};

/// ND filter mode for Sony FR7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum NdFilterMode {
    /// Preset mode - ND is set to discrete levels (user-defined presets).
    Preset,
    /// Variable mode - ND can be adjusted in fine steps.
    Variable,
}

impl From<NdFilterMode> for u8 {
    fn from(mode: NdFilterMode) -> u8 {
        match mode {
            NdFilterMode::Preset => 0x00,
            NdFilterMode::Variable => 0x01,
        }
    }
}

/// Set the ND filter mode (preset or variable).
///
/// # Sony FR7 Specific
/// Command: `8x 01 7E 04 52 0p FF`
/// - p = 0 (Preset mode)
/// - p = 1 (Variable mode)
#[derive(Debug, Clone, Copy)]
pub struct NdFilterModeCommand {
    mode: NdFilterMode,
}

impl ViscaCommand for NdFilterModeCommand {
    type Response = ();
    const MAX_SIZE: usize = 7;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        ConstCommandBuilder::<7>::new()
            .append(constants::nd_filter::CONTROL_PREFIX)
            .push(u8::from(self.mode))
            .with_camera_id(camera_id)
            .terminate()
            .build_into(buffer)
    }

    fn response_kind(&self) -> Option<InquiryKind> {
        None
    }

    fn validate_for_model(&self, model: CameraVariant) -> Result<(), Error> {
        match model {
            CameraVariant::SonyFR7 => Ok(()),
            _ => Err(Error::ModelValidation {
                model,
                command: Cow::Borrowed("NdFilterModeCommand"),
                reason: Cow::Borrowed("ND filter commands are only supported on Sony FR7 cameras"),
            }),
        }
    }
}

impl NdFilterModeCommand {
    /// Create a new ND filter mode command.
    pub fn new(mode: NdFilterMode) -> Self {
        Self { mode }
    }
}

/// Direct ND filter value command for variable mode.
///
/// # Sony FR7 Specific
/// Command: `8x 01 7E 04 42 00 0p 0q FF`
/// - Value 0x0000 = ND 1/4 (2 stops, minimum ND)
/// - Value 0x0014 = ND 1/128 (7 stops, maximum density)
/// - Linear scale for optical density (each increment ~0.5 stop)
#[derive(Debug, Clone, Copy)]
pub struct NdFilterValue {
    value: u16,
}

impl ViscaCommand for NdFilterValue {
    type Response = ();
    const MAX_SIZE: usize = 9;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        ConstCommandBuilder::<9>::new()
            .append(constants::nd_filter::DIRECT_PREFIX)
            .push_nibble_pair(self.value)
            .with_camera_id(camera_id)
            .terminate()
            .build_into(buffer)
    }

    fn response_kind(&self) -> Option<InquiryKind> {
        None
    }

    fn validate_for_model(&self, model: CameraVariant) -> Result<(), Error> {
        match model {
            CameraVariant::SonyFR7 => Ok(()),
            _ => Err(Error::ModelValidation {
                model,
                command: Cow::Borrowed("NdFilterValue"),
                reason: Cow::Borrowed("ND filter commands are only supported on Sony FR7 cameras"),
            }),
        }
    }
}

impl NdFilterValue {
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

/// ND filter step adjustment direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NdFilterStep {
    /// Increase ND by one step (more density).
    Up,
    /// Decrease ND by one step (less density).
    Down,
}

impl From<NdFilterStep> for u8 {
    fn from(step: NdFilterStep) -> u8 {
        match step {
            NdFilterStep::Up => 0x02,
            NdFilterStep::Down => 0x03,
        }
    }
}

/// ND filter step adjustment command.
///
/// # Sony FR7 Specific
/// Command: `8x 01 7E 04 12 0p FF`
/// - p = 02 (ND Filter Up - increase ND one step)
/// - p = 03 (ND Filter Down - decrease ND one step)
///   Works in Variable mode to bump ND in small increments.
#[derive(Debug, Clone, Copy)]
pub struct NdFilterStepCommand {
    direction: NdFilterStep,
}

impl ViscaCommand for NdFilterStepCommand {
    type Response = ();
    const MAX_SIZE: usize = 7;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        ConstCommandBuilder::<7>::new()
            .append(constants::nd_filter::MODE_PREFIX)
            .push(u8::from(self.direction))
            .with_camera_id(camera_id)
            .terminate()
            .build_into(buffer)
    }

    fn response_kind(&self) -> Option<InquiryKind> {
        None
    }

    fn validate_for_model(&self, model: CameraVariant) -> Result<(), Error> {
        match model {
            CameraVariant::SonyFR7 => Ok(()),
            _ => Err(Error::ModelValidation {
                model,
                command: Cow::Borrowed("NdFilterStepCommand"),
                reason: Cow::Borrowed("ND filter commands are only supported on Sony FR7 cameras"),
            }),
        }
    }
}

impl NdFilterStepCommand {
    /// Create a new ND filter step command.
    pub fn new(direction: NdFilterStep) -> Self {
        Self { direction }
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
#[derive(Debug, Clone, Copy)]
pub struct AutoNdCommand {
    enabled: bool,
}

impl AutoNdCommand {
    /// Create a new auto ND command.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

impl ViscaCommand for AutoNdCommand {
    type Response = ();
    const MAX_SIZE: usize = 7;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        ConstCommandBuilder::<7>::new()
            .append(constants::nd_filter::LEVEL_PREFIX)
            .push(if self.enabled { 0x02 } else { 0x03 })
            .with_camera_id(camera_id)
            .terminate()
            .build_into(buffer)
    }

    fn response_kind(&self) -> Option<InquiryKind> {
        None
    }

    fn validate_for_model(&self, model: CameraVariant) -> Result<(), Error> {
        match model {
            CameraVariant::SonyFR7 => Ok(()),
            _ => Err(Error::ModelValidation {
                model,
                command: Cow::Borrowed("AutoNdCommand"),
                reason: Cow::Borrowed("ND filter commands are only supported on Sony FR7 cameras"),
            }),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    use crate::{command::bytes::VISCA_TERMINATOR, macros::test_utils::visca_test};

    visca_test!(
        NdFilterMode,
        test_nd_filter_mode_preset,
        NdFilterModeCommand::new(NdFilterMode::Preset),
        &[0x81, 0x01, 0x7E, 0x04, 0x52, 0x00, VISCA_TERMINATOR]
    );

    visca_test!(
        NdFilterMode,
        test_nd_filter_mode_variable,
        NdFilterModeCommand::new(NdFilterMode::Variable),
        &[0x81, 0x01, 0x7E, 0x04, 0x52, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        NdFilterValue,
        test_nd_filter_value_min,
        NdFilterValue::new(0x0000).expect("Failed to create NdFilterValue with valid value"),
        &[
            0x81,
            0x01,
            0x7E,
            0x04,
            0x42,
            0x00,
            0x00,
            0x00,
            VISCA_TERMINATOR
        ]
    );

    visca_test!(
        NdFilterValue,
        test_nd_filter_value_max,
        NdFilterValue::new(0x0014).expect("Failed to create NdFilterValue with valid value"),
        &[
            0x81,
            0x01,
            0x7E,
            0x04,
            0x42,
            0x00,
            0x01,
            0x04,
            VISCA_TERMINATOR
        ]
    );

    #[test]
    fn test_nd_filter_value_out_of_range() {
        assert!(NdFilterValue::new(0x0015).is_err());
    }

    #[test]
    fn test_nd_filter_from_stops() {
        let cmd = NdFilterValue::from_stops(2.0)
            .expect("Failed to create NdFilterValue from valid stops");
        assert_eq!(cmd.value, 0x0000);

        let cmd = NdFilterValue::from_stops(7.0)
            .expect("Failed to create NdFilterValue from valid stops");
        assert_eq!(cmd.value, 0x0014);

        let cmd = NdFilterValue::from_stops(4.5)
            .expect("Failed to create NdFilterValue from valid stops");
        assert_eq!(cmd.value, 0x000A); // (4.5 - 2) * 4 = 10

        assert!(NdFilterValue::from_stops(1.5).is_err());
        assert!(NdFilterValue::from_stops(8.0).is_err());
    }

    visca_test!(
        NdFilterStep,
        test_nd_filter_step_up,
        NdFilterStepCommand::new(NdFilterStep::Up),
        &[0x81, 0x01, 0x7E, 0x04, 0x12, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        NdFilterStep,
        test_nd_filter_step_down,
        NdFilterStepCommand::new(NdFilterStep::Down),
        &[0x81, 0x01, 0x7E, 0x04, 0x12, 0x03, VISCA_TERMINATOR]
    );

    visca_test!(
        AutoNdCommand,
        test_auto_nd_on,
        AutoNdCommand::new(true),
        &[0x81, 0x01, 0x7E, 0x04, 0x53, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        AutoNdCommand,
        test_auto_nd_off,
        AutoNdCommand::new(false),
        &[0x81, 0x01, 0x7E, 0x04, 0x53, 0x03, VISCA_TERMINATOR]
    );
}
