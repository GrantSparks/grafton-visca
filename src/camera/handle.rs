//! Generic camera implementation using the unified Executor trait.
//!
//! This module provides the refactored Camera<P, T> struct that uses
//! the unified Executor trait instead of separate Runtime and Spawner.

use std::marker::PhantomData;

#[cfg(feature = "async")]
use std::{sync::Arc, time::Duration};

#[cfg(feature = "async")]
use crate::{camera::AsyncMode, executor::Executor, runtime, transport::AsyncTransport};

use crate::{
    camera::BlockingMode,
    camera_id::CameraId,
    capabilities::Profile,
    command::{const_encoding::VISCA_TERMINATOR, response::ViscaResponse, EncodeVisca},
    error::Error,
    timeout::TimeoutConfig,
    transport::{envelope::TransportEnvelope, BlockingTransport},
};

// Type aliases for backward compatibility and ergonomics
/// Blocking camera handle that owns the transport and requires `&mut self` for operations.
pub type CameraBlocking<P, T> = Camera<BlockingMode, P, T, ()>;

/// Async camera handle with shared runtime that allows `&self` operations.
#[cfg(feature = "async")]
pub type CameraAsync<P, T, E> = Camera<AsyncMode, P, T, E>;

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
/// use grafton_visca::{CameraBuilder, camera::profiles::PtzOpticsG2};
/// # #[cfg(feature = "rt-tokio")]
/// use grafton_visca::TokioExecutor;
///
/// // Async camera with explicit executor
/// let executor = TokioExecutor::from_current()?;
/// let camera = CameraBuilder::with_executor(executor)
///     .tcp("192.168.0.110:52381")
///     .profile::<PtzOpticsG2>()
///     .build()
///     .await?;
/// ```
pub struct Camera<M, P, T, E = ()>
where
    P: Profile,
{
    // For blocking mode, we own the transport directly (no Arc)
    // For async mode, the transport is moved into the runtime
    transport: Option<T>,
    camera_id: CameraId,
    // Runtime handle for async command handling (directly clonable)
    #[cfg(feature = "async")]
    runtime_handle: Option<Arc<runtime::RuntimeHandle>>,
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
            transport: Some(transport), // Own directly, no Arc
            camera_id: CameraId::default(),
            #[cfg(feature = "async")]
            runtime_handle: None,
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
    ///
    /// Note: This method requires `&mut self` because the underlying transport
    /// requires mutable access for sending and receiving.
    pub fn send_command<C>(&mut self, command: &C) -> Result<ViscaResponse, Error>
    where
        C: EncodeVisca,
    {
        // Get the transport mutably
        let transport =
            self.transport
                .as_mut()
                .ok_or(Error::InvalidState(std::borrow::Cow::Borrowed(
                    "Transport not available",
                )))?;

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
        transport.send_blocking(&request)?;

        // For non-inquiry commands, we need to handle ACK/Completion sequence
        if !is_inquiry {
            // Read first response (should be ACK or error)
            let first_response_bytes = transport.recv_blocking()?;
            let first_visca = self.envelope.extract_response(&first_response_bytes)?;
            let first_response = ViscaResponse::parse(&first_visca)?;

            match first_response {
                ViscaResponse::Error(e) => Err(e),
                ViscaResponse::CmdAck => {
                    // Got ACK, now wait for completion
                    let second_response_bytes = transport.recv_blocking()?;
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
            // For inquiry commands, just read one response
            let response_bytes = transport.recv_blocking()?;
            let visca_response = self.envelope.extract_response(&response_bytes)?;
            let response = ViscaResponse::parse(&visca_response)?;

            match response {
                ViscaResponse::Error(e) => Err(e),
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
    T: AsyncTransport + 'static,
    E: Executor,
{
    /// Create a new async camera with the specified transport and executor.
    ///
    /// This ensures that all async operations use the same executor,
    /// preventing runtime/spawner mismatches.
    ///
    /// Note: This is now an async function that initializes the runtime immediately.
    pub async fn with_executor(transport: T, executor: E) -> Result<Self, Error> {
        let executor_arc = Arc::new(executor);

        // Create the runtime handle immediately
        let runtime_handle =
            runtime::RuntimeHandle::new(transport, Arc::clone(&executor_arc)).await?;

        Ok(Self {
            // No transport stored - it's owned by the runtime
            transport: None,
            camera_id: CameraId::default(),
            runtime_handle: Some(Arc::new(runtime_handle)),
            envelope: TransportEnvelope::new(P::PROTOCOL_STYLE),
            executor: executor_arc,
            timeout_config: TimeoutConfig::default(),
            _mode: PhantomData,
            _profile: PhantomData,
            _executor: PhantomData,
        })
    }

    /// Get a reference to the executor.
    pub(crate) fn executor(&self) -> &Arc<E> {
        &self.executor
    }
}

// Convenience constructors for Tokio
#[cfg(all(feature = "async", feature = "rt-tokio"))]
impl<P, T> Camera<AsyncMode, P, T, crate::executor::TokioExecutor>
where
    P: Profile,
    T: AsyncTransport + 'static,
{
    /// Create a new async camera with Tokio executor from the current runtime.
    ///
    /// This is a convenience constructor for the common case of using Tokio.
    ///
    /// # Example
    /// ```ignore
    /// let camera = Camera::<_, PtzOpticsG2, _, _>::tokio(transport).await?;
    /// ```
    pub async fn tokio(transport: T) -> Result<Self, Error> {
        let executor = crate::executor::TokioExecutor::from_current()?;
        Self::with_executor(transport, executor).await
    }

    /// Create a new async camera with Tokio executor from a runtime handle.
    ///
    /// This allows creating a camera from a specific Tokio runtime handle.
    ///
    /// # Example
    /// ```ignore
    /// let handle = tokio::runtime::Handle::current();
    /// let camera = Camera::<_, PtzOpticsG2, _, _>::tokio_with_handle(transport, handle).await?;
    /// ```
    pub async fn tokio_with_handle(
        transport: T,
        handle: tokio::runtime::Handle,
    ) -> Result<Self, Error> {
        let executor = crate::executor::TokioExecutor::from_handle(handle);
        Self::with_executor(transport, executor).await
    }
}

// Clone is only available for async mode where the runtime handle is shareable
#[cfg(feature = "async")]
impl<P, T, E> Clone for Camera<AsyncMode, P, T, E>
where
    P: Profile,
    T: AsyncTransport + 'static,
    E: Executor,
{
    fn clone(&self) -> Self {
        Self {
            transport: None, // Transport is owned by runtime
            camera_id: self.camera_id,
            runtime_handle: self.runtime_handle.as_ref().map(Arc::clone),
            envelope: TransportEnvelope::new(P::PROTOCOL_STYLE),
            executor: Arc::clone(&self.executor),
            timeout_config: self.timeout_config,
            _mode: PhantomData,
            _profile: PhantomData,
            _executor: PhantomData,
        }
    }
}

// Blocking mode cameras cannot be cloned since they own the transport

// Implement Drop for Camera to ensure graceful shutdown for async mode
impl<M, P, T, E> Drop for Camera<M, P, T, E>
where
    P: Profile,
{
    fn drop(&mut self) {
        // Only handle runtime handle shutdown for async mode
        #[cfg(feature = "async")]
        {
            if let Some(runtime_handle) = self.runtime_handle.take() {
                // Trigger shutdown by dropping the Arc reference
                // The runtime will shut down when all references are dropped
                drop(runtime_handle);
                log::debug!("Runtime handle dropped during Camera drop");
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
            .field("transport", &self.transport.is_some());
        #[cfg(feature = "async")]
        {
            debug.field("executor", &"<Executor>");
            debug.field("runtime_handle", &self.runtime_handle.is_some());
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
    /// Send a command via the runtime handle.
    pub async fn send_command<C>(&self, command: &C) -> Result<ViscaResponse, Error>
    where
        C: EncodeVisca,
    {
        // Get runtime handle (it's always initialized in async mode)
        let runtime_handle =
            self.runtime_handle
                .as_ref()
                .ok_or(Error::InvalidState(std::borrow::Cow::Borrowed(
                    "Runtime handle not available",
                )))?;

        // Check if this is an inquiry command
        let mut buffer = [0u8; 64];
        let _size = command.encode_into(self.camera_id, &mut buffer)?;
        let is_inquiry = buffer.get(1).map(|&b| b == 0x09).unwrap_or(false);

        log::debug!(
            "Sending command via runtime: is_inquiry={}, response_type={:?}",
            is_inquiry,
            command.response_type()
        );

        // Use the appropriate runtime method
        if is_inquiry {
            runtime_handle.send_inquiry(command, self.camera_id).await
        } else {
            runtime_handle
                .send_command(command, self.camera_id, None)
                .await
        }
    }

    /// Wait for a command completion message.
    ///
    /// This waits for a 0x51 completion message from the camera, indicating
    /// that a movement command has finished executing.
    pub async fn wait_for_completion(&self) -> Result<(), Error> {
        self.wait_for_completion_with_timeout(Duration::from_secs(30))
            .await
    }

    /// Wait for a command completion message with a custom timeout.
    pub async fn wait_for_completion_with_timeout(&self, timeout: Duration) -> Result<(), Error> {
        // Monitor runtime events for completion
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            // Check runtime metrics to see if we're idle
            let runtime = self
                .runtime_handle
                .as_ref()
                .ok_or_else(|| Error::InvalidState("No runtime configured".into()))?;
            let metrics = runtime.metrics().await?;

            // If all queues are empty, we're done
            if metrics.current_queue_depth == 0 && metrics.current_retry_queue_depth == 0 {
                return Ok(());
            }

            // Small delay before checking again
            self.executor.sleep(Duration::from_millis(50)).await;
        }

        Err(Error::Timeout)
    }

    /// Check if the runtime is idle (no pending commands).
    pub async fn is_idle(&self) -> Result<bool, Error> {
        let runtime = self
            .runtime_handle
            .as_ref()
            .ok_or_else(|| Error::InvalidState("No runtime configured".into()))?;
        let metrics = runtime.metrics().await?;
        Ok(metrics.current_queue_depth == 0 && metrics.current_retry_queue_depth == 0)
    }

    /// Wait for all operations to complete (barrier synchronization).
    ///
    /// This waits until all command queues are empty, providing a
    /// synchronization point for coordinated operations.
    pub async fn wait_for_idle(&self, timeout: Duration) -> Result<(), Error> {
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if self.is_idle().await? {
                return Ok(());
            }

            // Small delay before checking again
            self.executor.sleep(Duration::from_millis(50)).await;
        }

        Err(Error::Timeout)
    }

    /// Send a command and return a command ID and response future.
    ///
    /// This allows canceling the command by its ID.
    ///
    /// # Example
    /// ```ignore
    /// let (cmd_id, response_future) = camera.send_command_with_id(&Zoom::TeleStandard).await?;
    /// // Later, cancel the command
    /// camera.cancel_command(cmd_id).await?;
    /// ```
    pub async fn send_command_with_id<C>(
        &self,
        command: &C,
    ) -> Result<(u32, impl std::future::Future<Output = Result<ViscaResponse, Error>>), Error>
    where
        C: EncodeVisca,
    {
        // Get runtime handle
        let runtime_handle = self
            .runtime_handle
            .as_ref()
            .ok_or(Error::InvalidState(std::borrow::Cow::Borrowed(
                "Runtime handle not available",
            )))?;

        // Use the runtime's send_command_with_id method
        runtime_handle
            .send_command_with_id(command, self.camera_id, None)
            .await
    }

    /// Cancel a command by its ID.
    ///
    /// Note: Currently this cancels both sockets as command-to-socket mapping
    /// is not yet implemented.
    ///
    /// # Example
    /// ```ignore
    /// let (cmd_id, _) = camera.send_command_with_id(&Zoom::TeleStandard).await?;
    /// camera.cancel_command(cmd_id).await?;
    /// ```
    pub async fn cancel_command(&self, command_id: u32) -> Result<(), Error> {
        let runtime_handle = self
            .runtime_handle
            .as_ref()
            .ok_or(Error::InvalidState(std::borrow::Cow::Borrowed(
                "Runtime handle not available",
            )))?;

        runtime_handle.cancel(command_id).await
    }

    /// Cancel all commands on a specific socket.
    ///
    /// This directly cancels the specified socket without needing to know the command ID.
    ///
    /// # Example
    /// ```ignore
    /// use grafton_visca::runtime::SocketId;
    /// camera.cancel_socket(SocketId::Socket1).await?;
    /// ```
    pub async fn cancel_socket(&self, socket: crate::runtime::SocketId) -> Result<(), Error> {
        let runtime_handle = self
            .runtime_handle
            .as_ref()
            .ok_or(Error::InvalidState(std::borrow::Cow::Borrowed(
                "Runtime handle not available",
            )))?;

        runtime_handle.cancel_socket(socket).await
    }
}
