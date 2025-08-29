//! Unified camera implementation with separate blocking and async types.
//!
//! This module provides two concrete camera types that remove invalid states
//! and phantom generics by using dedicated types for blocking vs async modes.

#[cfg(feature = "async")]
use std::sync::Arc;

use crate::{
    camera_id::CameraId,
    capabilities::Profile,
    command::{response::ViscaResponse, typed::ViscaCommand, ViscaEncode},
    error::Error,
    timeout::TimeoutConfig,
};
#[cfg(feature = "async")]
use crate::{
    command::bytes::VISCA_TERMINATOR,
    executor::Executor,
    runtime,
    transport::{
        buffer::{BufferConfig, BufferManager},
        envelope::TransportEnvelope,
        AsyncTransport,
    },
};
#[cfg(not(feature = "async"))]
use crate::{
    command::bytes::VISCA_TERMINATOR,
    transport::{
        buffer::{BufferConfig, BufferManager},
        envelope::TransportEnvelope,
        SyncTransport,
    },
};

/// Blocking camera client with compile-time profile selection.
///
/// This struct provides type-safe camera control for blocking I/O.
/// All profile-specific constants and behaviors are resolved at compile time.
///
/// # Type Parameters
///
/// * `P` - Camera profile implementing the `Profile` trait
/// * `Tr` - Blocking transport implementing the `SyncTransport` trait
///
/// # Examples
///
/// ```ignore
/// use grafton_visca::{BlockingCamera, camera::profiles::PtzOpticsG2};
/// use grafton_visca::net::blocking::TransportBuilder;
///
/// let transport = TransportBuilder::tcp("192.168.0.110:5678").connect()?;
/// let mut camera = BlockingCamera::<PtzOpticsG2, _>::new(transport);
/// camera.power_on()?;
/// ```
#[cfg(not(feature = "async"))]
#[derive(Debug)]
pub struct BlockingCamera<P, Tr>
where
    P: Profile,
    Tr: SyncTransport,
{
    camera_id: CameraId,
    envelope: TransportEnvelope,
    envelope_buffer_manager: BufferManager,
    timeout_config: TimeoutConfig,
    transport: Tr,
    _phantom_profile: core::marker::PhantomData<P>,
}

/// Async camera client with compile-time profile selection.
///
/// This struct provides type-safe camera control for async I/O.
/// All profile-specific constants and behaviors are resolved at compile time.
///
/// # Type Parameters
///
/// * `P` - Camera profile implementing the `Profile` trait
/// * `Tr` - Async transport implementing the `AsyncTransport` trait  
/// * `Exec` - Executor implementing the `Executor` trait
///
/// # Examples
///
/// ```ignore
/// use grafton_visca::{AsyncCamera, camera::profiles::PtzOpticsG2};
/// use grafton_visca::net::r#async::TransportBuilder;
/// use grafton_visca::TokioExecutor;
///
/// let transport = TransportBuilder::tcp("192.168.0.110:52381").connect_dyn().await?;
/// let executor = TokioExecutor::from_current()?;
/// let camera = AsyncCamera::<PtzOpticsG2, _, _>::with_executor(transport, executor).await?;
/// camera.power_on().await?;
/// ```
#[cfg(feature = "async")]
#[derive(Debug)]
pub struct AsyncCamera<P, Tr, Exec>
where
    P: Profile,
    Tr: AsyncTransport,
    Exec: Executor,
{
    camera_id: CameraId,
    timeout_config: TimeoutConfig,
    runtime_handle: Arc<runtime::RuntimeHandle>,
    executor: Arc<Exec>,
    envelope: TransportEnvelope,
    envelope_buffer_manager: BufferManager,
    _phantom_profile: core::marker::PhantomData<P>,
    _phantom_transport: core::marker::PhantomData<Tr>,
}

// Forward the generic Camera interface to the concrete implementations
// This provides backward compatibility for trait implementations
#[cfg(not(feature = "async"))]
pub use BlockingCamera as Camera;

#[cfg(feature = "async")]
pub use AsyncCamera as Camera;

// New BlockingCamera implementations
#[cfg(not(feature = "async"))]
impl<P, Tr> BlockingCamera<P, Tr>
where
    P: Profile + Default,
    Tr: SyncTransport,
{
    /// Create a new blocking camera with the specified transport.
    pub fn new(transport: Tr) -> Self {
        Self {
            camera_id: CameraId::default(),
            envelope: TransportEnvelope::new(P::PROTOCOL_STYLE),
            envelope_buffer_manager: BufferManager::new(BufferConfig::default()),
            timeout_config: TimeoutConfig::default(),
            transport,
            _phantom_profile: core::marker::PhantomData,
        }
    }

    /// Create a new blocking camera with the specified transport.
    ///
    /// Alias for `new()` for consistency with the `from_transport` naming pattern.
    pub fn from_transport(transport: Tr) -> Self {
        Self::new(transport)
    }

    /// Set the camera ID for VISCA addressing.
    pub fn with_camera_id(mut self, camera_id: CameraId) -> Self {
        self.camera_id = camera_id;
        self
    }

    /// Set the timeout configuration.
    pub fn with_timeout_config(mut self, timeout_config: TimeoutConfig) -> Self {
        self.timeout_config = timeout_config;
        self
    }

    /// Get the camera ID.
    pub fn camera_id(&self) -> CameraId {
        self.camera_id
    }

    /// Get a reference to the timeout configuration.
    pub fn timeout_config(&self) -> &TimeoutConfig {
        &self.timeout_config
    }

    /// Set the camera ID (mutable method for builder compatibility).
    pub(crate) fn set_camera_id(&mut self, camera_id: CameraId) {
        self.camera_id = camera_id;
    }

    /// Set the timeout configuration (mutable method for builder compatibility).
    pub(crate) fn set_timeout_config(&mut self, timeout_config: TimeoutConfig) {
        self.timeout_config = timeout_config;
    }

    /// Send a command with typed response parsing.
    pub(crate) fn send_command_typed<C>(&mut self, command: &C) -> Result<C::Response, Error>
    where
        C: ViscaEncode + ViscaCommand,
    {
        let response = self.send_command(command)?;
        C::from_response(response)
    }

    /// Send a command to the camera and wait for a response (blocking).
    ///
    /// Note: This method requires `&mut self` because the underlying transport
    /// requires mutable access for sending and receiving.
    pub(crate) fn send_command<C>(&mut self, command: &C) -> Result<ViscaResponse, Error>
    where
        C: ViscaEncode,
    {
        // Encode command bytes using ViscaEncode
        let mut buffer = [0u8; 64];
        let size = command.encode_into(self.camera_id, &mut buffer)?;
        let cmd_bytes = &buffer[..size];

        // Add VISCA terminator if not present
        let mut cmd_vec = cmd_bytes.to_vec();
        if !cmd_vec.ends_with(&[VISCA_TERMINATOR]) {
            cmd_vec.push(VISCA_TERMINATOR);
        }

        // Determine if this is an inquiry based on the typed response
        let is_inquiry = command.response_type().is_some();

        // Apply envelope and send command
        let request =
            self.envelope
                .frame_command(&cmd_vec, is_inquiry, &self.envelope_buffer_manager);

        // Send command
        self.transport.send(&request)?;

        // For non-inquiry commands, we need to handle ACK/Completion sequence
        if !is_inquiry {
            // Read first response (should be ACK or error) with ACK timeout
            let first_response_bytes = self
                .transport
                .recv_with_timeout(self.timeout_config.ack_timeout)?;
            let first_visca = self.envelope.extract_response(&first_response_bytes)?;
            let first_response = ViscaResponse::parse(&first_visca)?;

            match first_response {
                ViscaResponse::Error(e) => Err(e),
                ViscaResponse::CmdAck => {
                    // Got ACK, now wait for completion
                    // Use per-category timeout for completion
                    let completion_timeout = self.timeout_config.get_timeout(C::TIMEOUT_CATEGORY);
                    let second_response_bytes =
                        self.transport.recv_with_timeout(completion_timeout)?;
                    let second_visca = self.envelope.extract_response(&second_response_bytes)?;
                    let second_response = ViscaResponse::parse(&second_visca)?;

                    match second_response {
                        ViscaResponse::Error(e) => Err(e),
                        _ => Ok(second_response),
                    }
                }
                // If first response is already completion (some cameras skip ACK)
                ViscaResponse::Completion => Ok(first_response),
                _ => Ok(first_response),
            }
        } else {
            // For inquiry commands, just read one response with quick timeout
            let quick_timeout = self.timeout_config.get_timeout(C::TIMEOUT_CATEGORY);
            let response_bytes = self.transport.recv_with_timeout(quick_timeout)?;
            let visca = self.envelope.extract_response(&response_bytes)?;
            ViscaResponse::parse(&visca)
        }
    }
}

// New AsyncCamera implementations
#[cfg(feature = "async")]
impl<P, Tr, Exec> AsyncCamera<P, Tr, Exec>
where
    P: Profile + Default,
    Tr: AsyncTransport + Send + Sync + 'static,
    Exec: Executor,
{
    /// Create a new async camera with the specified transport and executor.
    pub async fn with_executor(transport: Tr, executor: Exec) -> Result<Self, Error>
    where
        Exec: Clone,
    {
        let runtime_handle = Arc::new(
            runtime::RuntimeHandle::spawn_with_transport(transport, executor.clone()).await?,
        );
        let executor_arc = Arc::new(executor);

        Ok(Self {
            camera_id: CameraId::default(),
            timeout_config: TimeoutConfig::default(),
            runtime_handle,
            executor: executor_arc,
            envelope: TransportEnvelope::new(P::PROTOCOL_STYLE),
            envelope_buffer_manager: BufferManager::new(BufferConfig::default()),
            _phantom_profile: core::marker::PhantomData,
            _phantom_transport: core::marker::PhantomData,
        })
    }

    /// Set the camera ID for VISCA addressing.
    pub fn with_camera_id(mut self, camera_id: CameraId) -> Self {
        self.camera_id = camera_id;
        self
    }

    /// Set the timeout configuration.
    pub fn with_timeout_config(mut self, timeout_config: TimeoutConfig) -> Self {
        self.timeout_config = timeout_config;
        self
    }

    /// Get the camera ID.
    pub fn camera_id(&self) -> CameraId {
        self.camera_id
    }

    /// Get a reference to the timeout configuration.
    pub fn timeout_config(&self) -> &TimeoutConfig {
        &self.timeout_config
    }

    /// Get a reference to the executor.
    pub fn executor(&self) -> &Arc<Exec> {
        &self.executor
    }

    /// Set the camera ID (mutable method for builder compatibility).
    pub(crate) fn set_camera_id(&mut self, camera_id: CameraId) {
        self.camera_id = camera_id;
    }

    /// Set the timeout configuration (mutable method for builder compatibility).
    pub(crate) fn set_timeout_config(&mut self, timeout_config: TimeoutConfig) {
        self.timeout_config = timeout_config;
    }

    /// Send a command with typed response parsing (async).
    pub(crate) async fn send_command_typed<C>(&self, command: &C) -> Result<C::Response, Error>
    where
        C: ViscaEncode + ViscaCommand,
    {
        let response = self.send_command(command).await?;
        C::from_response(response)
    }

    /// Send a command to the camera (async).
    pub(crate) async fn send_command<C>(&self, command: &C) -> Result<ViscaResponse, Error>
    where
        C: ViscaEncode,
    {
        // Encode command bytes using ViscaEncode
        let mut buffer = [0u8; 64];
        let size = command.encode_into(self.camera_id, &mut buffer)?;
        let cmd_bytes = &buffer[..size];

        // Add VISCA terminator if not present
        let mut cmd_vec = cmd_bytes.to_vec();
        if !cmd_vec.ends_with(&[VISCA_TERMINATOR]) {
            cmd_vec.push(VISCA_TERMINATOR);
        }

        // Determine if this is an inquiry based on the typed response
        let is_inquiry = command.response_type().is_some();

        // Apply envelope and get pre-framed bytes
        let framed_bytes =
            self.envelope
                .frame_command(&cmd_vec, is_inquiry, &self.envelope_buffer_manager);

        // Send pre-framed bytes to runtime
        if is_inquiry {
            self.runtime_handle
                .send_inquiry_framed(&framed_bytes, self.camera_id, command.response_type())
                .await
        } else {
            let category = C::TIMEOUT_CATEGORY;
            self.runtime_handle
                .send_command_framed(&framed_bytes, self.camera_id, None, category)
                .await
        }
    }

    /// Send a command and return a command ID and response future.
    ///
    /// This allows advanced users to track and potentially cancel commands.
    /// Most users should use the high-level trait methods instead.
    pub async fn send_command_with_id<C>(
        &self,
        command: &C,
    ) -> Result<
        (
            u32,
            impl std::future::Future<Output = Result<ViscaResponse, Error>>,
        ),
        Error,
    >
    where
        C: ViscaEncode,
    {
        // Use the runtime's send_command_with_id method
        self.runtime_handle
            .send_command_with_id(command, self.camera_id, None)
            .await
    }

    /// Cancel a command by its ID.
    pub async fn cancel_command(&self, command_id: u32) -> Result<(), Error> {
        self.runtime_handle.cancel(command_id).await
    }

    /// Cancel all commands on a specific socket.
    ///
    /// This method is only available when the `test-utils` feature is enabled.
    #[cfg(any(test, feature = "test-utils"))]
    pub async fn cancel_socket(&self, socket: runtime::SocketId) -> Result<(), Error> {
        self.runtime_handle.cancel_socket(socket).await
    }

    /// Send a command directly and get the response.
    ///
    /// This method is only available when the `test-utils` feature is enabled.
    #[cfg(any(test, feature = "test-utils"))]
    pub async fn send_command_direct<C>(&self, command: &C) -> Result<ViscaResponse, Error>
    where
        C: ViscaEncode,
    {
        self.runtime_handle
            .send_command(command, self.camera_id, None)
            .await
    }
}
