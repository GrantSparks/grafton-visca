//! Motion Sync commands for PtzOptics cameras.
//!
//! Motion Sync is a PtzOptics-specific feature (firmware 1.1.6+) that coordinates
//! pan, tilt, and zoom movements to start and stop simultaneously for preset recalls.
//! This provides smoother and more synchronized arrival on preset positions.

use crate::{
    command::{bytes::builder::ConstCommandBuilder, encode::WireEncode, MotionSyncMode},
    error::Error,
    types::MotionSyncSpeed,
};

/// Command to control Motion Sync mode (on/off).
///
/// PtzOptics-specific command that enables or disables synchronized movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetMotionSyncMode {
    /// The motion sync mode to set.
    pub mode: MotionSyncMode,
}

impl SetMotionSyncMode {
    /// Creates a new motion sync mode command.
    pub const fn new(mode: MotionSyncMode) -> Self {
        Self { mode }
    }
}

impl WireEncode for SetMotionSyncMode {
    fn write_into(
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
}

/// Command to set Motion Sync speed.
///
/// PtzOptics-specific command that sets the maximum speed for synchronized
/// movements. [`MotionSyncSpeed`] owns the `1..=24` range and the
/// [`MotionSyncPreset`](crate::MotionSyncPreset) mapping; the selected profile's maximum is applied
/// during request preparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetMotionSyncPreset {
    speed: MotionSyncSpeed,
}

impl SetMotionSyncPreset {
    /// Creates a motion sync speed command.
    #[must_use]
    pub const fn new(speed: MotionSyncSpeed) -> Self {
        Self { speed }
    }

    /// Returns the configured motion-sync speed.
    #[must_use]
    pub const fn speed(self) -> MotionSyncSpeed {
        self.speed
    }
}

impl WireEncode for SetMotionSyncPreset {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::bytes::constants;

        ConstCommandBuilder::<6>::from_prefix(constants::motion_sync::SPEED_PREFIX)
            .with_camera_id(camera_id)
            .push(self.speed.value())
            .terminate()
            .build_into(buffer)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::macros::test_utils::visca_test;
    use crate::MotionSyncPreset;

    visca_test!(
        MotionSyncMode,
        test_motion_sync_mode_on,
        SetMotionSyncMode::new(MotionSyncMode::On),
        &[0x81, 0x0A, 0x11, 0x13, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        MotionSyncMode,
        test_motion_sync_mode_off,
        SetMotionSyncMode::new(MotionSyncMode::Off),
        &[0x81, 0x0A, 0x11, 0x13, 0x03, VISCA_TERMINATOR]
    );

    visca_test!(
        MotionSyncPreset,
        test_motion_sync_speed_min,
        SetMotionSyncPreset::new(MotionSyncSpeed::new(1).unwrap()),
        &[0x81, 0x0A, 0x11, 0x14, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        MotionSyncPreset,
        test_motion_sync_speed_max,
        SetMotionSyncPreset::new(MotionSyncSpeed::new(24).unwrap()),
        &[0x81, 0x0A, 0x11, 0x14, 0x18, VISCA_TERMINATOR]
    );

    #[test]
    fn test_motion_sync_speed_out_of_range() {
        assert!(MotionSyncSpeed::new(0).is_err());
        assert!(MotionSyncSpeed::new(25).is_err());
    }

    visca_test!(
        MotionSyncPreset,
        test_motion_sync_speed_slow,
        SetMotionSyncPreset::new(MotionSyncPreset::Slow.into()),
        &[0x81, 0x0A, 0x11, 0x14, 0x08, VISCA_TERMINATOR]
    );

    visca_test!(
        MotionSyncPreset,
        test_motion_sync_speed_normal,
        SetMotionSyncPreset::new(MotionSyncPreset::Normal.into()),
        &[0x81, 0x0A, 0x11, 0x14, 0x10, VISCA_TERMINATOR]
    );

    visca_test!(
        MotionSyncPreset,
        test_motion_sync_speed_fast,
        SetMotionSyncPreset::new(MotionSyncPreset::Fast.into()),
        &[0x81, 0x0A, 0x11, 0x14, 0x18, VISCA_TERMINATOR]
    );
}
