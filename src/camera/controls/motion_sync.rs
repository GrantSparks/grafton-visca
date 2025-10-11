//! Motion sync control implementation for PTZ cameras.
//!
//! This module provides motion synchronization control functionality including:
//! - Motion sync mode enable/disable for coordinated movements
//! - Speed control for synchronized pan/tilt/zoom operations
//! - Preset speed settings for common movement patterns
//! - Status inquiry for current motion sync configuration
//!
//! Motion sync coordinates pan, tilt, and zoom movements to provide smoother
//! and more natural camera operation, particularly during preset recalls.
//! This feature helps eliminate the jarring effect of sequential movements
//! by synchronizing all axes to complete their movement simultaneously.
//!
//! This feature is primarily available on PtzOptics cameras and other
//! professional PTZ systems that support coordinated movement control.
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{
    camera::ViscaClient, mode::Mode, types::MotionSyncSpeed, Error, MotionSyncMode,
    MotionSyncPreset,
};

/// Motion sync control methods for cameras that support this feature.
///
/// This trait provides motion sync control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # Motion Sync Benefits
///
/// - **Coordinated Movement**: All axes (pan/tilt/zoom) move in harmony
/// - **Smoother Presets**: Preset recalls appear more natural and professional
/// - **Reduced Jarring**: Eliminates the sequential movement effect
/// - **Professional Appearance**: Creates more polished camera movements
///
/// # Speed Control
///
/// Motion sync speed can be controlled in two ways:
/// - **Numeric Values**: Precise control with values 1-24
/// - **Preset Speeds**: Convenient presets (Slow, Normal, Fast)
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// camera.set_motion_sync_mode(MotionSyncMode::On)?;  // Enable motion sync
/// camera.set_motion_sync_preset_speed(MotionSyncPreset::Normal)?;  // Set speed
/// camera.preset_recall(1)?;  // Smooth coordinated movement to preset
/// ```
///
/// ## Async mode
/// ```ignore
/// camera.set_motion_sync_mode(MotionSyncMode::On).await?;  // Enable motion sync
/// camera.set_motion_sync_preset_speed(MotionSyncPreset::Normal).await?;  // Set speed
/// camera.preset_recall(1).await?;  // Smooth coordinated movement to preset
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait MotionSyncControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Sets the motion sync mode (on/off).
    ///
    /// Enables or disables motion synchronization, which coordinates pan, tilt,
    /// and zoom movements for smoother preset recalls and camera operations.
    ///
    /// # Parameters
    /// - `mode`: The motion sync mode to set (On or Off)
    ///
    /// # Note
    /// This is primarily a PtzOptics-specific feature, though other professional
    /// cameras may support similar functionality.
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync or the command fails.
    fn set_motion_sync_mode(
        &self,
        mode: MotionSyncMode,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Sets the motion sync speed.
    ///
    /// Controls how fast the synchronized movements execute. Lower values
    /// result in slower, more deliberate movements, while higher values
    /// create faster movements.
    ///
    /// # Parameters
    /// - `speed`: Speed value from 1 (slowest) to 24 (fastest)
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync,
    /// the speed is outside the valid range, or the command fails.
    fn set_motion_sync_speed(
        &self,
        speed: MotionSyncSpeed,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Sets the motion sync speed using a preset value.
    ///
    /// Provides convenient preset speeds instead of numeric values.
    /// This is often more intuitive than using specific numeric speeds.
    ///
    /// # Parameters
    /// - `speed`: Preset speed level (Slow, Normal, or Fast)
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync or the command fails.
    fn set_motion_sync_preset_speed(
        &self,
        speed: MotionSyncPreset,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Gets the current motion sync mode.
    ///
    /// Returns whether motion sync is currently enabled or disabled.
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync or the inquiry fails.
    fn motion_sync_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<MotionSyncMode, Error>>;

    /// Gets the current motion sync speed.
    ///
    /// Returns the current motion sync speed as a preset value
    /// (Slow, Normal, or Fast).
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync or the inquiry fails.
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
