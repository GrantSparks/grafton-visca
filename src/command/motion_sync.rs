//! Motion Sync commands for PtzOptics cameras.
//!
//! Motion Sync is a PtzOptics-specific feature (firmware 1.1.6+) that coordinates
//! pan, tilt, and zoom movements to start and stop simultaneously for preset recalls.
//! This provides smoother and more synchronized arrival on preset positions.

use crate::{
    command::{
        bytes::builder::ConstCommandBuilder, encode_visca::ViscaEncode, MotionSyncMode,
        MotionSyncPreset, ViscaResponseType,
    },
    error::Error,
    timeout::CommandCategory,
};

/// Command to control Motion Sync mode (on/off).
///
/// PtzOptics-specific command that enables or disables synchronized movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionSyncModeCommand {
    /// The motion sync mode to set.
    pub mode: MotionSyncMode,
}

impl MotionSyncModeCommand {
    /// Creates a new motion sync mode command.
    pub const fn new(mode: MotionSyncMode) -> Self {
        Self { mode }
    }
}

impl ViscaEncode for MotionSyncModeCommand {
    type ViscaResponse = ();
    const MAX_SIZE: usize = 6;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::bytes::constants;

        let mode_byte = self.mode as u8;

        ConstCommandBuilder::<6>::from_prefix(constants::motion_sync::MODE_PREFIX)
            .with_camera_id(camera_id)
            .push(mode_byte)
            .terminate()
            .build_into(buffer)
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None // Command response, not inquiry
    }

    // Override validate_for_model to restrict to PTZOptics cameras
    fn validate_for_model(&self, model: crate::constants::CameraVariant) -> Result<(), Error> {
        use crate::constants::CameraVariant;
        use std::borrow::Cow;

        match model {
            CameraVariant::PtzOpticsG2 | CameraVariant::PtzOpticsG3 => Ok(()),
            _ => Err(Error::ModelValidation {
                model,
                command: Cow::Borrowed("MotionSyncMode"),
                reason: Cow::Borrowed(
                    "Motion Sync is only supported on PTZOptics cameras with firmware 1.1.6+",
                ),
            }),
        }
    }
}

/// Command to set Motion Sync speed.
///
/// PtzOptics-specific command that sets the maximum speed for synchronized movements.
/// Speed values range from 1 to 24.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionSyncPresetCommand {
    /// The speed value (1-24).
    speed: u8,
}

impl MotionSyncPresetCommand {
    /// Creates a new motion sync speed command.
    ///
    /// # Arguments
    /// * `speed` - Speed value from 1 to 24
    ///
    /// # Errors
    /// Returns an error if speed is outside the valid range (1-24).
    pub fn new(speed: u8) -> Result<Self, Error> {
        if speed == 0 || speed > 24 {
            return Err(Error::ParameterOutOfRange {
                parameter: "motion sync speed",
                value: speed as i32,
                min: 1,
                max: 24,
            });
        }
        Ok(Self { speed })
    }

    /// Creates a motion sync speed command from a preset speed enum.
    pub fn from_preset(speed: MotionSyncPreset) -> Self {
        let speed_value = match speed {
            MotionSyncPreset::Slow => 8,
            MotionSyncPreset::Normal => 16,
            MotionSyncPreset::Fast => 24,
        };
        Self { speed: speed_value }
    }
}

impl ViscaEncode for MotionSyncPresetCommand {
    type ViscaResponse = ();
    const MAX_SIZE: usize = 6;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::bytes::constants;

        // The VISCA protocol uses 0x01-0x18 for speeds 1-24
        ConstCommandBuilder::<6>::from_prefix(constants::motion_sync::SPEED_PREFIX)
            .with_camera_id(camera_id)
            .push(self.speed)
            .terminate()
            .build_into(buffer)
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        None // Command response, not inquiry
    }

    // Override validate_for_model to restrict to PTZOptics cameras
    fn validate_for_model(&self, model: crate::constants::CameraVariant) -> Result<(), Error> {
        use crate::constants::CameraVariant;
        use std::borrow::Cow;

        match model {
            CameraVariant::PtzOpticsG2 | CameraVariant::PtzOpticsG3 => Ok(()),
            _ => Err(Error::ModelValidation {
                model,
                command: Cow::Borrowed("MotionSyncPreset"),
                reason: Cow::Borrowed(
                    "Motion Sync is only supported on PTZOptics cameras with firmware 1.1.6+",
                ),
            }),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::macros::test_utils::visca_test;

    visca_test!(
        MotionSyncMode,
        test_motion_sync_mode_on,
        MotionSyncModeCommand::new(MotionSyncMode::On),
        &[0x81, 0x0A, 0x11, 0x13, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        MotionSyncMode,
        test_motion_sync_mode_off,
        MotionSyncModeCommand::new(MotionSyncMode::Off),
        &[0x81, 0x0A, 0x11, 0x13, 0x03, VISCA_TERMINATOR]
    );

    visca_test!(
        MotionSyncPreset,
        test_motion_sync_speed_min,
        MotionSyncPresetCommand::new(1).unwrap(),
        &[0x81, 0x0A, 0x11, 0x14, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        MotionSyncPreset,
        test_motion_sync_speed_max,
        MotionSyncPresetCommand::new(24).unwrap(),
        &[0x81, 0x0A, 0x11, 0x14, 0x18, VISCA_TERMINATOR]
    );

    #[test]
    fn test_motion_sync_speed_out_of_range() {
        assert!(MotionSyncPresetCommand::new(0).is_err());
        assert!(MotionSyncPresetCommand::new(25).is_err());
    }

    visca_test!(
        MotionSyncPreset,
        test_motion_sync_speed_slow,
        MotionSyncPresetCommand::from_preset(MotionSyncPreset::Slow),
        &[0x81, 0x0A, 0x11, 0x14, 0x08, VISCA_TERMINATOR]
    );

    visca_test!(
        MotionSyncPreset,
        test_motion_sync_speed_normal,
        MotionSyncPresetCommand::from_preset(MotionSyncPreset::Normal),
        &[0x81, 0x0A, 0x11, 0x14, 0x10, VISCA_TERMINATOR]
    );

    visca_test!(
        MotionSyncPreset,
        test_motion_sync_speed_fast,
        MotionSyncPresetCommand::from_preset(MotionSyncPreset::Fast),
        &[0x81, 0x0A, 0x11, 0x14, 0x18, VISCA_TERMINATOR]
    );
}
