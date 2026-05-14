//! Motion sync metadata trait for camera profiles.

/// Metadata for a camera profile's Motion Sync capability.
///
/// This trait supplies runtime discovery defaults. It does not mean the typed
/// Motion Sync control API is available for a profile; use
/// [`crate::capabilities::HasMotionSync`] for that compile-time support marker.
pub trait MotionSyncMetadata {
    /// Whether this camera supports motion sync.
    const SUPPORTS_MOTION_SYNC: bool = false;

    /// Maximum motion sync speed supported (1-24).
    const MAX_MOTION_SYNC_SPEED: u8 = 24;
}
