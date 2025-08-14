//! Motion Sync control methods for PTZOptics cameras.

use crate::{error::Error, MotionSyncMode, MotionSyncSpeed};

/// Motion Sync control methods for cameras that support this feature.
#[cfg(feature = "async")]
pub trait MotionSyncControl {
    /// Sets the motion sync mode (on/off).
    ///
    /// This PTZOptics-specific feature coordinates pan, tilt, and zoom movements
    /// for smoother preset recalls.
    ///
    /// # Arguments
    /// * `mode` - The motion sync mode to set
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    async fn set_motion_sync_mode(&self, mode: MotionSyncMode) -> Result<(), Error>;

    /// Sets the motion sync speed.
    ///
    /// # Arguments
    /// * `speed` - Speed value from 1 to 24
    ///
    /// # Errors
    /// Returns an error if:
    /// - The camera doesn't support motion sync
    /// - The speed is outside the valid range (1-24)
    async fn set_motion_sync_speed(&self, speed: u8) -> Result<(), Error>;

    /// Sets the motion sync speed using a preset value.
    ///
    /// # Arguments
    /// * `speed` - Preset speed (Slow, Normal, Fast)
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    async fn set_motion_sync_preset_speed(&self, speed: MotionSyncSpeed) -> Result<(), Error>;

    /// Gets the current motion sync mode.
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    async fn get_motion_sync_mode(&self) -> Result<MotionSyncMode, Error>;

    /// Gets the current motion sync speed.
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    async fn get_motion_sync_speed(&self) -> Result<MotionSyncSpeed, Error>;
}

/// Blocking version of motion sync control methods.
pub trait MotionSyncControlBlocking {
    /// Sets the motion sync mode (on/off).
    fn set_motion_sync_mode(&self, mode: MotionSyncMode) -> Result<(), Error>;

    /// Sets the motion sync speed.
    fn set_motion_sync_speed(&self, speed: u8) -> Result<(), Error>;

    /// Sets the motion sync speed using a preset value.
    fn set_motion_sync_preset_speed(&self, speed: MotionSyncSpeed) -> Result<(), Error>;

    /// Gets the current motion sync mode.
    fn get_motion_sync_mode(&self) -> Result<MotionSyncMode, Error>;

    /// Gets the current motion sync speed.
    fn get_motion_sync_speed(&self) -> Result<MotionSyncSpeed, Error>;
}

// Async implementation
