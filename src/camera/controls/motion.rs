//! Motion control utilities for PTZ cameras.
//!
//! This module provides convenience functions and handles for controlling
//! camera motion, including a unified `stop_all_motion()` function and a
//! `MotionGuard` for explicit motion stopping.

use core::marker::PhantomData;

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

/// Handle for stopping camera motion on specific axes.
///
/// This type provides an explicit, non-RAII interface for stopping camera motion.
/// Unlike traditional RAII guards, dropping a `MotionGuard` does **not** stop motion.
/// You must explicitly call `stop_now()` to issue stop commands.
///
/// This design aligns with the crate's async patterns (e.g., `InFlight` handles)
/// where async operations require explicit handling rather than implicit Drop behavior.
///
/// # Usage
///
/// Create a guard for the motion type you want to control, then call `stop_now()`
/// when you want to stop that motion:
///
/// ```ignore
/// {
///     let guard = MotionGuard::new_pan_tilt(&camera);
///     camera.pan_tilt_move(
///         PanTiltDirection::Right,
///         PanSpeed::new(12)?,
///         TiltSpeed::new(0)?
///     ).await?;
///
///     // Camera pans right...
///     tokio::time::sleep(Duration::from_secs(2)).await;
///
///     // Explicitly stop the motion
///     guard.stop_now().await?;
/// }
/// ```
///
/// # Motion Types
///
/// - `new_pan_tilt()`: Creates a guard that stops pan/tilt motion
/// - `new_zoom()`: Creates a guard that stops zoom motion
/// - `new_focus()`: Creates a guard that stops focus motion
/// - `new_all()`: Creates a guard that stops all motion axes (pan/tilt, zoom, and focus)
///
/// # Note on Drop
///
/// Dropping a `MotionGuard` without calling `stop_now()` is explicitly a no-op.
/// This is intentional: async stop operations cannot be reliably executed in Drop,
/// and hiding background work in destructors would conflict with the crate's
/// explicit async patterns.
#[derive(Debug)]
pub struct MotionGuard<'cam, M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile,
    Exec: crate::executor::Executor,
{
    camera: &'cam crate::camera::Camera<M, P, Tr, Exec>,
    motion_type: MotionType,
    _phantom: PhantomData<(M, P, Tr)>,
}

/// Type of motion being guarded.
#[derive(Debug, Clone, Copy)]
enum MotionType {
    PanTilt,
    Zoom,
    Focus,
    All,
}

impl<'cam, M, P, Tr, Exec> MotionGuard<'cam, M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    crate::camera::Camera<M, P, Tr, Exec>: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    /// Create a new handle for pan/tilt motion.
    ///
    /// After creating the handle, start the motion using the camera's
    /// `pan_tilt_move()` method. Call `stop_now()` to stop the motion.
    pub fn new_pan_tilt(camera: &'cam crate::camera::Camera<M, P, Tr, Exec>) -> Self {
        Self {
            camera,
            motion_type: MotionType::PanTilt,
            _phantom: PhantomData,
        }
    }

    /// Create a new handle for zoom motion.
    ///
    /// After creating the handle, start the zoom using the camera's
    /// `zoom_tele()` or `zoom_wide()` methods. Call `stop_now()` to stop the zoom.
    pub fn new_zoom(camera: &'cam crate::camera::Camera<M, P, Tr, Exec>) -> Self {
        Self {
            camera,
            motion_type: MotionType::Zoom,
            _phantom: PhantomData,
        }
    }

    /// Create a new handle for focus motion.
    ///
    /// After creating the handle, start the focus using the camera's
    /// `focus_near()` or `focus_far()` methods. Call `stop_now()` to stop the focus.
    pub fn new_focus(camera: &'cam crate::camera::Camera<M, P, Tr, Exec>) -> Self {
        Self {
            camera,
            motion_type: MotionType::Focus,
            _phantom: PhantomData,
        }
    }

    /// Create a handle that will stop all motion axes when `stop_now()` is called.
    ///
    /// This is useful when multiple motion types are active and you want
    /// to stop all of them together. When `stop_now()` is called, this issues
    /// stop commands for pan/tilt, zoom, and focus in sequence.
    pub fn new_all(camera: &'cam crate::camera::Camera<M, P, Tr, Exec>) -> Self {
        Self {
            camera,
            motion_type: MotionType::All,
            _phantom: PhantomData,
        }
    }

    /// Stop the motion associated with this handle.
    ///
    /// Issues the appropriate stop command(s) based on the motion type:
    /// - `PanTilt`: Stops pan/tilt motion only
    /// - `Zoom`: Stops zoom motion only
    /// - `Focus`: Stops focus motion only
    /// - `All`: Stops pan/tilt, zoom, and focus (all axes)
    ///
    /// For `MotionType::All`, all stop commands are attempted even if earlier
    /// ones fail, and the first error encountered is returned.
    ///
    /// # Errors
    ///
    /// Returns an error if any stop command fails to send or receive a response.
    pub fn stop_now(self) -> M::Fut<'cam, Result<(), Error>>
    where
        P: crate::capabilities::PanTilt
            + crate::capabilities::zoom::Zoom
            + crate::capabilities::focus::Focus,
        crate::camera::Camera<M, P, Tr, Exec>: MotionControl<Mode = M>,
    {
        match self.motion_type {
            MotionType::PanTilt => self.camera.pan_tilt_stop(),
            MotionType::Zoom => self.camera.zoom_stop(),
            MotionType::Focus => self.camera.focus_stop(),
            MotionType::All => self.camera.stop_all_motion(),
        }
    }
}

// Note: No Drop implementation - dropping a MotionGuard without calling stop_now()
// is explicitly a no-op. See the MotionGuard documentation for rationale.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_motion_type_size() {
        // Ensure MotionType is small and efficient
        assert_eq!(size_of::<MotionType>(), 1);
    }

    // Additional tests would require mock camera implementations
}
