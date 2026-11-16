//! Motion Sync commands for PtzOptics cameras.
//!
//! Motion Sync is a PtzOptics-specific feature (firmware 1.1.6+) that coordinates
//! pan, tilt, and zoom movements to start and stop simultaneously for preset recalls.
//! This provides smoother and more synchronized arrival on preset positions.

use crate::{
    command::{
        bytes::{constants::motion_sync, FrameWriter},
        encode::WireEncode,
    },
    error::Error,
    types::MotionSyncSpeed,
};

crate::macros::consolidated::on_off_command! {
    /// Command to control Motion Sync mode (on/off).
    ///
    /// PtzOptics-specific command that enables or disables synchronized movement.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SetMotionSyncMode {
        /// The motion sync mode to set.
        pub mode: bool,
    };
    prefix = motion_sync::MODE;
    convention = crate::wire::BoolConvention::OnIs02;
}

impl SetMotionSyncMode {
    /// Creates a new motion sync mode command.
    pub const fn new(mode: bool) -> Self {
        Self { mode }
    }
}

/// Command to set Motion Sync speed.
///
/// PtzOptics-specific command that sets the maximum speed for synchronized
/// movements. [`crate::types::MotionSyncSpeed`] owns the `1..=24` range and the
/// speed constants; the selected profile's maximum is applied
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
        FrameWriter::new(camera_id, buffer)
            .bytes(&motion_sync::SPEED)
            .byte(self.speed.value())
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::macros::test_utils::visca_test;

    visca_test!(
        SetMotionSyncMode,
        test_motion_sync_mode_on,
        SetMotionSyncMode::new(true),
        &[0x81, 0x0A, 0x11, 0x13, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        SetMotionSyncMode,
        test_motion_sync_mode_off,
        SetMotionSyncMode::new(false),
        &[0x81, 0x0A, 0x11, 0x13, 0x03, VISCA_TERMINATOR]
    );

    visca_test!(
        SetMotionSyncPreset,
        test_motion_sync_speed_min,
        SetMotionSyncPreset::new(MotionSyncSpeed::new(1).unwrap()),
        &[0x81, 0x0A, 0x11, 0x14, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        SetMotionSyncPreset,
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
        SetMotionSyncPreset,
        test_motion_sync_speed_slow,
        SetMotionSyncPreset::new(MotionSyncSpeed::SLOW),
        &[0x81, 0x0A, 0x11, 0x14, 0x08, VISCA_TERMINATOR]
    );

    visca_test!(
        SetMotionSyncPreset,
        test_motion_sync_speed_normal,
        SetMotionSyncPreset::new(MotionSyncSpeed::NORMAL),
        &[0x81, 0x0A, 0x11, 0x14, 0x10, VISCA_TERMINATOR]
    );

    visca_test!(
        SetMotionSyncPreset,
        test_motion_sync_speed_fast,
        SetMotionSyncPreset::new(MotionSyncSpeed::FAST),
        &[0x81, 0x0A, 0x11, 0x14, 0x18, VISCA_TERMINATOR]
    );
}
