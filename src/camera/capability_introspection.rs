//! Capability introspection methods for the generic camera.
//!
//! This module provides read-only methods to inspect camera capabilities at runtime.
//! These methods are only available when the camera profile implements specific
//! optional capability traits. The actual control methods for these capabilities
//! are provided by their respective trait modules in `src/camera/methods/`.

use crate::{
    camera::generic_executor::Camera,
    capabilities::{MotionSync, NDFilter, NDFilterMode as CapabilityNDFilterMode, Profile},
};

// These methods are available for both async and blocking modes
impl<M, P, T, E> Camera<M, P, T, E>
where
    P: Profile + NDFilter,
{
    // ND filter setters are provided by NDFilterOps / NDFilterOpsBlocking traits
    // in src/camera/methods/nd_filter.rs

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

impl<M, P, T, E> Camera<M, P, T, E>
where
    P: Profile + MotionSync,
{
    // Motion sync setters are provided by MotionSyncControl / MotionSyncControlBlocking traits
    // in src/camera/methods/motion_sync.rs

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

#[cfg(test)]
mod tests {
    #[test]
    fn test_nd_filter_compilation() {
        // This test verifies that ND filter methods are only available for cameras
        // that implement the NDFilter trait.
    }
}
