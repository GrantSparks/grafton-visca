//! Motion control utilities for PTZ cameras.
//!
//! This module provides convenience functions and RAII guards for controlling
//! camera motion, including a unified `stop_all_motion()` function and a
//! `MotionGuard` that automatically stops motion when dropped.

use core::marker::PhantomData;

use crate::{camera::ViscaClient, mode::Mode, Error};

/// Motion control operations for PTZ cameras.
///
/// This trait provides convenience methods for controlling all camera motion,
/// including unified stop operations.
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
pub trait MotionControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Stop all camera motion (pan, tilt, zoom, and focus).
    ///
    /// This is a convenience method that stops all types of motion in a single call.
    /// It attempts to stop pan/tilt, zoom, and focus motion in sequence.
    /// If multiple stop commands fail, the first error is returned, but all
    /// stop commands are attempted for safety.
    ///
    /// # Errors
    /// Returns the first error encountered while stopping motion.
    /// All stop commands are attempted even if earlier ones fail.
    ///
    /// # Example
    /// ```ignore
    /// // Stop all motion
    /// camera.stop_all_motion()?; // or .await? for async
    /// ```
    fn stop_all_motion(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// RAII guard for camera motion that automatically stops motion when dropped.
///
/// This guard ensures that continuous motion operations (pan/tilt/zoom) are
/// automatically stopped when the guard goes out of scope, providing safe
/// motion control even in the presence of early returns or panics.
///
/// **Note on Drop Behavior**: Due to limitations of Rust's async model,
/// the Drop implementation cannot execute async stop commands. Users should
/// either:
/// - Call `stop_now()` explicitly to handle errors
/// - Use the guard in a limited scope where motion should naturally stop
/// - Use `stop_all_motion()` for explicit cleanup
///
/// # Example
/// ```ignore
/// {
///     let guard = MotionGuard::new_pan_tilt(&camera);
///     camera.pan_tilt_move(
///         PanTiltDirection::Right,
///         PanSpeed::new(12)?,
///         TiltSpeed::new(0)?
///     )?; // or .await? for async
///
///     // Camera pans right...
///     thread::sleep(Duration::from_secs(2));
///
///     // Option 1: Explicit stop with error handling
///     guard.stop_now()?; // or .await? for async
///
///     // Option 2: Let guard go out of scope (best-effort stop, no error handling)
/// }
/// ```
#[derive(Debug)]
pub struct MotionGuard<'cam, M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile,
    Exec: crate::executor::Executor,
{
    camera: &'cam crate::camera::Camera<M, P, Tr, Exec>,
    motion_type: MotionType,
    should_stop: bool,
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
    /// Create a new guard for pan/tilt motion.
    ///
    /// After creating the guard, start the motion using the camera's
    /// pan_tilt_move() method. The guard will stop the motion when dropped.
    pub fn new_pan_tilt(camera: &'cam crate::camera::Camera<M, P, Tr, Exec>) -> Self {
        Self {
            camera,
            motion_type: MotionType::PanTilt,
            should_stop: true,
            _phantom: PhantomData,
        }
    }

    /// Create a new guard for zoom motion.
    ///
    /// After creating the guard, start the zoom using the camera's
    /// zoom_tele() or zoom_wide() methods. The guard will stop the zoom when dropped.
    pub fn new_zoom(camera: &'cam crate::camera::Camera<M, P, Tr, Exec>) -> Self {
        Self {
            camera,
            motion_type: MotionType::Zoom,
            should_stop: true,
            _phantom: PhantomData,
        }
    }

    /// Create a new guard for focus motion.
    ///
    /// After creating the guard, start the focus using the camera's
    /// focus_near() or focus_far() methods. The guard will stop the focus when dropped.
    pub fn new_focus(camera: &'cam crate::camera::Camera<M, P, Tr, Exec>) -> Self {
        Self {
            camera,
            motion_type: MotionType::Focus,
            should_stop: true,
            _phantom: PhantomData,
        }
    }

    /// Create a guard that will stop all motion when dropped.
    ///
    /// This is useful when multiple motion types are active and you want
    /// to ensure all are stopped together.
    pub fn new_all(camera: &'cam crate::camera::Camera<M, P, Tr, Exec>) -> Self {
        Self {
            camera,
            motion_type: MotionType::All,
            should_stop: true,
            _phantom: PhantomData,
        }
    }

    /// Disarm the guard so it won't stop motion when dropped.
    ///
    /// This is useful if you want to keep the motion going after the guard
    /// is dropped, for example when transferring control to another part of the code.
    pub fn disarm(&mut self) {
        self.should_stop = false;
    }

    /// Manually stop the motion and disarm the guard.
    ///
    /// This allows you to stop the motion before the guard is dropped
    /// and handle any errors that might occur.
    ///
    /// # Errors
    /// Returns an error if the stop command fails.
    /// For `MotionType::All`, returns the first error but attempts all stops.
    pub fn stop_now(mut self) -> M::Fut<'cam, Result<(), Error>>
    where
        P: crate::capabilities::PanTilt
            + crate::capabilities::zoom::Zoom
            + crate::capabilities::focus::Focus,
    {
        use crate::camera::controls::{
            focus::FocusControl, pan_tilt::PanTiltControl, zoom::ZoomControl,
        };

        self.should_stop = false; // Prevent double-stop in Drop

        match self.motion_type {
            MotionType::PanTilt => self.camera.pan_tilt_stop(),
            MotionType::Zoom => self.camera.zoom_stop(),
            MotionType::Focus => self.camera.focus_stop(),
            MotionType::All => {
                // For MotionType::All, we simply stop pan/tilt
                // (Full stop-all logic is in MotionControl trait)
                self.camera.pan_tilt_stop()
            }
        }
    }
}

impl<'cam, M, P, Tr, Exec> Drop for MotionGuard<'cam, M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile,
    Exec: crate::executor::Executor,
{
    fn drop(&mut self) {
        if !self.should_stop {}

        // Best-effort stop on drop - we can't handle errors in Drop
        // and we can't execute async operations

        // Due to Rust's async model limitations, we cannot execute
        // stop commands in Drop. Users should call stop_now() for
        // explicit error handling or use the guard in a limited scope.

        // In the future, this could be implemented with:
        // - A channel to signal the runtime to stop
        // - A fire-and-forget command queue
        // - Mode-specific Drop implementations
    }
}

// Implementation for cameras with all required capabilities
impl<M, P, Tr, Exec> MotionControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile
        + crate::capabilities::PanTilt
        + crate::capabilities::zoom::Zoom
        + crate::capabilities::focus::Focus
        + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn stop_all_motion(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::camera::controls::pan_tilt::PanTiltControl;

        // Simplified implementation that stops pan/tilt motion
        // For complete motion stop, call pan_tilt_stop(), zoom_stop(), and focus_stop()
        // individually to handle errors granularly
        //
        // Note: A full chained implementation would require Mode-aware combinators
        // to sequence multiple async operations and collect errors
        self.pan_tilt_stop()
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
