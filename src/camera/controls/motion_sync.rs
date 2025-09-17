//! Motion Sync control implementation using Mode trait.

use crate::{
    camera::ViscaClient, mode::Mode, types::MotionSyncSpeed, Error, MotionSyncMode,
    MotionSyncPreset,
};

/// Motion Sync control methods for cameras that support this feature.
///
/// This trait provides motion sync control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::delegate_to_session]
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
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Sets the motion sync speed.
    ///
    /// # Arguments
    /// * `speed` - Speed value from 1 to 24
    ///
    /// # Errors
    /// Returns an error if:
    /// - The camera doesn't support motion sync
    /// - The speed is outside the valid range (1-24)
    fn set_motion_sync_speed(
        &self,
        speed: MotionSyncSpeed,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Sets the motion sync speed using a preset value.
    ///
    /// # Arguments
    /// * `speed` - Preset speed (Slow, Normal, Fast)
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    fn set_motion_sync_preset_speed(
        &self,
        speed: MotionSyncPreset,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Gets the current motion sync mode.
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    fn motion_sync_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<MotionSyncMode, Error>>;

    /// Gets the current motion sync speed.
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    fn motion_sync_speed(&self) -> <Self::Mode as Mode>::Fut<'_, Result<MotionSyncPreset, Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> MotionSyncControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::motion_sync::MotionSync,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_motion_sync_mode(&self, mode: MotionSyncMode) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::motion_sync::SetMotionSyncMode;
        let cmd = SetMotionSyncMode::new(mode);
        self.execute(cmd)
    }

    fn set_motion_sync_speed(&self, speed: MotionSyncSpeed) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::motion_sync::SetMotionSyncPreset;
        // MotionSyncSpeed is already validated to be in range 1-24
        // SetMotionSyncPreset::new() validates the same range, so this should never fail
        // But we handle the error properly to satisfy clippy
        match SetMotionSyncPreset::new(speed.value()) {
            Ok(cmd) => self.execute(cmd),
            Err(e) => self.error(e),
        }
    }

    fn set_motion_sync_preset_speed(
        &self,
        speed: MotionSyncPreset,
    ) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::motion_sync::SetMotionSyncPreset;
        let cmd = SetMotionSyncPreset::from_preset(speed);
        self.execute(cmd)
    }

    fn motion_sync_mode(&self) -> M::Fut<'_, Result<MotionSyncMode, Error>> {
        use crate::command::inquiry_structs::MotionSyncModeInquiry;
        self.query(MotionSyncModeInquiry)
    }

    fn motion_sync_speed(&self) -> M::Fut<'_, Result<MotionSyncPreset, Error>> {
        use crate::command::inquiry_structs::MotionSyncPresetInquiry;
        self.query(MotionSyncPresetInquiry)
    }
}
