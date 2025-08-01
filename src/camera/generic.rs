//! Generic camera implementation using compile-time profiles.
//!
//! This module provides the new generic Camera<P, T> struct that uses
//! compile-time profile selection for zero-cost abstractions.

use std::borrow::Cow;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use crate::{
    camera_id::CameraId,
    capabilities::Profile,
    command::{encode_visca::EncodeVisca, Response, ResponseType},
    error::Error,
    transport::{TransportEnvelope, UnifiedTransport},
};

#[cfg(feature = "async")]
use crate::socket_manager::SocketManagerHandle;

#[cfg(feature = "async")]
use crate::executor::Spawner;

#[cfg(feature = "async")]
use crate::runtime::RuntimeSpawner;

/// Generic camera client with compile-time profile selection.
///
/// This struct provides type-safe camera control with zero runtime overhead.
/// All profile-specific constants and behaviors are resolved at compile time.
///
/// # Type Parameters
///
/// * `P` - Camera profile implementing the `Profile` trait
/// * `T` - Transport implementing `UnifiedTransport`
///
/// # Examples
///
/// ```ignore
/// use grafton_visca::camera::generic::Camera;
/// use grafton_visca::camera::profiles::PTZOpticsG2;
/// use grafton_visca::transport::blocking::Tcp;
///
/// // Create a camera with explicit profile type
/// let transport = Tcp::connect("192.168.1.100:52381")?;
/// let camera = Camera::<PTZOpticsG2, _>::new(transport);
///
/// // Or use the type alias
/// use grafton_visca::prelude::PTZOpticsG2Cam;
/// let camera = PTZOpticsG2Cam::new(transport);
/// ```
pub struct Camera<P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    transport: Arc<T>,
    camera_id: CameraId,
    #[cfg(feature = "async")]
    socket_manager: Option<SocketManagerHandle>,
    envelope: TransportEnvelope,
    #[cfg(feature = "async")]
    spawner: Option<Arc<dyn Spawner>>,
    _profile: PhantomData<P>,
}

impl<P, T> Clone for Camera<P, T>
where
    P: Profile,
    T: UnifiedTransport,
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
            _profile: PhantomData,
        }
    }
}

impl<P, T> std::fmt::Debug for Camera<P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("Camera");
        debug
            .field("profile", &P::MODEL_NAME)
            .field("camera_id", &self.camera_id)
            .field("transport", &"<UnifiedTransport>");
        #[cfg(feature = "async")]
        debug.field("socket_manager", &self.socket_manager.is_some());
        debug.finish()
    }
}

impl<P, T> Camera<P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    /// Create a new camera from a unified transport.
    pub fn from_transport(transport: T) -> Self {
        let camera_id = CameraId::new(P::DEFAULT_ADDRESS).unwrap_or(CameraId::CAMERA_1);

        Self {
            envelope: TransportEnvelope::new(P::PROTOCOL_STYLE),
            transport: Arc::new(transport),
            camera_id,
            #[cfg(feature = "async")]
            socket_manager: None,
            #[cfg(feature = "async")]
            spawner: None,
            _profile: PhantomData,
        }
    }

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

    /// Send a command asynchronously and wait for response.
    #[cfg(feature = "async")]
    pub async fn send_command<C>(&self, command: &C) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Check if socket manager is available
        if let Some(socket_manager) = &self.socket_manager {
            return self
                .send_command_via_socket_manager(command, socket_manager)
                .await;
        }

        // Fall back to direct transport (legacy behavior)
        self.send_command_direct(command).await
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
        let mut buffer = [0u8; 64]; // Use a reasonable max size
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

        // Send via socket manager
        socket_manager
            .send_command(framed_bytes, category, is_inquiry)
            .await
    }

    /// Send command directly via transport (legacy approach)
    #[cfg(feature = "async")]
    async fn send_command_direct<C>(&self, command: &C) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Get command bytes using EncodeVisca
        let mut buffer = [0u8; 64]; // Use a reasonable max size
        let size = command.encode_into(self.camera_id, &mut buffer)?;
        let mut cmd_bytes = buffer[..size].to_vec();

        // Add VISCA terminator if not present
        if cmd_bytes.last() != Some(&crate::command::const_encoding::VISCA_TERMINATOR) {
            cmd_bytes.push(crate::command::const_encoding::VISCA_TERMINATOR);
        }

        // Apply protocol-specific framing using transport envelope
        let is_inquiry = command.response_type().is_some();
        let framed_bytes = self.envelope.frame_command(&cmd_bytes, is_inquiry);

        log::debug!("Sending VISCA command: {framed_bytes:02X?}");

        // Send command
        self.transport.send(&framed_bytes).await?;

        // Handle response based on command type
        match command.response_type() {
            None => {
                // Action command - wait for ACK then Completion
                let ack_response = self.wait_for_response(P::ACK_TIMEOUT).await?;
                match ack_response {
                    Response::CmdAck => {
                        // Wait for completion
                        self.wait_for_response(P::COMPLETION_TIMEOUT).await
                    }
                    Response::Completion => {
                        // Some cameras send completion directly
                        Ok(Response::Completion)
                    }
                    Response::Error(e) => Err(e),
                    _ => Err(Error::ParseError(Cow::Owned(format!(
                        "Unexpected response: {ack_response:?}"
                    )))),
                }
            }
            Some(response_type) => {
                // Inquiry command - wait for specific response type
                let timeout = P::COMPLETION_TIMEOUT;
                self.wait_for_response_with_type(response_type, timeout)
                    .await
            }
        }
    }

    /// Wait for any response with timeout.
    #[cfg(feature = "async")]
    async fn wait_for_response(&self, _timeout: Duration) -> Result<Response, Error> {
        // TODO: Implement proper timeout handling based on runtime
        // For now, just receive without timeout

        match self.transport.recv().await {
            Ok(bytes) => {
                // Extract VISCA payload from envelope if needed
                let visca_bytes = self.envelope.extract_response(&bytes)?;
                Response::parse(&visca_bytes)
            }
            Err(e) => {
                // Preserve the original error type
                if e.to_string().contains("Operation timed out") {
                    Err(Error::Timeout)
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Wait for a specific type of response with timeout.
    #[cfg(feature = "async")]
    async fn wait_for_response_with_type(
        &self,
        expected_type: ResponseType,
        #[allow(unused_variables)] timeout: Duration,
    ) -> Result<Response, Error> {
        // TODO: Implement timeout using runtime-specific timeout mechanisms
        // Currently, timeout is not implemented as it requires runtime-specific code
        // For inquiry commands, we may receive an ACK first, then the inquiry response
        loop {
            match self.transport.recv().await {
                Ok(bytes) => {
                    // Extract VISCA payload from envelope if needed
                    let visca_bytes = match self.envelope.extract_response(&bytes) {
                        Ok(payload) => payload,
                        Err(e) => return Err(e),
                    };

                    // First try to parse as a regular response
                    match Response::parse(&visca_bytes) {
                        Ok(Response::CmdAck) => {
                            // Skip ACK for inquiry commands and wait for the actual response
                            log::debug!("Skipping ACK response for inquiry command");
                            continue;
                        }
                        Ok(Response::Error(e)) => return Err(e),
                        Ok(Response::Completion) => {
                            // Unexpected completion for inquiry
                            return Err(Error::UnexpectedResponseType);
                        }
                        Ok(other) => {
                            // This shouldn't happen with parse() but handle it
                            return Ok(other);
                        }
                        Err(_) => {
                            // If regular parse fails, it might be an inquiry response
                            // Try parsing with the expected type
                            match Response::parse_with_type(&visca_bytes, &expected_type) {
                                Ok(response) => return Ok(response),
                                Err(e) => return Err(e),
                            }
                        }
                    }
                }
                Err(e) => {
                    // Preserve the original error type
                    if e.to_string().contains("Operation timed out") {
                        return Err(Error::Timeout);
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }

    /// Send a command synchronously (blocking).
    pub(crate) fn send_command_blocking<C>(&self, command: &C) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        #[cfg(feature = "async")]
        {
            // Use a minimal executor to block on the async method
            futures::executor::block_on(self.send_command(command))
        }

        #[cfg(not(feature = "async"))]
        {
            // Direct blocking implementation
            self.send_command_blocking_direct(command)
        }
    }

    /// Direct blocking implementation without async
    #[cfg(not(feature = "async"))]
    fn send_command_blocking_direct<C>(&self, command: &C) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Get command bytes using EncodeVisca
        let mut buffer = [0u8; 64]; // Use a reasonable max size
        let size = command.encode_into(self.camera_id, &mut buffer)?;
        let mut cmd_bytes = buffer[..size].to_vec();

        // Add VISCA terminator if not present
        if cmd_bytes.last() != Some(&crate::command::const_encoding::VISCA_TERMINATOR) {
            cmd_bytes.push(crate::command::const_encoding::VISCA_TERMINATOR);
        }

        // Apply protocol-specific framing using transport envelope
        let is_inquiry = command.response_type().is_some();
        let framed_bytes = self.envelope.frame_command(&cmd_bytes, is_inquiry);

        log::debug!("Sending VISCA command: {framed_bytes:02X?}");

        // Send command using blocking transport
        self.transport.send_blocking(&framed_bytes)?;

        // Handle response based on command type
        match command.response_type() {
            None => {
                // Action command - wait for ACK then Completion
                let ack_response = self.wait_for_response_blocking(P::ACK_TIMEOUT)?;
                match ack_response {
                    Response::CmdAck => {
                        // Wait for completion
                        self.wait_for_response_blocking(P::COMPLETION_TIMEOUT)
                    }
                    Response::Completion => {
                        // Some cameras send completion directly
                        Ok(Response::Completion)
                    }
                    Response::Error(e) => Err(e),
                    _ => Err(Error::ParseError(Cow::Owned(format!(
                        "Unexpected response: {ack_response:?}"
                    )))),
                }
            }
            Some(expected_type) => {
                // Inquiry command - wait for specific response with type
                self.wait_for_response_with_type_blocking(expected_type, P::COMPLETION_TIMEOUT)
            }
        }
    }

    /// Wait for a response with timeout (blocking version)
    #[cfg(not(feature = "async"))]
    fn wait_for_response_blocking(&self, timeout: Duration) -> Result<Response, Error> {
        let start = std::time::Instant::now();
        loop {
            // Try to receive a response
            match self.transport.recv_blocking_timeout(timeout) {
                Ok(bytes) => {
                    log::debug!("Received response: {bytes:02X?}");

                    // Deframe the response
                    let response_bytes = self.envelope.extract_response(&bytes)?;

                    // Parse using the generic Response parser
                    match Response::parse(&response_bytes) {
                        Ok(response) => return Ok(response),
                        Err(e) => {
                            log::warn!("Failed to parse response: {e:?}");
                            // Continue waiting for a valid response
                        }
                    }
                }
                Err(Error::CommandTimeout { .. }) => {
                    if start.elapsed() >= timeout {
                        return Err(Error::CommandTimeout {
                            duration: timeout,
                            command: Cow::Borrowed("wait_for_response_blocking"),
                        });
                    }
                    // Continue waiting
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Wait for a specific type of response with timeout (blocking version).
    #[cfg(not(feature = "async"))]
    fn wait_for_response_with_type_blocking(
        &self,
        expected_type: ResponseType,
        timeout: Duration,
    ) -> Result<Response, Error> {
        let start = std::time::Instant::now();
        loop {
            // Try to receive a response
            match self.transport.recv_blocking_timeout(timeout) {
                Ok(bytes) => {
                    log::debug!("Received response: {bytes:02X?}");

                    // Deframe the response
                    let response_bytes = self.envelope.extract_response(&bytes)?;

                    // First try to parse as a regular response
                    match Response::parse(&response_bytes) {
                        Ok(Response::CmdAck) => {
                            // Skip ACK for inquiry commands and wait for the actual response
                            log::debug!("Skipping ACK response for inquiry command");
                            continue;
                        }
                        Ok(Response::Error(e)) => return Err(e),
                        Ok(Response::Completion) => {
                            // Unexpected completion for inquiry
                            return Err(Error::UnexpectedResponseType);
                        }
                        Ok(other) => {
                            // This shouldn't happen with parse() but handle it
                            return Ok(other);
                        }
                        Err(_) => {
                            // If regular parse fails, it might be an inquiry response
                            // Try parsing with the expected type
                            match Response::parse_with_type(&response_bytes, &expected_type) {
                                Ok(response) => return Ok(response),
                                Err(e) => return Err(e),
                            }
                        }
                    }
                }
                Err(Error::CommandTimeout { .. }) => {
                    if start.elapsed() >= timeout {
                        return Err(Error::CommandTimeout {
                            duration: timeout,
                            command: Cow::Borrowed("wait_for_response_with_type_blocking"),
                        });
                    }
                    // If we haven't exceeded our timeout, continue waiting
                }
                Err(e) => return Err(e),
            }
        }
    }
}

// Connection factory methods for specific transport types
impl<P> Camera<P, crate::transport::blocking::Tcp>
where
    P: Profile,
{
    /// Connect to a camera via blocking TCP.
    ///
    /// # Examples
    /// ```no_run
    /// # use grafton_visca::camera::{Camera, profiles::PTZOpticsG2};
    /// # use grafton_visca::Result;
    /// # fn example() -> Result<()> {
    /// let camera = Camera::<PTZOpticsG2, _>::connect_tcp("192.168.1.100:52381")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn connect_tcp(addr: &str) -> Result<Self, Error> {
        let transport = crate::transport::blocking::Tcp::connect(addr)?;
        Ok(Self::from_transport(transport))
    }
}

impl<P> Camera<P, crate::transport::blocking::Udp>
where
    P: Profile,
{
    /// Connect to a camera via blocking UDP.
    ///
    /// # Examples
    /// ```no_run
    /// # use grafton_visca::camera::{Camera, profiles::PTZOpticsG2};
    /// # use grafton_visca::Result;
    /// # fn example() -> Result<()> {
    /// let camera = Camera::<PTZOpticsG2, _>::connect_udp("192.168.1.100:52381")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn connect_udp(addr: &str) -> Result<Self, Error> {
        let transport = crate::transport::blocking::Udp::connect(addr)?;
        Ok(Self::from_transport(transport))
    }
}

#[cfg(feature = "tokio")]
impl<P> Camera<P, crate::transport::tokio::Tcp>
where
    P: Profile,
{
    /// Connect to a camera via async TCP (tokio).
    ///
    /// # Examples
    /// ```no_run
    /// # use grafton_visca::camera::{Camera, profiles::PTZOpticsG2};
    /// # use grafton_visca::Result;
    /// # #[tokio::main]
    /// # async fn example() -> Result<()> {
    /// let camera = Camera::<PTZOpticsG2, _>::connect_tokio_tcp("192.168.1.100:52381").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect_tokio_tcp(addr: &str) -> Result<Self, Error> {
        let transport = crate::transport::tokio::Tcp::connect(addr).await?;
        Ok(Self::from_transport(transport))
    }
}

#[cfg(feature = "tokio")]
impl<P> Camera<P, crate::transport::tokio::Udp>
where
    P: Profile,
{
    /// Connect to a camera via async UDP (tokio).
    ///
    /// # Examples
    /// ```no_run
    /// # use grafton_visca::camera::{Camera, profiles::PTZOpticsG2};
    /// # use grafton_visca::Result;
    /// # #[tokio::main]
    /// # async fn example() -> Result<()> {
    /// let camera = Camera::<PTZOpticsG2, _>::connect_tokio_udp("192.168.1.100:52381").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect_tokio_udp(addr: &str) -> Result<Self, Error> {
        let transport = crate::transport::tokio::Udp::connect(addr).await?;
        Ok(Self::from_transport(transport))
    }
}

// Generic constructor for custom transports
impl<P, T> Camera<P, T>
where
    P: Profile,
    T: UnifiedTransport + 'static,
{
    /// Create a new camera with a custom transport.
    ///
    /// For standard TCP/UDP transports, prefer using the connection factory methods:
    /// - `connect_tcp()` for blocking TCP
    /// - `connect_udp()` for blocking UDP
    /// - `connect_tokio_tcp()` for async TCP with tokio
    /// - `connect_tokio_udp()` for async UDP with tokio
    pub fn new(transport: T) -> Self {
        Self::from_transport(transport)
    }

    /// Set a spawner for async operations.
    #[cfg(feature = "async")]
    pub fn with_spawner<S>(mut self, spawner: S) -> Self
    where
        S: Spawner,
    {
        self.spawner = Some(Arc::new(spawner));

        // Initialize socket manager automatically for better reliability
        if let Err(e) = self.initialize_socket_manager() {
            log::warn!("Failed to initialize socket manager: {e}");
        }

        self
    }

    /// Set a runtime for async operations.
    #[cfg(feature = "async")]
    pub fn with_runtime<R>(mut self, runtime: Arc<R>) -> Self
    where
        R: crate::runtime::Runtime,
    {
        let runtime_dyn: crate::runtime::SharedRuntime = runtime;
        self.spawner = Some(Arc::new(RuntimeSpawner::new(runtime_dyn)));

        // Initialize socket manager automatically for better reliability
        if let Err(e) = self.initialize_socket_manager() {
            log::warn!("Failed to initialize socket manager: {e}");
        }

        self
    }

    /// Initialize the socket manager for this camera.
    pub fn initialize_socket_manager(&mut self) -> Result<(), Error> {
        #[cfg(feature = "async")]
        if self.socket_manager.is_some() {
            return Ok(()); // Already initialized
        }

        // Socket manager is only available in async builds
        #[cfg(feature = "async")]
        {
            // Create socket manager components
            let (command_sender, command_receiver) = crate::channels::unbounded();

            // Store the handle
            let handle = SocketManagerHandle::new(command_sender);
            self.socket_manager = Some(handle);

            // Start the socket manager actor
            let transport = Arc::clone(&self.transport);
            let actor = crate::socket_manager::SocketManagerActor::new(
                transport,
                command_receiver,
                P::ACK_TIMEOUT,
                P::COMPLETION_TIMEOUT,
                self.camera_id,
            );

            if let Some(spawner) = &self.spawner {
                // Use the provided spawner
                let future = Box::pin(async move {
                    if let Err(e) = actor.run().await {
                        log::error!("Socket manager actor failed: {e}");
                    }
                });
                spawner.spawn(future);
            } else {
                // No spawner provided - this is expected when using standard constructors
                log::error!("Cannot initialize socket manager without a spawner");
                return Err(Error::InvalidState(
                    Cow::Borrowed("Socket manager requires a spawner. Use Camera::new().with_spawner() to provide one."),
                ));
            }
        }

        // For blocking-only builds, we cannot use the socket manager
        #[cfg(not(feature = "async"))]
        {
            log::warn!("Socket manager not available in blocking-only builds");
            // Don't return an error, just don't initialize the socket manager
        }

        Ok(())
    }
}
