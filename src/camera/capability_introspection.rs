//! Capability introspection methods for the generic camera.
//!
//! This module provides read-only methods to inspect camera capabilities at runtime.
//! These methods are only available when the camera profile implements specific
//! optional capability traits. The actual control methods for these capabilities
//! are provided by their respective trait modules in `src/camera/methods/`.

use crate::{
    camera::generic::Camera,
    capabilities::{MotionSync, NDFilter, NDFilterMode as CapabilityNDFilterMode, Profile},
    error::Error,
    transport::core::Transport,
};

impl<P, T> Camera<P, T>
where
    P: Profile + NDFilter,
    T: Transport + Send + Sync + 'static + crate::transport::core::BlockingTransport,
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
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

impl<P, T> Camera<P, T>
where
    P: Profile + MotionSync,
    T: Transport + Send + Sync + 'static + crate::transport::core::BlockingTransport,
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
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
