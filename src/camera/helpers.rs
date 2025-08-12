//! Helper methods for camera movement operations.
//!
//! This module provides convenience methods for movement detection and control,
//! with both blocking and async variants.

use std::time::Duration;

use super::MovementConfig;
use crate::{camera::Camera, capabilities::Profile, error::Error, units::Degrees};

#[cfg(feature = "async")]
use crate::transport::async_transport::AsyncTransport;

#[cfg(not(feature = "async"))]
use crate::transport::blocking_transport::BlockingTransport;

/// Helper methods for camera movement operations (blocking).
#[cfg(not(feature = "async"))]
pub trait MovementOpsBlocking: Sized {
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
pub trait MovementOps: Sized {
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

#[cfg(not(feature = "async"))]
impl<P: Profile, T: BlockingTransport + Send + Sync + 'static> MovementOpsBlocking
    for Camera<crate::camera::BlockingMode, P, T>
{
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
        use crate::types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed};

        let pan_pos = PanPosition::from_degrees(pan.0)?;
        let tilt_pos = TiltPosition::from_degrees(tilt.0)?;
        let pan_speed = PanSpeed::new(18).unwrap(); // Fast speed
        let tilt_speed = TiltSpeed::new(18).unwrap(); // Fast speed
        self.pan_tilt_absolute(pan_pos, tilt_pos, pan_speed, tilt_speed)?;
        MovementOpsBlocking::await_idle(self, timeout)
    }

    fn is_moving(&self) -> Result<bool, Error> {
        Camera::is_moving(self)
    }
}

#[cfg(feature = "rt-tokio")]
impl<P: Profile, T: AsyncTransport + Send + Sync + 'static> MovementOps
    for Camera<crate::camera::AsyncMode, P, T>
{
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
        use crate::types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed};

        let pan_pos = PanPosition::from_degrees(pan.0)?;
        let tilt_pos = TiltPosition::from_degrees(tilt.0)?;
        let pan_speed = PanSpeed::new(18).unwrap(); // Fast speed
        let tilt_speed = TiltSpeed::new(18).unwrap(); // Fast speed
        self.pan_tilt_absolute(pan_pos, tilt_pos, pan_speed, tilt_speed)
            .await?;
        MovementOps::await_idle(self, timeout).await
    }

    async fn is_moving(&self) -> Result<bool, Error> {
        self.is_moving_async().await
    }
}

#[cfg(all(feature = "async", not(feature = "rt-tokio")))]
impl<P: Profile, T: AsyncTransport + Send + Sync + 'static> MovementOps
    for Camera<crate::camera::AsyncMode, P, T>
{
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
        use crate::types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed};

        let pan_pos = PanPosition::from_degrees(pan.0)?;
        let tilt_pos = TiltPosition::from_degrees(tilt.0)?;
        let pan_speed = PanSpeed::new(18).unwrap(); // Fast speed
        let tilt_speed = TiltSpeed::new(18).unwrap(); // Fast speed
        self.pan_tilt_absolute(pan_pos, tilt_pos, pan_speed, tilt_speed)
            .await?;
        MovementOps::await_idle(self, timeout).await
    }

    async fn is_moving(&self) -> Result<bool, Error> {
        self.is_moving_async().await
    }
}
