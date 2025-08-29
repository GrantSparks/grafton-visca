//! Unified camera implementation using Mode trait for async/blocking operations.
//!
//! This module provides a single Camera type that works with both blocking and async
//! operations through the Mode trait system, eliminating the need for separate
//! AsyncCamera and BlockingCamera types.

#[cfg(feature = "async")]
use std::sync::Arc;

#[cfg(not(feature = "async"))]
use crate::transport::SyncTransport;
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
#[cfg(feature = "async")]
use crate::{executor::Executor, transport::AsyncTransport};

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
    envelope_buffer_manager: BufferManager,
    timeout_config: TimeoutConfig,
    transport: Tr,

    // Mode-specific fields - only for async mode
    #[cfg(feature = "async")]
    executor: Option<Arc<Exec>>,

    // Phantom data for compile-time parameters
    _phantom_mode: core::marker::PhantomData<M>,
    _phantom_profile: core::marker::PhantomData<P>,
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
    pub fn new_async(transport: Tr, executor: Exec) -> Result<Self, Error> {
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
            transport,
            executor: Some(Arc::new(executor)),
            _phantom_mode: core::marker::PhantomData,
            _phantom_profile: core::marker::PhantomData,
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
            transport,
            _phantom_mode: core::marker::PhantomData,
            _phantom_profile: core::marker::PhantomData,
            _phantom_exec: core::marker::PhantomData,
        })
    }
}

// Common methods that work for both modes
impl<M, P, Tr, Exec> Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile + Default,
{
    /// Send a command using the mode-specific return type.
    pub fn send_command<C>(&self, _command: &C) -> M::Ret<Result<(), Error>>
    where
        C: ViscaEncode + Send + Sync,
    {
        // This is a simplified example - real implementation would need
        // to handle the actual command sending logic
        M::ret(Ok(()))
    }

    /// Send a typed command and return the response.
    pub fn send_command_typed<C>(&self, _command: &C) -> M::Ret<Result<C::Response, Error>>
    where
        C: ViscaCommand + Send + Sync,
        C::Response: Send + 'static,
    {
        // This is a simplified example - real implementation would need
        // to handle the actual command sending and response parsing
        // For now, we'll return a default error
        M::ret(Err(Error::InvalidState("Not implemented yet".into())))
    }
}

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
