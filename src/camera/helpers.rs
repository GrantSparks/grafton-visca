//! Helper methods for camera movement operations.
//!
//! This module provides convenience methods for movement detection and control,
//! with both blocking and async variants.

use std::time::Duration;

use crate::{
    camera::Camera, capabilities::Profile, error::Error, transport::UnifiedTransport,
    units::Degrees,
};

use super::MovementConfig;

/// Helper methods for camera movement operations (blocking).
pub trait MovementOps: Sized {
    /// Wait for all movements to complete.
    ///
    /// This waits for pan/tilt, zoom, and focus movements to finish.
    fn await_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error>;

    /// Wait for pan/tilt movement to complete.
    ///
    /// Note: This now waits for all movements, not just pan/tilt.
    /// Use `await_idle` for clarity in new code.
    fn await_pan_tilt_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error> {
        self.await_idle(timeout)
    }

    /// Wait for zoom movement to complete.
    ///
    /// Note: This now waits for all movements, not just zoom.
    /// Use `await_idle` for clarity in new code.
    fn await_zoom_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error> {
        self.await_idle(timeout)
    }

    /// Wait for focus movement to complete.
    ///
    /// Note: This now waits for all movements, not just focus.
    /// Use `await_idle` for clarity in new code.
    fn await_focus_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error> {
        self.await_idle(timeout)
    }

    /// Move to a position and wait for completion.
    ///
    /// This sends an absolute pan/tilt command and waits for the movement to finish.
    fn move_to(
        &self,
        pan: Degrees,
        tilt: Degrees,
        timeout: impl Into<Duration>,
    ) -> Result<(), Error>;

    /// Check if the camera is currently moving.
    ///
    /// Returns true if any axis (pan/tilt/zoom/focus) is in motion.
    fn is_moving(&self) -> Result<bool, Error>;
}

/// Async helper methods for camera movement operations.
#[cfg(feature = "async")]
pub trait MovementOpsAsync: Sized {
    /// Wait for all movements to complete.
    ///
    /// This waits for pan/tilt, zoom, and focus movements to finish.
    async fn await_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error>;

    /// Wait for pan/tilt movement to complete.
    ///
    /// Note: This now waits for all movements, not just pan/tilt.
    /// Use `await_idle` for clarity in new code.
    async fn await_pan_tilt_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error> {
        self.await_idle(timeout).await
    }

    /// Wait for zoom movement to complete.
    ///
    /// Note: This now waits for all movements, not just zoom.
    /// Use `await_idle` for clarity in new code.
    async fn await_zoom_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error> {
        self.await_idle(timeout).await
    }

    /// Wait for focus movement to complete.
    ///
    /// Note: This now waits for all movements, not just focus.
    /// Use `await_idle` for clarity in new code.
    async fn await_focus_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error> {
        self.await_idle(timeout).await
    }

    /// Move to a position and wait for completion.
    ///
    /// This sends an absolute pan/tilt command and waits for the movement to finish.
    async fn move_to(
        &self,
        pan: Degrees,
        tilt: Degrees,
        timeout: impl Into<Duration>,
    ) -> Result<(), Error>;

    /// Check if the camera is currently moving.
    ///
    /// Returns true if any axis (pan/tilt/zoom/focus) is in motion.
    async fn is_moving(&self) -> Result<bool, Error>;
}

// Blocking implementation
impl<P: Profile, T: UnifiedTransport> MovementOps for Camera<P, T> {
    fn await_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error> {
        let config = MovementConfig::with_timeout(timeout.into());
        self.wait_for_movement(&config)
    }

    fn move_to(
        &self,
        pan: Degrees,
        tilt: Degrees,
        timeout: impl Into<Duration>,
    ) -> Result<(), Error> {
        use crate::camera::methods::pan_tilt::PanTiltOpsBlocking;
        use crate::types::SpeedLevel;

        // Send the movement command
        self.pan_tilt_absolute(pan, tilt, SpeedLevel::Fast)?;

        // Wait for it to complete
        MovementOps::await_idle(self, timeout)
    }

    fn is_moving(&self) -> Result<bool, Error> {
        // Delegate to the camera's is_moving method
        Camera::is_moving(self)
    }
}

// Async implementation with tokio
#[cfg(feature = "tokio")]
impl<P: Profile, T: UnifiedTransport> MovementOpsAsync for Camera<P, T> {
    async fn await_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error> {
        let config = MovementConfig::with_timeout(timeout.into());
        self.wait_for_movement_async(&config).await
    }

    async fn move_to(
        &self,
        pan: Degrees,
        tilt: Degrees,
        timeout: impl Into<Duration>,
    ) -> Result<(), Error> {
        use crate::camera::methods::pan_tilt::PanTiltOps;
        use crate::types::SpeedLevel;

        // Send the movement command
        self.pan_tilt_absolute(pan, tilt, SpeedLevel::Fast).await?;

        // Wait for it to complete
        MovementOpsAsync::await_idle(self, timeout).await
    }

    async fn is_moving(&self) -> Result<bool, Error> {
        // Delegate to the camera's is_moving_async method
        self.is_moving_async().await
    }
}

// Generic async implementation for non-tokio async runtimes
#[cfg(all(feature = "async", not(feature = "tokio")))]
impl<P: Profile, T: UnifiedTransport> MovementOpsAsync for Camera<P, T> {
    async fn await_idle(&self, _timeout: impl Into<Duration>) -> Result<(), Error> {
        Err(Error::InvalidState(
            "Async movement helpers require tokio feature".into(),
        ))
    }

    async fn move_to(
        &self,
        _pan: Degrees,
        _tilt: Degrees,
        _timeout: impl Into<Duration>,
    ) -> Result<(), Error> {
        Err(Error::InvalidState(
            "Async movement helpers require tokio feature".into(),
        ))
    }

    async fn is_moving(&self) -> Result<bool, Error> {
        Err(Error::InvalidState(
            "Async movement helpers require tokio feature".into(),
        ))
    }
}
