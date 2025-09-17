//! Camera session management with RAII and explicit close.
//!
//! This module provides the CameraSession type which represents an active
//! connection to a VISCA camera. Sessions provide both RAII (automatic cleanup
//! on drop) and explicit close() methods for controlled shutdown.

use std::{marker::PhantomData, time::Duration};

#[cfg(feature = "mode-async")]
use crate::camera::ViscaClient;

use crate::{
    camera::Camera,
    camera_id::CameraId,
    capabilities::Profile,
    command::{response::InquiryKind, typed::ResponseParser, CommandKind, ViscaCommand},
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

impl ViscaCommand for RawCommand {
    type Response = ();
    const MAX_SIZE: usize = 256; // Allow reasonably sized raw commands
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Custom;

    fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
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

    fn response_kind(&self) -> Option<InquiryKind> {
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
#[cfg(feature = "mode-async")]
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
#[cfg(not(feature = "mode-async"))]
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

#[cfg(feature = "mode-async")]
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

#[cfg(not(feature = "mode-async"))]
impl<M, P, Tr, Exec> std::fmt::Debug for CameraSession<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: crate::executor::Executor,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraSession")
            .field("closed", &self.closed)
            .finish()
    }
}

#[cfg(feature = "mode-async")]
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
#[cfg(feature = "mode-async")]
impl<P, R> CameraSession<crate::mode::Async, P, crate::runtime::TransportHandle<R>, R>
where
    P: Profile,
    R: crate::runtime::Runtime,
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
    ) -> Option<RawSender<'_, crate::mode::Async, P, crate::runtime::TransportHandle<R>, R>> {
        self.camera.as_ref().map(|cam| RawSender {
            camera: cam,
            _phantom: PhantomData,
        })
    }
}

// Add movement detection methods for async sessions with more specific bounds
#[cfg(feature = "mode-async")]
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
#[cfg(not(feature = "mode-async"))]
impl<P, Tr> CameraSession<crate::mode::Blocking, P, Tr, ()>
where
    P: Profile,
    Tr: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + Send + 'static,
{
    /// Create a new session from a camera instance.
    pub(crate) fn new(camera: Camera<crate::mode::Blocking, P, Tr, ()>) -> Self {
        Self {
            camera: Some(camera),
            closed: false,
            _phantom: PhantomData,
        }
    }

    /// Get a reference to the underlying camera.
    ///
    /// Returns None if the session has been closed.
    pub fn camera(&self) -> Option<&Camera<crate::mode::Blocking, P, Tr, ()>> {
        self.camera.as_ref()
    }

    /// Get a mutable reference to the underlying camera.
    ///
    /// Returns None if the session has been closed.
    pub fn camera_mut(&mut self) -> Option<&mut Camera<crate::mode::Blocking, P, Tr, ()>> {
        self.camera.as_mut()
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
#[cfg(not(feature = "mode-async"))]
impl<P, Tr> CameraSession<crate::mode::Blocking, P, Tr, ()>
where
    P: Profile + crate::capabilities::ProfileMetadata + Default,
    Tr: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + Send + 'static,
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
// All control trait delegations are auto-generated by macros

// Implement Drop for RAII cleanup
#[cfg(feature = "mode-async")]
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
#[cfg(feature = "mode-async")]
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
#[cfg(not(feature = "mode-async"))]
pub struct RawSender<'a, M, P, Tr, Exec = ()>
where
    M: Mode,
    P: Profile,
{
    camera: &'a Camera<M, P, Tr, Exec>,
    _phantom: PhantomData<(M, P, Tr, Exec)>,
}

#[cfg(feature = "mode-async")]
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

#[cfg(not(feature = "mode-async"))]
impl<'a, M, P, Tr, Exec> std::fmt::Debug for RawSender<'a, M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawSender").finish()
    }
}

#[cfg(feature = "mode-async")]
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

        use crate::command::response::Response;
        match response {
            Response::Completion { .. } => Ok(()),
            Response::Error(e) => Err(e),
            _ => Ok(()),
        }
    }

    /// Send a typed VISCA command.
    pub async fn send_command<C>(
        &self,
        command: C,
    ) -> Result<<C as ResponseParser>::Response, Error>
    where
        C: ResponseParser + ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static,
        <C as ResponseParser>::Response: Send + 'static,
    {
        // Use the camera's typed command method
        self.camera.send_command_typed(&command).await
    }
}

#[cfg(not(feature = "mode-async"))]
impl<'a, P, Tr> RawSender<'a, crate::mode::Blocking, P, Tr, ()>
where
    P: Profile + Default,
    Tr: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + Send + 'static,
{
    /// Send raw bytes as a VISCA command.
    pub fn send_bytes(&self, bytes: &[u8]) -> Result<(), Error> {
        use crate::mode::BlockingFutureExt;

        // Create a RawCommand wrapper for the raw bytes
        let command = RawCommand::new(bytes.to_vec(), CommandKind::Command);

        // Send through the camera's scheduler
        let response = self.camera.send_command(&command).block()?;

        use crate::command::response::Response;
        match response {
            Response::Completion { .. } => Ok(()),
            Response::Error(e) => Err(e),
            _ => Ok(()),
        }
    }

    /// Send a typed VISCA command.
    pub fn send_command<C>(&self, command: C) -> Result<<C as ResponseParser>::Response, Error>
    where
        C: ResponseParser + ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static,
        <C as ResponseParser>::Response: Send + 'static,
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
#[cfg(feature = "mode-async")]
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

#[cfg(feature = "mode-async")]
use crate::executor::Executor;

#[cfg(feature = "mode-async")]
impl<M, P, Tr, Exec> ViscaClient<M> for CameraSession<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Camera<M, P, Tr, Exec>: ViscaClient<M>,
    Exec: Executor,
{
    fn execute<C>(&self, command: C) -> M::Fut<'_, Result<(), Error>>
    where
        C: ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static,
    {
        if let Some(camera) = &self.camera {
            camera.execute(command)
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn query<C>(&self, command: C) -> M::Fut<'_, Result<<C as ResponseParser>::Response, Error>>
    where
        C: ResponseParser + ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static,
        <C as ResponseParser>::Response: Send + 'static,
    {
        if let Some(camera) = &self.camera {
            camera.query(command)
        } else {
            self.error(Error::InvalidState("Session is closed".into()))
        }
    }

    fn error<T>(&self, error: Error) -> M::Fut<'_, Result<T, Error>>
    where
        T: Send + 'static,
    {
        if let Some(camera) = &self.camera {
            camera.error(error)
        } else {
            // Return error directly using Mode::ready
            M::ready(Err(error))
        }
    }
}

// All inquiry trait delegations are auto-generated by macros
