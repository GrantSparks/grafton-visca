//! Motion sync metadata trait for camera profiles.

/// Metadata for a camera profile's Motion Sync capability.
///
/// This trait supplies runtime discovery defaults. It does not mean the typed
/// Motion Sync control API is available for a profile; use
/// [`crate::capabilities::HasMotionSync`] for that compile-time support marker.
pub trait MotionSyncMetadata {
    /// The motion-sync speeds this camera accepts, or `None` when it has no
    /// motion sync. This one fact is the motion-sync discovery inventory;
    /// both bounds lie in the [`crate::types::MotionSyncSpeed`] domain.
    const MOTION_SYNC_SPEED_RANGE: Option<super::CapabilityRange<u8>> = None;
}
