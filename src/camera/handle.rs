//! Generic camera implementation using the unified Executor trait.
//!
//! This module provides the refactored Camera<P, T> struct that uses
//! the unified Executor trait instead of separate Runtime and Spawner.

#[cfg(feature = "async")]
use std::sync::Mutex;
use std::{marker::PhantomData, sync::Arc};

#[cfg(feature = "async")]
use crate::{camera::AsyncMode, executor::Executor, runtime, transport::AsyncTransport};

use crate::{
    camera::BlockingMode,
    camera_id::CameraId,
    capabilities::Profile,
    command::{const_encoding::VISCA_TERMINATOR, response::ViscaResponse, EncodeVisca},
    error::Error,
    timeout::TimeoutConfig,
    transport::{BlockingTransport, TransportEnvelope},
};

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
    // For blocking mode, we keep the transport
    // For async mode, the transport is moved into the runtime Camera
    transport: Option<Arc<T>>,
    camera_id: CameraId,
    // Runtime camera for async command handling
    #[cfg(feature = "async")]
    runtime_camera: Arc<Mutex<Option<Arc<runtime::RuntimeHandle>>>>,
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
            transport: Some(Arc::new(transport)),
            camera_id: CameraId::default(),
            #[cfg(feature = "async")]
            runtime_camera: Arc::new(Mutex::new(None)),
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
    pub fn send_command<C>(&self, command: &C) -> Result<ViscaResponse, Error>
    where
        C: EncodeVisca,
    {
        // Get the transport
        let transport =
            self.transport
                .as_ref()
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

        // Create the runtime camera immediately
        let runtime_camera =
            runtime::RuntimeHandle::new(transport, Arc::clone(&executor_arc)).await?;

        Ok(Self {
            // No transport stored - it's owned by the runtime
            transport: None,
            camera_id: CameraId::default(),
            runtime_camera: Arc::new(Mutex::new(Some(Arc::new(runtime_camera)))),
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

impl<M, P, T, E> Clone for Camera<M, P, T, E>
where
    P: Profile,
    E: Clone,
{
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.as_ref().map(Arc::clone),
            camera_id: self.camera_id,
            #[cfg(feature = "async")]
            runtime_camera: self.runtime_camera.clone(),
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
        // Only handle runtime camera shutdown for async mode
        #[cfg(feature = "async")]
        {
            if let Ok(mut runtime_camera_lock) = self.runtime_camera.try_lock() {
                if let Some(runtime_camera) = runtime_camera_lock.take() {
                    // Trigger shutdown asynchronously since Drop is not async
                    // The runtime will shut down when its submit channel is dropped
                    drop(runtime_camera);
                    log::debug!("Runtime camera dropped during Camera drop");
                }
            } else {
                log::debug!("Could not acquire runtime camera lock during Camera drop");
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
            if let Ok(rc) = self.runtime_camera.try_lock() {
                debug.field("runtime_camera", &rc.is_some());
            } else {
                debug.field("runtime_camera", &"<locked>");
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
    /// Send a command via the runtime camera.
    pub async fn send_command<C>(&self, command: &C) -> Result<ViscaResponse, Error>
    where
        C: EncodeVisca,
    {
        // Get runtime camera handle (it's always initialized in async mode)
        let runtime_camera = {
            let runtime_camera_lock = self
                .runtime_camera
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            Arc::clone(runtime_camera_lock.as_ref().ok_or(Error::InvalidState(
                std::borrow::Cow::Borrowed("Runtime camera not available"),
            ))?)
        };

        self.send_command_via_runtime(command, &runtime_camera)
            .await
    }

    /// Send command via runtime camera (internal implementation).
    async fn send_command_via_runtime<C>(
        &self,
        command: &C,
        runtime_camera: &Arc<runtime::RuntimeHandle>,
    ) -> Result<ViscaResponse, Error>
    where
        C: EncodeVisca,
    {
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
            runtime_camera.send_inquiry(command, self.camera_id).await
        } else {
            runtime_camera
                .send_command(command, self.camera_id, None)
                .await
        }
    }

    /// Wait for a command completion message.
    pub async fn wait_for_completion(&self) -> Result<(), Error> {
        // For now, we don't have a direct wait_for_completion in the runtime
        // This would need to be implemented by monitoring runtime events
        log::debug!("wait_for_completion: not yet implemented with runtime camera");

        // Return OK for now to avoid breaking existing code
        // TODO: Implement proper completion waiting through runtime events
        Ok(())
    }
}
