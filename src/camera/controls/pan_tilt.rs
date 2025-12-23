//! Pan/Tilt control implementation for PTZ cameras.
//!
//! This module provides comprehensive pan/tilt control functionality including:
//! - Absolute and relative positioning in degrees
//! - Variable speed movement control
//! - Home position management
//! - Movement limit configuration
//! - Multi-directional movement with independent pan/tilt speeds
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{
    camera::ViscaClient,
    command::pan_tilt::{PanTiltDirection, PanTiltLimitCorner},
    mode::Mode,
    types::{PanPosition, PanSpeed, SpeedLevel, TiltPosition, TiltSpeed},
    units::Degrees,
    Error,
};

/// Pan/Tilt operations for PTZ cameras.
///
/// This trait provides comprehensive pan/tilt control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # Coordinate System
///
/// - Pan: Horizontal rotation (left/right)
///   - Positive values = right
///   - Negative values = left
///   - Range varies by camera model (typically ±170°)
///
/// - Tilt: Vertical rotation (up/down)
///   - Positive values = up
///   - Negative values = down
///   - Range varies by camera model (typically -30° to +90°)
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// camera.pan_tilt_home()?;  // Move to home position
/// camera.pan_tilt_absolute(45.0_f64, 15.0_f64, SpeedLevel::Fast)?;  // f64 values work!
/// camera.pan_tilt_absolute(Degrees(45.0), Degrees(15.0), SpeedLevel::Fast)?;  // Or explicit Degrees
/// ```
///
/// ## Async mode
/// ```ignore
/// camera.pan_tilt_home().await?;  // Move to home position
/// camera.pan_tilt_absolute(45.0_f64, 15.0_f64, SpeedLevel::Fast).await?;  // f64 values work!
/// camera.pan_tilt_absolute(Degrees(45.0), Degrees(15.0), SpeedLevel::Fast).await?;  // Or explicit Degrees
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait PanTiltControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Stop all pan/tilt movement immediately.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn pan_tilt_stop(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Move to the home position (pan=0°, tilt=0°).
    ///
    /// The movement speed is determined by the camera's default settings.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn pan_tilt_home(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Move to an absolute pan/tilt position in degrees.
    ///
    /// # Arguments
    /// * `pan` - Target pan position in degrees (accepts f32 or f64)
    /// * `tilt` - Target tilt position in degrees (accepts f32 or f64)
    /// * `speed` - Movement speed (Slow, Medium, Fast)
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn pan_tilt_absolute(
        &self,
        pan: impl Into<Degrees>,
        tilt: impl Into<Degrees>,
        speed: SpeedLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Move relative to the current position in degrees.
    ///
    /// # Arguments
    /// * `pan` - Pan offset in degrees (positive=right, negative=left, accepts f32 or f64)
    /// * `tilt` - Tilt offset in degrees (positive=up, negative=down, accepts f32 or f64)
    /// * `speed` - Movement speed (Slow, Medium, Fast)
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn pan_tilt_relative(
        &self,
        pan: impl Into<Degrees>,
        tilt: impl Into<Degrees>,
        speed: SpeedLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Start continuous movement in a specific direction.
    ///
    /// The camera will continue moving until `pan_tilt_stop()` is called or a limit is reached.
    ///
    /// # Arguments
    /// * `direction` - Direction of movement (Up, Down, Left, Right, UpLeft, UpRight, DownLeft, DownRight, Stop)
    /// * `pan_speed` - Pan speed (0-24, camera-specific range)
    /// * `tilt_speed` - Tilt speed (0-20, camera-specific range)
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset pan/tilt mechanism to factory defaults.
    ///
    /// This recalibrates the pan/tilt motors and may take several seconds.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn pan_tilt_reset(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set a pan/tilt movement limit for a specific corner.
    ///
    /// Limits define the allowed movement range. When two corners are set,
    /// the camera movement is restricted to the rectangular area between them.
    ///
    /// # Arguments
    /// * `corner` - Which corner to set (UpRight or DownLeft)
    /// * `pan` - Pan position for the limit
    /// * `tilt` - Tilt position for the limit
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Clear a pan/tilt movement limit for a specific corner.
    ///
    /// # Arguments
    /// * `corner` - Which corner limit to clear (UpRight or DownLeft)
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn pan_tilt_limit_clear(
        &self,
        corner: PanTiltLimitCorner,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> PanTiltControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + crate::capabilities::PanTilt + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn pan_tilt_stop(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::from(SpeedLevel::Medium),
            tilt_speed: TiltSpeed::from(SpeedLevel::Medium),
        };
        self.execute(cmd)
    }

    fn pan_tilt_home(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::pan_tilt::PanTilt;
        self.execute(PanTilt::Home)
    }

    fn pan_tilt_absolute(
        &self,
        pan: impl Into<Degrees>,
        tilt: impl Into<Degrees>,
        speed: SpeedLevel,
    ) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::pan_tilt::PanTilt;

        // Convert inputs to Degrees
        let pan_deg = pan.into();
        let tilt_deg = tilt.into();

        // Convert Degrees to Position and SpeedLevel to individual speeds
        let pan_pos = match PanPosition::from_degrees(pan_deg.0) {
            Ok(pos) => pos,
            Err(e) => return self.error(e),
        };
        let tilt_pos = match TiltPosition::from_degrees(tilt_deg.0) {
            Ok(pos) => pos,
            Err(e) => return self.error(e),
        };

        // Convert logical positions to camera coordinates using profile's coordinate system
        let (pan_u16, tilt_u16) =
            P::COORDINATE_SYSTEM.to_camera_coords(pan_pos.value(), tilt_pos.value());

        let pan_speed = PanSpeed::from(speed);
        let tilt_speed = TiltSpeed::from(speed);
        let cmd = PanTilt::AbsolutePositionRaw {
            pan_u16,
            tilt_u16,
            pan_speed,
            tilt_speed,
        };
        self.execute(cmd)
    }

    fn pan_tilt_relative(
        &self,
        pan: impl Into<Degrees>,
        tilt: impl Into<Degrees>,
        speed: SpeedLevel,
    ) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::pan_tilt::PanTilt;

        // Convert inputs to Degrees
        let pan_deg = pan.into();
        let tilt_deg = tilt.into();

        // Convert Degrees to Position and SpeedLevel to individual speeds
        let pan_pos = match PanPosition::from_degrees(pan_deg.0) {
            Ok(pos) => pos,
            Err(e) => return self.error(e),
        };
        let tilt_pos = match TiltPosition::from_degrees(tilt_deg.0) {
            Ok(pos) => pos,
            Err(e) => return self.error(e),
        };

        // For relative positioning, we still need to convert to camera coordinates
        // The relative offset is also subject to the coordinate system
        let (pan_u16, tilt_u16) =
            P::COORDINATE_SYSTEM.to_camera_coords(pan_pos.value(), tilt_pos.value());

        let pan_speed = PanSpeed::from(speed);
        let tilt_speed = TiltSpeed::from(speed);
        let cmd = PanTilt::RelativePositionRaw {
            pan_u16,
            tilt_u16,
            pan_speed,
            tilt_speed,
        };
        self.execute(cmd)
    }

    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::Move {
            direction,
            pan_speed,
            tilt_speed,
        };
        self.execute(cmd)
    }

    fn pan_tilt_reset(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::pan_tilt::PanTilt;
        self.execute(PanTilt::Reset)
    }

    fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::pan_tilt::PanTilt;

        // Convert logical positions to camera coordinates using profile's coordinate system
        let (pan_u16, tilt_u16) = P::COORDINATE_SYSTEM.to_camera_coords(pan.value(), tilt.value());

        let cmd = PanTilt::LimitSetRaw {
            corner,
            pan_u16,
            tilt_u16,
        };
        // Update the state cache after successful command
        self.execute_updating_cache(cmd, move |cache| {
            cache.set_pan_tilt_limit(corner, pan, tilt);
        })
    }

    fn pan_tilt_limit_clear(&self, corner: PanTiltLimitCorner) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::LimitClear { corner };
        // Update the state cache after successful command
        self.execute_updating_cache(cmd, move |cache| {
            cache.clear_pan_tilt_limit(corner);
        })
    }
}

// Separate implementation for async-mode _op methods on async Camera
#[cfg(feature = "mode-async")]
impl<P, Tr, Exec> crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + crate::capabilities::PanTilt + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    /// Move to an absolute pan/tilt position and return an operation handle.
    pub async fn pan_tilt_absolute_op(
        &self,
        pan: impl Into<Degrees>,
        tilt: impl Into<Degrees>,
        speed: SpeedLevel,
    ) -> Result<crate::camera::inflight::InFlight<'_, crate::camera::inflight::PanTilt, Self>, Error>
    {
        use crate::command::pan_tilt::PanTilt;

        // Convert inputs to Degrees
        let pan_deg = pan.into();
        let tilt_deg = tilt.into();

        // Convert Degrees to Position and SpeedLevel to individual speeds
        let pan_pos = PanPosition::from_degrees(pan_deg.0)?;
        let tilt_pos = TiltPosition::from_degrees(tilt_deg.0)?;

        // Convert logical positions to camera coordinates using profile's coordinate system
        let (pan_u16, tilt_u16) =
            P::COORDINATE_SYSTEM.to_camera_coords(pan_pos.value(), tilt_pos.value());

        let pan_speed = PanSpeed::from(speed);
        let tilt_speed = TiltSpeed::from(speed);
        let cmd = PanTilt::AbsolutePositionRaw {
            pan_u16,
            tilt_u16,
            pan_speed,
            tilt_speed,
        };

        // Use start_command_with_id to get the response future without awaiting it
        let (id, response_future) = self.start_command_with_id(&cmd).await?;

        // Return InFlight handle with the response future
        Ok(crate::camera::inflight::InFlight::new(
            id,
            self.camera_id(),
            self,
            response_future,
        ))
    }

    /// Move relative to the current position and return an operation handle.
    pub async fn pan_tilt_relative_op(
        &self,
        pan: impl Into<Degrees>,
        tilt: impl Into<Degrees>,
        speed: SpeedLevel,
    ) -> Result<crate::camera::inflight::InFlight<'_, crate::camera::inflight::PanTilt, Self>, Error>
    {
        use crate::command::pan_tilt::PanTilt;

        // Convert inputs to Degrees
        let pan_deg = pan.into();
        let tilt_deg = tilt.into();

        // Convert Degrees to Position and SpeedLevel to individual speeds
        let pan_pos = PanPosition::from_degrees(pan_deg.0)?;
        let tilt_pos = TiltPosition::from_degrees(tilt_deg.0)?;

        // For relative positioning, we still need to convert to camera coordinates
        // The relative offset is also subject to the coordinate system
        let (pan_u16, tilt_u16) =
            P::COORDINATE_SYSTEM.to_camera_coords(pan_pos.value(), tilt_pos.value());

        let pan_speed = PanSpeed::from(speed);
        let tilt_speed = TiltSpeed::from(speed);
        let cmd = PanTilt::RelativePositionRaw {
            pan_u16,
            tilt_u16,
            pan_speed,
            tilt_speed,
        };

        // Use start_command_with_id to get the response future without awaiting it
        let (id, response_future) = self.start_command_with_id(&cmd).await?;

        // Return InFlight handle with the response future
        Ok(crate::camera::inflight::InFlight::new(
            id,
            self.camera_id(),
            self,
            response_future,
        ))
    }

    /// Move to the home position and return an operation handle.
    ///
    /// This is the `_op` variant that returns an InFlight handle for fine-grained
    /// control over timeouts and cancellation. The movement speed is determined
    /// by the camera's default settings.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use std::time::Duration;
    ///
    /// let handle = camera.pan_tilt_home_op().await?;
    /// handle.await_completion(Duration::from_secs(30)).await?;
    /// ```
    ///
    /// # Errors
    /// Returns an error if the command fails to send.
    pub async fn pan_tilt_home_op(
        &self,
    ) -> Result<crate::camera::inflight::InFlight<'_, crate::camera::inflight::PanTilt, Self>, Error>
    {
        use crate::command::pan_tilt::PanTilt;

        // Use start_command_with_id to get the response future without awaiting it
        let (id, response_future) = self.start_command_with_id(&PanTilt::Home).await?;

        // Return InFlight handle with the response future
        Ok(crate::camera::inflight::InFlight::new(
            id,
            self.camera_id(),
            self,
            response_future,
        ))
    }

    /// Reset pan/tilt mechanism and return an operation handle.
    ///
    /// This is the `_op` variant that returns an InFlight handle for fine-grained
    /// control over timeouts and cancellation. This recalibrates the pan/tilt motors
    /// and may take several seconds to complete.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use std::time::Duration;
    ///
    /// let handle = camera.pan_tilt_reset_op().await?;
    /// handle.await_completion(Duration::from_secs(30)).await?;
    /// ```
    ///
    /// # Errors
    /// Returns an error if the command fails to send.
    pub async fn pan_tilt_reset_op(
        &self,
    ) -> Result<crate::camera::inflight::InFlight<'_, crate::camera::inflight::PanTilt, Self>, Error>
    {
        use crate::command::pan_tilt::PanTilt;

        // Use start_command_with_id to get the response future without awaiting it
        let (id, response_future) = self.start_command_with_id(&PanTilt::Reset).await?;

        // Return InFlight handle with the response future
        Ok(crate::camera::inflight::InFlight::new(
            id,
            self.camera_id(),
            self,
            response_future,
        ))
    }
}
