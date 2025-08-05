//! Motion Sync commands for PTZOptics cameras.
//!
//! Motion Sync is a PTZOptics-specific feature (firmware 1.1.6+) that coordinates
//! pan, tilt, and zoom movements to start and stop simultaneously for preset recalls.
//! This provides smoother and more synchronized arrival on preset positions.

use crate::{
    command::{
        const_encoding::builder::CommandBuilder, encode_visca::EncodeVisca, MotionSyncMode,
        MotionSyncSpeed, ResponseType,
    },
    error::Error,
    timeout::CommandCategory,
};

/// Command to control Motion Sync mode (on/off).
///
/// PTZOptics-specific command that enables or disables synchronized movement.
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

impl EncodeVisca for MotionSyncModeCommand {
    type Response = ();
    const MAX_SIZE: usize = 6;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::const_encoding::constants;

        let mode_byte = match self.mode {
            MotionSyncMode::On => 0x02,
            MotionSyncMode::Off => 0x03,
        };

        let command = CommandBuilder::<6>::from_prefix(constants::motion_sync::MODE_PREFIX)
            .with_camera_id(camera_id)
            .push(mode_byte)
            .build();

        buffer[..6].copy_from_slice(&command);
        Ok(6)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None // Command response, not inquiry
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Command to set Motion Sync speed.
///
/// PTZOptics-specific command that sets the maximum speed for synchronized movements.
/// Speed values range from 1 to 24.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionSyncSpeedCommand {
    /// The speed value (1-24).
    speed: u8,
}

impl MotionSyncSpeedCommand {
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
    pub fn from_preset(speed: MotionSyncSpeed) -> Self {
        let speed_value = match speed {
            MotionSyncSpeed::Slow => 8,
            MotionSyncSpeed::Normal => 16,
            MotionSyncSpeed::Fast => 24,
        };
        Self { speed: speed_value }
    }
}

impl EncodeVisca for MotionSyncSpeedCommand {
    type Response = ();
    const MAX_SIZE: usize = 6;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::const_encoding::constants;

        // The VISCA protocol uses 0x01-0x18 for speeds 1-24
        let command = CommandBuilder::<6>::from_prefix(constants::motion_sync::SPEED_PREFIX)
            .with_camera_id(camera_id)
            .push(self.speed)
            .build();

        buffer[..6].copy_from_slice(&command);
        Ok(6)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None // Command response, not inquiry
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::macros::test_utils::visca_test;

    visca_test!(
        MotionSyncModeCommand,
        test_motion_sync_mode_on,
        MotionSyncModeCommand::new(MotionSyncMode::On),
        &[0x81, 0x0A, 0x11, 0x13, 0x02, 0xFF]
    );

    visca_test!(
        MotionSyncModeCommand,
        test_motion_sync_mode_off,
        MotionSyncModeCommand::new(MotionSyncMode::Off),
        &[0x81, 0x0A, 0x11, 0x13, 0x03, 0xFF]
    );

    visca_test!(
        MotionSyncSpeedCommand,
        test_motion_sync_speed_min,
        MotionSyncSpeedCommand::new(1).unwrap(),
        &[0x81, 0x0A, 0x11, 0x14, 0x01, 0xFF]
    );

    visca_test!(
        MotionSyncSpeedCommand,
        test_motion_sync_speed_max,
        MotionSyncSpeedCommand::new(24).unwrap(),
        &[0x81, 0x0A, 0x11, 0x14, 0x18, 0xFF]
    );

    #[test]
    fn test_motion_sync_speed_out_of_range() {
        assert!(MotionSyncSpeedCommand::new(0).is_err());
        assert!(MotionSyncSpeedCommand::new(25).is_err());
    }

    visca_test!(
        MotionSyncSpeedCommand,
        test_motion_sync_speed_slow,
        MotionSyncSpeedCommand::from_preset(MotionSyncSpeed::Slow),
        &[0x81, 0x0A, 0x11, 0x14, 0x08, 0xFF]
    );

    visca_test!(
        MotionSyncSpeedCommand,
        test_motion_sync_speed_normal,
        MotionSyncSpeedCommand::from_preset(MotionSyncSpeed::Normal),
        &[0x81, 0x0A, 0x11, 0x14, 0x10, 0xFF]
    );

    visca_test!(
        MotionSyncSpeedCommand,
        test_motion_sync_speed_fast,
        MotionSyncSpeedCommand::from_preset(MotionSyncSpeed::Fast),
        &[0x81, 0x0A, 0x11, 0x14, 0x18, 0xFF]
    );
}
