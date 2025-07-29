//! Motion sync capability trait for PTZOptics cameras.

/// Trait for cameras that support Motion Sync functionality.
///
/// Motion Sync is a PTZOptics-specific feature that coordinates pan, tilt, and zoom
/// movements for smoother preset recalls.
pub trait MotionSync {
    /// Whether this camera supports motion sync.
    const SUPPORTS_MOTION_SYNC: bool = false;

    /// Maximum motion sync speed supported (1-24).
    const MAX_MOTION_SYNC_SPEED: u8 = 24;
}
