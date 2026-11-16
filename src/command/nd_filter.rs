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

use crate::wire::WireValue;

use crate::{
    command::{
        bytes::{constants::nd_filter, FrameWriter},
        encode::WireEncode,
    },
    error::Error,
};

/// ND filter mode for Sony FR7.
#[crate::value_enum]
pub enum NdFilterMode {
    /// Preset mode - ND is set to discrete levels (user-defined presets).
    Preset = 0,
    /// Variable mode - ND can be adjusted in fine steps.
    Variable = 1,
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

impl WireEncode for NdFilterModeCommand {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&nd_filter::MODE)
            .byte(self.mode.to_wire())
            .finish()
    }
}

impl NdFilterModeCommand {
    /// Create a new ND filter mode command.
    pub fn new(mode: NdFilterMode) -> Self {
        Self { mode }
    }

    /// Returns the requested ND filter mode.
    #[must_use]
    pub const fn mode(self) -> NdFilterMode {
        self.mode
    }
}

/// Direct ND filter value command for variable mode.
///
/// # Sony FR7 Specific
/// Command: `8x 01 7E 04 42 00 00 0p 0q FF` (R7, p.14)
/// - Value 0x0000 = ND 1/4 (2 stops, minimum ND)
/// - Value 0x0014 = ND 1/128 (7 stops, maximum density)
/// - Density codes advance in quarter-stop steps
#[crate::value(min = 0, max = 0x0014, display = "ND Density {}")]
pub struct NdFilterDensity(u16);

impl WireEncode for NdFilterDensity {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&nd_filter::DIRECT)
            .nibbles::<2>(self.value())
            .finish()
    }
}

impl NdFilterDensity {
    /// The densest variable-ND position.
    pub(crate) const MAX_VALUE: u16 = 0x0014;

    /// Create from a stop value (2.0 to 7.0 stops).
    ///
    /// Each raw unit is a quarter stop: `2.0` maps to the minimum density
    /// (`0x0000`) and `7.0` to the maximum (`0x0014`). Like other unit
    /// conversions, this uses `f32` arithmetic rounded to the nearest unit,
    /// with halves away from zero.
    pub fn from_stops(stops: f32) -> Result<Self, Error> {
        if !(2.0..=7.0).contains(&stops) {
            return Err(Error::InvalidParameter {
                parameter: "ND filter stops",
                value: stops.to_string().into(),
                reason: "Value must be finite and between 2.0 and 7.0 stops".into(),
            });
        }
        // Convert stops to value: 2 stops = 0x0000, 7 stops = 0x0014
        // Linear mapping: (stops - 2) * 4 = value
        let value = ((stops - 2.0) * 4.0).round() as u16;
        Self::new(value)
    }

    /// Returns the optical attenuation in stops, from 2 through 7.
    #[must_use]
    pub fn to_stops(self) -> f32 {
        2.0 + f32::from(self.value()) / 4.0
    }
}

/// ND filter step adjustment direction.
#[crate::value_enum]
pub enum NdFilterStep {
    /// Increase ND by one step (more density).
    Up = 2,
    /// Decrease ND by one step (less density).
    Down = 3,
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

impl WireEncode for NdFilterStepCommand {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&nd_filter::STEP)
            .byte(self.direction.to_wire())
            .finish()
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

    /// Returns whether automatic ND control is enabled.
    #[must_use]
    pub const fn enabled(self) -> bool {
        self.enabled
    }
}

impl WireEncode for AutoNdCommand {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        FrameWriter::new(camera_id, buffer)
            .bytes(&nd_filter::AUTO)
            .byte(if self.enabled { 0x02 } else { 0x03 })
            .finish()
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
        NdFilterDensity,
        test_nd_filter_value_min,
        NdFilterDensity::new(0x0000).expect("Failed to create NdFilterDensity with valid value"),
        &[
            0x81,
            0x01,
            0x7E,
            0x04,
            0x42,
            0x00,
            0x00,
            0x00,
            0x00,
            VISCA_TERMINATOR
        ]
    );

    visca_test!(
        NdFilterDensity,
        test_nd_filter_value_max,
        NdFilterDensity::new(0x0014).expect("Failed to create NdFilterDensity with valid value"),
        &[
            0x81,
            0x01,
            0x7E,
            0x04,
            0x42,
            0x00,
            0x00,
            0x01,
            0x04,
            VISCA_TERMINATOR
        ]
    );

    #[test]
    fn test_nd_filter_value_out_of_range() {
        assert!(NdFilterDensity::new(0x0015).is_err());
    }

    #[test]
    fn test_nd_filter_from_stops() {
        let cmd = NdFilterDensity::from_stops(2.0)
            .expect("Failed to create NdFilterDensity from valid stops");
        assert_eq!(cmd.value(), 0x0000);

        let cmd = NdFilterDensity::from_stops(7.0)
            .expect("Failed to create NdFilterDensity from valid stops");
        assert_eq!(cmd.value(), 0x0014);

        let cmd = NdFilterDensity::from_stops(4.5)
            .expect("Failed to create NdFilterDensity from valid stops");
        assert_eq!(cmd.value(), 0x000A); // (4.5 - 2) * 4 = 10

        assert!(NdFilterDensity::from_stops(1.5).is_err());
        assert!(NdFilterDensity::from_stops(8.0).is_err());
        assert_eq!(NdFilterDensity::MIN.to_stops(), 2.0);
        assert_eq!(NdFilterDensity::MAX.to_stops(), 7.0);
        for code in 0..=20 {
            let density = NdFilterDensity::new(code).expect("source-backed quarter-stop code");
            assert_eq!(
                NdFilterDensity::from_stops(density.to_stops()).expect("exact stop step"),
                density
            );
        }
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(NdFilterDensity::from_stops(invalid).is_err());
        }
    }

    #[test]
    fn stop_conversion_rounds_every_half_quarter_stop() {
        for unit in 0..20 {
            let half = 2.0 + (f32::from(unit) + 0.5) / 4.0;
            assert_eq!(
                NdFilterDensity::from_stops(half)
                    .expect("half-quarter in range")
                    .value(),
                unit + 1
            );
            assert_eq!(
                NdFilterDensity::from_stops(half - 0.001)
                    .expect("below half-quarter in range")
                    .value(),
                unit
            );
        }
        assert_eq!(
            NdFilterDensity::from_stops(6.99)
                .expect("6.99 stops")
                .value(),
            0x14
        );
        assert_eq!(
            NdFilterDensity::from_stops(2.24)
                .expect("2.24 stops")
                .value(),
            0x01
        );
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

#[cfg(test)]
mod float_range_tests {
    use super::*;

    #[test]
    #[allow(clippy::expect_used)]
    fn invalid_stop_errors_preserve_the_float_input() {
        for stops in [1.5, 7.5, f32::INFINITY, f32::NAN] {
            let error = NdFilterDensity::from_stops(stops).expect_err("outside range");
            assert!(
                matches!(error, Error::InvalidParameter { value, .. } if value == stops.to_string())
            );
        }
    }
}
