//! Unified camera implementation using Mode trait for async/blocking operations.
//!
//! This module provides a single Camera type that works with both blocking and async
//! operations through the Mode trait system, eliminating the need for separate
//! AsyncCamera and BlockingCamera types.

use crate::{
    camera_id::CameraId,
    capabilities::{Profile, ProtocolStyle},
    command::{typed::ViscaCommand, ViscaEncode},
    error::Error,
    mode::Mode,
    timeout::TimeoutConfig,
    transport::{
        buffer::{BufferConfig, BufferManager},
        envelope::TransportEnvelope,
    },
};

#[cfg(not(feature = "async"))]
use crate::transport::SyncTransport;
#[cfg(feature = "async")]
use crate::{executor::Executor, runtime::RuntimeHandle, transport::AsyncTransport};

#[cfg(not(feature = "async"))]
use std::cell::RefCell;

/// Unified camera client that works in both blocking and async modes.
///
/// This struct provides type-safe camera control that adapts to the chosen
/// execution mode through the Mode trait system. All mode-specific behavior
/// is resolved at compile time through the Mode type parameter.
///
/// # Type Parameters
///
/// * `M` - Mode type (`crate::mode::Async` or `crate::mode::Blocking`)
/// * `P` - Camera profile implementing the `Profile` trait
/// * `Tr` - Transport implementing either `AsyncTransport` or `SyncTransport`
/// * `Exec` - Executor type (only used in async mode)
///
/// # Examples
///
/// ```rust,ignore
/// use grafton_visca::{Camera, mode::{Async, Blocking}, camera::profiles::PtzOpticsG2};
/// use grafton_visca::transport::Transport;
///
/// // Async camera
/// let transport = Transport::tcp().address("192.168.0.110:5678").build_async().await?;
/// let camera = Camera::<Async, PtzOpticsG2, _, _>::new_async(transport, executor).await?;
/// camera.power_on().await?;
///
/// // Blocking camera  
/// let transport = Transport::tcp().address("192.168.0.110:5678").build_blocking()?;
/// let mut camera = Camera::<Blocking, PtzOpticsG2, _, ()>::new_blocking(transport)?;
/// camera.power_on().await?; // .await works for both modes via Mode trait
/// ```
pub struct Camera<M, P, Tr, Exec = ()>
where
    M: Mode,
    P: Profile,
{
    camera_id: CameraId,
    envelope: TransportEnvelope,
    #[allow(dead_code)] // TODO: integrate with response parsing pipeline
    envelope_buffer_manager: BufferManager,
    timeout_config: TimeoutConfig,

    // Mode-specific transport/runtime
    #[cfg(feature = "async")]
    runtime_handle: Option<RuntimeHandle>,
    #[cfg(not(feature = "async"))]
    transport: Option<RefCell<Tr>>,

    // Phantom data for compile-time parameters
    _phantom_mode: core::marker::PhantomData<M>,
    _phantom_profile: core::marker::PhantomData<P>,
    _phantom_tr: core::marker::PhantomData<Tr>,
    _phantom_exec: core::marker::PhantomData<Exec>,
}

// Implementation for async mode
#[cfg(feature = "async")]
impl<P, Tr, Exec> Camera<crate::mode::Async, P, Tr, Exec>
where
    P: Profile + Default,
    Tr: AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    /// Create a new async camera instance.
    pub async fn new_async(transport: Tr, executor: Exec) -> Result<Self, Error> {
        let camera_id = CameraId::new(1)?; // Default camera ID
        let buffer_config = BufferConfig::default();
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);
        let envelope_buffer_manager = BufferManager::new(buffer_config);
        let timeout_config = TimeoutConfig::default();

        // Create RuntimeHandle with the transport and executor
        let runtime_handle = RuntimeHandle::spawn_with_transport(transport, executor).await?;

        Ok(Self {
            camera_id,
            envelope,
            envelope_buffer_manager,
            timeout_config,
            runtime_handle: Some(runtime_handle),
            _phantom_mode: core::marker::PhantomData,
            _phantom_profile: core::marker::PhantomData,
            _phantom_tr: core::marker::PhantomData,
            _phantom_exec: core::marker::PhantomData,
        })
    }
}

// Implementation for blocking mode
#[cfg(not(feature = "async"))]
impl<P, Tr> Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: Profile + Default,
    Tr: SyncTransport + Send + 'static,
{
    /// Create a new blocking camera instance.
    pub fn new_blocking(transport: Tr) -> Result<Self, Error> {
        let camera_id = CameraId::new(1)?; // Default camera ID
        let buffer_config = BufferConfig::default();
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);
        let envelope_buffer_manager = BufferManager::new(buffer_config);
        let timeout_config = TimeoutConfig::default();

        Ok(Self {
            camera_id,
            envelope,
            envelope_buffer_manager,
            timeout_config,
            transport: Some(RefCell::new(transport)),
            _phantom_mode: core::marker::PhantomData,
            _phantom_profile: core::marker::PhantomData,
            _phantom_tr: core::marker::PhantomData,
            _phantom_exec: core::marker::PhantomData,
        })
    }
}

// Common methods that work for both modes
impl<M, P, Tr, Exec> Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile + Default,
    Tr: Send + Sync,
{
    /// Send a command using the mode-specific return type.
    ///
    /// This method connects to the actual transport and command execution system,
    /// working correctly in both async and blocking modes through the Mode trait.
    /// The same method signature works for both modes while providing proper
    /// runtime execution.
    pub fn send_command<C>(
        &self,
        command: &C,
    ) -> M::Ret<Result<crate::command::response::ViscaResponse, Error>>
    where
        C: ViscaEncode + Send + Sync + Clone + 'static,
    {
        // Mode-specific command execution that returns the raw ViscaResponse
        M::send_command(self, command)
    }

    /// Send a typed command and return the response.
    ///
    /// This method demonstrates how typed commands work in the unified API.
    /// The return type adapts to the Mode parameter while maintaining type
    /// safety for the response.
    pub fn send_command_typed<C>(&self, command: &C) -> M::Ret<Result<C::Response, Error>>
    where
        C: ViscaCommand + ViscaEncode + Send + Sync + Clone + 'static,
        C::Response: Send + 'static,
    {
        // Delegate to mode-specific typed command execution
        M::send_command_typed(self, command)
    }

    /// Get the camera ID.
    pub fn camera_id(&self) -> CameraId {
        self.camera_id
    }

    /// Set the camera ID.
    pub fn set_camera_id(&mut self, camera_id: CameraId) {
        self.camera_id = camera_id;
    }

    /// Get the current timeout configuration.
    pub fn timeout_config(&self) -> &TimeoutConfig {
        &self.timeout_config
    }

    /// Set the timeout configuration.
    pub fn set_timeout_config(&mut self, timeout_config: TimeoutConfig) {
        self.timeout_config = timeout_config;
    }

    /// Get access to the runtime handle for async mode.
    #[cfg(feature = "async")]
    pub fn runtime_handle(&self) -> Option<&RuntimeHandle> {
        self.runtime_handle.as_ref()
    }

    /// Get access to the executor for async mode.
    #[cfg(feature = "async")]
    pub fn executor(&self) -> Option<&Exec> {
        // The executor is stored in the runtime handle for the unified design
        // For now, return None as the executor access pattern needs to be redesigned
        None
    }

    /// Sleep for the specified duration using the runtime's executor.
    ///
    /// This method provides runtime-agnostic sleeping functionality that works
    /// with the unified camera design by delegating to the RuntimeHandle.
    #[cfg(feature = "async")]
    pub async fn sleep(&self, duration: std::time::Duration) {
        if let Some(_runtime_handle) = &self.runtime_handle {
            // We need to extract the executor from the runtime handle to call sleep
            // For now, we'll fall back to tokio::time::sleep as a temporary measure
            // until we can redesign the RuntimeHandle API to expose sleep directly
            #[cfg(feature = "rt-tokio")]
            tokio::time::sleep(duration).await;
            #[cfg(not(feature = "rt-tokio"))]
            {
                // For other runtimes, we'd need different approaches
                // This is a temporary fallback until the RuntimeHandle API is enhanced
            }
        } else {
            // Fallback when no runtime handle is available
            #[cfg(feature = "rt-tokio")]
            tokio::time::sleep(duration).await;
            #[cfg(not(feature = "rt-tokio"))]
            {
                // For other runtimes, we'd need different approaches
            }
        }
    }

    /// Get access to the transport for blocking mode.
    #[cfg(not(feature = "async"))]
    pub fn transport(&self) -> Option<&RefCell<Tr>> {
        self.transport.as_ref()
    }

    /// Send a command and return both the command ID and response future.
    ///
    /// This method allows for command cancellation by providing the command ID.
    /// Only available in async mode.
    #[cfg(feature = "async")]
    pub async fn send_command_with_id<C>(
        &self,
        command: &C,
    ) -> Result<
        (
            u32,
            impl std::future::Future<Output = Result<crate::command::response::ViscaResponse, Error>>,
        ),
        Error,
    >
    where
        C: ViscaEncode + Send + Sync + Clone + 'static,
    {
        if let Some(runtime_handle) = &self.runtime_handle {
            runtime_handle
                .send_command_with_id(command, self.camera_id, None)
                .await
        } else {
            Err(Error::InvalidState("No runtime handle available".into()))
        }
    }

    /// Cancel a command by its ID.
    ///
    /// This method attempts to cancel a command using the provided command ID.
    /// Only available in async mode.
    #[cfg(feature = "async")]
    pub async fn cancel_command(&self, command_id: u32) -> Result<(), Error> {
        if let Some(runtime_handle) = &self.runtime_handle {
            runtime_handle
                .cancel(command_id)
                .await
                .map_err(|e| Error::TransportError(e.to_string().into()))
        } else {
            Err(Error::InvalidState("No runtime handle available".into()))
        }
    }

    /// Cancel all commands on a specific socket.
    ///
    /// This method cancels all pending commands on the specified socket.
    /// Only available in async mode.
    #[cfg(feature = "async")]
    pub async fn cancel_socket(&self, socket: crate::ViscaSocket) -> Result<(), Error> {
        if let Some(runtime_handle) = &self.runtime_handle {
            runtime_handle
                .cancel_socket(socket)
                .await
                .map_err(|e| Error::TransportError(e.to_string().into()))
        } else {
            Err(Error::InvalidState("No runtime handle available".into()))
        }
    }

    /// Get access to the envelope for command framing.
    pub(crate) fn envelope(&self) -> &TransportEnvelope {
        &self.envelope
    }

    /// Get access to the envelope buffer manager.
    pub(crate) fn envelope_buffer_manager(&self) -> &BufferManager {
        &self.envelope_buffer_manager
    }
}

// NOTE: For the complete runtime integration, we will need to implement
// mode-specific command sending using the RuntimeHandle for async mode
// and direct transport access for blocking mode. For now, this provides
// a working foundation that demonstrates the unified API pattern.

impl<M, P, Tr, Exec> core::fmt::Debug for Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Camera")
            .field("camera_id", &self.camera_id)
            .field("envelope", &self.envelope)
            .field("timeout_config", &self.timeout_config)
            .finish()
    }
}
