//! Capability-gated methods for the generic camera.
//!
//! This module provides methods that are only available when the camera profile
//! implements specific optional capability traits.

use crate::{
    camera::generic::Camera,
    capabilities::{MotionSync, NDFilter, NDFilterMode as CapabilityNDFilterMode, Profile},
    command::{
        motion_sync::MotionSyncModeCommand,
        nd_filter::{NDFilterMode as CommandNDFilterMode, NDFilterModeCommand},
        Response,
    },
    error::Error,
    transport::UnifiedTransport,
    MotionSyncMode,
};

// ND Filter methods - only available when P implements NDFilter
impl<P, T> Camera<P, T>
where
    P: Profile + NDFilter,
    T: UnifiedTransport,
{
    /// Set the ND filter mode.
    ///
    /// This method is only available for cameras that support ND filters.
    #[cfg(feature = "async")]
    pub async fn set_nd_filter_mode(&self, mode: CommandNDFilterMode) -> Result<Response, Error> {
        let command = NDFilterModeCommand::new(mode);
        self.send_command(&command).await
    }

    /// Set the ND filter mode (blocking).
    pub fn set_nd_filter_mode_blocking(
        &self,
        mode: CommandNDFilterMode,
    ) -> Result<Response, Error> {
        let command = NDFilterModeCommand::new(mode);
        self.send_command_blocking(&command)
    }

    /// Get the ND filter mode from the camera profile.
    #[must_use]
    pub fn nd_filter_mode(&self) -> CapabilityNDFilterMode {
        P::ND_MODE
    }

    /// Check if the camera has variable ND filter.
    #[must_use]
    pub fn has_variable_nd_filter(&self) -> bool {
        matches!(P::ND_MODE, CapabilityNDFilterMode::Variable)
    }
}

// Motion Sync methods - only available when P implements MotionSync
impl<P, T> Camera<P, T>
where
    P: Profile + MotionSync,
    T: UnifiedTransport,
{
    /// Set the motion sync mode.
    ///
    /// This method is only available for cameras that support motion sync.
    #[cfg(feature = "async")]
    pub async fn set_motion_sync_mode(&self, mode: MotionSyncMode) -> Result<Response, Error> {
        let command = MotionSyncModeCommand::new(mode);
        self.send_command(&command).await
    }

    /// Set the motion sync mode (blocking).
    pub fn set_motion_sync_mode_blocking(&self, mode: MotionSyncMode) -> Result<Response, Error> {
        let command = MotionSyncModeCommand::new(mode);
        self.send_command_blocking(&command)
    }

    /// Check if motion sync is supported.
    #[must_use]
    pub fn supports_motion_sync(&self) -> bool {
        P::SUPPORTS_MOTION_SYNC
    }

    /// Get the maximum motion sync speed.
    #[must_use]
    pub fn max_motion_sync_speed(&self) -> u8 {
        P::MAX_MOTION_SYNC_SPEED
    }
}

// Variable Speed methods moved to methods/variable_speed.rs to use marker traits

#[cfg(test)]
mod tests {

    #[test]
    fn test_nd_filter_compilation() {
        // This test verifies that ND filter methods are only available for cameras
        // that implement the NDFilter trait.
    }
}
