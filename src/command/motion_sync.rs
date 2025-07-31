//! Motion Sync commands for PTZOptics cameras.
//!
//! Motion Sync is a PTZOptics-specific feature (firmware 1.1.6+) that coordinates
//! pan, tilt, and zoom movements to start and stop simultaneously for preset recalls.
//! This provides smoother and more synchronized arrival on preset positions.

use crate::{
    command::{encode_visca::EncodeVisca, MotionSyncMode, MotionSyncSpeed, ResponseType},
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
        if buffer.len() < 6 {
            return Err(Error::BufferTooSmall {
                required: 6,
                actual: buffer.len(),
            });
        }
        let mode_byte = match self.mode {
            MotionSyncMode::On => 0x02,
            MotionSyncMode::Off => 0x03,
        };
        buffer[0] = camera_id.to_address_byte();
        buffer[1] = 0x0A;
        buffer[2] = 0x11;
        buffer[3] = 0x13;
        buffer[4] = mode_byte;
        buffer[5] = 0xFF;
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
        if buffer.len() < 6 {
            return Err(Error::BufferTooSmall {
                required: 6,
                actual: buffer.len(),
            });
        }
        // The VISCA protocol uses 0x01-0x18 for speeds 1-24
        buffer[0] = camera_id.to_address_byte();
        buffer[1] = 0x0A;
        buffer[2] = 0x11;
        buffer[3] = 0x14;
        buffer[4] = self.speed;
        buffer[5] = 0xFF;
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

    #[test]
    fn test_motion_sync_mode_command() {
        let cmd = MotionSyncModeCommand::new(MotionSyncMode::On);
        let mut buffer = [0u8; 10];
        let len = cmd
            .encode_into(crate::camera_id::CameraId::default(), &mut buffer)
            .unwrap();
        assert_eq!(&buffer[..len], &[0x81, 0x0A, 0x11, 0x13, 0x02, 0xFF]);

        let cmd = MotionSyncModeCommand::new(MotionSyncMode::Off);
        let len = cmd
            .encode_into(crate::camera_id::CameraId::default(), &mut buffer)
            .unwrap();
        assert_eq!(&buffer[..len], &[0x81, 0x0A, 0x11, 0x13, 0x03, 0xFF]);
    }

    #[test]
    fn test_motion_sync_speed_command() {
        let cmd = MotionSyncSpeedCommand::new(1).unwrap();
        let mut buffer = [0u8; 10];
        let len = cmd
            .encode_into(crate::camera_id::CameraId::default(), &mut buffer)
            .unwrap();
        assert_eq!(&buffer[..len], &[0x81, 0x0A, 0x11, 0x14, 0x01, 0xFF]);

        let cmd = MotionSyncSpeedCommand::new(24).unwrap();
        let len = cmd
            .encode_into(crate::camera_id::CameraId::default(), &mut buffer)
            .unwrap();
        assert_eq!(&buffer[..len], &[0x81, 0x0A, 0x11, 0x14, 0x18, 0xFF]);
    }

    #[test]
    fn test_motion_sync_speed_out_of_range() {
        assert!(MotionSyncSpeedCommand::new(0).is_err());
        assert!(MotionSyncSpeedCommand::new(25).is_err());
    }

    #[test]
    fn test_motion_sync_speed_from_preset() {
        let mut buffer = [0u8; 10];

        let slow = MotionSyncSpeedCommand::from_preset(MotionSyncSpeed::Slow);
        let len = slow
            .encode_into(crate::camera_id::CameraId::default(), &mut buffer)
            .unwrap();
        assert_eq!(&buffer[..len], &[0x81, 0x0A, 0x11, 0x14, 0x08, 0xFF]);

        let normal = MotionSyncSpeedCommand::from_preset(MotionSyncSpeed::Normal);
        let len = normal
            .encode_into(crate::camera_id::CameraId::default(), &mut buffer)
            .unwrap();
        assert_eq!(&buffer[..len], &[0x81, 0x0A, 0x11, 0x14, 0x10, 0xFF]);

        let fast = MotionSyncSpeedCommand::from_preset(MotionSyncSpeed::Fast);
        let len = fast
            .encode_into(crate::camera_id::CameraId::default(), &mut buffer)
            .unwrap();
        assert_eq!(&buffer[..len], &[0x81, 0x0A, 0x11, 0x14, 0x18, 0xFF]);
    }
}
