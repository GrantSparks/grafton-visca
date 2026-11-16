//! Synchronized-motion settings.
//!
//! Readback requires a custom inquiry with a sourced register and decoder.

use crate::command::nouns::forward_request;
use crate::types::MotionSyncSpeed;

/// Enables or disables synchronized motion.
#[derive(Debug, Clone, Copy)]
pub struct SetEnabled {
    inner: crate::command::SetMotionSyncMode,
}

impl SetEnabled {
    /// Creates a synchronized-motion enable setting.
    #[must_use]
    pub const fn new(enabled: bool) -> Self {
        Self {
            inner: crate::command::SetMotionSyncMode::new(enabled),
        }
    }

    /// Returns the requested enable state.
    #[must_use]
    pub const fn enabled(self) -> bool {
        self.inner.mode
    }
}
forward_request!(
    SetEnabled,
    crate::command::SetMotionSyncMode,
    Plain,
    rows = [MotionSyncMode]
);

/// Sets a checked synchronized-motion speed.
#[derive(Debug, Clone, Copy)]
pub struct SetSpeed {
    inner: crate::command::SetMotionSyncPreset,
}

impl SetSpeed {
    /// Creates a speed setting; target preparation validates the profile range.
    #[must_use]
    pub const fn new(speed: MotionSyncSpeed) -> Self {
        Self {
            inner: crate::command::SetMotionSyncPreset::new(speed),
        }
    }

    /// Returns the requested synchronized-motion speed.
    #[must_use]
    pub const fn speed(self) -> MotionSyncSpeed {
        self.inner.speed()
    }
}
forward_request!(
    SetSpeed,
    crate::command::SetMotionSyncPreset,
    Plain,
    rows = [MotionSyncPreset]
);
