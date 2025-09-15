//! Camera session management with RAII and explicit close.
//!
//! This module provides the CameraSession type which represents an active
//! connection to a VISCA camera. Sessions provide both RAII (automatic cleanup
//! on drop) and explicit close() methods for controlled shutdown.

use std::marker::PhantomData;
use std::time::Duration;

use crate::{
    camera::UnifiedCamera as Camera,
    camera_id::CameraId,
    capabilities::Profile,
    command::{response::ViscaResponseType, CommandKind, ViscaEncode},
    error::Error,
    mode::Mode,
    timeout::CommandCategory,
};

/// A wrapper for raw VISCA command bytes.
#[derive(Clone, Debug)]
struct RawCommand {
    bytes: Vec<u8>,
    kind: CommandKind,
}

impl RawCommand {
    fn new(bytes: Vec<u8>, kind: CommandKind) -> Self {
        Self { bytes, kind }
    }
}

impl ViscaEncode for RawCommand {
    type ViscaResponse = ();
    const MAX_SIZE: usize = 256; // Allow reasonably sized raw commands
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Custom;

    fn encode_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        let len = self.bytes.len();
        if buffer.len() < len {
            return Err(Error::BufferTooSmall {
                required: len,
                actual: buffer.len(),
            });
        }
        buffer[..len].copy_from_slice(&self.bytes);
        Ok(len)
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        // Raw commands don't specify a response type
        None
    }

    fn command_kind(&self) -> CommandKind {
        self.kind
    }
}

/// An active camera session with connection lifecycle management.
///
/// CameraSession wraps a Camera instance and provides both RAII cleanup
/// (via Drop) and explicit close() methods. This ensures proper cleanup
/// of resources regardless of how the session ends.
///
/// # Example
///
/// ```ignore
/// use grafton_visca::camera::{CameraConfig, profiles::PtzOpticsG2};
///
/// // Create and use a session
/// let config = CameraConfig::for::<PtzOpticsG2>()
///     .address("192.168.0.110");
///
/// let session = config.open_async(&runtime).await?;
///
/// // Use the camera
/// session.power().on().await?;
/// session.zoom().tele().await?;
///
/// // Explicit close (optional - also happens on drop)
/// session.close().await?;
/// ```
#[cfg(feature = "async")]
pub struct CameraSession<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    /// The underlying camera instance.
    camera: Option<Camera<M, P, Tr, Exec>>,
    /// Track if we've been explicitly closed.
    closed: bool,
    /// Phantom data to ensure type parameters are used.
    _phantom: PhantomData<(M, P, Tr, Exec)>,
}

/// A camera session represents an active connection to a VISCA camera.
///
/// This type provides both RAII (automatic cleanup on drop) and explicit
/// close() methods for controlled shutdown. All camera control operations
/// can be performed directly through the session.
#[cfg(not(feature = "async"))]
pub struct CameraSession<M, P, Tr, Exec = ()>
where
    M: Mode,
    P: Profile,
{
    /// The underlying camera instance.
    camera: Option<Camera<M, P, Tr, Exec>>,
    /// Track if we've been explicitly closed.
    closed: bool,
    _phantom: PhantomData<(M, P, Tr, Exec)>,
}

#[cfg(feature = "async")]
impl<M, P, Tr, Exec> std::fmt::Debug for CameraSession<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraSession")
            .field("closed", &self.closed)
            .field("has_camera", &self.camera.is_some())
            .finish()
    }
}

#[cfg(not(feature = "async"))]
impl<M, P, Tr, Exec> std::fmt::Debug for CameraSession<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraSession")
            .field("closed", &self.closed)
            .finish()
    }
}

#[cfg(feature = "async")]
impl<M, P, Tr, Exec> CameraSession<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    /// Create a new session from a camera instance.
    pub(crate) fn new(camera: Camera<M, P, Tr, Exec>) -> Self {
        Self {
            camera: Some(camera),
            closed: false,
            _phantom: PhantomData,
        }
    }

    /// Get a reference to the underlying camera.
    ///
    /// Returns None if the session has been closed.
    pub fn camera(&self) -> Option<&Camera<M, P, Tr, Exec>> {
        self.camera.as_ref()
    }

    /// Get a mutable reference to the underlying camera.
    ///
    /// Returns None if the session has been closed.
    pub fn camera_mut(&mut self) -> Option<&mut Camera<M, P, Tr, Exec>> {
        self.camera.as_mut()
    }

    /// Check if the session has been closed.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Take ownership of the underlying camera, leaving None in its place.
    fn take_camera(&mut self) -> Option<Camera<M, P, Tr, Exec>> {
        self.camera.take()
    }
}

// Async-specific close implementation for Runtime-based cameras
#[cfg(feature = "async")]
impl<P, R> CameraSession<crate::mode::Async, P, crate::runtime_trait::TransportHandle<R>, R>
where
    P: Profile,
    R: crate::runtime_trait::Runtime,
{
    /// Explicitly close the camera session.
    ///
    /// This method is idempotent - calling it multiple times is safe.
    /// The session is also automatically closed when dropped.
    pub async fn close(mut self) -> Result<(), Error> {
        if self.closed {
            return Ok(());
        }

        self.closed = true;
        // For now, just let the camera drop - the runtime handle will clean up
        let _ = self.take_camera();

        Ok(())
    }

    /// Get access to the raw command interface.
    ///
    /// This provides a way to send raw VISCA commands while still going through
    /// the session's command scheduler to maintain proper sequencing.
    pub fn raw(
        &self,
    ) -> Option<RawSender<'_, crate::mode::Async, P, crate::runtime_trait::TransportHandle<R>, R>>
    {
        self.camera.as_ref().map(|cam| RawSender {
            camera: cam,
            _phantom: PhantomData,
        })
    }
}

// Add movement detection methods for async sessions with more specific bounds
#[cfg(feature = "async")]
impl<P, T, E> CameraSession<crate::mode::Async, P, T, E>
where
    P: Profile + crate::capabilities::ProfileMetadata + Default,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: Executor,
{
    /// Wait for all movements to complete.
    ///
    /// Convenience method that waits for all motors (pan/tilt, zoom, focus) to stop.
    ///
    /// # Arguments
    /// * `timeout` - Maximum time to wait for movement to complete
    ///
    /// # Returns
    /// * `Ok(())` - Movement completed successfully
    /// * `Err(Error::Timeout)` - Movement did not complete within timeout
    /// * `Err(Error::*)` - Other communication or camera errors
    pub async fn await_idle(&self, timeout: Duration) -> Result<(), Error> {
        self.camera
            .as_ref()
            .ok_or_else(|| Error::InvalidState("Session is closed".into()))?
            .await_idle(timeout)
            .await
    }

    /// Wait for pan/tilt movement to complete.
    ///
    /// Convenience method that waits for pan and tilt motors to stop moving.
    ///
    /// # Arguments
    /// * `timeout` - Maximum time to wait for movement to complete
    ///
    /// # Returns
    /// * `Ok(())` - Movement completed successfully
    /// * `Err(Error::Timeout)` - Movement did not complete within timeout
    pub async fn await_pan_tilt_idle(&self, timeout: Duration) -> Result<(), Error> {
        self.camera
            .as_ref()
            .ok_or_else(|| Error::InvalidState("Session is closed".into()))?
            .await_pan_tilt_idle(timeout)
            .await
    }

    /// Wait for zoom movement to complete.
    ///
    /// Convenience method that waits for zoom motor to stop moving.
    ///
    /// # Arguments
    /// * `timeout` - Maximum time to wait for movement to complete
    ///
    /// # Returns
    /// * `Ok(())` - Movement completed successfully
    /// * `Err(Error::Timeout)` - Movement did not complete within timeout
    pub async fn await_zoom_idle(&self, timeout: Duration) -> Result<(), Error> {
        self.camera
            .as_ref()
            .ok_or_else(|| Error::InvalidState("Session is closed".into()))?
            .await_zoom_idle(timeout)
            .await
    }

    /// Wait for focus movement to complete.
    ///
    /// Convenience method that waits for focus motor to stop moving.
    ///
    /// # Arguments
    /// * `timeout` - Maximum time to wait for movement to complete
    ///
    /// # Returns
    /// * `Ok(())` - Movement completed successfully
    /// * `Err(Error::Timeout)` - Movement did not complete within timeout
    pub async fn await_focus_idle(&self, timeout: Duration) -> Result<(), Error> {
        self.camera
            .as_ref()
            .ok_or_else(|| Error::InvalidState("Session is closed".into()))?
            .await_focus_idle(timeout)
            .await
    }

    /// Check if the camera is currently moving.
    ///
    /// This checks pan/tilt, zoom, and focus positions to detect movement.
    ///
    /// # Returns
    /// * `Ok(true)` - Camera is moving
    /// * `Ok(false)` - Camera is idle
    /// * `Err(Error::*)` - Communication error
    pub async fn is_moving(&self) -> Result<bool, Error> {
        self.camera
            .as_ref()
            .ok_or_else(|| Error::InvalidState("Session is closed".into()))?
            .is_moving_async()
            .await
    }
}

// Blocking-specific close implementation
#[cfg(not(feature = "async"))]
impl<P, Tr> CameraSession<crate::mode::Blocking, P, Tr, ()>
where
    P: Profile,
    Tr: crate::transport::SyncTransport + Send + 'static,
{
    /// Create a new session from a camera instance.
    pub(crate) fn new(camera: Camera<crate::mode::Blocking, P, Tr, ()>) -> Self {
        Self {
            camera: Some(camera),
            closed: false,
            _phantom: PhantomData,
        }
    }

    /// Take ownership of the underlying camera, leaving None in its place.
    fn take_camera(&mut self) -> Option<Camera<crate::mode::Blocking, P, Tr, ()>> {
        self.camera.take()
    }

    /// Explicitly close the camera session.
    ///
    /// This method is idempotent - calling it multiple times is safe.
    /// The session is also automatically closed when dropped.
    pub fn close(mut self) -> Result<(), Error> {
        if self.closed {
            return Ok(());
        }

        self.closed = true;
        if let Some(camera) = self.take_camera() {
            // For blocking mode, just let the camera drop
            // The Drop impl will handle cleanup
            drop(camera);
        }

        Ok(())
    }

    /// Get access to the raw command interface.
    ///
    /// This provides a way to send raw VISCA commands while still going through
    /// the session's command scheduler to maintain proper sequencing.
    pub fn raw(&self) -> Option<RawSender<'_, crate::mode::Blocking, P, Tr, ()>> {
        self.camera.as_ref().map(|cam| RawSender {
            camera: cam,
            _phantom: PhantomData,
        })
    }
}

// Add movement detection methods for blocking sessions
#[cfg(not(feature = "async"))]
impl<P, Tr> CameraSession<crate::mode::Blocking, P, Tr, ()>
where
    P: Profile + crate::capabilities::ProfileMetadata + Default,
    Tr: crate::transport::SyncTransport + Send + 'static,
{
    /// Wait for all movements to complete.
    ///
    /// Convenience method that waits for all motors (pan/tilt, zoom, focus) to stop.
    ///
    /// # Arguments
    /// * `timeout` - Maximum time to wait for movement to complete
    ///
    /// # Returns
    /// * `Ok(())` - Movement completed successfully
    /// * `Err(Error::Timeout)` - Movement did not complete within timeout
    /// * `Err(Error::*)` - Other communication or camera errors
    pub fn await_idle(&mut self, timeout: Duration) -> Result<(), Error> {
        self.camera
            .as_mut()
            .ok_or_else(|| Error::InvalidState("Session is closed".into()))?
            .await_idle(timeout)
    }

    /// Wait for pan/tilt movement to complete.
    ///
    /// Convenience method that waits for pan and tilt motors to stop moving.
    ///
    /// # Arguments
    /// * `timeout` - Maximum time to wait for movement to complete
    ///
    /// # Returns
    /// * `Ok(())` - Movement completed successfully
    /// * `Err(Error::Timeout)` - Movement did not complete within timeout
    pub fn await_pan_tilt_idle(&mut self, timeout: Duration) -> Result<(), Error> {
        self.camera
            .as_mut()
            .ok_or_else(|| Error::InvalidState("Session is closed".into()))?
            .await_pan_tilt_idle(timeout)
    }

    /// Wait for zoom movement to complete.
    ///
    /// Convenience method that waits for zoom motor to stop moving.
    ///
    /// # Arguments
    /// * `timeout` - Maximum time to wait for movement to complete
    ///
    /// # Returns
    /// * `Ok(())` - Movement completed successfully
    /// * `Err(Error::Timeout)` - Movement did not complete within timeout
    pub fn await_zoom_idle(&mut self, timeout: Duration) -> Result<(), Error> {
        self.camera
            .as_mut()
            .ok_or_else(|| Error::InvalidState("Session is closed".into()))?
            .await_zoom_idle(timeout)
    }

    /// Wait for focus movement to complete.
    ///
    /// Convenience method that waits for focus motor to stop moving.
    ///
    /// # Arguments
    /// * `timeout` - Maximum time to wait for movement to complete
    ///
    /// # Returns
    /// * `Ok(())` - Movement completed successfully
    /// * `Err(Error::Timeout)` - Movement did not complete within timeout
    pub fn await_focus_idle(&mut self, timeout: Duration) -> Result<(), Error> {
        self.camera
            .as_mut()
            .ok_or_else(|| Error::InvalidState("Session is closed".into()))?
            .await_focus_idle(timeout)
    }

    /// Check if the camera is currently moving.
    ///
    /// This checks pan/tilt, zoom, and focus positions to detect movement.
    ///
    /// # Returns
    /// * `Ok(true)` - Camera is moving
    /// * `Ok(false)` - Camera is idle
    /// * `Err(Error::*)` - Communication error
    pub fn is_moving(&mut self) -> Result<bool, Error> {
        self.camera
            .as_mut()
            .ok_or_else(|| Error::InvalidState("Session is closed".into()))?
            .is_moving()
    }

    // Accessor methods for blocking mode

    /// Access power-related controls and inquiries.
    #[allow(clippy::expect_used)]
    pub fn power(
        &self,
    ) -> crate::camera::accessors::PowerAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::accessors::PowerAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access zoom-related controls and inquiries.
    #[allow(clippy::expect_used)]
    pub fn zoom(
        &self,
    ) -> crate::camera::accessors::ZoomAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::accessors::ZoomAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access pan/tilt-related controls and inquiries.
    #[allow(clippy::expect_used)]
    pub fn pan_tilt(
        &self,
    ) -> crate::camera::accessors::PanTiltAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::accessors::PanTiltAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access focus-related controls and inquiries.
    #[allow(clippy::expect_used)]
    pub fn focus(
        &self,
    ) -> crate::camera::accessors::FocusAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::accessors::FocusAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access exposure-related controls and inquiries.
    #[allow(clippy::expect_used)]
    pub fn exposure(
        &self,
    ) -> crate::camera::accessors::ExposureAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::accessors::ExposureAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access white balance controls.
    #[allow(clippy::expect_used)]
    pub fn white_balance(
        &self,
    ) -> crate::camera::accessors::WhiteBalanceAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::accessors::WhiteBalanceAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access menu navigation controls.
    #[allow(clippy::expect_used)]
    pub fn menu(
        &self,
    ) -> crate::camera::accessors::MenuAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::accessors::MenuAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access preset controls.
    #[allow(clippy::expect_used)]
    pub fn presets(
        &self,
    ) -> crate::camera::accessors::PresetsAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::accessors::PresetsAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access tally light controls.
    #[allow(clippy::expect_used)]
    pub fn tally(
        &self,
    ) -> crate::camera::accessors::TallyAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::accessors::TallyAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access system-related controls and inquiries.
    #[allow(clippy::expect_used)]
    pub fn system(
        &self,
    ) -> crate::camera::accessors::SystemAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::accessors::SystemAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access image-related controls and inquiries.
    #[allow(clippy::expect_used)]
    pub fn image(
        &self,
    ) -> crate::camera::accessors::ImageAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::accessors::ImageAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }
}

// Implement control traits for blocking CameraSession by delegating to the camera
#[cfg(not(feature = "async"))]
use crate::camera::controls::{focus::FocusControl, pan_tilt::PanTiltControl, zoom::ZoomControl};

#[cfg(not(feature = "async"))]
#[allow(clippy::expect_used)]
impl<P, Tr> PanTiltControl for CameraSession<crate::mode::Blocking, P, Tr, ()>
where
    P: Profile,
    Tr: crate::transport::SyncTransport + Send + 'static,
    Camera<crate::mode::Blocking, P, Tr, ()>: PanTiltControl<Mode = crate::mode::Blocking>,
{
    type Mode = crate::mode::Blocking;

    fn pan_tilt_stop(&self) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .pan_tilt_stop()
    }

    fn pan_tilt_home(&self) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .pan_tilt_home()
    }

    fn pan_tilt_absolute(
        &self,
        pan: crate::units::Degrees,
        tilt: crate::units::Degrees,
        speed: crate::types::SpeedLevel,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .pan_tilt_absolute(pan, tilt, speed)
    }

    fn pan_tilt_relative(
        &self,
        pan: crate::units::Degrees,
        tilt: crate::units::Degrees,
        speed: crate::types::SpeedLevel,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .pan_tilt_relative(pan, tilt, speed)
    }

    fn pan_tilt_move(
        &self,
        direction: crate::command::pan_tilt::PanTiltDirection,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .pan_tilt_move(direction, pan_speed, tilt_speed)
    }

    fn pan_tilt_reset(&self) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .pan_tilt_reset()
    }

    fn pan_tilt_limit_set(
        &self,
        corner: crate::command::pan_tilt::PanTiltLimitCorner,
        pan: crate::types::PanPosition,
        tilt: crate::types::TiltPosition,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .pan_tilt_limit_set(corner, pan, tilt)
    }

    fn pan_tilt_limit_clear(
        &self,
        corner: crate::command::pan_tilt::PanTiltLimitCorner,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .pan_tilt_limit_clear(corner)
    }
}

#[cfg(not(feature = "async"))]
#[allow(clippy::expect_used)]
impl<P, Tr> ZoomControl for CameraSession<crate::mode::Blocking, P, Tr, ()>
where
    P: Profile,
    Tr: crate::transport::SyncTransport + Send + 'static,
    Camera<crate::mode::Blocking, P, Tr, ()>: ZoomControl<Mode = crate::mode::Blocking>,
{
    type Mode = crate::mode::Blocking;

    fn zoom_stop(&self) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .zoom_stop()
    }

    fn zoom_tele_std(&self) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .zoom_tele_std()
    }

    fn zoom_wide_std(&self) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .zoom_wide_std()
    }

    fn zoom_tele_variable(
        &self,
        speed: crate::command::zoom::ZoomSpeed,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .zoom_tele_variable(speed)
    }

    fn zoom_wide_variable(
        &self,
        speed: crate::command::zoom::ZoomSpeed,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .zoom_wide_variable(speed)
    }

    fn zoom_absolute(
        &self,
        position: crate::units::Normalized,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .zoom_absolute(position)
    }

    fn zoom_position(
        &self,
        position: crate::types::ZoomPosition,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .zoom_position(position)
    }

    fn set_digital_zoom(
        &self,
        enabled: bool,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .set_digital_zoom(enabled)
    }
}

#[cfg(not(feature = "async"))]
#[allow(clippy::expect_used)]
impl<P, Tr> FocusControl for CameraSession<crate::mode::Blocking, P, Tr, ()>
where
    P: Profile,
    Tr: crate::transport::SyncTransport + Send + 'static,
    Camera<crate::mode::Blocking, P, Tr, ()>: FocusControl<Mode = crate::mode::Blocking>,
{
    type Mode = crate::mode::Blocking;

    fn focus_auto(&self) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .focus_auto()
    }

    fn focus_manual(&self) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .focus_manual()
    }

    fn focus_near(
        &self,
        speed: crate::types::SpeedLevel,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .focus_near(speed)
    }

    fn focus_far(
        &self,
        speed: crate::types::SpeedLevel,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .focus_far(speed)
    }

    fn focus_stop(&self) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .focus_stop()
    }

    fn focus_one_push(&self) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .focus_one_push()
    }

    fn set_focus(
        &self,
        position: crate::types::FocusPosition,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .set_focus(position)
    }

    fn focus_infinity(&self) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .focus_infinity()
    }

    fn enable_focus_lock(&self) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .enable_focus_lock()
    }

    fn disable_focus_lock(&self) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .disable_focus_lock()
    }

    fn push_af_press(&self) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .push_af_press()
    }

    fn push_af_release(&self) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .push_af_release()
    }

    fn set_focus_zone(
        &self,
        zone: crate::command::FocusZone,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .set_focus_zone(zone)
    }

    fn set_auto_focus_sensitivity(
        &self,
        sensitivity: crate::command::focus::AutoFocusSensitivity,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .set_auto_focus_sensitivity(sensitivity)
    }

    fn set_focus_near_limit(
        &self,
        position: crate::types::FocusPosition,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .set_focus_near_limit(position)
    }
}

// Implement Drop for RAII cleanup
#[cfg(feature = "async")]
impl<M, P, Tr, Exec> Drop for CameraSession<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    fn drop(&mut self) {
        if !self.closed {
            // Take the camera to trigger its Drop impl
            let _ = self.take_camera();
        }
    }
}

/// Raw command sender interface.
///
/// Provides access to send raw VISCA commands through the session's
/// command scheduler, ensuring proper sequencing and timing.
#[cfg(feature = "async")]
pub struct RawSender<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    camera: &'a Camera<M, P, Tr, Exec>,
    _phantom: PhantomData<(M, P, Tr, Exec)>,
}

/// A raw sender for sending VISCA commands through the session's scheduler.
///
/// This type provides access to send raw VISCA commands while ensuring
/// proper sequencing and timing through the camera's command scheduler.
#[cfg(not(feature = "async"))]
pub struct RawSender<'a, M, P, Tr, Exec = ()>
where
    M: Mode,
    P: Profile,
{
    camera: &'a Camera<M, P, Tr, Exec>,
    _phantom: PhantomData<(M, P, Tr, Exec)>,
}

#[cfg(feature = "async")]
impl<'a, M, P, Tr, Exec> std::fmt::Debug for RawSender<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawSender").finish()
    }
}

#[cfg(not(feature = "async"))]
impl<'a, M, P, Tr, Exec> std::fmt::Debug for RawSender<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawSender").finish()
    }
}

#[cfg(feature = "async")]
impl<'a, P, Tr, Exec> RawSender<'a, crate::mode::Async, P, Tr, Exec>
where
    P: Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + Clone + 'static,
{
    /// Send raw bytes as a VISCA command.
    pub async fn send_bytes(&self, bytes: &[u8]) -> Result<(), Error> {
        // Create a RawCommand wrapper for the raw bytes
        let command = RawCommand::new(bytes.to_vec(), CommandKind::Command);

        // Send through the camera's scheduler
        let response = self.camera.send_command(&command).await?;

        use crate::command::response::ViscaResponse;
        match response {
            ViscaResponse::Completion { .. } => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Ok(()),
        }
    }

    /// Send a typed VISCA command.
    pub async fn send_command<C>(&self, command: C) -> Result<C::Response, Error>
    where
        C: crate::command::typed::ViscaCommand
            + ViscaEncode
            + Send
            + Sync
            + Clone
            + std::fmt::Debug
            + 'static,
        C::Response: Send + 'static,
    {
        // Use the camera's typed command method
        self.camera.send_command_typed(&command).await
    }
}

#[cfg(not(feature = "async"))]
impl<'a, P, Tr> RawSender<'a, crate::mode::Blocking, P, Tr, ()>
where
    P: Profile + Default,
    Tr: crate::transport::SyncTransport + Send + 'static,
{
    /// Send raw bytes as a VISCA command.
    pub fn send_bytes(&self, bytes: &[u8]) -> Result<(), Error> {
        use crate::mode::BlockingFutureExt;

        // Create a RawCommand wrapper for the raw bytes
        let command = RawCommand::new(bytes.to_vec(), CommandKind::Command);

        // Send through the camera's scheduler
        let response = self.camera.send_command(&command).block()?;

        use crate::command::response::ViscaResponse;
        match response {
            ViscaResponse::Completion { .. } => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Ok(()),
        }
    }

    /// Send a typed VISCA command.
    pub fn send_command<C>(&self, command: C) -> Result<C::Response, Error>
    where
        C: crate::command::typed::ViscaCommand
            + ViscaEncode
            + Send
            + Sync
            + Clone
            + std::fmt::Debug
            + 'static,
        C::Response: Send + 'static,
    {
        use crate::mode::BlockingFutureExt;

        // Use the camera's typed command method
        self.camera.send_command_typed(&command).block()
    }
}

// Delegate control trait implementations to the underlying camera
// This allows using the session just like the camera for all control operations

// Delegate accessor methods to the underlying camera
// These methods are documented to panic if the session is closed, which is intentional behavior
// for the accessor pattern. Users should check is_closed() if they need to handle this case.
#[cfg(feature = "async")]
#[allow(clippy::expect_used)]
impl<M, P, Tr, Exec> CameraSession<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    /// Access power-related controls and inquiries.
    ///
    /// # Panics
    /// Panics if the session has been closed.
    pub fn power(&self) -> crate::camera::accessors::PowerAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::PowerAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access zoom-related controls and inquiries.
    ///
    /// # Panics
    /// Panics if the session has been closed.
    pub fn zoom(&self) -> crate::camera::accessors::ZoomAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::ZoomAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access system-related controls and inquiries.
    ///
    /// # Panics
    /// Panics if the session has been closed.
    pub fn system(&self) -> crate::camera::accessors::SystemAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::SystemAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access pan/tilt-related controls and inquiries.
    ///
    /// # Panics
    /// Panics if the session has been closed.
    pub fn pan_tilt(&self) -> crate::camera::accessors::PanTiltAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::PanTiltAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access focus-related controls and inquiries.
    ///
    /// # Panics
    /// Panics if the session has been closed.
    pub fn focus(&self) -> crate::camera::accessors::FocusAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::FocusAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access exposure-related controls and inquiries.
    ///
    /// # Panics
    /// Panics if the session has been closed.
    pub fn exposure(&self) -> crate::camera::accessors::ExposureAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::ExposureAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access white balance controls and inquiries.
    ///
    /// # Panics
    /// Panics if the session has been closed.
    pub fn white_balance(
        &self,
    ) -> crate::camera::accessors::WhiteBalanceAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::WhiteBalanceAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access image processing controls and inquiries.
    ///
    /// # Panics
    /// Panics if the session has been closed.
    pub fn image(&self) -> crate::camera::accessors::ImageAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::ImageAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access preset-related controls.
    ///
    /// # Panics
    /// Panics if the session has been closed.
    pub fn presets(&self) -> crate::camera::accessors::PresetsAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::PresetsAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access tally light controls and inquiries.
    ///
    /// # Panics
    /// Panics if the session has been closed.
    pub fn tally(&self) -> crate::camera::accessors::TallyAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::TallyAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access ND filter controls and inquiries.
    ///
    /// # Panics
    /// Panics if the session has been closed.
    pub fn nd_filter(&self) -> crate::camera::accessors::NdFilterAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::NdFilterAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access motion sync controls and inquiries.
    ///
    /// # Panics
    /// Panics if the session has been closed.
    pub fn motion_sync(&self) -> crate::camera::accessors::MotionSyncAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::MotionSyncAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access menu controls and inquiries.
    ///
    /// # Panics
    /// Panics if the session has been closed.
    pub fn menu(&self) -> crate::camera::accessors::MenuAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::MenuAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }

    /// Access advanced settings inquiries.
    ///
    /// # Panics
    /// Panics if the session has been closed.
    pub fn advanced(&self) -> crate::camera::accessors::AdvancedAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::AdvancedAccessor::new(
            self.camera
                .as_ref()
                .expect("Cannot access camera after session is closed"),
        )
    }
}

// Implement control traits by delegating to the underlying camera
// This allows using the session directly for all control operations

#[cfg(feature = "async")]
use crate::camera::controls::{
    focus::FocusControl,
    inquiry::{InquiryControl, PanTiltInquiryControl},
    pan_tilt::PanTiltControl,
    power::PowerControl,
    zoom::ZoomControl,
};
#[cfg(feature = "async")]
use crate::camera::CameraSend;
#[cfg(feature = "async")]
use crate::camera::PanTiltPosition;
#[cfg(feature = "async")]
use crate::command::focus::AutoFocusSensitivity;
#[cfg(feature = "async")]
use crate::command::system::MotionSyncMode;
#[cfg(feature = "async")]
use crate::command::typed::{FlipState, TallyStatusState, VersionInfo};
#[cfg(feature = "async")]
use crate::command::zoom::ZoomSpeed;
#[cfg(feature = "async")]
use crate::command::{
    BlackWhiteMode, ExposureMode, FocusMode, FocusZone, NrMode, SharpnessMode, WhiteBalanceMode,
};
#[cfg(feature = "async")]
use crate::command::{PanTiltDirection, PanTiltLimitCorner};
use crate::executor::Executor;
#[cfg(feature = "async")]
use crate::types::SpeedLevel;
#[cfg(feature = "async")]
use crate::types::{FocusPosition, PanPosition, PanSpeed, TiltPosition, TiltSpeed, ZoomPosition};
#[cfg(feature = "async")]
use crate::units::Degrees;
#[cfg(feature = "async")]
use crate::units::Normalized;

// PowerControl delegation
#[cfg(feature = "async")]
#[allow(clippy::expect_used)]
impl<M, P, Tr, Exec> PowerControl for CameraSession<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Camera<M, P, Tr, Exec>: PowerControl<Mode = M>,
    Exec: Executor,
{
    type Mode = M;

    fn power_on(&self) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .power_on()
    }

    fn power_off(&self) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .power_off()
    }
}

// ZoomControl delegation
#[cfg(feature = "async")]
#[allow(clippy::expect_used)]
impl<M, P, Tr, Exec> ZoomControl for CameraSession<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Camera<M, P, Tr, Exec>: ZoomControl<Mode = M>,
    Exec: Executor,
{
    type Mode = M;

    fn zoom_stop(&self) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .zoom_stop()
    }

    fn zoom_tele_std(&self) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .zoom_tele_std()
    }

    fn zoom_wide_std(&self) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .zoom_wide_std()
    }

    fn zoom_tele_variable(&self, speed: ZoomSpeed) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .zoom_tele_variable(speed)
    }

    fn zoom_wide_variable(&self, speed: ZoomSpeed) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .zoom_wide_variable(speed)
    }

    fn zoom_position(&self, position: ZoomPosition) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .zoom_position(position)
    }

    fn zoom_absolute(&self, position: Normalized) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .zoom_absolute(position)
    }

    fn set_digital_zoom(&self, enabled: bool) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .set_digital_zoom(enabled)
    }
}

// PanTiltControl delegation
#[cfg(feature = "async")]
#[allow(clippy::expect_used)]
impl<M, P, Tr, Exec> PanTiltControl for CameraSession<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Camera<M, P, Tr, Exec>: PanTiltControl<Mode = M>,
    Exec: Executor,
{
    type Mode = M;

    fn pan_tilt_stop(&self) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .pan_tilt_stop()
    }

    fn pan_tilt_home(&self) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .pan_tilt_home()
    }

    fn pan_tilt_absolute(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .pan_tilt_absolute(pan, tilt, speed)
    }

    fn pan_tilt_relative(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .pan_tilt_relative(pan, tilt, speed)
    }

    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .pan_tilt_move(direction, pan_speed, tilt_speed)
    }

    fn pan_tilt_reset(&self) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .pan_tilt_reset()
    }

    fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .pan_tilt_limit_set(corner, pan, tilt)
    }

    fn pan_tilt_limit_clear(&self, corner: PanTiltLimitCorner) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .pan_tilt_limit_clear(corner)
    }
}

// FocusControl delegation
#[cfg(feature = "async")]
#[allow(clippy::expect_used)]
impl<M, P, Tr, Exec> FocusControl for CameraSession<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Camera<M, P, Tr, Exec>: FocusControl<Mode = M>,
    Exec: Executor,
{
    type Mode = M;

    fn focus_auto(&self) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .focus_auto()
    }

    fn focus_manual(&self) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .focus_manual()
    }

    fn focus_near(&self, speed: SpeedLevel) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .focus_near(speed)
    }

    fn focus_far(&self, speed: SpeedLevel) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .focus_far(speed)
    }

    fn focus_stop(&self) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .focus_stop()
    }

    fn focus_one_push(&self) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .focus_one_push()
    }

    fn set_focus(&self, position: FocusPosition) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .set_focus(position)
    }

    fn focus_infinity(&self) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .focus_infinity()
    }

    fn enable_focus_lock(&self) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .enable_focus_lock()
    }

    fn disable_focus_lock(&self) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .disable_focus_lock()
    }

    fn push_af_press(&self) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .push_af_press()
    }

    fn push_af_release(&self) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .push_af_release()
    }

    fn set_focus_zone(&self, zone: FocusZone) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .set_focus_zone(zone)
    }

    fn set_auto_focus_sensitivity(
        &self,
        sensitivity: AutoFocusSensitivity,
    ) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .set_auto_focus_sensitivity(sensitivity)
    }

    fn set_focus_near_limit(&self, position: FocusPosition) -> M::Ret<'_, Result<(), Error>> {
        self.camera
            .as_ref()
            .expect("Cannot access camera after session is closed")
            .set_focus_near_limit(position)
    }
}

#[cfg(feature = "async")]
impl<M, P, Tr, Exec> CameraSend<M> for CameraSession<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Camera<M, P, Tr, Exec>: CameraSend<M>,
    Exec: Executor,
{
    fn send_and_complete<C>(&self, command: C) -> M::Ret<'_, Result<(), Error>>
    where
        C: ViscaEncode + Send + Sync + Clone + std::fmt::Debug + 'static,
    {
        if let Some(camera) = &self.camera {
            camera.send_and_complete(command)
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn send_and_parse<C>(&self, command: C) -> M::Ret<'_, Result<C::Response, Error>>
    where
        C: crate::command::typed::ViscaCommand
            + ViscaEncode
            + Send
            + Sync
            + Clone
            + std::fmt::Debug
            + 'static,
        C::Response: Send + 'static,
    {
        if let Some(camera) = &self.camera {
            camera.send_and_parse(command)
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn error<T>(&self, error: Error) -> M::Ret<'_, Result<T, Error>>
    where
        T: Send + 'static,
    {
        if let Some(camera) = &self.camera {
            camera.error(error)
        } else {
            // Return error directly using Mode::ret
            M::ret(Err(error))
        }
    }
}

// Implement InquiryControl for CameraSession
#[cfg(feature = "async")]
impl<M, P, Tr, Exec> InquiryControl for CameraSession<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile + Default,
    Camera<M, P, Tr, Exec>: InquiryControl<Mode = M> + CameraSend<M>,
    Self: CameraSend<M>,
    Exec: Executor,
{
    type Mode = M;

    fn get_power_state(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_power_state()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_zoom_position(&self) -> <Self::Mode as Mode>::Ret<'_, Result<ZoomPosition, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_zoom_position()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_focus_position(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u16, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_focus_position()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_focus_near_limit(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u16, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_focus_near_limit()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_focus_zone(&self) -> <Self::Mode as Mode>::Ret<'_, Result<FocusZone, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_focus_zone()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_exposure_mode(&self) -> <Self::Mode as Mode>::Ret<'_, Result<ExposureMode, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_exposure_mode()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_exposure_compensation(&self) -> <Self::Mode as Mode>::Ret<'_, Result<i8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_exposure_compensation()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_exposure_compensation_enabled(
        &self,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_exposure_compensation_enabled()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_iris(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_iris()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_shutter(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u16, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_shutter()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_gain(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_gain()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_gain_limit(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_gain_limit()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_white_balance_mode(
        &self,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<WhiteBalanceMode, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_white_balance_mode()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_red_gain(&self) -> <Self::Mode as Mode>::Ret<'_, Result<i8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_red_gain()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_blue_gain(&self) -> <Self::Mode as Mode>::Ret<'_, Result<i8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_blue_gain()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_red_tuning(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_red_tuning()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_blue_tuning(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_blue_tuning()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_color_temperature(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u16, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_color_temperature()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_gamma(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_gamma()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_brightness(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u16, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_brightness()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_sharpness_mode(&self) -> <Self::Mode as Mode>::Ret<'_, Result<SharpnessMode, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_sharpness_mode()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_saturation(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_saturation()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_hue(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_hue()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_black_white(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_black_white()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_resolution(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_resolution()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_picture_effect(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_picture_effect()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_nd_filter_position(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_nd_filter_position()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_version(&self) -> <Self::Mode as Mode>::Ret<'_, Result<VersionInfo, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_version()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_backlight_enabled(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_backlight_enabled()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_image_flip(&self) -> <Self::Mode as Mode>::Ret<'_, Result<FlipState, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_image_flip()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_focus_mode(&self) -> <Self::Mode as Mode>::Ret<'_, Result<FocusMode, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_focus_mode()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_menu_status(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_menu_status()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_tally_light_status(
        &self,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<TallyStatusState, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_tally_light_status()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_night_day_mode(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_night_day_mode()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_flip_mode(&self) -> <Self::Mode as Mode>::Ret<'_, Result<FlipState, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_flip_mode()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_standby_enabled(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_standby_enabled()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_iris_control(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_iris_control()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_defog_level(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_defog_level()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_digital_ptz_enabled(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_digital_ptz_enabled()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_exposure_compensation_position(
        &self,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<u16, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_exposure_compensation_position()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_auto_trace_enabled(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_auto_trace_enabled()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_focus_unlock(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_focus_unlock()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_noise_reduction_level(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_noise_reduction_level()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_noise_reduction_2d(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_noise_reduction_2d()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_noise_reduction_3d(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_noise_reduction_3d()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_broadcast_domain(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_broadcast_domain()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_noise_reduction_mode(&self) -> <Self::Mode as Mode>::Ret<'_, Result<NrMode, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_noise_reduction_mode()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_black_white_mode(&self) -> <Self::Mode as Mode>::Ret<'_, Result<BlackWhiteMode, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_black_white_mode()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_usb_audio_enabled(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_usb_audio_enabled()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_two_tone_mode_enabled(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_two_tone_mode_enabled()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_nd_filter_preset(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_nd_filter_preset()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_digital_mode_enabled(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_digital_mode_enabled()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_tally_auto_adjust_enabled(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_tally_auto_adjust_enabled()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn get_motion_sync_mode(&self) -> <Self::Mode as Mode>::Ret<'_, Result<MotionSyncMode, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_motion_sync_mode()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }
}

// Implement PanTiltInquiryControl for CameraSession
#[cfg(feature = "async")]
impl<M, P, Tr, Exec> PanTiltInquiryControl for CameraSession<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile + Default,
    Camera<M, P, Tr, Exec>: PanTiltInquiryControl<Mode = M> + CameraSend<M>,
    Self: CameraSend<M>,
    Exec: Executor,
{
    type Mode = M;

    fn get_pan_tilt_position(
        &self,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<PanTiltPosition, Error>> {
        if let Some(camera) = &self.camera {
            camera.get_pan_tilt_position()
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }
}
