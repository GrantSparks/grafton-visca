//! Gain control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera gain settings,
//! including manual gain adjustment, gain limit control, and anti-flicker settings.

// Crate imports
use crate::macros::internal::*;

use crate::{
    command::{
        const_encoding::builder::CommandBuilder, encode_visca::EncodeVisca, response::ResponseType,
    },
    error::Error,
    timeout::CommandCategory,
    types::{GainLevel, GainLimit},
};

/// Commands for controlling gain values.
///
/// Provides standard VISCA control operations:
/// - Reset to default value
/// - Increment/decrement by one step
/// - Set to a specific value
#[derive(Debug, Copy, Clone)]
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

impl Gain {
    // Legacy method - removed in new API
    // pub fn direct<P: crate::camera::CameraProfile>(gain: P::Gain) -> Result<Self, Error> {
    //     ...
    // }
}

// Manual implementation to add model validation
impl EncodeVisca for Gain {
    type Response = ();
    const MAX_SIZE: usize = 9;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn encode_into(
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

                CommandBuilder::<6>::from_prefix(
                    crate::command::const_encoding::constants::gain::CONTROL_PREFIX,
                )
                .with_camera_id(camera_id)
                .push(control_byte)
                .build_into(buffer)
            }
            Self::SetValue(level) => {
                let value = level.value();
                let high = (value >> 4) & 0x0F;
                let low = value & 0x0F;

                CommandBuilder::<9>::from_prefix(
                    crate::command::const_encoding::constants::gain::DIRECT_PREFIX,
                )
                .with_camera_id(camera_id)
                .push(high)
                .push(low)
                .build_into(buffer)
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }
}

visca_builder! {
    /// Command to set the automatic gain control limit.
    pub struct GainLimitCommand {
        /// The gain limit to set.
        limit: GainLimit,
    }
    builder<6> => |builder, limit| {
        builder
            .append(crate::command::const_encoding::constants::gain::GAIN_LIMIT_PREFIX)
            .push(limit.value())
    }
    timeout = Quick;
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
    use crate::command::const_encoding::VISCA_TERMINATOR;
    use crate::command::encode_visca::EncodeVisca;
    use crate::constants::CameraVariant;
    use crate::macros::test_utils::visca_test;

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
        // Test various gain values
        let test_values = vec![0x00, 0x01, 0x03, 0x05, 0x07];
        for value in test_values {
            let gain =
                GainLevel::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = Gain::SetValue(gain);
            let high = (value >> 4) & 0x0F;
            let low = value & 0x0F;
            assert_eq!(
                cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
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
    fn test_gain_command_g2_validation() {
        // Test valid G2 gain values (0x00-0x07)
        for value in 0x00..=0x07 {
            let gain =
                GainLevel::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = Gain::SetValue(gain);
            assert!(cmd.validate_for_model(CameraVariant::PTZOpticsG2).is_ok());
        }

        // Gain itself is limited to 0x00-0x07, which are all valid for G2
        // So all valid Gain instances should pass G2 validation

        // Non-direct commands should always be valid
        assert!(Gain::Reset
            .validate_for_model(CameraVariant::PTZOpticsG2)
            .is_ok());
        assert!(Gain::Up
            .validate_for_model(CameraVariant::PTZOpticsG2)
            .is_ok());
        assert!(Gain::Down
            .validate_for_model(CameraVariant::PTZOpticsG2)
            .is_ok());
    }

    #[test]
    fn test_gain_limit_command() {
        // Test various gain limit values
        let test_values = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
        for value in test_values {
            let limit =
                GainLimit::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = GainLimitCommand::new(limit);
            assert_eq!(
                cmd.try_into_vec(crate::camera_id::CameraId::CAMERA_1)
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
                vec![0x81, 0x01, 0x04, 0x2C, value, VISCA_TERMINATOR]
            );
        }
    }

    #[test]
    fn test_gain_limit_g2_validation() {
        // Test valid G2 gain limit values
        for value in GainLimit::G2_VALID_VALUES {
            let limit =
                GainLimit::new(*value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = GainLimitCommand::new(limit);
            assert!(cmd.validate_for_model(CameraVariant::PTZOpticsG2).is_ok());
        }

        // Test that non-G2 values might fail (depends on what G2_VALID_VALUES contains)
        // Check if value 0x08 is not in G2_VALID_VALUES
        if !GainLimit::G2_VALID_VALUES.contains(&0x08) {
            if let Ok(limit) = GainLimit::new(0x08) {
                let cmd = GainLimitCommand::new(limit);
                assert!(cmd.validate_for_model(CameraVariant::PTZOpticsG2).is_err());
            }
        }
    }

    #[test]
    fn test_command_categories() {
        // All gain commands should be Quick category
        assert_eq!(Gain::Reset.timeout_kind(), CommandCategory::Quick);
        assert_eq!(Gain::Up.timeout_kind(), CommandCategory::Quick);
        assert_eq!(Gain::Down.timeout_kind(), CommandCategory::Quick);
        assert_eq!(
            Gain::SetValue(
                GainLevel::new(0x05).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
            )
            .timeout_kind(),
            CommandCategory::Quick
        );
        assert_eq!(
            GainLimitCommand::new(
                GainLimit::new(0x03).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
            )
            .timeout_kind(),
            CommandCategory::Quick
        );
    }

    #[test]
    fn test_response_types() {
        // All gain commands should return None for response_type
        assert!(Gain::Reset.response_type().is_none());
        assert!(Gain::Up.response_type().is_none());
        assert!(Gain::Down.response_type().is_none());
        assert!(Gain::SetValue(
            GainLevel::new(0x05).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        )
        .response_type()
        .is_none());
        assert!(GainLimitCommand::new(
            GainLimit::new(0x03).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        )
        .response_type()
        .is_none());
    }

    #[test]
    fn test_gain_command_debug() {
        // Test Debug trait implementation
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
