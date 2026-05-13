//! Motion control utilities for PTZ cameras.
//!
//! This module provides convenience functions for controlling camera motion,
//! including a unified `stop_all_motion()` function.

use crate::{camera::ViscaClient, mode::Mode, Error};

use super::{focus::FocusControl, pan_tilt::PanTiltControl, zoom::ZoomControl};

/// Motion control operations for PTZ cameras.
///
/// This trait provides convenience methods for controlling all camera motion,
/// including unified stop operations.
///
/// # Motion Semantics
///
/// The `stop_all_motion` method issues stop commands for all motion axes:
/// pan/tilt, zoom, and focus. This aligns with the crate's movement detection
/// semantics (e.g., `await_idle`, `is_moving_async`) which consider the camera
/// as moving if *any* axis is changing position.
///
/// # Examples
///
/// ## Stop all motion
/// ```ignore
/// // Blocking
/// camera.stop_all_motion()?;
///
/// // Async
/// camera.stop_all_motion().await?;
/// ```
///
/// ## Stop and wait for idle
/// ```ignore
/// camera.stop_all_motion().await?;
/// camera.await_idle(Duration::from_secs(5)).await?;
/// ```
pub trait MotionControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Stop all camera motion (pan, tilt, zoom, and focus).
    ///
    /// This method issues stop commands for pan/tilt, zoom, and focus axes
    /// in sequence. All stop commands are attempted even if earlier ones fail,
    /// and the first error encountered is returned.
    ///
    /// This method is only available for cameras that support all three motion
    /// types (pan/tilt, zoom, and focus), as enforced by the capability bounds.
    ///
    /// # Errors
    ///
    /// Returns the first error encountered while stopping motion.
    /// All stop commands are attempted for safety even if earlier ones fail.
    ///
    /// # Example
    /// ```ignore
    /// // Stop all motion
    /// camera.stop_all_motion()?; // or .await? for async
    /// ```
    fn stop_all_motion(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Async implementation for cameras with all required capabilities
#[cfg(feature = "mode-async")]
impl<P, Tr, Exec> MotionControl for crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile
        + crate::capabilities::PanTilt
        + crate::capabilities::zoom::Zoom
        + crate::capabilities::focus::Focus
        + Default,
    Self: ViscaClient<crate::mode::Async>,
    Exec: crate::executor::Executor,
{
    type Mode = crate::mode::Async;

    fn stop_all_motion(&self) -> <crate::mode::Async as Mode>::Fut<'_, Result<(), Error>> {
        // Get all three stop futures before entering the async block.
        // This ensures the async block captures the futures (which are Send)
        // rather than &self (which may not be Sync).
        let pt_fut = self.pan_tilt_stop();
        let zoom_fut = self.zoom_stop();
        let focus_fut = self.focus_stop();

        crate::mode::Async::from_future(async move {
            // Stop all three motion axes, collecting the first error if any.
            // All stop commands are attempted even if earlier ones fail.
            let pt_result = pt_fut.await;
            let zoom_result = zoom_fut.await;
            let focus_result = focus_fut.await;

            // Return the first error encountered, or Ok(()) if all succeeded
            pt_result.and(zoom_result).and(focus_result)
        })
    }
}

// Blocking implementation for cameras with all required capabilities
#[cfg(not(feature = "mode-async"))]
impl<P, Tr> MotionControl for crate::camera::Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: crate::capabilities::Profile
        + crate::capabilities::PanTilt
        + crate::capabilities::zoom::Zoom
        + crate::capabilities::focus::Focus
        + Default,
    Self: ViscaClient<crate::mode::Blocking>,
{
    type Mode = crate::mode::Blocking;

    fn stop_all_motion(&self) -> <crate::mode::Blocking as Mode>::Fut<'_, Result<(), Error>> {
        use crate::mode::BlockingFutureExt;

        // In blocking mode, execute each stop command synchronously
        let pt_result = self.pan_tilt_stop().block();
        let zoom_result = self.zoom_stop().block();
        let focus_result = self.focus_stop().block();

        // Return the first error encountered, or Ok(()) if all succeeded
        std::future::ready(pt_result.and(zoom_result).and(focus_result))
    }
}
