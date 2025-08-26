//! Generic camera implementation using the unified Executor trait.
//!
//! This module provides the refactored Camera type that removes invalid
//! states by using an internal enum to represent blocking vs async modes.
//! It also uses the unified Executor trait instead of separate Runtime
//! and Spawner concepts.

#[cfg(feature = "async")]
use std::{sync::Arc, time::Duration};

#[cfg(feature = "async")]
use crate::{camera::AsyncMode, executor::Executor, runtime, transport::AsyncTransport};

#[cfg(not(feature = "async"))]
use crate::camera::BlockingMode;
#[cfg(not(feature = "async"))]
use crate::command::const_encoding::VISCA_TERMINATOR;
#[cfg(not(feature = "async"))]
use crate::transport::buffer::{BufferConfig, BufferManager};
#[cfg(not(feature = "async"))]
use crate::transport::envelope::TransportEnvelope;
use crate::{
    camera_id::CameraId,
    capabilities::Profile,
    command::{response::ViscaResponse, EncodeVisca},
    error::Error,
    timeout::TimeoutConfig,
};

// Type aliases are defined in camera::mode to avoid duplication.

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
    camera_id: CameraId,
    #[cfg(not(feature = "async"))]
    envelope: TransportEnvelope,
    #[cfg(not(feature = "async"))]
    envelope_buffer_manager: BufferManager,
    timeout_config: TimeoutConfig,
    inner: CameraInner<T, E>,
    #[cfg(feature = "async")]
    _phantom_t: core::marker::PhantomData<T>,
    // Store the profile type to bind P at the type level without PhantomData
    #[allow(dead_code)]
    profile: P,
    // Keep a minimal PhantomData to bind M until a future refactor
    // removes the mode generic entirely.
    mode: core::marker::PhantomData<M>,
}

/// Internal representation of camera state for blocking vs async.
enum CameraInner<T, E> {
    #[cfg(not(feature = "async"))]
    Blocking {
        transport: T,
        _phantom: core::marker::PhantomData<E>,
    },
    #[cfg(feature = "async")]
    Async {
        runtime_handle: Arc<runtime::RuntimeHandle>,
        executor: Arc<E>,
        _phantom_t: core::marker::PhantomData<T>,
    },
}

// For blocking mode, we don't need an executor
#[cfg(not(feature = "async"))]
impl<P, T> Camera<BlockingMode, P, T, ()>
where
    P: Profile + Default,
    T: crate::transport::BlockingTransport,
{
    /// Create a new blocking camera with the specified transport.
    pub fn new(transport: T) -> Self {
        Self {
            camera_id: CameraId::default(),
            envelope: TransportEnvelope::new(P::PROTOCOL_STYLE),
            envelope_buffer_manager: BufferManager::new(BufferConfig::default()),
            timeout_config: TimeoutConfig::default(),
            inner: CameraInner::Blocking {
                transport,
                _phantom: core::marker::PhantomData,
            },
            profile: P::default(),
            mode: core::marker::PhantomData,
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
    pub(crate) fn send_command<C>(&mut self, command: &C) -> Result<ViscaResponse, Error>
    where
        C: EncodeVisca,
    {
        // Get the transport mutably (always present for blocking cameras)
        let transport = match &mut self.inner {
            CameraInner::Blocking { transport, .. } => transport,
            #[cfg(feature = "async")]
            CameraInner::Async { .. } => {
                unreachable!("internal error: async inner in blocking Camera variant")
            }
        };

        // Encode command bytes using EncodeVisca
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
        transport.send_blocking(&request)?;

        // For non-inquiry commands, we need to handle ACK/Completion sequence
        if !is_inquiry {
            // Read first response (should be ACK or error) with ACK timeout
            let first_response_bytes =
                transport.recv_blocking_with_timeout(self.timeout_config.ack_timeout)?;
            let first_visca = self.envelope.extract_response(&first_response_bytes)?;
            let first_response = ViscaResponse::parse(&first_visca)?;

            match first_response {
                ViscaResponse::Error(e) => Err(e),
                ViscaResponse::CmdAck => {
                    // Got ACK, now wait for completion
                    // Use per-category timeout for completion
                    let completion_timeout = self.timeout_config.get_timeout(C::TIMEOUT_CATEGORY);
                    let second_response_bytes =
                        transport.recv_blocking_with_timeout(completion_timeout)?;
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
            let quick = self
                .timeout_config
                .get_timeout(crate::timeout::CommandCategory::Quick);
            let response_bytes = transport.recv_blocking_with_timeout(quick)?;
            let visca_response = self.envelope.extract_response(&response_bytes)?;
            let response = ViscaResponse::parse(&visca_response)?;

            match response {
                ViscaResponse::Error(e) => Err(e),
                _ => Ok(response),
            }
        }
    }

    /// Send a command and return its typed response.
    ///
    /// This provides strongly-typed responses for inquiry commands,
    /// eliminating manual parsing and providing compile-time safety.
    pub(crate) fn send_command_typed<C>(
        &mut self,
        command: &C,
    ) -> Result<<C as crate::command::typed::ViscaCommand>::Response, Error>
    where
        C: EncodeVisca + crate::command::typed::ViscaCommand,
    {
        let resp = self.send_command(command)?;
        <C as crate::command::typed::ViscaCommand>::from_response(resp)
    }
}

// For async mode, we require an executor
#[cfg(feature = "async")]
impl<P, T, E> Camera<AsyncMode, P, T, E>
where
    P: Profile + Default,
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
            camera_id: CameraId::default(),
            timeout_config: TimeoutConfig::default(),
            inner: CameraInner::Async {
                runtime_handle: Arc::new(runtime_handle),
                executor: executor_arc,
                _phantom_t: core::marker::PhantomData,
            },
            _phantom_t: core::marker::PhantomData,
            profile: P::default(),
            mode: core::marker::PhantomData,
        })
    }

    /// Get a reference to the executor.
    pub(crate) fn executor(&self) -> &Arc<E> {
        match &self.inner {
            CameraInner::Async { executor, .. } => executor,
            #[allow(unreachable_patterns)]
            _ => unreachable!("executor requested on blocking Camera variant"),
        }
    }
}

// Convenience constructors for Tokio
#[cfg(all(feature = "async", feature = "rt-tokio"))]
impl<P, T> Camera<AsyncMode, P, T, crate::executor::TokioExecutor>
where
    P: Profile + Default,
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
    P: Profile + Default + Copy,
    T: AsyncTransport + 'static,
    E: Executor,
{
    fn clone(&self) -> Self {
        let (runtime_handle, executor) = match &self.inner {
            CameraInner::Async {
                runtime_handle,
                executor,
                ..
            } => (Arc::clone(runtime_handle), Arc::clone(executor)),
            #[allow(unreachable_patterns)]
            _ => unreachable!("attempted to clone blocking camera as async"),
        };

        Self {
            camera_id: self.camera_id,
            timeout_config: self.timeout_config,
            inner: CameraInner::Async {
                runtime_handle,
                executor,
                _phantom_t: core::marker::PhantomData,
            },
            _phantom_t: core::marker::PhantomData,
            profile: self.profile,
            mode: core::marker::PhantomData,
        }
    }
}

// Blocking mode cameras cannot be cloned since they own the transport

// No explicit Drop implementation needed; dropping the Arc in the async variant
// naturally releases the runtime tasks when all handles go out of scope.

impl<M, P, T, E> std::fmt::Debug for Camera<M, P, T, E>
where
    P: Profile,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("Camera");
        debug.field("profile", &P::MODEL_NAME);
        debug.field("camera_id", &self.camera_id);
        match &self.inner {
            #[cfg(not(feature = "async"))]
            CameraInner::Blocking { .. } => {
                debug.field("mode", &"blocking");
                debug.field("transport", &true);
            }
            #[cfg(feature = "async")]
            CameraInner::Async { .. } => {
                debug.field("mode", &"async");
                debug.field("executor", &"<Executor>");
                debug.field("runtime_handle", &true);
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
    /// Send a command via the runtime handle.
    pub(crate) async fn send_command<C>(&self, command: &C) -> Result<ViscaResponse, Error>
    where
        C: EncodeVisca,
    {
        // Get runtime handle (always present for async cameras)
        let runtime_handle = match &self.inner {
            CameraInner::Async { runtime_handle, .. } => runtime_handle,
            #[allow(unreachable_patterns)]
            _ => unreachable!("runtime handle requested on blocking Camera variant"),
        };

        // Determine inquiry by response_type to avoid double-encoding
        let is_inquiry = command.response_type().is_some();

        tracing::debug!(
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

    /// Send a command and return its typed response (async).
    ///
    /// This provides strongly-typed responses for inquiry commands,
    /// eliminating manual parsing and providing compile-time safety.
    pub(crate) async fn send_command_typed<C>(
        &self,
        command: &C,
    ) -> Result<<C as crate::command::typed::ViscaCommand>::Response, Error>
    where
        C: EncodeVisca + crate::command::typed::ViscaCommand,
    {
        let resp = self.send_command(command).await?;
        <C as crate::command::typed::ViscaCommand>::from_response(resp)
    }

    /// Wait for a command completion message.
    ///
    /// This waits for a 0x51 completion message from the camera, indicating
    /// that a movement command has finished executing.
    ///
    /// Uses the configured movement timeout from `TimeoutConfig`.
    pub async fn wait_for_completion(&self) -> Result<(), Error> {
        self.wait_for_completion_with_timeout(self.timeout_config.movement_timeout)
            .await
    }

    /// Wait for a command completion message with a custom timeout.
    pub async fn wait_for_completion_with_timeout(&self, timeout: Duration) -> Result<(), Error> {
        // Monitor runtime events for completion
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            // Check runtime metrics to see if we're idle
            let runtime = match &self.inner {
                CameraInner::Async { runtime_handle, .. } => runtime_handle,
                #[allow(unreachable_patterns)]
                _ => unreachable!("runtime handle requested on blocking Camera variant"),
            };
            let metrics = runtime.metrics().await?;

            // If all queues are empty, we're done
            if metrics.current_queue_depth == 0 && metrics.current_retry_queue_depth == 0 {
                return Ok(());
            }

            // Small delay before checking again
            let exec = match &self.inner {
                CameraInner::Async { executor, .. } => executor,
                #[allow(unreachable_patterns)]
                _ => unreachable!("executor requested on blocking Camera variant"),
            };
            exec.sleep(Duration::from_millis(50)).await;
        }

        Err(Error::Timeout)
    }

    /// Check if the runtime is idle (no pending commands).
    pub async fn is_idle(&self) -> Result<bool, Error> {
        let runtime = match &self.inner {
            CameraInner::Async { runtime_handle, .. } => runtime_handle,
            #[allow(unreachable_patterns)]
            _ => unreachable!("runtime handle requested on blocking Camera variant"),
        };
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
            let exec = match &self.inner {
                CameraInner::Async { executor, .. } => executor,
                #[allow(unreachable_patterns)]
                _ => unreachable!("executor requested on blocking Camera variant"),
            };
            exec.sleep(Duration::from_millis(50)).await;
        }

        Err(Error::Timeout)
    }

    /// Send a command and return a command ID and response future.
    ///
    /// This allows advanced users to track and potentially cancel commands.
    /// Most users should use the high-level trait methods instead.
    ///
    /// # Example
    /// ```ignore
    /// let (cmd_id, response_future) = camera.send_command_with_id(&Zoom::TeleStandard).await?;
    /// // Command is now executing asynchronously
    /// // Later, can cancel if needed:
    /// camera.cancel_command(cmd_id).await?;
    /// ```
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
        C: EncodeVisca,
    {
        // Get runtime handle
        let runtime_handle = match &self.inner {
            CameraInner::Async { runtime_handle, .. } => runtime_handle,
            #[allow(unreachable_patterns)]
            _ => unreachable!("runtime handle requested on blocking Camera variant"),
        };

        // Use the runtime's send_command_with_id method
        runtime_handle
            .send_command_with_id(command, self.camera_id, None)
            .await
    }

    /// Cancel a command by its ID.
    ///
    /// This cancels a previously issued command using the ID returned by
    /// `send_command_with_id`. Note that the command may have already
    /// completed by the time this is called.
    ///
    /// # Example
    /// ```ignore
    /// let (cmd_id, _) = camera.send_command_with_id(&Zoom::TeleStandard).await?;
    /// // Later, cancel the command
    /// camera.cancel_command(cmd_id).await?;
    /// ```
    pub async fn cancel_command(&self, command_id: u32) -> Result<(), Error> {
        let runtime_handle = match &self.inner {
            CameraInner::Async { runtime_handle, .. } => runtime_handle,
            #[allow(unreachable_patterns)]
            _ => unreachable!("runtime handle requested on blocking Camera variant"),
        };

        runtime_handle.cancel(command_id).await
    }

    /// Cancel all commands on a specific socket.
    ///
    /// This method is only available when the `test-utils` feature is enabled.
    /// It provides direct access to socket-level cancellation for testing purposes.
    #[cfg(any(test, feature = "test-utils"))]
    pub async fn cancel_socket(&self, socket: runtime::SocketId) -> Result<(), Error> {
        let runtime_handle = match &self.inner {
            CameraInner::Async { runtime_handle, .. } => runtime_handle,
            #[allow(unreachable_patterns)]
            _ => unreachable!("runtime handle requested on blocking Camera variant"),
        };

        runtime_handle.cancel_socket(socket).await
    }

    /// Send a command directly and get the response.
    ///
    /// This method is only available when the `test-utils` feature is enabled.
    /// It provides low-level command sending for testing purposes.
    #[cfg(any(test, feature = "test-utils"))]
    pub async fn send_command_direct<C>(&self, command: &C) -> Result<ViscaResponse, Error>
    where
        C: EncodeVisca,
    {
        match &self.inner {
            CameraInner::Async { runtime_handle, .. } => {
                runtime_handle
                    .send_command(command, self.camera_id, None)
                    .await
            }
            #[allow(unreachable_patterns)]
            _ => unreachable!("async method called on blocking Camera variant"),
        }
    }
}
