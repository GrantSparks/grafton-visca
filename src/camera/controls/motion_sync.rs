//! Unified Motion Sync control implementation using Mode trait.

use crate::{camera::CameraSend, mode::Mode, Error, MotionSyncMode, MotionSyncSpeed};

/// Unified Motion Sync control methods for cameras that support this feature.
///
/// This trait provides motion sync control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
pub trait MotionSyncControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Sets the motion sync mode (on/off).
    ///
    /// This PtzOptics-specific feature coordinates pan, tilt, and zoom movements
    /// for smoother preset recalls.
    ///
    /// # Arguments
    /// * `mode` - The motion sync mode to set
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    fn set_motion_sync_mode(
        &self,
        mode: MotionSyncMode,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Sets the motion sync speed.
    ///
    /// # Arguments
    /// * `speed` - Speed value from 1 to 24
    ///
    /// # Errors
    /// Returns an error if:
    /// - The camera doesn't support motion sync
    /// - The speed is outside the valid range (1-24)
    fn set_motion_sync_speed(&self, speed: u8) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Sets the motion sync speed using a preset value.
    ///
    /// # Arguments
    /// * `speed` - Preset speed (Slow, Normal, Fast)
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    fn set_motion_sync_preset_speed(
        &self,
        speed: MotionSyncSpeed,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Gets the current motion sync mode.
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    fn get_motion_sync_mode(&self) -> <Self::Mode as Mode>::Ret<'_, Result<MotionSyncMode, Error>>;

    /// Gets the current motion sync speed.
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    fn get_motion_sync_speed(
        &self,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<MotionSyncSpeed, Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> MotionSyncControl for crate::camera::UnifiedCamera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::motion_sync::MotionSync,
    Self: CameraSend<M>,
{
    type Mode = M;

    fn set_motion_sync_mode(&self, mode: MotionSyncMode) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::motion_sync::MotionSyncModeCommand;
        let cmd = MotionSyncModeCommand::new(mode);
        self.send_and_complete(cmd)
    }

    fn set_motion_sync_speed(&self, speed: u8) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::motion_sync::MotionSyncSpeedCommand;
        match MotionSyncSpeedCommand::new(speed) {
            Ok(cmd) => self.send_and_complete(cmd),
            Err(_) => self.error(Error::InvalidParameter {
                parameter: "speed",
                value: speed.to_string().into(),
                reason: "must be between 1 and 24".into(),
            }),
        }
    }

    fn set_motion_sync_preset_speed(
        &self,
        speed: MotionSyncSpeed,
    ) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::motion_sync::MotionSyncSpeedCommand;
        let cmd = MotionSyncSpeedCommand::from_preset(speed);
        self.send_and_complete(cmd)
    }

    fn get_motion_sync_mode(&self) -> M::Ret<'_, Result<MotionSyncMode, Error>> {
        use crate::command::inquiry_structs::MotionSyncModeInquiry;
        self.send_and_parse(MotionSyncModeInquiry)
    }

    fn get_motion_sync_speed(&self) -> M::Ret<'_, Result<MotionSyncSpeed, Error>> {
        use crate::command::inquiry_structs::MotionSyncSpeedInquiry;
        self.send_and_parse(MotionSyncSpeedInquiry)
    }
}
