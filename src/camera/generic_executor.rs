//! Generic camera implementation using the unified Executor trait.
//!
//! This module provides the refactored Camera<P, T> struct that uses
//! the unified Executor trait instead of separate Runtime and Spawner.

use std::{marker::PhantomData, sync::Arc};

#[cfg(feature = "async")]
use crate::camera::AsyncMode;
use crate::camera::BlockingMode;

#[cfg(feature = "async")]
use crate::{executor_unified::Executor, socket_manager::SocketManagerHandle};

#[cfg(feature = "async")]
use std::sync::Mutex;

use crate::{
    camera_id::CameraId, capabilities::Profile, command::const_encoding::VISCA_TERMINATOR,
    command::EncodeVisca, error::Error, timeout::TimeoutConfig, transport::TransportEnvelope,
};

#[cfg(feature = "async")]
use crate::transport::AsyncTransport;

use crate::transport::BlockingTransport;

use crate::command::response::Response;

/// Generic camera client with compile-time mode and profile selection.
///
/// This struct provides type-safe camera control with zero runtime overhead.
/// All mode and profile-specific constants and behaviors are resolved at compile time.
///
/// The key improvement in this version is the use of a unified `Executor` trait
/// that prevents runtime/spawner mismatches at compile time.
///
/// # Type Parameters
///
/// * `M` - Camera mode (AsyncMode or BlockingMode)
/// * `P` - Camera profile implementing the `Profile` trait
/// * `T` - Transport (AsyncTransport for async mode, BlockingTransport for blocking mode)
/// * `E` - Executor type (only for async mode)
///
/// # Examples
///
/// ```ignore
/// use grafton_visca::{CameraBuilder, camera::profiles::PTZOpticsG2};
/// # #[cfg(feature = "rt-tokio")]
/// use grafton_visca::TokioExecutor;
///
/// // Async camera with explicit executor
/// let executor = TokioExecutor::from_current()?;
/// let camera = CameraBuilder::with_executor(executor)
///     .tcp("192.168.0.110:52381")
///     .profile::<PTZOpticsG2>()
///     .build()
///     .await?;
/// ```
pub struct Camera<M, P, T, E = ()>
where
    P: Profile,
{
    transport: Arc<T>,
    camera_id: CameraId,
    #[cfg(feature = "async")]
    socket_manager: Arc<Mutex<Option<SocketManagerHandle>>>,
    envelope: TransportEnvelope,
    #[cfg(feature = "async")]
    executor: Arc<E>,
    timeout_config: TimeoutConfig,
    _mode: PhantomData<M>,
    _profile: PhantomData<P>,
    _executor: PhantomData<E>,
}

// For blocking mode, we don't need an executor
impl<P, T> Camera<BlockingMode, P, T, ()>
where
    P: Profile,
    T: BlockingTransport,
{
    /// Create a new blocking camera with the specified transport.
    pub fn new(transport: T) -> Self {
        Self {
            transport: Arc::new(transport),
            camera_id: CameraId::default(),
            #[cfg(feature = "async")]
            socket_manager: Arc::new(Mutex::new(None)),
            envelope: TransportEnvelope::new(P::PROTOCOL_STYLE),
            #[cfg(feature = "async")]
            executor: Arc::new(()),
            timeout_config: TimeoutConfig::default(),
            _mode: PhantomData,
            _profile: PhantomData,
            _executor: PhantomData,
        }
    }

    /// Create a new blocking camera with the specified transport.
    ///
    /// Alias for `new()` for consistency with the `from_transport` naming pattern.
    pub fn from_transport(transport: T) -> Self {
        Self::new(transport)
    }

    /// Send a command to the camera and wait for a response (blocking).
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
        if !cmd_vec.ends_with(&[VISCA_TERMINATOR]) {
            cmd_vec.push(VISCA_TERMINATOR);
        }

        // Apply envelope and send command
        // Check if this is an inquiry command (second byte is 0x09)
        let is_inquiry = cmd_vec.get(1).map(|&b| b == 0x09).unwrap_or(false);
        let request = self.envelope.frame_command(&cmd_vec, is_inquiry);

        // Send command
        self.transport.send_blocking(&request)?;

        // For non-inquiry commands, we need to handle ACK/Completion sequence
        if !is_inquiry {
            // Read first response (should be ACK or error)
            let first_response_bytes = self.transport.recv_blocking()?;
            let first_visca = self.envelope.extract_response(&first_response_bytes)?;
            let first_response = Response::parse(&first_visca)?;

            match first_response {
                Response::Error(e) => Err(e),
                Response::CmdAck => {
                    // Got ACK, now wait for completion
                    let second_response_bytes = self.transport.recv_blocking()?;
                    let second_visca = self.envelope.extract_response(&second_response_bytes)?;
                    let second_response = Response::parse(&second_visca)?;

                    match second_response {
                        Response::Error(e) => Err(e),
                        _ => Ok(second_response),
                    }
                }
                // If first response is already completion (some cameras skip ACK)
                Response::Completion => Ok(first_response),
                _ => Ok(first_response),
            }
        } else {
            // For inquiry commands, just read one response
            let response_bytes = self.transport.recv_blocking()?;
            let visca_response = self.envelope.extract_response(&response_bytes)?;
            let response = Response::parse(&visca_response)?;

            match response {
                Response::Error(e) => Err(e),
                _ => Ok(response),
            }
        }
    }
}

// For async mode, we require an executor
#[cfg(feature = "async")]
impl<P, T, E> Camera<AsyncMode, P, T, E>
where
    P: Profile,
    T: AsyncTransport,
    E: Executor,
{
    /// Create a new async camera with the specified transport and executor.
    ///
    /// This ensures that all async operations use the same executor,
    /// preventing runtime/spawner mismatches.
    pub fn with_executor(transport: T, executor: E) -> Self {
        Self {
            transport: Arc::new(transport),
            camera_id: CameraId::default(),
            socket_manager: Arc::new(Mutex::new(None)),
            envelope: TransportEnvelope::new(P::PROTOCOL_STYLE),
            executor: Arc::new(executor),
            timeout_config: TimeoutConfig::default(),
            _mode: PhantomData,
            _profile: PhantomData,
            _executor: PhantomData,
        }
    }

    /// Get a reference to the executor.
    pub(crate) fn executor(&self) -> &Arc<E> {
        &self.executor
    }
}

// Convenience constructors for Tokio
#[cfg(all(feature = "async", feature = "rt-tokio"))]
impl<P, T> Camera<AsyncMode, P, T, crate::executor_unified::TokioExecutor>
where
    P: Profile,
    T: AsyncTransport,
{
    /// Create a new async camera with Tokio executor from the current runtime.
    ///
    /// This is a convenience constructor for the common case of using Tokio.
    ///
    /// # Example
    /// ```ignore
    /// let camera = Camera::<_, PTZOpticsG2, _, _>::tokio(transport)?;
    /// ```
    pub fn tokio(transport: T) -> Result<Self, Error> {
        let executor = crate::executor_unified::TokioExecutor::from_current()?;
        Ok(Self::with_executor(transport, executor))
    }

    /// Create a new async camera with Tokio executor from a runtime handle.
    ///
    /// This allows creating a camera from a specific Tokio runtime handle.
    ///
    /// # Example
    /// ```ignore
    /// let handle = tokio::runtime::Handle::current();
    /// let camera = Camera::<_, PTZOpticsG2, _, _>::tokio_with_handle(transport, handle);
    /// ```
    pub fn tokio_with_handle(transport: T, handle: tokio::runtime::Handle) -> Self {
        let executor = crate::executor_unified::TokioExecutor::from_handle(handle);
        Self::with_executor(transport, executor)
    }
}

impl<M, P, T, E> Clone for Camera<M, P, T, E>
where
    P: Profile,
    E: Clone,
{
    fn clone(&self) -> Self {
        Self {
            transport: Arc::clone(&self.transport),
            camera_id: self.camera_id,
            #[cfg(feature = "async")]
            socket_manager: self.socket_manager.clone(),
            envelope: TransportEnvelope::new(P::PROTOCOL_STYLE),
            #[cfg(feature = "async")]
            executor: Arc::clone(&self.executor),
            timeout_config: self.timeout_config,
            _mode: PhantomData,
            _profile: PhantomData,
            _executor: PhantomData,
        }
    }
}

// Implement Drop for Camera to ensure graceful shutdown for async mode
impl<M, P, T, E> Drop for Camera<M, P, T, E>
where
    P: Profile,
{
    fn drop(&mut self) {
        // Only handle socket manager shutdown for async mode
        #[cfg(feature = "async")]
        {
            if let Ok(mut socket_manager_lock) = self.socket_manager.try_lock() {
                if let Some(socket_manager) = socket_manager_lock.take() {
                    socket_manager.shutdown_nowait();
                    log::debug!("Sent shutdown signal to socket manager during Camera drop");
                }
            } else {
                log::debug!("Could not acquire socket manager lock during Camera drop");
            }
        }
    }
}

impl<M, P, T, E> std::fmt::Debug for Camera<M, P, T, E>
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
            debug.field("executor", &"<Executor>");
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
impl<M, P, T, E> Camera<M, P, T, E>
where
    P: Profile,
{
    /// Get the camera ID for this instance.
    pub fn camera_id(&self) -> CameraId {
        self.camera_id
    }

    /// Set the camera ID for this instance.
    pub fn set_camera_id(&mut self, id: CameraId) {
        self.camera_id = id;
    }

    /// Get the current timeout configuration.
    pub fn timeout_config(&self) -> &TimeoutConfig {
        &self.timeout_config
    }

    /// Set a new timeout configuration.
    pub fn set_timeout_config(&mut self, config: TimeoutConfig) {
        self.timeout_config = config;
    }

    /// Get the camera profile type name.
    pub fn profile_name(&self) -> &'static str {
        P::MODEL_NAME
    }
}

// Async-specific methods for Camera
#[cfg(feature = "async")]
impl<P, T, E> Camera<AsyncMode, P, T, E>
where
    P: Profile,
    T: AsyncTransport + 'static,
    E: Executor,
{
    /// Initialize the socket manager if not already initialized.
    async fn ensure_socket_manager(&self) -> Result<(), Error> {
        let mut socket_manager_lock = self
            .socket_manager
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        if socket_manager_lock.is_some() {
            return Ok(());
        }

        // Create channels for communication
        let (command_sender, command_receiver) = crate::channels::unbounded();

        // Store the handle
        let handle = SocketManagerHandle::new(command_sender);
        *socket_manager_lock = Some(handle);

        // Get the executor
        let executor = self.executor();
        let transport = Arc::clone(&self.transport);
        let timeout_config = self.timeout_config;
        let camera_id = self.camera_id;

        // Create and spawn the socket manager actor
        let actor = crate::socket_manager::SocketManagerActor::with_executor(
            transport,
            command_receiver,
            timeout_config,
            camera_id,
            Arc::clone(executor),
        );

        // Spawn the actor using the executor
        let _handle = executor.spawn(async move {
            if let Err(e) = actor.run().await {
                log::error!("Socket manager actor failed: {e}");
            }
        });

        Ok(())
    }

    /// Send a command via the socket manager.
    pub async fn send_command<C>(&self, command: &C) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Ensure socket manager is initialized
        self.ensure_socket_manager().await?;

        // Get socket manager handle
        let socket_manager = {
            let socket_manager_lock = self
                .socket_manager
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            socket_manager_lock
                .as_ref()
                .ok_or(Error::InvalidState(std::borrow::Cow::Borrowed(
                    "Socket manager not initialized",
                )))?
                .clone()
        };

        self.send_command_via_socket_manager(command, &socket_manager)
            .await
    }

    /// Send command via socket manager (internal implementation).
    async fn send_command_via_socket_manager<C>(
        &self,
        command: &C,
        socket_manager: &SocketManagerHandle,
    ) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Encode the command
        let command_bytes = command.try_into_vec(self.camera_id)?;
        let response_type = command.response_type();

        // Check if this is an inquiry command
        let is_inquiry = command_bytes.get(1).map(|&b| b == 0x09).unwrap_or(false);

        // Frame the command
        let framed_bytes = self.envelope.frame_command(&command_bytes, is_inquiry);

        // Determine the command category for timeout
        let category = command.timeout_kind();

        log::debug!(
            "Sending command via socket manager: category={:?}, is_inquiry={}, response_type={:?}",
            category,
            is_inquiry,
            response_type
        );

        // Send via socket manager
        socket_manager
            .send_command(framed_bytes, category, is_inquiry, response_type)
            .await
    }

    /// Wait for a command completion message.
    pub async fn wait_for_completion(&self) -> Result<(), Error> {
        // Ensure socket manager is initialized
        self.ensure_socket_manager().await?;

        // Get socket manager handle (clone it to avoid lifetime issues)
        let socket_manager = {
            let socket_manager_lock = self
                .socket_manager
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            socket_manager_lock
                .as_ref()
                .ok_or(Error::InvalidState(std::borrow::Cow::Borrowed(
                    "Socket manager not available for wait_for_completion",
                )))?
                .clone()
        };

        log::debug!("wait_for_completion: using socket manager to wait for completion message");

        let wait_fut = socket_manager.wait_for_completion();

        // Get executor for timeout
        let executor = self.executor();
        let timeout_duration = self
            .timeout_config
            .get_timeout(crate::timeout::CommandCategory::Movement);

        // Apply timeout using executor
        executor.timeout(timeout_duration, wait_fut).await?
    }
}
