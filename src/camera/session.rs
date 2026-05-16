//! Camera session management with RAII and explicit close.
//!
//! This module provides the CameraSession type which represents an active
//! connection to a VISCA camera. Sessions provide both RAII (automatic cleanup
//! on drop) and explicit close() methods for controlled shutdown.

use std::{marker::PhantomData, time::Duration};

use crate::{
    camera::Camera,
    camera_id::CameraId,
    capabilities::Profile,
    command::{CommandKind, InquiryKind, ResponseParser, ViscaCommand},
    error::Error,
    mode::Mode,
    timeout::CommandCategory,
};

#[cfg(feature = "mode-async")]
use crate::{camera::ViscaClient, executor::Executor};

/// Marker type for an open session state.
#[derive(Debug, Copy, Clone)]
pub enum Open {}

/// Marker type for a closed session state.
#[derive(Debug, Copy, Clone)]
pub enum Closed {}

/// A closed camera session that no longer provides any operations.
///
/// This type is returned by the `close()` method and indicates that
/// the session has been explicitly closed. No camera operations can
/// be performed on a closed session.
#[derive(Debug, Copy, Clone)]
pub struct ClosedSession;

/// A borrowed wrapper for raw VISCA command bytes.
///
/// Unlike `RawCommand`, this type borrows the bytes without allocation,
/// allowing zero-copy encoding directly from user-provided byte slices.
#[derive(Debug)]
struct RawBytesRef<'a> {
    bytes: &'a [u8],
    kind: CommandKind,
}

impl<'a> RawBytesRef<'a> {
    fn new(bytes: &'a [u8], kind: CommandKind) -> Self {
        Self { bytes, kind }
    }
}

impl ViscaCommand for RawBytesRef<'_> {
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
        buffer[..len].copy_from_slice(self.bytes);
        Ok(len)
    }

    fn response_kind(&self) -> Option<InquiryKind> {
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
/// The session uses a typestate pattern to ensure compile-time safety:
/// - `CameraSession<..., Open>` - An active session that can be used
/// - `CameraSession<..., Closed>` - A closed session with no operations available
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
/// let _closed = session.close().await?;
/// ```
#[cfg(feature = "mode-async")]
pub struct CameraSession<M, P, Tr, Exec, S = Open>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    /// The underlying camera instance.
    camera: Camera<M, P, Tr, Exec>,
    /// Phantom data to ensure type parameters are used.
    _state: PhantomData<S>,
}

/// A camera session represents an active connection to a VISCA camera.
///
/// This type provides both RAII (automatic cleanup on drop) and explicit
/// close() methods for controlled shutdown. All camera control operations
/// can be performed directly through the session.
///
/// The session uses a typestate pattern to ensure compile-time safety:
/// - `CameraSession<..., Open>` - An active session that can be used
/// - `CameraSession<..., Closed>` - A closed session with no operations available
#[cfg(not(feature = "mode-async"))]
pub struct CameraSession<M, P, Tr, Exec = (), S = Open>
where
    M: Mode,
    P: Profile,
{
    /// The underlying camera instance.
    camera: Camera<M, P, Tr, Exec>,
    /// Phantom data to ensure type parameters are used.
    _state: PhantomData<S>,
}

#[cfg(feature = "mode-async")]
impl<M, P, Tr, Exec, S> std::fmt::Debug for CameraSession<M, P, Tr, Exec, S>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state_name = std::any::type_name::<S>()
            .split("::")
            .last()
            .unwrap_or("Unknown");
        f.debug_struct("CameraSession")
            .field("state", &state_name)
            .finish()
    }
}

#[cfg(not(feature = "mode-async"))]
impl<M, P, Tr, Exec, S> std::fmt::Debug for CameraSession<M, P, Tr, Exec, S>
where
    M: Mode,
    P: Profile,
    Exec: crate::executor::Executor,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state_name = std::any::type_name::<S>()
            .split("::")
            .last()
            .unwrap_or("Unknown");
        f.debug_struct("CameraSession")
            .field("state", &state_name)
            .finish()
    }
}

#[cfg(feature = "mode-async")]
impl<M, P, Tr, Exec> CameraSession<M, P, Tr, Exec, Open>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    /// Create a new open session from a camera instance.
    pub(crate) fn new(camera: Camera<M, P, Tr, Exec>) -> Self {
        Self {
            camera,
            _state: PhantomData,
        }
    }

    /// Get a reference to the underlying camera.
    pub fn camera(&self) -> &Camera<M, P, Tr, Exec> {
        &self.camera
    }

    /// Get a mutable reference to the underlying camera.
    pub fn camera_mut(&mut self) -> &mut Camera<M, P, Tr, Exec> {
        &mut self.camera
    }
}

// Async-specific close implementation for Runtime-based cameras
#[cfg(feature = "mode-async")]
impl<P, R> CameraSession<crate::mode::Async, P, crate::runtime::TransportHandle<R>, R, Open>
where
    P: Profile,
    R: crate::runtime::Runtime,
{
    /// Explicitly close the camera session.
    ///
    /// This consumes the session and returns a closed session marker.
    /// The session is also automatically closed when dropped.
    pub async fn close(self) -> Result<ClosedSession, Error> {
        drop(self);
        Ok(ClosedSession)
    }

    /// Get access to the raw command interface.
    ///
    /// This provides a way to send raw VISCA commands while still going through
    /// the session's command scheduler to maintain proper sequencing.
    pub fn raw(
        &self,
    ) -> RawSender<'_, crate::mode::Async, P, crate::runtime::TransportHandle<R>, R> {
        RawSender {
            camera: &self.camera,
            _phantom: PhantomData,
        }
    }
}

// Add movement detection methods for async sessions with more specific bounds
#[cfg(feature = "mode-async")]
impl<P, T, E> CameraSession<crate::mode::Async, P, T, E, Open>
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
        self.camera.await_idle(timeout).await
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
        self.camera.await_pan_tilt_idle(timeout).await
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
        self.camera.await_zoom_idle(timeout).await
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
        self.camera.await_focus_idle(timeout).await
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
        self.camera.is_moving_async().await
    }

    /// Wait for movement completion with configurable options.
    ///
    /// This method provides fine-grained control over movement detection,
    /// including which axes to monitor, timeout, and debug logging.
    ///
    /// # Arguments
    /// * `config` - Configuration for the wait operation
    ///
    /// # Returns
    /// * `Ok(())` - Movement completed successfully
    /// * `Err(Error::Timeout)` - Movement did not complete within timeout
    /// * `Err(Error::*)` - Other communication or camera errors
    pub async fn await_with_config(&self, config: &super::AwaitConfig) -> Result<(), Error> {
        self.camera.await_with_config(config).await
    }

    /// Wait for specific axes to become idle.
    ///
    /// This is a convenience method for selective axis monitoring. Use when
    /// you know which axes were affected by your command.
    ///
    /// # Arguments
    /// * `axes` - Which axes to monitor
    /// * `timeout` - Maximum time to wait
    ///
    /// # Returns
    /// * `Ok(())` - All monitored axes idle
    /// * `Err(Error::Timeout)` - Movement did not complete within timeout
    /// * `Err(Error::*)` - Communication error
    pub async fn await_axes_idle(&self, axes: super::Axes, timeout: Duration) -> Result<(), Error> {
        self.camera.await_axes_idle(axes, timeout).await
    }
}

// Blocking-specific implementation for Open sessions (without transport bounds)
#[cfg(not(feature = "mode-async"))]
impl<P, Tr> CameraSession<crate::mode::Blocking, P, Tr, (), Open>
where
    P: Profile,
{
    /// Get a reference to the underlying camera.
    pub fn camera(&self) -> &Camera<crate::mode::Blocking, P, Tr, ()> {
        &self.camera
    }

    /// Get a mutable reference to the underlying camera.
    pub fn camera_mut(&mut self) -> &mut Camera<crate::mode::Blocking, P, Tr, ()> {
        &mut self.camera
    }

    /// Extract the inner camera, consuming the session.
    pub fn into_inner(self) -> Camera<crate::mode::Blocking, P, Tr, ()> {
        self.camera
    }
}

// Additional methods requiring transport bounds
#[cfg(not(feature = "mode-async"))]
impl<P, Tr> CameraSession<crate::mode::Blocking, P, Tr, (), Open>
where
    P: Profile,
    Tr: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + Send + 'static,
{
    /// Explicitly close the camera session.
    ///
    /// This consumes the session and returns a closed session marker.
    /// The session is also automatically closed when dropped.
    pub fn close(self) -> Result<ClosedSession, Error> {
        drop(self);
        Ok(ClosedSession)
    }

    /// Get access to the raw command interface.
    ///
    /// This provides a way to send raw VISCA commands while still going through
    /// the session's command scheduler to maintain proper sequencing.
    pub fn raw(&self) -> RawSender<'_, crate::mode::Blocking, P, Tr, ()> {
        RawSender {
            camera: &self.camera,
            _phantom: PhantomData,
        }
    }
}

// Add movement detection methods for blocking sessions
#[cfg(not(feature = "mode-async"))]
impl<P, Tr> CameraSession<crate::mode::Blocking, P, Tr, (), Open>
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
        self.camera.await_idle(timeout)
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
        self.camera.await_pan_tilt_idle(timeout)
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
        self.camera.await_zoom_idle(timeout)
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
        self.camera.await_focus_idle(timeout)
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
        self.camera.is_moving()
    }

    /// Wait for movement completion with configurable options.
    ///
    /// This method provides fine-grained control over movement detection,
    /// including which axes to monitor, timeout, and debug logging.
    ///
    /// # Arguments
    /// * `config` - Configuration for the wait operation
    ///
    /// # Returns
    /// * `Ok(())` - Movement completed successfully
    /// * `Err(Error::Timeout)` - Movement did not complete within timeout
    /// * `Err(Error::*)` - Other communication or camera errors
    pub fn await_with_config(&mut self, config: &super::AwaitConfig) -> Result<(), Error> {
        self.camera.await_with_config(config)
    }

    /// Wait for specific axes to become idle.
    ///
    /// This is a convenience method for selective axis monitoring. Use when
    /// you know which axes were affected by your command.
    ///
    /// # Arguments
    /// * `axes` - Which axes to monitor
    /// * `timeout` - Maximum time to wait
    ///
    /// # Returns
    /// * `Ok(())` - All monitored axes idle
    /// * `Err(Error::Timeout)` - Movement did not complete within timeout
    /// * `Err(Error::*)` - Communication error
    pub fn await_axes_idle(&mut self, axes: super::Axes, timeout: Duration) -> Result<(), Error> {
        self.camera.await_axes_idle(axes, timeout)
    }

    /// Access power-related controls and inquiries.
    pub fn power(&self) -> crate::camera::PowerAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::PowerAccessor::new(&self.camera)
    }

    /// Access zoom-related controls and inquiries.
    pub fn zoom(&self) -> crate::camera::ZoomAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::ZoomAccessor::new(&self.camera)
    }

    /// Access pan/tilt-related controls and inquiries.
    pub fn pan_tilt(&self) -> crate::camera::PanTiltAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::PanTiltAccessor::new(&self.camera)
    }

    /// Access focus-related controls and inquiries.
    pub fn focus(&self) -> crate::camera::FocusAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::FocusAccessor::new(&self.camera)
    }

    /// Access exposure-related controls and inquiries.
    pub fn exposure(
        &self,
    ) -> crate::camera::ExposureAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::ExposureAccessor::new(&self.camera)
    }

    /// Access white balance controls.
    pub fn white_balance(
        &self,
    ) -> crate::camera::WhiteBalanceAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::WhiteBalanceAccessor::new(&self.camera)
    }

    /// Access menu navigation controls.
    pub fn menu(&self) -> crate::camera::MenuAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::MenuAccessor::new(&self.camera)
    }

    /// Access preset controls.
    pub fn presets(&self) -> crate::camera::PresetsAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::PresetsAccessor::new(&self.camera)
    }

    /// Access tally light controls.
    pub fn tally(&self) -> crate::camera::TallyAccessor<'_, crate::mode::Blocking, P, Tr, ()>
    where
        P: crate::capabilities::HasTally,
    {
        crate::camera::TallyAccessor::new(&self.camera)
    }

    /// Access system-related controls and inquiries.
    pub fn system(&self) -> crate::camera::SystemAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::SystemAccessor::new(&self.camera)
    }

    /// Access image-related controls and inquiries.
    pub fn image(&self) -> crate::camera::ImageAccessor<'_, crate::mode::Blocking, P, Tr, ()> {
        crate::camera::ImageAccessor::new(&self.camera)
    }
}

// Drop is automatically handled by Rust - the camera field will be dropped
// when the Open session is dropped. No explicit Drop impl needed since
// we don't need special cleanup logic anymore.

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
    ///
    /// This method encodes the bytes directly without intermediate allocation,
    /// copying them once into the encoded command payload.
    pub async fn send_bytes(&self, bytes: &[u8]) -> Result<(), Error> {
        let command = RawBytesRef::new(bytes, CommandKind::Command);
        self.camera.execute(command).await
    }

    /// Execute a typed VISCA command and require a successful completion.
    ///
    /// This is the raw-command escape hatch for custom command types that are
    /// not yet represented by a typed control method.
    pub async fn execute<C>(&self, command: C) -> Result<(), Error>
    where
        C: ViscaCommand,
    {
        self.camera.execute(command).await
    }

    /// Send a typed VISCA command.
    pub async fn send_command<C>(
        &self,
        command: C,
    ) -> Result<<C as ResponseParser>::Response, Error>
    where
        C: ResponseParser + ViscaCommand,
        <C as ResponseParser>::Response: Send + 'static,
    {
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
    ///
    /// This method encodes the bytes directly without intermediate allocation,
    /// copying them once into the encoded command payload.
    pub fn send_bytes(&self, bytes: &[u8]) -> Result<(), Error> {
        use crate::mode::BlockingFutureExt;

        let command = RawBytesRef::new(bytes, CommandKind::Command);
        self.camera.execute(command).block()
    }

    /// Execute a typed VISCA command and require a successful completion.
    ///
    /// This is the raw-command escape hatch for custom command types that are
    /// not yet represented by a typed control method.
    pub fn execute<C>(&self, command: C) -> Result<(), Error>
    where
        C: ViscaCommand,
    {
        use crate::mode::BlockingFutureExt;

        self.camera.execute(command).block()
    }

    /// Send a typed VISCA command.
    pub fn send_command<C>(&self, command: C) -> Result<<C as ResponseParser>::Response, Error>
    where
        C: ResponseParser + ViscaCommand,
        <C as ResponseParser>::Response: Send + 'static,
    {
        use crate::mode::BlockingFutureExt;

        self.camera.send_command_typed(&command).block()
    }
}

// Delegate accessor methods to the underlying camera
// These methods are only available for Open sessions, ensuring compile-time safety.
#[cfg(feature = "mode-async")]
impl<M, P, Tr, Exec> CameraSession<M, P, Tr, Exec, Open>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    /// Access power-related controls and inquiries.
    pub fn power(&self) -> crate::camera::PowerAccessor<'_, M, P, Tr, Exec> {
        crate::camera::PowerAccessor::new(&self.camera)
    }

    /// Access zoom-related controls and inquiries.
    pub fn zoom(&self) -> crate::camera::ZoomAccessor<'_, M, P, Tr, Exec> {
        crate::camera::ZoomAccessor::new(&self.camera)
    }

    /// Access system-related controls and inquiries.
    pub fn system(&self) -> crate::camera::SystemAccessor<'_, M, P, Tr, Exec> {
        crate::camera::SystemAccessor::new(&self.camera)
    }

    /// Access pan/tilt-related controls and inquiries.
    pub fn pan_tilt(&self) -> crate::camera::PanTiltAccessor<'_, M, P, Tr, Exec> {
        crate::camera::PanTiltAccessor::new(&self.camera)
    }

    /// Access focus-related controls and inquiries.
    pub fn focus(&self) -> crate::camera::FocusAccessor<'_, M, P, Tr, Exec> {
        crate::camera::FocusAccessor::new(&self.camera)
    }

    /// Access exposure-related controls and inquiries.
    pub fn exposure(&self) -> crate::camera::ExposureAccessor<'_, M, P, Tr, Exec> {
        crate::camera::ExposureAccessor::new(&self.camera)
    }

    /// Access white balance controls and inquiries.
    pub fn white_balance(&self) -> crate::camera::WhiteBalanceAccessor<'_, M, P, Tr, Exec> {
        crate::camera::WhiteBalanceAccessor::new(&self.camera)
    }

    /// Access image processing controls and inquiries.
    pub fn image(&self) -> crate::camera::ImageAccessor<'_, M, P, Tr, Exec> {
        crate::camera::ImageAccessor::new(&self.camera)
    }

    /// Access preset-related controls.
    pub fn presets(&self) -> crate::camera::PresetsAccessor<'_, M, P, Tr, Exec> {
        crate::camera::PresetsAccessor::new(&self.camera)
    }

    /// Access tally light controls and inquiries.
    pub fn tally(&self) -> crate::camera::TallyAccessor<'_, M, P, Tr, Exec>
    where
        P: crate::capabilities::HasTally,
    {
        crate::camera::TallyAccessor::new(&self.camera)
    }

    /// Access ND filter controls and inquiries.
    pub fn nd_filter(&self) -> crate::camera::NdFilterAccessor<'_, M, P, Tr, Exec>
    where
        P: crate::capabilities::HasNdFilter,
    {
        crate::camera::NdFilterAccessor::new(&self.camera)
    }

    /// Access motion sync controls and inquiries.
    pub fn motion_sync(&self) -> crate::camera::MotionSyncAccessor<'_, M, P, Tr, Exec>
    where
        P: crate::capabilities::HasMotionSync,
    {
        crate::camera::MotionSyncAccessor::new(&self.camera)
    }

    /// Access menu controls and inquiries.
    pub fn menu(&self) -> crate::camera::MenuAccessor<'_, M, P, Tr, Exec> {
        crate::camera::MenuAccessor::new(&self.camera)
    }

    /// Access advanced settings inquiries.
    pub fn advanced(&self) -> crate::camera::AdvancedAccessor<'_, M, P, Tr, Exec> {
        crate::camera::AdvancedAccessor::new(&self.camera)
    }
}

#[cfg(feature = "mode-async")]
impl<M, P, Tr, Exec> ViscaClient<M> for CameraSession<M, P, Tr, Exec, Open>
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
        self.camera.execute(command)
    }

    fn query<C>(&self, command: C) -> M::Fut<'_, Result<<C as ResponseParser>::Response, Error>>
    where
        C: ResponseParser + ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static,
        <C as ResponseParser>::Response: Send + 'static,
    {
        self.camera.query(command)
    }

    fn error<T>(&self, error: Error) -> M::Fut<'_, Result<T, Error>>
    where
        T: Send + 'static,
    {
        self.camera.error(error)
    }

    fn cache(&self) -> &crate::cache::StateCache {
        self.camera.cache()
    }

    fn execute_updating_cache<C, F>(
        &self,
        command: C,
        update_fn: F,
    ) -> M::Fut<'_, Result<(), Error>>
    where
        C: ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static,
        F: FnOnce(&crate::cache::StateCache) + Send + 'static,
    {
        self.camera.execute_updating_cache(command, update_fn)
    }
}
