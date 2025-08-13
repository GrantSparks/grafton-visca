//! Generic camera implementation using compile-time profiles.
//!
//! This module provides the new generic Camera<P, T> struct that uses
//! compile-time profile selection for zero-cost abstractions.

use std::{borrow::Cow, marker::PhantomData, sync::Arc, time::Duration};

#[cfg(feature = "async")]
use crate::camera::AsyncMode;
use crate::camera::BlockingMode;

#[cfg(feature = "async")]
use crate::{executor::Spawner, runtime::RuntimeSpawner, socket_manager::SocketManagerHandle};

#[cfg(feature = "async")]
use std::sync::Mutex;

use crate::{
    camera_id::CameraId,
    capabilities::Profile,
    command::{encode_visca::EncodeVisca, Response},
    error::Error,
    timeout::TimeoutConfig,
    transport::TransportEnvelope,
};

#[cfg(feature = "async")]
use crate::transport::AsyncTransport;

use crate::transport::BlockingTransport;

/// Generic camera client with compile-time mode and profile selection.
///
/// This struct provides type-safe camera control with zero runtime overhead.
/// All mode and profile-specific constants and behaviors are resolved at compile time.
///
/// # Type Parameters
///
/// * `M` - Camera mode (AsyncMode or BlockingMode)
/// * `P` - Camera profile implementing the `Profile` trait
/// * `T` - Transport (AsyncTransport for async mode, BlockingTransport for blocking mode)
///
/// # Examples
///
/// ```ignore
/// use grafton_visca::{CameraBuilder, camera::profiles::PTZOpticsG2};
/// use grafton_visca::camera::{CameraAsync, CameraBlocking};
///
/// // Async camera
/// use grafton_visca::transport::tokio::Tcp;
/// let transport = Tcp::connect("192.168.0.110:52381").await?;
/// let camera: CameraAsync<PTZOpticsG2, _> = Camera::new(transport);
///
/// // Blocking camera
/// use grafton_visca::transport::blocking::Tcp;
/// let transport = Tcp::connect("192.168.0.110:52381")?;
/// let camera: CameraBlocking<PTZOpticsG2, _> = Camera::new(transport);
/// ```
pub struct Camera<M, P, T>
where
    P: Profile,
{
    transport: Arc<T>,
    camera_id: CameraId,
    #[cfg(feature = "async")]
    socket_manager: Arc<Mutex<Option<SocketManagerHandle>>>,
    envelope: TransportEnvelope,
    #[cfg(feature = "async")]
    spawner: Arc<Mutex<Option<Arc<dyn Spawner>>>>,
    #[cfg(feature = "async")]
    runtime: Arc<Mutex<Option<crate::runtime::SharedRuntime>>>,
    timeout_config: TimeoutConfig,
    _mode: PhantomData<M>,
    _profile: PhantomData<P>,
}

impl<M, P, T> Clone for Camera<M, P, T>
where
    P: Profile,
{
    fn clone(&self) -> Self {
        Self {
            transport: Arc::clone(&self.transport),
            camera_id: self.camera_id,
            #[cfg(feature = "async")]
            socket_manager: self.socket_manager.clone(),
            envelope: TransportEnvelope::new(P::PROTOCOL_STYLE),
            #[cfg(feature = "async")]
            spawner: self.spawner.clone(),
            #[cfg(feature = "async")]
            runtime: self.runtime.clone(),
            timeout_config: self.timeout_config,
            _mode: PhantomData,
            _profile: PhantomData,
        }
    }
}

// Implement Drop for Camera to ensure graceful shutdown for async mode
impl<M, P, T> Drop for Camera<M, P, T>
where
    P: Profile,
{
    fn drop(&mut self) {
        // Only handle socket manager shutdown for async mode
        #[cfg(feature = "async")]
        {
            // Check if this is an async camera by trying to get the socket manager
            // Since socket_manager is only populated for async cameras, this is safe
            if let Ok(mut socket_manager_lock) = self.socket_manager.try_lock() {
                if let Some(socket_manager) = socket_manager_lock.take() {
                    // Send shutdown signal without waiting for completion
                    // The actor will handle the shutdown gracefully
                    socket_manager.shutdown_nowait();
                    log::debug!("Sent shutdown signal to socket manager during Camera drop");
                }
            } else {
                // If we can't get the lock, the socket manager is likely in use
                // It will be dropped when the lock is released
                log::debug!("Could not acquire socket manager lock during Camera drop");
            }
        }
    }
}

impl<M, P, T> std::fmt::Debug for Camera<M, P, T>
where
    P: Profile,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("Camera");
        debug
            .field("profile", &P::MODEL_NAME)
            .field("camera_id", &self.camera_id)
            .field("transport", &"<Transport>");
        #[cfg(feature = "async")]
        {
            if let Ok(sm) = self.socket_manager.try_lock() {
                debug.field("socket_manager", &sm.is_some());
            } else {
                debug.field("socket_manager", &"<locked>");
            }
        }
        debug.finish()
    }
}

// Common methods that work for any mode
impl<M, P, T> Camera<M, P, T>
where
    P: Profile,
{
    /// Get the camera's model name.
    #[must_use]
    pub fn model_name(&self) -> &'static str {
        P::MODEL_NAME
    }

    /// Set the camera ID for this camera.
    ///
    /// # Errors
    /// Returns an error if the ID is not in the valid range (1-8).
    pub fn set_camera_id(&mut self, id: u8) -> Result<(), Error> {
        self.camera_id = CameraId::new(id)?;
        Ok(())
    }

    /// Get the current camera ID.
    #[must_use]
    pub fn camera_id(&self) -> CameraId {
        self.camera_id
    }

    /// Get profile-specific constants directly.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let zoom_range = camera.zoom_speed_range();
    /// let max_pan_speed = camera.max_pan_speed();
    /// ```
    #[must_use]
    pub fn zoom_speed_range(&self) -> std::ops::Range<u8> {
        P::ZOOM_SPEED_RANGE
    }

    /// Get the maximum pan speed.
    #[must_use]
    pub fn max_pan_speed(&self) -> u8 {
        P::MAX_PAN_SPEED
    }

    /// Get the maximum tilt speed.
    #[must_use]
    pub fn max_tilt_speed(&self) -> u8 {
        P::MAX_TILT_SPEED
    }

    /// Get the maximum optical zoom value.
    #[must_use]
    pub fn optical_zoom_max(&self) -> u16 {
        P::OPTICAL_ZOOM_MAX
    }

    /// Get the maximum digital zoom value.
    #[must_use]
    pub fn digital_zoom_max(&self) -> Option<u16> {
        P::DIGITAL_ZOOM_MAX
    }

    /// Get the maximum number of presets.
    #[must_use]
    pub fn max_presets(&self) -> u8 {
        P::MAX_PRESETS
    }

    /// Get the power on time duration.
    #[must_use]
    pub fn power_on_time(&self) -> Duration {
        P::POWER_ON_TIME
    }

    /// Get the standby time duration.
    #[must_use]
    pub fn standby_time(&self) -> Duration {
        P::STANDBY_TIME
    }

    /// Convert degrees to pan/tilt units.
    #[must_use]
    pub fn degrees_to_units(
        &self,
        pan_deg: crate::units::Degrees,
        tilt_deg: crate::units::Degrees,
    ) -> (crate::units::ViscaUnits<i16>, crate::units::ViscaUnits<i16>) {
        let pan_units = (pan_deg.0 * P::PAN_DEGREES_TO_UNITS) as i16;
        let tilt_units = (tilt_deg.0 * P::TILT_DEGREES_TO_UNITS) as i16;

        (
            crate::units::ViscaUnits(pan_units),
            crate::units::ViscaUnits(tilt_units),
        )
    }

    /// Convert pan/tilt units to degrees.
    #[must_use]
    pub fn units_to_degrees(
        &self,
        pan_units: i16,
        tilt_units: i16,
    ) -> (crate::units::Degrees, crate::units::Degrees) {
        let pan_deg = pan_units as f32 / P::PAN_DEGREES_TO_UNITS;
        let tilt_deg = tilt_units as f32 / P::TILT_DEGREES_TO_UNITS;

        (
            crate::units::Degrees(pan_deg),
            crate::units::Degrees(tilt_deg),
        )
    }

    /// Validate pan position.
    pub fn validate_pan(&self, pan_units: i16) -> Result<i16, Error> {
        use crate::capabilities::ValidationError;

        if P::PAN_RANGE.contains(&pan_units) {
            Ok(pan_units)
        } else {
            Err(Error::ValidationError(ValidationError::OutOfRange {
                parameter: "pan",
                value: pan_units as f64,
                min: P::PAN_RANGE.start as f64,
                max: P::PAN_RANGE.end as f64,
            }))
        }
    }

    /// Validate tilt position.
    pub fn validate_tilt(&self, tilt_units: i16) -> Result<i16, Error> {
        use crate::capabilities::ValidationError;

        if P::TILT_RANGE.contains(&tilt_units) {
            Ok(tilt_units)
        } else {
            Err(Error::ValidationError(ValidationError::OutOfRange {
                parameter: "tilt",
                value: tilt_units as f64,
                min: P::TILT_RANGE.start as f64,
                max: P::TILT_RANGE.end as f64,
            }))
        }
    }

    /// Get the coordinate system used by this camera.
    #[must_use]
    pub fn coordinate_system(&self) -> crate::capabilities::CoordinateSystem {
        P::COORDINATE_SYSTEM
    }

    /// Convert logical pan/tilt coordinates to camera-specific coordinates.
    #[must_use]
    pub fn to_camera_coords(&self, pan: i16, tilt: i16) -> (u16, u16) {
        self.coordinate_system().to_camera_coords(pan, tilt)
    }

    /// Convert camera-specific coordinates to logical pan/tilt coordinates.
    #[must_use]
    pub fn from_camera_coords(&self, pan: u16, tilt: u16) -> (i16, i16) {
        self.coordinate_system()
            .convert_from_camera_coords(pan, tilt)
    }

    /// Get the current timeout configuration.
    #[must_use]
    pub fn timeout_config(&self) -> &TimeoutConfig {
        &self.timeout_config
    }

    /// Set a new timeout configuration.
    ///
    /// If the socket manager is initialized (async mode), this will also update
    /// its timeout configuration.
    pub fn set_timeout_config(&mut self, config: TimeoutConfig) {
        self.timeout_config = config;

        // Update socket manager's timeout config if it's initialized
        #[cfg(feature = "async")]
        {
            if let Ok(socket_manager_lock) = self.socket_manager.lock() {
                if let Some(ref socket_manager) = *socket_manager_lock {
                    if let Err(e) = socket_manager.update_timeout_config(config) {
                        log::warn!("Failed to update socket manager timeout config: {}", e);
                    }
                }
            }
        }
    }

    /// Create a new camera with a custom timeout configuration.
    #[must_use]
    pub fn with_timeout_config(mut self, config: TimeoutConfig) -> Self {
        self.timeout_config = config;
        self
    }
}

// Async mode implementation
#[cfg(feature = "async")]
impl<P, T> Camera<AsyncMode, P, T>
where
    P: Profile,
    T: AsyncTransport + 'static,
{
    // Note: shutdown_socket_manager was removed since Drop now handles cleanup automatically

    /// Create a new async camera from an async transport.
    pub fn from_transport(transport: T) -> Self {
        let camera_id = CameraId::new(P::DEFAULT_ADDRESS).unwrap_or(CameraId::CAMERA_1);

        Self {
            envelope: TransportEnvelope::new(P::PROTOCOL_STYLE),
            transport: Arc::new(transport),
            camera_id,
            socket_manager: Arc::new(Mutex::new(None)),
            spawner: Arc::new(Mutex::new(None)),
            runtime: Arc::new(Mutex::new(None)),
            timeout_config: TimeoutConfig::default(),
            _mode: PhantomData,
            _profile: PhantomData,
        }
    }

    /// Set a spawner for async operations.
    pub fn with_spawner<S>(self, spawner: S) -> Self
    where
        S: Spawner,
    {
        {
            let mut spawner_lock = self.spawner.lock().unwrap();
            *spawner_lock = Some(Arc::new(spawner));
        }

        // Note: We can't initialize socket manager here anymore since we need async context
        // It will be initialized lazily on first command

        self
    }

    /// Set a runtime for async operations.
    pub fn with_runtime(self, runtime: crate::runtime::SharedRuntime) -> Self {
        {
            let mut spawner_lock = self.spawner.lock().unwrap();
            *spawner_lock = Some(Arc::new(RuntimeSpawner::new(Arc::clone(&runtime))));

            let mut runtime_lock = self.runtime.lock().unwrap();
            *runtime_lock = Some(runtime);
        }

        // Note: We can't initialize socket manager here anymore since we need async context
        // It will be initialized lazily on first command

        self
    }

    /// Get the runtime if configured.
    #[cfg(feature = "async")]
    pub(crate) fn runtime(&self) -> Option<crate::runtime::SharedRuntime> {
        let runtime_lock = self.runtime.lock().unwrap();
        runtime_lock.clone()
    }

    /// Initialize the socket manager for this camera (internal use only).
    /// This requires holding locks, so it should be called carefully.
    fn initialize_socket_manager_internal(
        &self,
        socket_manager: &mut Option<SocketManagerHandle>,
        spawner: &Option<Arc<dyn Spawner>>,
        runtime: &Option<crate::runtime::SharedRuntime>,
    ) -> Result<(), Error> {
        if socket_manager.is_some() {
            return Ok(());
        }

        // Create socket manager components
        let (command_sender, command_receiver) = crate::channels::unbounded();

        // Store the handle
        let handle = SocketManagerHandle::new(command_sender);
        *socket_manager = Some(handle);

        // Start the socket manager actor with camera's timeout config
        let transport = Arc::clone(&self.transport);
        let timeout_config = self.timeout_config;

        // Get runtime (required for socket manager)
        let runtime = if let Some(ref runtime) = runtime {
            Arc::clone(runtime)
        } else {
            return Err(Error::MissingRuntime);
        };

        let actor = crate::socket_manager::SocketManagerActor::new(
            transport,
            command_receiver,
            timeout_config,
            self.camera_id,
            runtime,
        );

        if let Some(spawner) = spawner {
            // Use the provided spawner
            let future = Box::pin(async move {
                if let Err(e) = actor.run().await {
                    log::error!("Socket manager actor failed: {e}");
                }
            });
            spawner.spawn(future);
        } else {
            log::error!("Cannot initialize socket manager without a spawner");
            return Err(Error::InvalidState(
                Cow::Borrowed("Socket manager requires a spawner. Use Camera::new().with_spawner() to provide one."),
            ));
        }

        Ok(())
    }

    /// Automatically initialize the socket manager if needed.
    #[cfg(feature = "async")]
    pub(crate) async fn auto_init_orchestrator_if_needed(&self) -> Result<(), Error> {
        // Check if already initialized without holding the lock too long
        {
            let socket_manager_lock = self.socket_manager.lock().unwrap();
            if socket_manager_lock.is_some() {
                return Ok(());
            }
        }

        // Acquire all locks we need
        let mut socket_manager_lock = self.socket_manager.lock().unwrap();
        let mut spawner_lock = self.spawner.lock().unwrap();
        let mut runtime_lock = self.runtime.lock().unwrap();

        // Double-check after acquiring locks
        if socket_manager_lock.is_some() {
            return Ok(());
        }

        // If rt-tokio feature is enabled and no runtime is set, use default tokio runtime
        #[cfg(feature = "rt-tokio")]
        if runtime_lock.is_none() {
            let default_runtime: crate::runtime::SharedRuntime =
                Arc::new(crate::runtime::TokioRuntime);
            *runtime_lock = Some(default_runtime.clone());
            *spawner_lock = Some(Arc::new(RuntimeSpawner::new(default_runtime)));
        }

        // Now try to initialize the socket manager
        self.initialize_socket_manager_internal(
            &mut *socket_manager_lock,
            &*spawner_lock,
            &*runtime_lock,
        )
    }

    /// Send a command asynchronously and wait for response.
    pub async fn send_command<C>(&self, command: &C) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Auto-initialize socket manager if needed
        self.auto_init_orchestrator_if_needed().await?;

        // Get socket manager handle
        let socket_manager = {
            let socket_manager_lock = self.socket_manager.lock().unwrap();
            socket_manager_lock
                .as_ref()
                .ok_or(Error::InvalidState(Cow::Borrowed(
                    "Failed to initialize socket manager.",
                )))?
                .clone()
        };

        self.send_command_via_socket_manager(command, &socket_manager)
            .await
    }

    /// Send command via socket manager (new queued approach)
    #[cfg(feature = "async")]
    async fn send_command_via_socket_manager<C>(
        &self,
        command: &C,
        socket_manager: &SocketManagerHandle,
    ) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Get command bytes using EncodeVisca
        let mut buffer = [0u8; 64];
        let size = command.encode_into(self.camera_id, &mut buffer)?;
        let mut cmd_bytes = buffer[..size].to_vec();

        // Add VISCA terminator if not present
        if cmd_bytes.last() != Some(&crate::command::const_encoding::VISCA_TERMINATOR) {
            cmd_bytes.push(crate::command::const_encoding::VISCA_TERMINATOR);
        }

        // Apply protocol-specific framing using transport envelope
        let is_inquiry = command.response_type().is_some();
        let framed_bytes = self.envelope.frame_command(&cmd_bytes, is_inquiry);

        log::debug!("Sending VISCA command via socket manager: {framed_bytes:02X?}");

        // Get timeout category from command
        let category = command.timeout_kind();

        // Get the expected response type from the command
        let response_type = command.response_type();

        // Send via socket manager
        socket_manager
            .send_command(framed_bytes, category, is_inquiry, response_type)
            .await
    }

    /// Wait for a completion message from the camera.
    ///
    /// This method is used for event-driven movement detection. It waits for the camera
    /// to send a completion message (0x51) indicating that a movement operation has finished.
    ///
    /// # Returns
    /// * `Ok(())` if a completion message was received
    /// * `Err(Error::Timeout)` if no completion message was received within the timeout
    #[cfg(feature = "async")]
    #[cfg_attr(not(feature = "rt-tokio"), allow(unused_variables))]
    pub(crate) async fn wait_for_completion(&self, timeout: Duration) -> Result<(), Error> {
        // Auto-initialize socket manager if needed
        self.auto_init_orchestrator_if_needed().await?;

        // Get socket manager handle
        let socket_manager = {
            let socket_manager_lock = self.socket_manager.lock().unwrap();
            socket_manager_lock.clone()
        };

        // Check if socket manager is available
        if let Some(socket_manager) = socket_manager {
            log::debug!("wait_for_completion: using socket manager to wait for completion message");

            let wait_fut = socket_manager.wait_for_completion();

            // Try to get runtime for timeout
            let runtime = self.runtime();

            if let Some(runtime) = runtime.as_ref() {
                match crate::runtime::timeout_with_runtime(runtime.as_ref(), timeout, wait_fut)
                    .await
                {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(e)) => Err(e),
                    Err(e) => Err(e),
                }
            } else {
                // No runtime configured - this is an error for async operations that require timeouts
                log::error!("No runtime configured for async timeout operation. Configure a runtime using CameraBuilder::runtime()");
                Err(Error::MissingRuntime)
            }
        } else {
            // No socket manager, can't wait for completion
            log::debug!("wait_for_completion: no socket manager available");
            Err(Error::Unsupported)
        }
    }

    // ================================================================================
    // Centralized command sending with typed results (Workstream C)
    // ================================================================================

    /// Send an action command and wait for completion.
    ///
    /// This method centralizes the ACK/Completion handling for action commands,
    /// automatically handling the response sequence and returning a simple Result.
    ///
    /// # Returns
    /// * `Ok(())` if the command completed successfully
    /// * `Err(Error)` if the command failed or timed out
    #[cfg(feature = "async")]
    pub(crate) async fn send_action_command<C>(&self, command: &C) -> Result<(), Error>
    where
        C: EncodeVisca,
    {
        let response = self.send_command(command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            Response::CmdAck => Err(Error::CommandPending), // Should not happen with current logic
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Send an inquiry command and extract the typed result.
    ///
    /// This method centralizes the response handling for inquiry commands,
    /// automatically extracting the inquiry data and returning it.
    ///
    /// # Returns
    /// * `Ok(InquiryResponse)` if the inquiry succeeded
    /// * `Err(Error)` if the inquiry failed or returned unexpected data
    #[cfg(feature = "async")]
    pub async fn send_inquiry_command<C>(
        &self,
        command: &C,
    ) -> Result<crate::command::InquiryResponse, Error>
    where
        C: EncodeVisca,
    {
        let response = self.send_command(command).await?;
        match response {
            Response::Inquiry(data) => Ok(data),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Power methods for async mode

    /// Power on the camera.
    pub async fn power_on(&self) -> Result<(), Error> {
        use crate::command::PowerCommand;
        let command = PowerCommand::On;
        self.send_action_command(&command).await?;
        // Wait for camera to be ready (if runtime is available)
        if let Some(runtime) = self.runtime() {
            runtime.sleep(P::POWER_ON_TIME).await;
        } else {
            // Without runtime, we can't enforce the power-on delay
            // The caller should handle their own delay if needed
            log::warn!(
                "No runtime available for power-on delay; camera may not be ready immediately"
            );
        }
        Ok(())
    }

    /// Power off the camera.
    pub async fn power_off(&self) -> Result<(), Error> {
        use crate::command::PowerCommand;
        let command = PowerCommand::Standby;
        self.send_action_command(&command).await?;
        // Wait for standby/off (if runtime is available)
        if let Some(runtime) = self.runtime() {
            runtime.sleep(P::STANDBY_TIME).await;
        } else {
            // Without runtime, we can't enforce the standby delay
            log::warn!("No runtime available for standby delay");
        }
        Ok(())
    }

    /// Inquiry: Get current power status.
    pub async fn power_inquiry(&self) -> Result<bool, Error> {
        use crate::command::inquiry::PowerInquiry;
        let command = PowerInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::Power { on } => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Pan/Tilt methods

    /// Stop all pan/tilt movement.
    #[allow(clippy::expect_used)]
    pub async fn pan_tilt_stop(&self) -> Result<(), Error> {
        use crate::command::pan_tilt::{PanTilt, PanTiltDirection};
        use crate::types::{PanSpeed, TiltSpeed};
        let command = PanTilt::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0).expect("0 is valid speed"),
            tilt_speed: TiltSpeed::new(0).expect("0 is valid speed"),
        };
        self.send_action_command(&command).await
    }

    /// Move to home position (0, 0).
    pub async fn pan_tilt_home(&self) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let command = PanTilt::Home;
        self.send_action_command(&command).await
    }

    /// Reset pan/tilt mechanism.
    pub async fn pan_tilt_reset(&self) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let command = PanTilt::Reset;
        self.send_action_command(&command).await
    }

    /// Move to absolute pan/tilt position.
    pub async fn pan_tilt_absolute(
        &self,
        pan: crate::types::PanPosition,
        tilt: crate::types::TiltPosition,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let command = PanTilt::AbsolutePosition {
            pan,
            tilt,
            pan_speed,
            tilt_speed,
        };
        self.send_action_command(&command).await
    }

    /// Move relative to current position.
    pub async fn pan_tilt_relative(
        &self,
        pan: crate::types::PanPosition,
        tilt: crate::types::TiltPosition,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let command = PanTilt::RelativePosition {
            pan,
            tilt,
            pan_speed,
            tilt_speed,
        };
        self.send_action_command(&command).await
    }

    /// Move pan/tilt in a specific direction.
    pub async fn pan_tilt_move(
        &self,
        direction: crate::command::pan_tilt::PanTiltDirection,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let command = PanTilt::Move {
            direction,
            pan_speed,
            tilt_speed,
        };
        self.send_action_command(&command).await
    }

    /// Set pan/tilt movement limit for a specific corner.
    pub async fn pan_tilt_limit_set(
        &self,
        corner: crate::command::pan_tilt::PanTiltLimitCorner,
        pan: crate::types::PanPosition,
        tilt: crate::types::TiltPosition,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let command = PanTilt::LimitSet { corner, pan, tilt };
        self.send_action_command(&command).await
    }

    /// Clear pan/tilt movement limit for a specific corner.
    pub async fn pan_tilt_limit_clear(
        &self,
        corner: crate::command::pan_tilt::PanTiltLimitCorner,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let command = PanTilt::LimitClear { corner };
        self.send_action_command(&command).await
    }

    /// Inquiry: Get current pan/tilt position.
    pub async fn pan_tilt_position_inquiry(
        &self,
    ) -> Result<(crate::types::PanPosition, crate::types::TiltPosition), Error> {
        use crate::command::inquiry::PanTiltPositionInquiry;
        let command = PanTiltPositionInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::PanTiltPosition { pan, tilt } => Ok((
                crate::types::PanPosition::try_from(pan)?,
                crate::types::TiltPosition::try_from(tilt)?,
            )),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Zoom methods

    /// Stop zoom movement.
    pub async fn zoom_stop(&self) -> Result<(), Error> {
        use crate::command::zoom::Zoom;
        let command = Zoom::Stop;
        self.send_action_command(&command).await
    }

    /// Zoom in at standard speed (telephoto).
    pub async fn zoom_tele_std(&self) -> Result<(), Error> {
        use crate::command::zoom::Zoom;
        let command = Zoom::TeleStd;
        self.send_action_command(&command).await
    }

    /// Zoom out at standard speed (wide).
    pub async fn zoom_wide_std(&self) -> Result<(), Error> {
        use crate::command::zoom::Zoom;
        let command = Zoom::WideStd;
        self.send_action_command(&command).await
    }

    /// Zoom in at variable speed.
    pub async fn zoom_tele_variable(
        &self,
        speed: crate::command::zoom::ZoomSpeed,
    ) -> Result<(), Error> {
        use crate::command::zoom::Zoom;
        let command = Zoom::TeleVariable(speed);
        self.send_action_command(&command).await
    }

    /// Zoom out at variable speed.
    pub async fn zoom_wide_variable(
        &self,
        speed: crate::command::zoom::ZoomSpeed,
    ) -> Result<(), Error> {
        use crate::command::zoom::Zoom;
        let command = Zoom::WideVariable(speed);
        self.send_action_command(&command).await
    }

    /// Set zoom to specific position.
    pub async fn zoom_position(&self, position: crate::types::ZoomPosition) -> Result<(), Error> {
        use crate::command::zoom::Zoom;
        let command = Zoom::Position(position);
        self.send_action_command(&command).await
    }

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    pub async fn zoom_absolute(&self, position: crate::units::Normalized) -> Result<(), Error> {
        let position_value = position.0;

        // Validate position
        if !(0.0..=1.0).contains(&position_value) {
            return Err(Error::InvalidParameter {
                parameter: "zoom position",
                value: Cow::Owned(position_value.to_string()),
                reason: Cow::Borrowed("must be between 0.0 and 1.0"),
            });
        }

        // Convert normalized to raw zoom position (0x0000 to 0x4000)
        let raw = (position_value * 16384.0) as u16;
        let zoom_pos = crate::types::ZoomPosition::try_from(raw)?;
        self.zoom_position(zoom_pos).await
    }

    /// Inquiry: Get current zoom position.
    pub async fn zoom_position_inquiry(&self) -> Result<crate::types::ZoomPosition, Error> {
        use crate::command::inquiry::ZoomPositionInquiry;
        let command = ZoomPositionInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::ZoomPosition { position } => {
                crate::types::ZoomPosition::try_from(position)
            }
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current zoom position as a raw value.
    pub async fn get_zoom_position(&self) -> Result<u16, Error> {
        let zoom_pos = self.zoom_position_inquiry().await?;
        Ok(zoom_pos.into())
    }

    /// Get the current pan and tilt position in degrees.
    pub async fn get_pan_tilt_degrees(
        &self,
    ) -> Result<(crate::units::Degrees, crate::units::Degrees), Error> {
        let (pan_pos, tilt_pos) = self.pan_tilt_position_inquiry().await?;
        let (pan_deg, tilt_deg) = self.units_to_degrees(pan_pos.into(), tilt_pos.into());
        Ok((pan_deg, tilt_deg))
    }

    // Focus methods

    /// Stop any focus movement.
    pub async fn focus_stop(&self) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::Stop;
        self.send_action_command(&command).await
    }

    /// Move focus far at standard speed.
    pub async fn focus_far(&self) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::Far;
        self.send_action_command(&command).await
    }

    /// Move focus near at standard speed.
    pub async fn focus_near(&self) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::Near;
        self.send_action_command(&command).await
    }

    /// Move focus far at variable speed.
    pub async fn focus_far_with_speed(
        &self,
        speed: crate::command::focus::FocusSpeed,
    ) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::FarWithSpeed(speed);
        self.send_action_command(&command).await
    }

    /// Move focus near at variable speed.
    pub async fn focus_near_with_speed(
        &self,
        speed: crate::command::focus::FocusSpeed,
    ) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::NearWithSpeed(speed);
        self.send_action_command(&command).await
    }

    /// Set focus to specific position.
    pub async fn focus_position(&self, position: crate::types::FocusPosition) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::Position(position);
        self.send_action_command(&command).await
    }

    /// Enable auto focus mode.
    pub async fn focus_auto(&self) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::Auto;
        self.send_action_command(&command).await
    }

    /// Enable manual focus mode.
    pub async fn focus_manual(&self) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::Manual;
        self.send_action_command(&command).await
    }

    /// Trigger one-push auto focus (focus once then return to manual).
    pub async fn focus_one_push(&self) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::OnePushTrigger;
        self.send_action_command(&command).await
    }

    /// Set focus to infinity.
    pub async fn focus_infinity(&self) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::Infinity;
        self.send_action_command(&command).await
    }

    /// Inquiry: Get current focus position.
    pub async fn focus_position_inquiry(&self) -> Result<crate::types::FocusPosition, Error> {
        use crate::command::inquiry::FocusPositionInquiry;
        let command = FocusPositionInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::FocusPosition { position } => {
                crate::types::FocusPosition::try_from(position)
            }
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current focus mode (auto/manual).
    pub async fn focus_mode_inquiry(&self) -> Result<crate::command::focus::FocusMode, Error> {
        use crate::command::inquiry::FocusModeInquiry;
        let command = FocusModeInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::FocusMode { mode } => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Additional inquiry helper methods

    /// Inquiry: Get camera version information.
    pub async fn version_inquiry(&self) -> Result<crate::command::InquiryResponse, Error> {
        use crate::command::inquiry::VersionInquiry;
        let command = VersionInquiry;
        self.send_inquiry_command(&command).await
    }

    /// Inquiry: Get current exposure mode.
    pub async fn exposure_mode_inquiry(
        &self,
    ) -> Result<crate::command::exposure::ExposureMode, Error> {
        use crate::command::inquiry::ExposureModeInquiry;
        let command = ExposureModeInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::ExposureMode { mode } => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current white balance mode.
    pub async fn white_balance_mode_inquiry(
        &self,
    ) -> Result<crate::command::white_balance::WhiteBalanceMode, Error> {
        use crate::command::inquiry::WhiteBalanceModeInquiry;
        let command = WhiteBalanceModeInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::WhiteBalanceMode { mode } => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current iris position.
    pub async fn iris_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::IrisInquiry;
        let command = IrisInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::Iris { position } => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current gain value.
    pub async fn gain_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::GainInquiry;
        let command = GainInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::GainLevel { gain } => Ok(gain),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current shutter speed.
    pub async fn shutter_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::ShutterInquiry;
        let command = ShutterInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::Shutter { position } => Ok(position as u8),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current brightness level.
    pub async fn brightness_inquiry(&self) -> Result<u16, Error> {
        use crate::command::inquiry::BrightInquiry;
        let command = BrightInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::Bright { position } => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get backlight compensation state.
    pub async fn backlight_inquiry(&self) -> Result<bool, Error> {
        use crate::command::inquiry::BacklightInquiry;
        let command = BacklightInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::Backlight { status } => Ok(status),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get image flip settings.
    pub async fn image_flip_inquiry(&self) -> Result<(bool, bool), Error> {
        use crate::command::inquiry::ImageFlipInquiry;
        let command = ImageFlipInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            } => Ok((horizontal, vertical)),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // NOTE: Sharpness inquiry command is not documented in VISCA specs
    // and has been disabled until proper documentation is found.
    // /// Inquiry: Get current sharpness level.
    // pub async fn sharpness_inquiry(&self) -> Result<u8, Error> {
    //     use crate::command::inquiry::SharpnessInquiry;
    //     let command = SharpnessInquiry;
    //     let response = self.send_inquiry_command(&command).await?;
    //     match response {
    //         crate::command::InquiryResponse::Sharpness { value } => Ok(value),
    //         _ => Err(Error::UnexpectedResponseType),
    //     }
    // }

    // NOTE: Contrast inquiry command is not documented in VISCA specs
    // and has been disabled until proper documentation is found.
    // /// Inquiry: Get current contrast level.
    // pub async fn contrast_inquiry(&self) -> Result<u8, Error> {
    //     use crate::command::inquiry::ContrastInquiry;
    //     let command = ContrastInquiry;
    //     let response = self.send_inquiry_command(&command).await?;
    //     match response {
    //         crate::command::InquiryResponse::Contrast(value) => Ok(value),
    //         _ => Err(Error::UnexpectedResponseType),
    //     }
    // }

    /// Inquiry: Get current saturation level.
    pub async fn saturation_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::SaturationInquiry;
        let command = SaturationInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::Saturation { level } => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // NOTE: AutoFocus inquiry is not documented in VISCA specs
    // and has been disabled until proper documentation is found.
    // /// Inquiry: Get auto focus on/off state.
    // pub async fn auto_focus_inquiry(&self) -> Result<bool, Error> {
    //     use crate::command::inquiry::AutoFocusInquiry;
    //     let command = AutoFocusInquiry;
    //     let response = self.send_inquiry_command(&command).await?;
    //     match response {
    //         crate::command::InquiryResponse::AutoFocus { enabled } => Ok(enabled),
    //         _ => Err(Error::UnexpectedResponseType),
    //     }
    // }

    /// Inquiry: Get current color temperature.
    pub async fn color_temperature_inquiry(&self) -> Result<u16, Error> {
        use crate::command::inquiry::ColorTemperatureInquiry;
        let command = ColorTemperatureInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::ColorTemperature { temperature } => Ok(temperature),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current hue adjustment.
    pub async fn hue_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::HueInquiry;
        let command = HueInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::Hue { hue } => Ok(hue),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current gain limit.
    pub async fn gain_limit_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::GainLimitInquiry;
        let command = GainLimitInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::GainLimit { limit } => Ok(limit),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get exposure compensation mode on/off.
    pub async fn exposure_compensation_mode_inquiry(&self) -> Result<bool, Error> {
        use crate::command::inquiry::ExposureCompensationModeInquiry;
        let command = ExposureCompensationModeInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::ExposureCompensationMode { on } => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get exposure compensation value.
    pub async fn exposure_compensation_inquiry(&self) -> Result<i8, Error> {
        use crate::command::inquiry::ExposureCompensationInquiry;
        let command = ExposureCompensationInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::ExposureCompensation { value } => Ok(value),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get 2D noise reduction level.
    pub async fn noise_reduction_2d_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::NoiseReduction2DInquiry;
        let command = NoiseReduction2DInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::NoiseReduction2D { level } => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get 3D noise reduction level.
    pub async fn noise_reduction_3d_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::NoiseReduction3DInquiry;
        let command = NoiseReduction3DInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::NoiseReduction3D { level } => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get focus near limit position.
    pub async fn focus_near_limit_inquiry(&self) -> Result<u16, Error> {
        use crate::command::inquiry::FocusNearLimitInquiry;
        let command = FocusNearLimitInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::FocusNearLimit { position } => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current resolution mode.
    pub async fn resolution_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::ResolutionInquiry;
        let command = ResolutionInquiry;
        let response = self.send_inquiry_command(&command).await?;
        match response {
            crate::command::InquiryResponse::Resolution(mode) => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Exposure methods

    /// Set exposure mode.
    pub async fn set_exposure_mode(
        &self,
        mode: crate::command::exposure::ExposureMode,
    ) -> Result<(), Error> {
        use crate::command::exposure::ExposureCommand;
        let command = ExposureCommand { mode };
        self.send_command(&command).await.map(|_| ())
    }

    /// Set auto exposure mode.
    pub async fn exposure_auto(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Auto)
            .await
    }

    /// Set manual exposure mode.
    pub async fn exposure_manual(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Manual)
            .await
    }

    /// Set shutter priority exposure mode.
    pub async fn exposure_shutter_priority(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Shutter)
            .await
    }

    /// Set iris priority exposure mode.
    pub async fn exposure_iris_priority(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Iris)
            .await
    }

    /// Set brightness priority exposure mode.
    pub async fn exposure_bright_mode(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Bright)
            .await
    }

    /// Set iris level.
    pub async fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error> {
        use crate::command::exposure::Iris;
        let command = Iris::SetAperture(level);
        self.send_command(&command).await.map(|_| ())
    }

    /// Reset iris to default.
    pub async fn reset_iris(&self) -> Result<(), Error> {
        use crate::command::exposure::Iris;
        let command = Iris::Reset;
        self.send_command(&command).await.map(|_| ())
    }

    /// Increase iris (open aperture).
    pub async fn increase_iris(&self) -> Result<(), Error> {
        use crate::command::exposure::Iris;
        let command = Iris::Up;
        self.send_command(&command).await.map(|_| ())
    }

    /// Decrease iris (close aperture).
    pub async fn decrease_iris(&self) -> Result<(), Error> {
        use crate::command::exposure::Iris;
        let command = Iris::Down;
        self.send_command(&command).await.map(|_| ())
    }

    /// Set brightness level.
    pub async fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
        use crate::command::exposure::Bright;
        let command = Bright::SetLevel(level);
        self.send_command(&command).await.map(|_| ())
    }

    /// Reset brightness to default.
    pub async fn reset_brightness(&self) -> Result<(), Error> {
        use crate::command::exposure::Bright;
        let command = Bright::Reset;
        self.send_command(&command).await.map(|_| ())
    }

    /// Increase brightness.
    pub async fn increase_brightness(&self) -> Result<(), Error> {
        use crate::command::exposure::Bright;
        let command = Bright::Up;
        self.send_command(&command).await.map(|_| ())
    }

    /// Decrease brightness.
    pub async fn decrease_brightness(&self) -> Result<(), Error> {
        use crate::command::exposure::Bright;
        let command = Bright::Down;
        self.send_command(&command).await.map(|_| ())
    }

    /// Set shutter speed.
    pub async fn set_shutter_speed(&self, speed: crate::types::ShutterSpeed) -> Result<(), Error> {
        use crate::command::exposure::Shutter;
        let command = Shutter::SetSpeed(speed);
        self.send_command(&command).await.map(|_| ())
    }

    /// Reset shutter speed to default.
    pub async fn reset_shutter_speed(&self) -> Result<(), Error> {
        use crate::command::exposure::Shutter;
        let command = Shutter::Reset;
        self.send_command(&command).await.map(|_| ())
    }

    /// Increase shutter speed (faster).
    pub async fn increase_shutter_speed(&self) -> Result<(), Error> {
        use crate::command::exposure::Shutter;
        let command = Shutter::Up;
        self.send_command(&command).await.map(|_| ())
    }

    /// Decrease shutter speed (slower).
    pub async fn decrease_shutter_speed(&self) -> Result<(), Error> {
        use crate::command::exposure::Shutter;
        let command = Shutter::Down;
        self.send_command(&command).await.map(|_| ())
    }

    /// Set gain value.
    pub async fn set_gain(&self, gain: crate::types::GainLevel) -> Result<(), Error> {
        use crate::command::gain::Gain;
        let command = Gain::SetValue(gain);
        self.send_command(&command).await.map(|_| ())
    }

    /// Reset gain to default.
    pub async fn reset_gain(&self) -> Result<(), Error> {
        use crate::command::gain::Gain;
        let command = Gain::Reset;
        self.send_command(&command).await.map(|_| ())
    }

    /// Increase gain.
    pub async fn increase_gain(&self) -> Result<(), Error> {
        use crate::command::gain::Gain;
        let command = Gain::Up;
        self.send_command(&command).await.map(|_| ())
    }

    /// Decrease gain.
    pub async fn decrease_gain(&self) -> Result<(), Error> {
        use crate::command::gain::Gain;
        let command = Gain::Down;
        self.send_command(&command).await.map(|_| ())
    }

    /// Set gain limit.
    pub async fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error> {
        use crate::command::gain::GainLimitCommand;
        let command = GainLimitCommand::new(limit);
        self.send_command(&command).await.map(|_| ())
    }

    /// Set exposure compensation level.
    pub async fn set_exposure_compensation_level(
        &self,
        level: crate::types::ExposureCompensationLevel,
    ) -> Result<(), Error> {
        use crate::command::exposure::ExposureCompensation;
        let command = ExposureCompensation::SetLevel(level);
        self.send_command(&command).await.map(|_| ())
    }

    /// Enable exposure compensation.
    pub async fn enable_exposure_compensation(&self) -> Result<(), Error> {
        use crate::command::exposure::ExposureCompensation;
        let command = ExposureCompensation::On;
        self.send_command(&command).await.map(|_| ())
    }

    /// Disable exposure compensation.
    pub async fn disable_exposure_compensation(&self) -> Result<(), Error> {
        use crate::command::exposure::ExposureCompensation;
        let command = ExposureCompensation::Off;
        self.send_command(&command).await.map(|_| ())
    }

    /// Reset exposure compensation.
    pub async fn reset_exposure_compensation(&self) -> Result<(), Error> {
        use crate::command::exposure::ExposureCompensation;
        let command = ExposureCompensation::Reset;
        self.send_command(&command).await.map(|_| ())
    }

    /// Increase exposure compensation.
    pub async fn increase_exposure_compensation(&self) -> Result<(), Error> {
        use crate::command::exposure::ExposureCompensation;
        let command = ExposureCompensation::Up;
        self.send_command(&command).await.map(|_| ())
    }

    /// Decrease exposure compensation.
    pub async fn decrease_exposure_compensation(&self) -> Result<(), Error> {
        use crate::command::exposure::ExposureCompensation;
        let command = ExposureCompensation::Down;
        self.send_command(&command).await.map(|_| ())
    }

    /// Set backlight compensation.
    pub async fn set_backlight(&self, enabled: bool) -> Result<(), Error> {
        use crate::command::image::BacklightCommand;
        let command = BacklightCommand::new(enabled);
        self.send_command(&command).await.map(|_| ())
    }

    /// Enable auto slow shutter.
    pub async fn enable_auto_slow_shutter(&self) -> Result<(), Error> {
        use crate::command::exposure::AutoSlowShutter;
        let command = AutoSlowShutter::On;
        self.send_command(&command).await.map(|_| ())
    }

    /// Disable auto slow shutter.
    pub async fn disable_auto_slow_shutter(&self) -> Result<(), Error> {
        use crate::command::exposure::AutoSlowShutter;
        let command = AutoSlowShutter::Off;
        self.send_command(&command).await.map(|_| ())
    }

    /// Enable spotlight mode.
    pub async fn enable_spotlight(&self) -> Result<(), Error> {
        use crate::command::exposure::Spotlight;
        let command = Spotlight::On;
        self.send_command(&command).await.map(|_| ())
    }

    /// Disable spotlight mode.
    pub async fn disable_spotlight(&self) -> Result<(), Error> {
        use crate::command::exposure::Spotlight;
        let command = Spotlight::Off;
        self.send_command(&command).await.map(|_| ())
    }

    /// Set brightness direct value.
    pub async fn set_brightness_direct(
        &self,
        value: crate::types::BrightnessLevel,
    ) -> Result<(), Error> {
        use crate::command::exposure::Bright;
        let command = Bright::SetLevel(value);
        self.send_command(&command).await.map(|_| ())
    }

    /// Set color temperature.
    pub async fn set_color_temperature(&self, temp: u16) -> Result<(), Error> {
        use crate::command::color::ColorTemperature;
        use crate::types::ColorTemp;
        let color_temp = ColorTemp::new(temp).map_err(|_| Error::InvalidParameter {
            parameter: "color_temperature",
            value: temp.to_string().into(),
            reason: "Invalid color temperature value".into(),
        })?;
        let command = ColorTemperature::SetTemperature(color_temp);
        self.send_command(&command).await.map(|_| ())
    }

    // White Balance Methods

    /// Set white balance mode to any supported mode.
    pub async fn set_white_balance_mode(
        &self,
        mode: crate::command::white_balance::WhiteBalanceMode,
    ) -> Result<(), Error> {
        use crate::command::white_balance::WhiteBalanceCommand;
        let command = WhiteBalanceCommand { mode };
        self.send_command(&command).await.map(|_| ())
    }

    /// Set auto white balance mode.
    pub async fn white_balance_auto(&self) -> Result<(), Error> {
        self.set_white_balance_mode(crate::command::white_balance::WhiteBalanceMode::Auto)
            .await
    }

    /// Set indoor white balance preset (optimized for incandescent/tungsten lighting).
    pub async fn white_balance_indoor(&self) -> Result<(), Error> {
        self.set_white_balance_mode(crate::command::white_balance::WhiteBalanceMode::Indoor)
            .await
    }

    /// Set outdoor white balance preset (optimized for daylight).
    pub async fn white_balance_outdoor(&self) -> Result<(), Error> {
        self.set_white_balance_mode(crate::command::white_balance::WhiteBalanceMode::Outdoor)
            .await
    }

    /// Set one-push white balance mode (calibrate once based on current scene).
    pub async fn white_balance_one_push(&self) -> Result<(), Error> {
        self.set_white_balance_mode(crate::command::white_balance::WhiteBalanceMode::OnePush)
            .await
    }

    /// Set auto tracking white balance (Sony FR7 specific).
    pub async fn white_balance_atw(&self) -> Result<(), Error> {
        self.set_white_balance_mode(crate::command::white_balance::WhiteBalanceMode::ATW)
            .await
    }

    /// Set manual white balance mode.
    pub async fn white_balance_manual(&self) -> Result<(), Error> {
        self.set_white_balance_mode(crate::command::white_balance::WhiteBalanceMode::Manual)
            .await
    }

    /// Set color temperature white balance mode.
    pub async fn white_balance_color_temperature(&self) -> Result<(), Error> {
        self.set_white_balance_mode(
            crate::command::white_balance::WhiteBalanceMode::ColorTemperature,
        )
        .await
    }

    /// Set AWB sensitivity level (PTZOptics specific).
    pub async fn set_awb_sensitivity(
        &self,
        sensitivity: crate::command::white_balance::AutoWhiteBalanceSensitivity,
    ) -> Result<(), Error> {
        use crate::command::white_balance::AWBSensitivityCommand;
        let command = AWBSensitivityCommand { sensitivity };
        self.send_command(&command).await.map(|_| ())
    }

    /// Set dynamic range.
    pub async fn set_dynamic_range(
        &self,
        _range: crate::types::DynamicRangeLevel,
    ) -> Result<(), Error> {
        // TODO: DynamicRangeCommand not yet implemented
        Err(Error::Unsupported)
    }

    /// Set the variable speed mode (24-step or 50-step).
    ///
    /// Only available on Sony FR7.
    pub async fn set_variable_speed_mode(
        &self,
        mode: crate::command::VariableSpeedMode,
    ) -> Result<(), Error> {
        use crate::command::{Response, VariableSpeedModeCommand};
        let cmd = VariableSpeedModeCommand::new(mode);
        match self.send_command(&cmd).await? {
            Response::CmdAck | Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // ========== Preset Methods ==========

    /// Store current position to a preset.
    pub async fn preset_set(
        &self,
        preset_number: crate::command::preset::PresetNumber,
    ) -> Result<(), Error> {
        use crate::command::preset::{PresetAction, PresetCommand};
        let command = PresetCommand {
            action: PresetAction::Set,
            preset_number,
        };
        self.send_action_command(&command).await
    }

    /// Recall a preset position.
    pub async fn preset_recall(
        &self,
        preset_number: crate::command::preset::PresetNumber,
    ) -> Result<(), Error> {
        use crate::command::preset::{PresetAction, PresetCommand};
        let command = PresetCommand {
            action: PresetAction::Recall,
            preset_number,
        };
        self.send_action_command(&command).await
    }

    /// Reset/clear a preset.
    pub async fn preset_reset(
        &self,
        preset_number: crate::command::preset::PresetNumber,
    ) -> Result<(), Error> {
        use crate::command::preset::{PresetAction, PresetCommand};
        let command = PresetCommand {
            action: PresetAction::Reset,
            preset_number,
        };
        self.send_action_command(&command).await
    }

    /// Enable image flip (vertical).
    pub async fn enable_flip(&self) -> Result<(), Error> {
        use crate::command::flip::{Flip, ImageFlipCommand};
        let command = ImageFlipCommand::new(Flip::On);
        self.send_action_command(&command).await
    }

    /// Disable image flip (vertical).
    pub async fn disable_flip(&self) -> Result<(), Error> {
        use crate::command::flip::{Flip, ImageFlipCommand};
        let command = ImageFlipCommand::new(Flip::Off);
        self.send_action_command(&command).await
    }

    /// Get image flip status.
    pub async fn get_image_flip(&self) -> Result<crate::command::ImageFlipStatus, Error> {
        use crate::command::inquiry::ImageFlipInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&ImageFlipInquiry).await?;
        match response {
            InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            } => Ok(crate::command::ImageFlipStatus {
                vertical,
                horizontal,
            }),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("ImageFlip inquiry response"),
                actual: vec![],
            }),
        }
    }

    // ============================================================================
    // Inquiry Methods - Complete Implementation
    // ============================================================================

    /// Get the current power state of the camera.
    /// Returns `true` if powered on, `false` if in standby.
    pub async fn get_power_state(&self) -> Result<bool, Error> {
        use crate::command::inquiry::PowerInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&PowerInquiry).await?;
        match response {
            InquiryResponse::Power { on } => Ok(on),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("Power inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current focus position.
    pub async fn get_focus_position(&self) -> Result<u16, Error> {
        use crate::command::inquiry::FocusPositionInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&FocusPositionInquiry).await?;
        match response {
            InquiryResponse::FocusPosition { position } => Ok(position),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("FocusPosition inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the focus near limit position.
    pub async fn get_focus_near_limit(&self) -> Result<u16, Error> {
        use crate::command::inquiry::FocusNearLimitInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&FocusNearLimitInquiry).await?;
        match response {
            InquiryResponse::FocusNearLimit { position } => Ok(position),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("FocusNearLimit inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current focus zone.
    pub async fn get_focus_zone(&self) -> Result<crate::command::FocusZone, Error> {
        use crate::command::inquiry::FocusZoneInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&FocusZoneInquiry).await?;
        match response {
            InquiryResponse::FocusZone { zone } => Ok(zone),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("FocusZone inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the auto-focus sensitivity setting.
    pub async fn get_auto_focus_sensitivity(
        &self,
    ) -> Result<crate::command::AutoFocusSensitivity, Error> {
        use crate::command::inquiry::AutoFocusSensitivityInquiry;
        use crate::command::InquiryResponse;
        let response = self
            .send_inquiry_command(&AutoFocusSensitivityInquiry)
            .await?;
        match response {
            InquiryResponse::AutoFocusSensitivity { sensitivity } => Ok(sensitivity),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("AutoFocusSensitivity inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current exposure mode.
    pub async fn get_exposure_mode(&self) -> Result<crate::command::exposure::ExposureMode, Error> {
        use crate::command::inquiry::ExposureModeInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&ExposureModeInquiry).await?;
        match response {
            InquiryResponse::ExposureMode { mode } => Ok(mode),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("ExposureMode inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the exposure compensation value.
    pub async fn get_exposure_compensation(&self) -> Result<i8, Error> {
        use crate::command::inquiry::ExposureCompensationInquiry;
        use crate::command::InquiryResponse;
        let response = self
            .send_inquiry_command(&ExposureCompensationInquiry)
            .await?;
        match response {
            InquiryResponse::ExposureCompensation { value } => Ok(value),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("ExposureCompensation inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Check if exposure compensation is enabled.
    pub async fn get_exposure_compensation_enabled(&self) -> Result<bool, Error> {
        use crate::command::inquiry::ExposureCompensationModeInquiry;
        use crate::command::InquiryResponse;
        let response = self
            .send_inquiry_command(&ExposureCompensationModeInquiry)
            .await?;
        match response {
            InquiryResponse::ExposureCompensationMode { on } => Ok(on),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("ExposureCompensationMode inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current iris value.
    pub async fn get_iris(&self) -> Result<u8, Error> {
        use crate::command::inquiry::IrisInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&IrisInquiry).await?;
        match response {
            InquiryResponse::Iris { position } => Ok(position),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("Iris inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current shutter speed.
    pub async fn get_shutter(&self) -> Result<u16, Error> {
        use crate::command::inquiry::ShutterInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&ShutterInquiry).await?;
        match response {
            InquiryResponse::Shutter { position } => Ok(position),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("Shutter inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current gain value.
    pub async fn get_gain(&self) -> Result<u8, Error> {
        use crate::command::inquiry::GainInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&GainInquiry).await?;
        match response {
            InquiryResponse::GainLevel { gain } => Ok(gain),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("Gain inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the gain limit value.
    pub async fn get_gain_limit(&self) -> Result<u8, Error> {
        use crate::command::inquiry::GainLimitInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&GainLimitInquiry).await?;
        match response {
            InquiryResponse::GainLimit { limit } => Ok(limit),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("GainLimit inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the white balance mode.
    pub async fn get_white_balance_mode(
        &self,
    ) -> Result<crate::command::white_balance::WhiteBalanceMode, Error> {
        use crate::command::inquiry::WhiteBalanceModeInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&WhiteBalanceModeInquiry).await?;
        match response {
            InquiryResponse::WhiteBalanceMode { mode } => Ok(mode),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("WhiteBalanceMode inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current red gain.
    pub async fn get_red_gain(&self) -> Result<u8, Error> {
        use crate::command::inquiry::RedGainInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&RedGainInquiry).await?;
        match response {
            InquiryResponse::RedChannel { gain } => Ok(gain.max(0) as u8),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("RedGain inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current blue gain.
    pub async fn get_blue_gain(&self) -> Result<u8, Error> {
        use crate::command::inquiry::BlueGainInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&BlueGainInquiry).await?;
        match response {
            InquiryResponse::BlueChannel { gain } => Ok(gain.max(0) as u8),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("BlueGain inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current focus mode (Auto/Manual).
    pub async fn get_focus_mode(&self) -> Result<crate::command::FocusMode, Error> {
        use crate::command::inquiry::FocusModeInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&FocusModeInquiry).await?;
        match response {
            InquiryResponse::FocusMode { mode } => Ok(mode),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("FocusMode inquiry response"),
                actual: vec![],
            }),
        }
    }
}

// Movement helper methods for async mode (require ProfileMetadata)
#[cfg(feature = "async")]
impl<P, T> Camera<AsyncMode, P, T>
where
    P: Profile + crate::capabilities::ProfileMetadata,
    T: AsyncTransport + 'static,
{
    /// Wait for all movements to complete.
    ///
    /// This waits for pan/tilt, zoom, and focus movements to finish.
    pub async fn await_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error> {
        let config = super::MovementConfig::with_timeout(timeout.into());
        self.wait_for_movement_async(&config).await
    }

    /// Wait for pan/tilt movement to complete.
    ///
    /// Note: This now waits for all movements, not just pan/tilt.
    /// Use `await_idle` for clarity in new code.
    pub async fn await_pan_tilt_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error> {
        self.await_idle(timeout).await
    }

    /// Wait for zoom movement to complete.
    ///
    /// Note: This now waits for all movements, not just zoom.
    /// Use `await_idle` for clarity in new code.
    pub async fn await_zoom_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error> {
        self.await_idle(timeout).await
    }

    /// Wait for focus movement to complete.
    ///
    /// Note: This now waits for all movements, not just focus.
    /// Use `await_idle` for clarity in new code.
    pub async fn await_focus_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error> {
        self.await_idle(timeout).await
    }

    /// Move to a position and wait for completion.
    ///
    /// This sends an absolute pan/tilt command and waits for the movement to finish.
    #[allow(clippy::expect_used)]
    pub async fn move_to(
        &self,
        pan: crate::units::Degrees,
        tilt: crate::units::Degrees,
        timeout: impl Into<Duration>,
    ) -> Result<(), Error> {
        use crate::types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed};

        let pan_pos = PanPosition::from_degrees(pan.0)?;
        let tilt_pos = TiltPosition::from_degrees(tilt.0)?;
        let pan_speed = PanSpeed::new(18).expect("18 is valid speed"); // Fast speed
        let tilt_speed = TiltSpeed::new(18).expect("18 is valid speed"); // Fast speed
        self.pan_tilt_absolute(pan_pos, tilt_pos, pan_speed, tilt_speed)
            .await?;
        self.await_idle(timeout).await
    }

    /// Check if the camera is currently moving.
    ///
    /// Returns true if any axis (pan/tilt/zoom/focus) is in motion.
    pub async fn is_moving(&self) -> Result<bool, Error> {
        self.is_moving_async().await
    }
}

// Blocking mode implementation
impl<P, T> Camera<BlockingMode, P, T>
where
    P: Profile,
    T: BlockingTransport,
{
    /// Create a new blocking camera from a blocking transport.
    pub fn from_transport(transport: T) -> Self {
        let camera_id = CameraId::new(P::DEFAULT_ADDRESS).unwrap_or(CameraId::CAMERA_1);

        Self {
            envelope: TransportEnvelope::new(P::PROTOCOL_STYLE),
            transport: Arc::new(transport),
            camera_id,
            #[cfg(feature = "async")]
            socket_manager: Arc::new(Mutex::new(None)),
            #[cfg(feature = "async")]
            spawner: Arc::new(Mutex::new(None)),
            #[cfg(feature = "async")]
            runtime: Arc::new(Mutex::new(None)),
            timeout_config: TimeoutConfig::default(),
            _mode: PhantomData,
            _profile: PhantomData,
        }
    }

    /// Send a command synchronously and wait for response.
    pub fn send_command<C>(&self, command: &C) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Get command bytes using EncodeVisca
        let mut buffer = [0u8; 64];
        let size = command.encode_into(self.camera_id, &mut buffer)?;
        let cmd_bytes = &buffer[..size];

        // Add VISCA terminator if not present
        let mut cmd_vec = cmd_bytes.to_vec();
        if cmd_vec.last() != Some(&crate::command::const_encoding::VISCA_TERMINATOR) {
            cmd_vec.push(crate::command::const_encoding::VISCA_TERMINATOR);
        }

        // Apply protocol-specific framing using transport envelope
        let is_inquiry = command.response_type().is_some();
        let framed_bytes = self.envelope.frame_command(&cmd_vec, is_inquiry);

        log::debug!("Sending VISCA command (blocking): {:02X?}", framed_bytes);

        // Get the appropriate timeout for this command category
        let command_timeout = self.timeout_config.get_timeout(command.timeout_kind());

        // Send command using BlockingTransport
        self.transport.send_blocking(&framed_bytes)?;

        // Handle response based on command type following VISCA protocol spec
        match command.response_type() {
            None => {
                // Action command - wait for ACK then Completion
                let ack_bytes = self
                    .transport
                    .recv_blocking_with_timeout(self.timeout_config.ack_timeout)?;
                // Extract VISCA payload from envelope if needed
                let visca_bytes = self.envelope.extract_response(&ack_bytes)?;
                let ack = Response::parse(&visca_bytes)?;

                match ack {
                    Response::CmdAck => {
                        // ACK received, now wait for completion
                        let completion_bytes =
                            self.transport.recv_blocking_with_timeout(command_timeout)?;
                        // Extract VISCA payload from envelope if needed
                        let visca_bytes = self.envelope.extract_response(&completion_bytes)?;
                        Response::parse(&visca_bytes)
                    }
                    Response::Completion => {
                        // Some cameras send completion directly without ACK
                        Ok(Response::Completion)
                    }
                    Response::Error(e) => {
                        // Error response received instead of ACK
                        Err(e)
                    }
                    _ => Err(Error::ParseError(Cow::Owned(format!(
                        "Expected ACK or Completion, got: {:?}",
                        ack
                    )))),
                }
            }
            Some(_response_type) => {
                // Inquiry command - wait for specific response
                let response_bytes = self.transport.recv_blocking_with_timeout(command_timeout)?;
                // Extract VISCA payload from envelope if needed
                let visca_bytes = self.envelope.extract_response(&response_bytes)?;
                let response = Response::parse(&visca_bytes)?;

                // Verify we got the expected response type
                // For now, just return the response
                Ok(response)
            }
        }
    }

    /// Send an action command and wait for completion.
    pub fn send_action_command<C>(&self, command: &C) -> Result<(), Error>
    where
        C: EncodeVisca,
    {
        let response = self.send_command(command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Send an inquiry command and extract the typed result.
    pub fn send_inquiry_command<C>(
        &self,
        command: &C,
    ) -> Result<crate::command::InquiryResponse, Error>
    where
        C: EncodeVisca,
    {
        let response = self.send_command(command)?;
        match response {
            Response::Inquiry(data) => Ok(data),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Power methods for blocking mode

    /// Power on the camera.
    pub fn power_on(&self) -> Result<(), Error> {
        use crate::command::PowerCommand;
        let command = PowerCommand::On;
        self.send_action_command(&command)?;
        // Wait for camera to be ready
        std::thread::sleep(P::POWER_ON_TIME);
        Ok(())
    }

    /// Power off the camera.
    pub fn power_off(&self) -> Result<(), Error> {
        use crate::command::PowerCommand;
        let command = PowerCommand::Standby;
        self.send_action_command(&command)?;
        // Wait for standby/off
        std::thread::sleep(P::STANDBY_TIME);
        Ok(())
    }

    /// Inquiry: Get current power status.
    pub fn power_inquiry(&self) -> Result<bool, Error> {
        use crate::command::inquiry::PowerInquiry;
        let command = PowerInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::Power { on } => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Pan/Tilt methods

    /// Stop all pan/tilt movement.
    #[allow(clippy::expect_used)]
    pub fn pan_tilt_stop(&self) -> Result<(), Error> {
        use crate::command::pan_tilt::{PanTilt, PanTiltDirection};
        use crate::types::{PanSpeed, TiltSpeed};
        let command = PanTilt::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0).expect("0 is valid speed"),
            tilt_speed: TiltSpeed::new(0).expect("0 is valid speed"),
        };
        self.send_action_command(&command)
    }

    /// Move to home position (0, 0).
    pub fn pan_tilt_home(&self) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let command = PanTilt::Home;
        self.send_action_command(&command)
    }

    /// Reset pan/tilt mechanism.
    pub fn pan_tilt_reset(&self) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let command = PanTilt::Reset;
        self.send_action_command(&command)
    }

    /// Move to absolute pan/tilt position.
    pub fn pan_tilt_absolute(
        &self,
        pan: crate::types::PanPosition,
        tilt: crate::types::TiltPosition,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let command = PanTilt::AbsolutePosition {
            pan,
            tilt,
            pan_speed,
            tilt_speed,
        };
        self.send_action_command(&command)
    }

    /// Move relative to current position.
    pub fn pan_tilt_relative(
        &self,
        pan: crate::types::PanPosition,
        tilt: crate::types::TiltPosition,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let command = PanTilt::RelativePosition {
            pan,
            tilt,
            pan_speed,
            tilt_speed,
        };
        self.send_action_command(&command)
    }

    /// Move pan/tilt in a specific direction.
    pub fn pan_tilt_move(
        &self,
        direction: crate::command::pan_tilt::PanTiltDirection,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let command = PanTilt::Move {
            direction,
            pan_speed,
            tilt_speed,
        };
        self.send_action_command(&command)
    }

    /// Set pan/tilt movement limit for a specific corner.
    pub fn pan_tilt_limit_set(
        &self,
        corner: crate::command::pan_tilt::PanTiltLimitCorner,
        pan: crate::types::PanPosition,
        tilt: crate::types::TiltPosition,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let command = PanTilt::LimitSet { corner, pan, tilt };
        self.send_action_command(&command)
    }

    /// Clear pan/tilt movement limit for a specific corner.
    pub fn pan_tilt_limit_clear(
        &self,
        corner: crate::command::pan_tilt::PanTiltLimitCorner,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let command = PanTilt::LimitClear { corner };
        self.send_action_command(&command)
    }

    /// Inquiry: Get current pan/tilt position.
    pub fn pan_tilt_position_inquiry(
        &self,
    ) -> Result<(crate::types::PanPosition, crate::types::TiltPosition), Error> {
        use crate::command::inquiry::PanTiltPositionInquiry;
        let command = PanTiltPositionInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::PanTiltPosition { pan, tilt } => Ok((
                crate::types::PanPosition::try_from(pan)?,
                crate::types::TiltPosition::try_from(tilt)?,
            )),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Zoom methods

    /// Stop zoom movement.
    pub fn zoom_stop(&self) -> Result<(), Error> {
        use crate::command::zoom::Zoom;
        let command = Zoom::Stop;
        self.send_action_command(&command)
    }

    /// Zoom in at standard speed (telephoto).
    pub fn zoom_tele_std(&self) -> Result<(), Error> {
        use crate::command::zoom::Zoom;
        let command = Zoom::TeleStd;
        self.send_action_command(&command)
    }

    /// Zoom out at standard speed (wide).
    pub fn zoom_wide_std(&self) -> Result<(), Error> {
        use crate::command::zoom::Zoom;
        let command = Zoom::WideStd;
        self.send_action_command(&command)
    }

    /// Zoom in at variable speed.
    pub fn zoom_tele_variable(&self, speed: crate::command::zoom::ZoomSpeed) -> Result<(), Error> {
        use crate::command::zoom::Zoom;
        let command = Zoom::TeleVariable(speed);
        self.send_action_command(&command)
    }

    /// Zoom out at variable speed.
    pub fn zoom_wide_variable(&self, speed: crate::command::zoom::ZoomSpeed) -> Result<(), Error> {
        use crate::command::zoom::Zoom;
        let command = Zoom::WideVariable(speed);
        self.send_action_command(&command)
    }

    /// Set zoom to specific position.
    pub fn zoom_position(&self, position: crate::types::ZoomPosition) -> Result<(), Error> {
        use crate::command::zoom::Zoom;
        let command = Zoom::Position(position);
        self.send_action_command(&command)
    }

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    pub fn zoom_absolute(&self, position: crate::units::Normalized) -> Result<(), Error> {
        let position_value = position.0;

        // Validate position
        if !(0.0..=1.0).contains(&position_value) {
            return Err(Error::InvalidParameter {
                parameter: "zoom position",
                value: Cow::Owned(position_value.to_string()),
                reason: Cow::Borrowed("must be between 0.0 and 1.0"),
            });
        }

        // Convert normalized to raw zoom position (0x0000 to 0x4000)
        let raw = (position_value * 16384.0) as u16;
        let zoom_pos = crate::types::ZoomPosition::try_from(raw)?;
        self.zoom_position(zoom_pos)
    }

    /// Inquiry: Get current zoom position.
    pub fn zoom_position_inquiry(&self) -> Result<crate::types::ZoomPosition, Error> {
        use crate::command::inquiry::ZoomPositionInquiry;
        let command = ZoomPositionInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::ZoomPosition { position } => {
                crate::types::ZoomPosition::try_from(position)
            }
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current zoom position as a raw value.
    pub fn get_zoom_position(&self) -> Result<u16, Error> {
        let zoom_pos = self.zoom_position_inquiry()?;
        Ok(zoom_pos.into())
    }

    /// Get the current pan and tilt position in degrees.
    pub fn get_pan_tilt_degrees(
        &self,
    ) -> Result<(crate::units::Degrees, crate::units::Degrees), Error> {
        let (pan_pos, tilt_pos) = self.pan_tilt_position_inquiry()?;
        let (pan_deg, tilt_deg) = self.units_to_degrees(pan_pos.into(), tilt_pos.into());
        Ok((pan_deg, tilt_deg))
    }

    // Focus methods

    /// Stop any focus movement.
    pub fn focus_stop(&self) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::Stop;
        self.send_action_command(&command)
    }

    /// Move focus far at standard speed.
    pub fn focus_far(&self) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::Far;
        self.send_action_command(&command)
    }

    /// Move focus near at standard speed.
    pub fn focus_near(&self) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::Near;
        self.send_action_command(&command)
    }

    /// Move focus far at variable speed.
    pub fn focus_far_with_speed(
        &self,
        speed: crate::command::focus::FocusSpeed,
    ) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::FarWithSpeed(speed);
        self.send_action_command(&command)
    }

    /// Move focus near at variable speed.
    pub fn focus_near_with_speed(
        &self,
        speed: crate::command::focus::FocusSpeed,
    ) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::NearWithSpeed(speed);
        self.send_action_command(&command)
    }

    /// Set focus to specific position.
    pub fn focus_position(&self, position: crate::types::FocusPosition) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::Position(position);
        self.send_action_command(&command)
    }

    /// Enable auto focus mode.
    pub fn focus_auto(&self) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::Auto;
        self.send_action_command(&command)
    }

    /// Enable manual focus mode.
    pub fn focus_manual(&self) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::Manual;
        self.send_action_command(&command)
    }

    /// Trigger one-push auto focus (focus once then return to manual).
    pub fn focus_one_push(&self) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::OnePushTrigger;
        self.send_action_command(&command)
    }

    /// Set focus to infinity.
    pub fn focus_infinity(&self) -> Result<(), Error> {
        use crate::command::focus::Focus;
        let command = Focus::Infinity;
        self.send_action_command(&command)
    }

    /// Inquiry: Get current focus position.
    pub fn focus_position_inquiry(&self) -> Result<crate::types::FocusPosition, Error> {
        use crate::command::inquiry::FocusPositionInquiry;
        let command = FocusPositionInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::FocusPosition { position } => {
                crate::types::FocusPosition::try_from(position)
            }
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current focus mode (auto/manual).
    pub fn focus_mode_inquiry(&self) -> Result<crate::command::focus::FocusMode, Error> {
        use crate::command::inquiry::FocusModeInquiry;
        let command = FocusModeInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::FocusMode { mode } => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Additional inquiry helper methods

    /// Inquiry: Get camera version information.
    pub fn version_inquiry(&self) -> Result<crate::command::InquiryResponse, Error> {
        use crate::command::inquiry::VersionInquiry;
        let command = VersionInquiry;
        self.send_inquiry_command(&command)
    }

    /// Inquiry: Get current exposure mode.
    pub fn exposure_mode_inquiry(&self) -> Result<crate::command::exposure::ExposureMode, Error> {
        use crate::command::inquiry::ExposureModeInquiry;
        let command = ExposureModeInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::ExposureMode { mode } => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current white balance mode.
    pub fn white_balance_mode_inquiry(
        &self,
    ) -> Result<crate::command::white_balance::WhiteBalanceMode, Error> {
        use crate::command::inquiry::WhiteBalanceModeInquiry;
        let command = WhiteBalanceModeInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::WhiteBalanceMode { mode } => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current iris position.
    pub fn iris_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::IrisInquiry;
        let command = IrisInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::Iris { position } => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current gain value.
    pub fn gain_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::GainInquiry;
        let command = GainInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::GainLevel { gain } => Ok(gain),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current shutter speed.
    pub fn shutter_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::ShutterInquiry;
        let command = ShutterInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::Shutter { position } => Ok(position as u8),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current brightness level.
    pub fn brightness_inquiry(&self) -> Result<u16, Error> {
        use crate::command::inquiry::BrightInquiry;
        let command = BrightInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::Bright { position } => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get backlight compensation state.
    pub fn backlight_inquiry(&self) -> Result<bool, Error> {
        use crate::command::inquiry::BacklightInquiry;
        let command = BacklightInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::Backlight { status } => Ok(status),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get image flip settings.
    pub fn image_flip_inquiry(&self) -> Result<(bool, bool), Error> {
        use crate::command::inquiry::ImageFlipInquiry;
        let command = ImageFlipInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            } => Ok((horizontal, vertical)),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // NOTE: Sharpness inquiry command is not documented in VISCA specs
    // and has been disabled until proper documentation is found.
    // /// Inquiry: Get current sharpness level.
    // pub fn sharpness_inquiry(&self) -> Result<u8, Error> {
    //     use crate::command::inquiry::SharpnessInquiry;
    //     let command = SharpnessInquiry;
    //     let response = self.send_inquiry_command(&command)?;
    //     match response {
    //         crate::command::InquiryResponse::Sharpness { value } => Ok(value),
    //         _ => Err(Error::UnexpectedResponseType),
    //     }
    // }

    // NOTE: Contrast inquiry command is not documented in VISCA specs
    // and has been disabled until proper documentation is found.
    // /// Inquiry: Get current contrast level.
    // pub fn contrast_inquiry(&self) -> Result<u8, Error> {
    //     use crate::command::inquiry::ContrastInquiry;
    //     let command = ContrastInquiry;
    //     let response = self.send_inquiry_command(&command)?;
    //     match response {
    //         crate::command::InquiryResponse::Contrast(value) => Ok(value),
    //         _ => Err(Error::UnexpectedResponseType),
    //     }
    // }

    /// Inquiry: Get current saturation level.
    pub fn saturation_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::SaturationInquiry;
        let command = SaturationInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::Saturation { level } => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // NOTE: AutoFocus inquiry is not documented in VISCA specs
    // and has been disabled until proper documentation is found.
    // /// Inquiry: Get auto focus on/off state.
    // pub fn auto_focus_inquiry(&self) -> Result<bool, Error> {
    //     use crate::command::inquiry::AutoFocusInquiry;
    //     let command = AutoFocusInquiry;
    //     let response = self.send_inquiry_command(&command)?;
    //     match response {
    //         crate::command::InquiryResponse::AutoFocus { enabled } => Ok(enabled),
    //         _ => Err(Error::UnexpectedResponseType),
    //     }
    // }

    /// Inquiry: Get current color temperature.
    pub fn color_temperature_inquiry(&self) -> Result<u16, Error> {
        use crate::command::inquiry::ColorTemperatureInquiry;
        let command = ColorTemperatureInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::ColorTemperature { temperature } => Ok(temperature),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current hue adjustment.
    pub fn hue_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::HueInquiry;
        let command = HueInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::Hue { hue } => Ok(hue),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current gain limit.
    pub fn gain_limit_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::GainLimitInquiry;
        let command = GainLimitInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::GainLimit { limit } => Ok(limit),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get exposure compensation mode on/off.
    pub fn exposure_compensation_mode_inquiry(&self) -> Result<bool, Error> {
        use crate::command::inquiry::ExposureCompensationModeInquiry;
        let command = ExposureCompensationModeInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::ExposureCompensationMode { on } => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get exposure compensation value.
    pub fn exposure_compensation_inquiry(&self) -> Result<i8, Error> {
        use crate::command::inquiry::ExposureCompensationInquiry;
        let command = ExposureCompensationInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::ExposureCompensation { value } => Ok(value),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get 2D noise reduction level.
    pub fn noise_reduction_2d_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::NoiseReduction2DInquiry;
        let command = NoiseReduction2DInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::NoiseReduction2D { level } => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get 3D noise reduction level.
    pub fn noise_reduction_3d_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::NoiseReduction3DInquiry;
        let command = NoiseReduction3DInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::NoiseReduction3D { level } => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get focus near limit position.
    pub fn focus_near_limit_inquiry(&self) -> Result<u16, Error> {
        use crate::command::inquiry::FocusNearLimitInquiry;
        let command = FocusNearLimitInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::FocusNearLimit { position } => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Inquiry: Get current resolution mode.
    pub fn resolution_inquiry(&self) -> Result<u8, Error> {
        use crate::command::inquiry::ResolutionInquiry;
        let command = ResolutionInquiry;
        let response = self.send_inquiry_command(&command)?;
        match response {
            crate::command::InquiryResponse::Resolution(mode) => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Exposure methods

    /// Set exposure mode.
    pub fn set_exposure_mode(
        &self,
        mode: crate::command::exposure::ExposureMode,
    ) -> Result<(), Error> {
        use crate::command::exposure::ExposureCommand;
        let command = ExposureCommand { mode };
        self.send_command(&command).map(|_| ())
    }

    /// Set auto exposure mode.
    pub fn exposure_auto(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Auto)
    }

    /// Set manual exposure mode.
    pub fn exposure_manual(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Manual)
    }

    /// Set shutter priority exposure mode.
    pub fn exposure_shutter_priority(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Shutter)
    }

    /// Set iris priority exposure mode.
    pub fn exposure_iris_priority(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Iris)
    }

    /// Set brightness priority exposure mode.
    pub fn exposure_bright_mode(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Bright)
    }

    /// Set iris level.
    pub fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error> {
        use crate::command::exposure::Iris;
        let command = Iris::SetAperture(level);
        self.send_command(&command).map(|_| ())
    }

    /// Reset iris to default.
    pub fn reset_iris(&self) -> Result<(), Error> {
        use crate::command::exposure::Iris;
        let command = Iris::Reset;
        self.send_command(&command).map(|_| ())
    }

    /// Increase iris (open aperture).
    pub fn increase_iris(&self) -> Result<(), Error> {
        use crate::command::exposure::Iris;
        let command = Iris::Up;
        self.send_command(&command).map(|_| ())
    }

    /// Decrease iris (close aperture).
    pub fn decrease_iris(&self) -> Result<(), Error> {
        use crate::command::exposure::Iris;
        let command = Iris::Down;
        self.send_command(&command).map(|_| ())
    }

    /// Set brightness level.
    pub fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
        use crate::command::exposure::Bright;
        let command = Bright::SetLevel(level);
        self.send_command(&command).map(|_| ())
    }

    /// Reset brightness to default.
    pub fn reset_brightness(&self) -> Result<(), Error> {
        use crate::command::exposure::Bright;
        let command = Bright::Reset;
        self.send_command(&command).map(|_| ())
    }

    /// Increase brightness.
    pub fn increase_brightness(&self) -> Result<(), Error> {
        use crate::command::exposure::Bright;
        let command = Bright::Up;
        self.send_command(&command).map(|_| ())
    }

    /// Decrease brightness.
    pub fn decrease_brightness(&self) -> Result<(), Error> {
        use crate::command::exposure::Bright;
        let command = Bright::Down;
        self.send_command(&command).map(|_| ())
    }

    /// Set shutter speed.
    pub fn set_shutter_speed(&self, speed: crate::types::ShutterSpeed) -> Result<(), Error> {
        use crate::command::exposure::Shutter;
        let command = Shutter::SetSpeed(speed);
        self.send_command(&command).map(|_| ())
    }

    /// Reset shutter speed to default.
    pub fn reset_shutter_speed(&self) -> Result<(), Error> {
        use crate::command::exposure::Shutter;
        let command = Shutter::Reset;
        self.send_command(&command).map(|_| ())
    }

    /// Increase shutter speed (faster).
    pub fn increase_shutter_speed(&self) -> Result<(), Error> {
        use crate::command::exposure::Shutter;
        let command = Shutter::Up;
        self.send_command(&command).map(|_| ())
    }

    /// Decrease shutter speed (slower).
    pub fn decrease_shutter_speed(&self) -> Result<(), Error> {
        use crate::command::exposure::Shutter;
        let command = Shutter::Down;
        self.send_command(&command).map(|_| ())
    }

    /// Set gain value.
    pub fn set_gain(&self, gain: crate::types::GainLevel) -> Result<(), Error> {
        use crate::command::gain::Gain;
        let command = Gain::SetValue(gain);
        self.send_command(&command).map(|_| ())
    }

    /// Reset gain to default.
    pub fn reset_gain(&self) -> Result<(), Error> {
        use crate::command::gain::Gain;
        let command = Gain::Reset;
        self.send_command(&command).map(|_| ())
    }

    /// Increase gain.
    pub fn increase_gain(&self) -> Result<(), Error> {
        use crate::command::gain::Gain;
        let command = Gain::Up;
        self.send_command(&command).map(|_| ())
    }

    /// Decrease gain.
    pub fn decrease_gain(&self) -> Result<(), Error> {
        use crate::command::gain::Gain;
        let command = Gain::Down;
        self.send_command(&command).map(|_| ())
    }

    /// Set gain limit.
    pub fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error> {
        use crate::command::gain::GainLimitCommand;
        let command = GainLimitCommand::new(limit);
        self.send_command(&command).map(|_| ())
    }

    /// Set exposure compensation level.
    pub fn set_exposure_compensation_level(
        &self,
        level: crate::types::ExposureCompensationLevel,
    ) -> Result<(), Error> {
        use crate::command::exposure::ExposureCompensation;
        let command = ExposureCompensation::SetLevel(level);
        self.send_command(&command).map(|_| ())
    }

    /// Enable exposure compensation.
    pub fn enable_exposure_compensation(&self) -> Result<(), Error> {
        use crate::command::exposure::ExposureCompensation;
        let command = ExposureCompensation::On;
        self.send_command(&command).map(|_| ())
    }

    /// Disable exposure compensation.
    pub fn disable_exposure_compensation(&self) -> Result<(), Error> {
        use crate::command::exposure::ExposureCompensation;
        let command = ExposureCompensation::Off;
        self.send_command(&command).map(|_| ())
    }

    /// Reset exposure compensation.
    pub fn reset_exposure_compensation(&self) -> Result<(), Error> {
        use crate::command::exposure::ExposureCompensation;
        let command = ExposureCompensation::Reset;
        self.send_command(&command).map(|_| ())
    }

    /// Increase exposure compensation.
    pub fn increase_exposure_compensation(&self) -> Result<(), Error> {
        use crate::command::exposure::ExposureCompensation;
        let command = ExposureCompensation::Up;
        self.send_command(&command).map(|_| ())
    }

    /// Decrease exposure compensation.
    pub fn decrease_exposure_compensation(&self) -> Result<(), Error> {
        use crate::command::exposure::ExposureCompensation;
        let command = ExposureCompensation::Down;
        self.send_command(&command).map(|_| ())
    }

    /// Set backlight compensation.
    pub fn set_backlight(&self, enabled: bool) -> Result<(), Error> {
        use crate::command::image::BacklightCommand;
        let command = BacklightCommand::new(enabled);
        self.send_command(&command).map(|_| ())
    }

    /// Enable auto slow shutter.
    pub fn enable_auto_slow_shutter(&self) -> Result<(), Error> {
        use crate::command::exposure::AutoSlowShutter;
        let command = AutoSlowShutter::On;
        self.send_command(&command).map(|_| ())
    }

    /// Disable auto slow shutter.
    pub fn disable_auto_slow_shutter(&self) -> Result<(), Error> {
        use crate::command::exposure::AutoSlowShutter;
        let command = AutoSlowShutter::Off;
        self.send_command(&command).map(|_| ())
    }

    /// Enable spotlight mode.
    pub fn enable_spotlight(&self) -> Result<(), Error> {
        use crate::command::exposure::Spotlight;
        let command = Spotlight::On;
        self.send_command(&command).map(|_| ())
    }

    /// Disable spotlight mode.
    pub fn disable_spotlight(&self) -> Result<(), Error> {
        use crate::command::exposure::Spotlight;
        let command = Spotlight::Off;
        self.send_command(&command).map(|_| ())
    }

    /// Set brightness direct value.
    pub fn set_brightness_direct(&self, value: crate::types::BrightnessLevel) -> Result<(), Error> {
        use crate::command::exposure::Bright;
        let command = Bright::SetLevel(value);
        self.send_command(&command).map(|_| ())
    }

    /// Set color temperature.
    pub fn set_color_temperature(&self, temp: u16) -> Result<(), Error> {
        use crate::command::color::ColorTemperature;
        use crate::types::ColorTemp;
        let color_temp = ColorTemp::new(temp).map_err(|_| Error::InvalidParameter {
            parameter: "color_temperature",
            value: temp.to_string().into(),
            reason: "Invalid color temperature value".into(),
        })?;
        let command = ColorTemperature::SetTemperature(color_temp);
        self.send_command(&command).map(|_| ())
    }

    // White Balance Methods (Blocking)

    /// Set white balance mode to any supported mode.
    pub fn set_white_balance_mode(
        &self,
        mode: crate::command::white_balance::WhiteBalanceMode,
    ) -> Result<(), Error> {
        use crate::command::white_balance::WhiteBalanceCommand;
        let command = WhiteBalanceCommand { mode };
        self.send_command(&command).map(|_| ())
    }

    /// Set auto white balance mode.
    pub fn white_balance_auto(&self) -> Result<(), Error> {
        self.set_white_balance_mode(crate::command::white_balance::WhiteBalanceMode::Auto)
    }

    /// Set indoor white balance preset (optimized for incandescent/tungsten lighting).
    pub fn white_balance_indoor(&self) -> Result<(), Error> {
        self.set_white_balance_mode(crate::command::white_balance::WhiteBalanceMode::Indoor)
    }

    /// Set outdoor white balance preset (optimized for daylight).
    pub fn white_balance_outdoor(&self) -> Result<(), Error> {
        self.set_white_balance_mode(crate::command::white_balance::WhiteBalanceMode::Outdoor)
    }

    /// Set one-push white balance mode (calibrate once based on current scene).
    pub fn white_balance_one_push(&self) -> Result<(), Error> {
        self.set_white_balance_mode(crate::command::white_balance::WhiteBalanceMode::OnePush)
    }

    /// Set auto tracking white balance (Sony FR7 specific).
    pub fn white_balance_atw(&self) -> Result<(), Error> {
        self.set_white_balance_mode(crate::command::white_balance::WhiteBalanceMode::ATW)
    }

    /// Set manual white balance mode.
    pub fn white_balance_manual(&self) -> Result<(), Error> {
        self.set_white_balance_mode(crate::command::white_balance::WhiteBalanceMode::Manual)
    }

    /// Set color temperature white balance mode.
    pub fn white_balance_color_temperature(&self) -> Result<(), Error> {
        self.set_white_balance_mode(
            crate::command::white_balance::WhiteBalanceMode::ColorTemperature,
        )
    }

    /// Set AWB sensitivity level (PTZOptics specific).
    pub fn set_awb_sensitivity(
        &self,
        sensitivity: crate::command::white_balance::AutoWhiteBalanceSensitivity,
    ) -> Result<(), Error> {
        use crate::command::white_balance::AWBSensitivityCommand;
        let command = AWBSensitivityCommand { sensitivity };
        self.send_command(&command).map(|_| ())
    }

    /// Set dynamic range.
    pub fn set_dynamic_range(&self, _range: crate::types::DynamicRangeLevel) -> Result<(), Error> {
        // TODO: DynamicRangeCommand not yet implemented
        Err(Error::Unsupported)
    }

    /// Set the variable speed mode (24-step or 50-step).
    ///
    /// Only available on Sony FR7.
    pub fn set_variable_speed_mode(
        &self,
        mode: crate::command::VariableSpeedMode,
    ) -> Result<(), Error> {
        use crate::command::{Response, VariableSpeedModeCommand};
        let cmd = VariableSpeedModeCommand::new(mode);
        match self.send_command(&cmd)? {
            Response::CmdAck | Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // ========== Preset Methods ==========

    /// Store current position to a preset.
    pub fn preset_set(
        &self,
        preset_number: crate::command::preset::PresetNumber,
    ) -> Result<(), Error> {
        use crate::command::preset::{PresetAction, PresetCommand};
        let command = PresetCommand {
            action: PresetAction::Set,
            preset_number,
        };
        self.send_action_command(&command)
    }

    /// Recall a preset position.
    pub fn preset_recall(
        &self,
        preset_number: crate::command::preset::PresetNumber,
    ) -> Result<(), Error> {
        use crate::command::preset::{PresetAction, PresetCommand};
        let command = PresetCommand {
            action: PresetAction::Recall,
            preset_number,
        };
        self.send_action_command(&command)
    }

    /// Reset/clear a preset.
    pub fn preset_reset(
        &self,
        preset_number: crate::command::preset::PresetNumber,
    ) -> Result<(), Error> {
        use crate::command::preset::{PresetAction, PresetCommand};
        let command = PresetCommand {
            action: PresetAction::Reset,
            preset_number,
        };
        self.send_action_command(&command)
    }

    /// Enable image flip (vertical).
    pub fn enable_flip(&self) -> Result<(), Error> {
        use crate::command::flip::{Flip, ImageFlipCommand};
        let command = ImageFlipCommand::new(Flip::On);
        self.send_action_command(&command)
    }

    /// Disable image flip (vertical).
    pub fn disable_flip(&self) -> Result<(), Error> {
        use crate::command::flip::{Flip, ImageFlipCommand};
        let command = ImageFlipCommand::new(Flip::Off);
        self.send_action_command(&command)
    }

    /// Get image flip status.
    pub fn get_image_flip(&self) -> Result<crate::command::ImageFlipStatus, Error> {
        use crate::command::inquiry::ImageFlipInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&ImageFlipInquiry)?;
        match response {
            InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            } => Ok(crate::command::ImageFlipStatus {
                vertical,
                horizontal,
            }),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("ImageFlip inquiry response"),
                actual: vec![],
            }),
        }
    }

    // ============================================================================
    // Inquiry Methods - Complete Implementation (Blocking)
    // ============================================================================

    /// Get the current power state of the camera.
    /// Returns `true` if powered on, `false` if in standby.
    pub fn get_power_state(&self) -> Result<bool, Error> {
        use crate::command::inquiry::PowerInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&PowerInquiry)?;
        match response {
            InquiryResponse::Power { on } => Ok(on),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("Power inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current focus position.
    pub fn get_focus_position(&self) -> Result<u16, Error> {
        use crate::command::inquiry::FocusPositionInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&FocusPositionInquiry)?;
        match response {
            InquiryResponse::FocusPosition { position } => Ok(position),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("FocusPosition inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the focus near limit position.
    pub fn get_focus_near_limit(&self) -> Result<u16, Error> {
        use crate::command::inquiry::FocusNearLimitInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&FocusNearLimitInquiry)?;
        match response {
            InquiryResponse::FocusNearLimit { position } => Ok(position),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("FocusNearLimit inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current focus zone.
    pub fn get_focus_zone(&self) -> Result<crate::command::FocusZone, Error> {
        use crate::command::inquiry::FocusZoneInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&FocusZoneInquiry)?;
        match response {
            InquiryResponse::FocusZone { zone } => Ok(zone),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("FocusZone inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the auto-focus sensitivity setting.
    pub fn get_auto_focus_sensitivity(
        &self,
    ) -> Result<crate::command::AutoFocusSensitivity, Error> {
        use crate::command::inquiry::AutoFocusSensitivityInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&AutoFocusSensitivityInquiry)?;
        match response {
            InquiryResponse::AutoFocusSensitivity { sensitivity } => Ok(sensitivity),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("AutoFocusSensitivity inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current exposure mode.
    pub fn get_exposure_mode(&self) -> Result<crate::command::exposure::ExposureMode, Error> {
        use crate::command::inquiry::ExposureModeInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&ExposureModeInquiry)?;
        match response {
            InquiryResponse::ExposureMode { mode } => Ok(mode),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("ExposureMode inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the exposure compensation value.
    pub fn get_exposure_compensation(&self) -> Result<i8, Error> {
        use crate::command::inquiry::ExposureCompensationInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&ExposureCompensationInquiry)?;
        match response {
            InquiryResponse::ExposureCompensation { value } => Ok(value),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("ExposureCompensation inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Check if exposure compensation is enabled.
    pub fn get_exposure_compensation_enabled(&self) -> Result<bool, Error> {
        use crate::command::inquiry::ExposureCompensationModeInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&ExposureCompensationModeInquiry)?;
        match response {
            InquiryResponse::ExposureCompensationMode { on } => Ok(on),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("ExposureCompensationMode inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current iris value.
    pub fn get_iris(&self) -> Result<u8, Error> {
        use crate::command::inquiry::IrisInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&IrisInquiry)?;
        match response {
            InquiryResponse::Iris { position } => Ok(position),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("Iris inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current shutter speed.
    pub fn get_shutter(&self) -> Result<u16, Error> {
        use crate::command::inquiry::ShutterInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&ShutterInquiry)?;
        match response {
            InquiryResponse::Shutter { position } => Ok(position),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("Shutter inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current gain value.
    pub fn get_gain(&self) -> Result<u8, Error> {
        use crate::command::inquiry::GainInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&GainInquiry)?;
        match response {
            InquiryResponse::GainLevel { gain } => Ok(gain),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("Gain inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the gain limit value.
    pub fn get_gain_limit(&self) -> Result<u8, Error> {
        use crate::command::inquiry::GainLimitInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&GainLimitInquiry)?;
        match response {
            InquiryResponse::GainLimit { limit } => Ok(limit),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("GainLimit inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the white balance mode.
    pub fn get_white_balance_mode(
        &self,
    ) -> Result<crate::command::white_balance::WhiteBalanceMode, Error> {
        use crate::command::inquiry::WhiteBalanceModeInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&WhiteBalanceModeInquiry)?;
        match response {
            InquiryResponse::WhiteBalanceMode { mode } => Ok(mode),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("WhiteBalanceMode inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current red gain.
    pub fn get_red_gain(&self) -> Result<u8, Error> {
        use crate::command::inquiry::RedGainInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&RedGainInquiry)?;
        match response {
            InquiryResponse::RedChannel { gain } => Ok(gain.max(0) as u8),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("RedGain inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current blue gain.
    pub fn get_blue_gain(&self) -> Result<u8, Error> {
        use crate::command::inquiry::BlueGainInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&BlueGainInquiry)?;
        match response {
            InquiryResponse::BlueChannel { gain } => Ok(gain.max(0) as u8),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("BlueGain inquiry response"),
                actual: vec![],
            }),
        }
    }

    /// Get the current focus mode (Auto/Manual).
    pub fn get_focus_mode(&self) -> Result<crate::command::FocusMode, Error> {
        use crate::command::inquiry::FocusModeInquiry;
        use crate::command::InquiryResponse;
        let response = self.send_inquiry_command(&FocusModeInquiry)?;
        match response {
            InquiryResponse::FocusMode { mode } => Ok(mode),
            _ => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("FocusMode inquiry response"),
                actual: vec![],
            }),
        }
    }
}

// Movement helper methods for blocking mode (require ProfileMetadata)
impl<P, T> Camera<BlockingMode, P, T>
where
    P: Profile + crate::capabilities::ProfileMetadata,
    T: BlockingTransport,
{
    /// Wait for all movements to complete.
    ///
    /// This waits for pan/tilt, zoom, and focus movements to finish.
    pub fn await_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error> {
        let config = super::MovementConfig::with_timeout(timeout.into());
        self.wait_for_movement(&config)
    }

    /// Wait for pan/tilt movement to complete.
    ///
    /// Note: This now waits for all movements, not just pan/tilt.
    /// Use `await_idle` for clarity in new code.
    pub fn await_pan_tilt_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error> {
        self.await_idle(timeout)
    }

    /// Wait for zoom movement to complete.
    ///
    /// Note: This now waits for all movements, not just zoom.
    /// Use `await_idle` for clarity in new code.
    pub fn await_zoom_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error> {
        self.await_idle(timeout)
    }

    /// Wait for focus movement to complete.
    ///
    /// Note: This now waits for all movements, not just focus.
    /// Use `await_idle` for clarity in new code.
    pub fn await_focus_idle(&self, timeout: impl Into<Duration>) -> Result<(), Error> {
        self.await_idle(timeout)
    }

    /// Move to a position and wait for completion.
    ///
    /// This sends an absolute pan/tilt command and waits for the movement to finish.
    #[allow(clippy::expect_used)]
    pub fn move_to(
        &self,
        pan: crate::units::Degrees,
        tilt: crate::units::Degrees,
        timeout: impl Into<Duration>,
    ) -> Result<(), Error> {
        use crate::types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed};

        let pan_pos = PanPosition::from_degrees(pan.0)?;
        let tilt_pos = TiltPosition::from_degrees(tilt.0)?;
        let pan_speed = PanSpeed::new(18).expect("18 is valid speed"); // Fast speed
        let tilt_speed = TiltSpeed::new(18).expect("18 is valid speed"); // Fast speed
        self.pan_tilt_absolute(pan_pos, tilt_pos, pan_speed, tilt_speed)?;
        self.await_idle(timeout)
    }

    // Note: is_moving is already defined as an inherent method in movement_detection.rs
}

// Legacy impl block removed - use CameraAsync::from_transport() or CameraBlocking::from_transport() instead
