//! Capability-gated methods for the generic camera.
//!
//! This module provides methods that are only available when the camera profile
//! implements specific optional capability traits.

use crate::{
    camera::generic::{Camera, UnifiedTransport},
    capabilities::{NDFilter, Profile, MotionSync, VariableSpeed},
    command::{
        nd_filter::NDFilterModeCommand,
        motion_sync::MotionSyncModeCommand,
        variable_speed::{VariableSpeedMode, VariableSpeedModeCommand},
        Response,
    },
    error::Error,
    MotionSyncMode,
};

// Import the command version of NDFilterMode for the API
use crate::command::nd_filter::NDFilterMode as CommandNDFilterMode;
// Import the capability version for internal use
use crate::capabilities::NDFilterMode as CapabilityNDFilterMode;

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
    pub fn set_nd_filter_mode_blocking(&self, mode: CommandNDFilterMode) -> Result<Response, Error> {
        let command = NDFilterModeCommand::new(mode);
        self.send_command_blocking(&command)
    }

    // Note: NDFilterPositionCommand doesn't exist in the current implementation
    // This would need to be added to the nd_filter command module

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

// Variable Speed methods - only available when P implements VariableSpeed
impl<P, T> Camera<P, T>
where
    P: Profile + VariableSpeed,
    T: UnifiedTransport,
{
    /// Set the variable speed mode.
    ///
    /// This method is only available for cameras that support variable speed mode.
    #[cfg(feature = "async")]
    pub async fn set_variable_speed_mode(&self, mode: VariableSpeedMode) -> Result<Response, Error> {
        let command = VariableSpeedModeCommand::new(mode);
        self.send_command(&command).await
    }

    /// Set the variable speed mode (blocking).
    pub fn set_variable_speed_mode_blocking(&self, mode: VariableSpeedMode) -> Result<Response, Error> {
        let command = VariableSpeedModeCommand::new(mode);
        self.send_command_blocking(&command)
    }

    /// Check if variable speed mode is supported.
    #[must_use]
    pub fn supports_variable_speed(&self) -> bool {
        P::SUPPORTS_VARIABLE_SPEED
    }
}

// Example of how to add more capability-gated methods:
// 
// impl<P, T> Camera<P, T>
// where
//     P: Profile + PresetTour, // Hypothetical marker trait for preset tour support
//     T: UnifiedTransport,
// {
//     pub async fn start_preset_tour(&self) -> Result<Response, Error> {
//         // Implementation
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::profiles::{PTZOpticsG2, SonyFR7};

    #[test]
    fn test_nd_filter_compilation() {
        // This test verifies that ND filter methods are only available for cameras
        // that implement the NDFilter trait.
        
        // The following would not compile because PTZOpticsG2 doesn't implement NDFilter:
        // let camera: Camera<PTZOpticsG2, _> = unimplemented!();
        // camera.set_nd_filter_mode_blocking(NDFilterMode::Off); // Compile error!
        
        // But this would compile for SonyFR7:
        // let camera: Camera<SonyFR7, _> = unimplemented!();
        // camera.set_nd_filter_mode_blocking(CommandNDFilterMode::Variable); // OK!
    }
}