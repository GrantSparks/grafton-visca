//! Capability introspection methods for cameras.
//!
//! This module provides read-only methods to inspect camera capabilities at runtime.
//! These methods are only available when the camera profile implements specific
//! optional capability traits. The actual control methods for these capabilities
//! are provided by their respective trait modules in `src/camera/methods/`.

use crate::capabilities::{MotionSync, NdFilter, NdFilterMode as CapabilityNdFilterMode, Profile};

#[cfg(feature = "mode-async")]
use crate::{camera::Camera, executor::Executor, mode, transport::AsyncTransport};
#[cfg(not(feature = "mode-async"))]
use crate::{camera::Camera, mode, transport::BlockingTransport};

// Blocking camera capabilities
#[cfg(not(feature = "mode-async"))]
impl<P, Tr> Camera<mode::Blocking, P, Tr, ()>
where
    P: Profile + NdFilter,
    Tr: BlockingTransport,
{
    /// Get the ND filter mode from the camera profile.
    #[must_use]
    pub fn nd_filter_mode(&self) -> CapabilityNdFilterMode {
        P::ND_MODE
    }

    /// Check if the camera has variable ND filter.
    #[must_use]
    pub fn has_variable_nd_filter(&self) -> bool {
        matches!(P::ND_MODE, CapabilityNdFilterMode::Variable)
    }
}

#[cfg(not(feature = "mode-async"))]
impl<P, Tr> Camera<mode::Blocking, P, Tr, ()>
where
    P: Profile + MotionSync,
    Tr: BlockingTransport,
{
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

// Async camera capabilities
#[cfg(feature = "mode-async")]
impl<P, Tr, Exec> Camera<mode::Async, P, Tr, Exec>
where
    P: Profile + NdFilter,
    Tr: AsyncTransport,
    Exec: Executor,
{
    /// Get the ND filter mode from the camera profile.
    #[must_use]
    pub fn nd_filter_mode(&self) -> CapabilityNdFilterMode {
        P::ND_MODE
    }

    /// Check if the camera has variable ND filter.
    #[must_use]
    pub fn has_variable_nd_filter(&self) -> bool {
        matches!(P::ND_MODE, CapabilityNdFilterMode::Variable)
    }
}

#[cfg(feature = "mode-async")]
impl<P, Tr, Exec> Camera<mode::Async, P, Tr, Exec>
where
    P: Profile + MotionSync,
    Tr: AsyncTransport,
    Exec: Executor,
{
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
        // that implement the NdFilter trait.
    }
}
