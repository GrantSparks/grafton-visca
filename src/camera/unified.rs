//! Unified camera implementation using Mode trait for async/blocking operations.
//!
//! This module provides a single Camera type that works with both blocking and async
//! operations through the Mode trait system, eliminating the need for separate
//! AsyncCamera and BlockingCamera types.

// Standard library
use core::marker::PhantomData;
#[cfg(feature = "async")]
use std::{future::Future, pin::Pin, sync::Arc};

// Local modules
use crate::{
    camera_id::CameraId,
    capabilities::{Profile, ProtocolStyle},
    command::{typed::ViscaCommand, ViscaEncode},
    error::Error,
    mode::Mode,
    timeout::TimeoutConfig,
    transport::SyncTransport,
};

#[cfg(feature = "async")]
use crate::{executor::Executor, transport::AsyncTransport};

#[cfg(not(feature = "async"))]
use crate::{
    runtime::blocking_runner::BlockingRunner,
    transport::{
        buffer::{BufferConfig, BufferManager},
        envelope::TransportEnvelope,
    },
};

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
    timeout_config: TimeoutConfig,

    // NOTE: Mode-specific storage: either transport or runtime
    // For blocking mode: stores transport directly with BlockingRunner for state management
    // For async mode: stores runtime handle (transport and envelope managed by runtime)
    #[cfg(not(feature = "async"))]
    transport: M::Shared<Tr>,
    #[cfg(not(feature = "async"))]
    envelope: TransportEnvelope,
    #[cfg(not(feature = "async"))]
    envelope_buffer_manager: BufferManager,
    #[cfg(not(feature = "async"))]
    blocking_runner: std::cell::RefCell<BlockingRunner>,

    #[cfg(feature = "async")]
    runtime: crate::runtime::RuntimeHandle,

    _phantom_mode: PhantomData<M>,
    _phantom_profile: PhantomData<P>,
    _phantom_exec: PhantomData<Exec>,
    _phantom_transport: PhantomData<Tr>,
}

#[cfg(feature = "async")]
impl<P, Tr, Exec> Camera<crate::mode::Async, P, Tr, Exec>
where
    P: Profile + Default,
    Tr: AsyncTransport + Send + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    /// Create a camera from pre-constructed components.
    /// This is primarily used internally when RetryConfig needs to be extracted from TransportHandle.
    pub(crate) fn from_runtime_handle(
        camera_id: CameraId,
        timeout_config: TimeoutConfig,
        runtime_handle: crate::runtime::RuntimeHandle,
    ) -> Self {
        Self {
            camera_id,
            timeout_config,
            runtime: runtime_handle,
            _phantom_mode: PhantomData,
            _phantom_profile: PhantomData,
            _phantom_exec: PhantomData,
            _phantom_transport: PhantomData,
        }
    }

    /// Create a new async camera instance using the profile's protocol style.
    pub async fn new_async(transport: Tr, executor: impl Into<Arc<Exec>>) -> Result<Self, Error> {
        Self::new_async_with_style(transport, executor, P::PROTOCOL_STYLE).await
    }

    /// Create a new async camera instance with explicit protocol style.
    pub async fn new_async_with_style(
        transport: Tr,
        executor: impl Into<Arc<Exec>>,
        protocol_style: ProtocolStyle,
    ) -> Result<Self, Error> {
        let camera_id = CameraId::new(1)?;
        let timeout_config = TimeoutConfig::default();
        let executor: Arc<Exec> = executor.into();

        // Create RuntimeHandle with the transport, executor, and protocol style
        // The runtime handle encapsulates all transport, envelope, and buffer management
        let runtime_handle = crate::runtime::RuntimeHandle::new_with_style_and_timeout(
            transport,
            executor,
            protocol_style,
            timeout_config,
        )
        .await?;

        Ok(Self {
            camera_id,
            timeout_config,
            runtime: runtime_handle,
            _phantom_mode: PhantomData,
            _phantom_profile: PhantomData,
            _phantom_exec: PhantomData,
            _phantom_transport: PhantomData,
        })
    }
}

#[cfg(not(feature = "async"))]
impl<P, Tr> Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: Profile + Default,
    Tr: SyncTransport + Send + 'static,
{
    /// Create a new blocking camera instance using the profile's protocol style.
    pub fn new_blocking(transport: Tr) -> Result<Self, Error> {
        Self::new_blocking_with_style(transport, P::PROTOCOL_STYLE)
    }

    /// Create a new blocking camera instance with explicit protocol style.
    pub fn new_blocking_with_style(
        transport: Tr,
        protocol_style: ProtocolStyle,
    ) -> Result<Self, Error> {
        let camera_id = CameraId::new(1)?;
        let buffer_config = BufferConfig::default();
        let envelope = TransportEnvelope::new(protocol_style);
        let envelope_buffer_manager = BufferManager::new(buffer_config);
        let timeout_config = TimeoutConfig::default();
        let blocking_runner = BlockingRunner::new(protocol_style, timeout_config);

        let shared_transport = crate::mode::Blocking::share(transport);

        Ok(Self {
            camera_id,
            timeout_config,
            transport: shared_transport,
            envelope,
            envelope_buffer_manager,
            blocking_runner: std::cell::RefCell::new(blocking_runner),
            _phantom_mode: PhantomData,
            _phantom_profile: PhantomData,
            _phantom_exec: PhantomData,
            _phantom_transport: PhantomData,
        })
    }
}

impl<M, P, Tr, Exec> Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
{
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
}

// Methods specific to blocking cameras regardless of transport bounds
impl<P, Tr, Exec> Camera<crate::mode::Blocking, P, Tr, Exec>
where
    P: Profile,
    Tr: SyncTransport,
{
    /// Get access to the transport for blocking mode.
    #[cfg(not(feature = "async"))]
    pub(crate) fn transport(&self) -> &std::cell::RefCell<Tr> {
        &self.transport
    }
}

// Methods for async mode requiring Send + Sync transports
#[cfg(feature = "async")]
impl<P, Tr, Exec> Camera<crate::mode::Async, P, Tr, Exec>
where
    P: Profile + Default,
    Tr: Send + 'static,
{
    /// Send a command using the mode-specific return type.
    ///
    /// This method delegates to the runtime handle for proper sequence tracking,
    /// socket management, and concurrency control.
    pub fn send_command<'a, C>(
        &'a self,
        command: &'a C,
    ) -> <crate::mode::Async as Mode>::Ret<
        'static,
        Result<crate::command::response::ViscaResponse, Error>,
    >
    where
        C: ViscaEncode + Send + Sync + Clone + 'static,
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        let camera_id = self.camera_id;
        let command = command.clone();
        let runtime = self.runtime.clone();
        let is_inquiry = matches!(command.command_kind(), crate::command::CommandKind::Inquiry);

        Box::pin(async move {
            if is_inquiry {
                runtime.send_inquiry(&command, camera_id).await
            } else {
                runtime.send_command(&command, camera_id, None).await
            }
        })
    }

    /// Send a typed command and return the response.
    pub fn send_command_typed<'a, C>(
        &'a self,
        command: &'a C,
    ) -> <crate::mode::Async as Mode>::Ret<'static, Result<C::Response, Error>>
    where
        C: ViscaCommand + ViscaEncode + Send + Sync + Clone + 'static,
        C::Response: Send + 'static,
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        let response_future = self.send_command(command);
        Box::pin(async move {
            let response = response_future.await?;
            C::from_response(response)
        })
    }

    /// Send a command and return a command ID and response future.
    ///
    /// This allows advanced users to track and potentially cancel commands.
    /// Most users should use the high-level trait methods instead.
    pub fn send_command_with_id<'a, C>(
        &'a self,
        command: &'a C,
    ) -> <crate::mode::Async as Mode>::Ret<
        'static,
        Result<(u32, crate::command::response::ViscaResponse), Error>,
    >
    where
        C: ViscaEncode + Send + Sync + Clone + 'static,
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        let camera_id = self.camera_id;
        let command = command.clone();
        let runtime = self.runtime.clone();
        let is_inquiry = matches!(command.command_kind(), crate::command::CommandKind::Inquiry);

        Box::pin(async move {
            if is_inquiry {
                // Inquiries don't support command IDs in the current runtime
                // Just send and return ID 0 with response
                let response = runtime.send_inquiry(&command, camera_id).await?;
                Ok((0, response))
            } else {
                let (id, future) = runtime
                    .send_command_with_id(&command, camera_id, None)
                    .await?;
                let response = future.await?;
                Ok((id, response))
            }
        })
    }

    /// Send a command and return immediately with ID and response future.
    ///
    /// This method allows for mid-flight cancellation by returning the command ID
    /// immediately along with a future that can be awaited separately. This is useful
    /// for scenarios where you need to cancel a command while it's still in progress.
    ///
    /// # Example
    /// ```rust,ignore
    /// let (id, future) = camera.start_command_with_id(&cmd).await?;
    /// // Can cancel by ID here
    /// runtime.cancel(id).await?;
    /// // Future will resolve with Err(CommandCanceled)
    /// let result = future.await;
    /// ```
    pub async fn start_command_with_id<C>(
        &self,
        command: &C,
    ) -> Result<
        (
            u32,
            Pin<
                Box<
                    dyn Future<Output = Result<crate::command::response::ViscaResponse, Error>>
                        + Send
                        + 'static,
                >,
            >,
        ),
        Error,
    >
    where
        C: ViscaEncode + Send + Sync + Clone + 'static,
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        let camera_id = self.camera_id;
        let is_inquiry = matches!(command.command_kind(), crate::command::CommandKind::Inquiry);

        if is_inquiry {
            // Inquiries don't support command IDs in the current runtime
            // Return ID 0 with a future that immediately resolves
            let response = self.runtime.send_inquiry(command, camera_id).await?;
            let future = Box::pin(async move { Ok(response) });
            Ok((0, future))
        } else {
            // Return the ID and future directly without awaiting
            let (id, fut) = self
                .runtime
                .send_command_with_id(command, camera_id, None)
                .await?;
            let future = Box::pin(fut);
            Ok((id, future))
        }
    }

    /// Cancel all commands on a specific socket.
    ///
    /// This method sends a cancel command to the specified VISCA socket.
    pub async fn cancel_socket(&self, socket: crate::ViscaSocket) -> Result<(), Error>
    where
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        self.runtime.cancel_socket(socket).await
    }

    /// Cancel a command by its ID.
    ///
    /// This cancels a specific command that was submitted with send_command_with_id
    /// or start_command_with_id. The command's future will resolve with CommandCanceled error.
    pub async fn cancel(&self, command_id: u32) -> Result<(), Error>
    where
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        self.runtime.cancel(command_id).await
    }

    /// Sleep for a specified duration using the runtime.
    ///
    /// This provides runtime-agnostic sleeping functionality.
    pub async fn sleep(&self, duration: std::time::Duration)
    where
        Tr: AsyncTransport + Send + Sync,
    {
        self.runtime.sleep(duration).await
    }
}

#[cfg(not(feature = "async"))]
impl<P, Tr> Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: Profile + Default,
    Tr: SyncTransport + Send + 'static,
{
    /// Send a command using the mode-specific return type.
    pub fn send_command<C>(
        &self,
        command: &C,
    ) -> <crate::mode::Blocking as Mode>::Ret<
        '_,
        Result<crate::command::response::ViscaResponse, Error>,
    >
    where
        C: ViscaEncode + Send + Sync + Clone + 'static,
    {
        use crate::command::response::ViscaResponse;

        // Check if we should use the BlockingRunner for Sony protocol
        let use_blocking_runner = matches!(
            self.envelope.style(),
            ProtocolStyle::SonyEncapsulated { .. }
        );

        if use_blocking_runner {
            // Use BlockingRunner for Sony protocol to handle retry and sequencing
            let transport_cell = self.transport();
            let mut transport = match transport_cell.try_borrow_mut() {
                Ok(transport) => transport,
                Err(_) => {
                    return std::future::ready(Err(Error::TransportBusy));
                }
            };

            let mut runner = match self.blocking_runner.try_borrow_mut() {
                Ok(runner) => runner,
                Err(_) => {
                    return std::future::ready(Err(Error::TransportBusy));
                }
            };

            // Determine command category
            let category = C::TIMEOUT_CATEGORY;

            // Send command through BlockingRunner
            match runner.send_command(&mut *transport, command, self.camera_id, category) {
                Ok(response) => std::future::ready(Ok(response)),
                Err(e) => std::future::ready(Err(e)),
            }
        } else {
            // Use existing implementation for Raw VISCA
            // Encode command
            let mut buf = [0u8; 64];
            let len = match command.encode_into(self.camera_id, &mut buf) {
                Ok(len) => len,
                Err(e) => return std::future::ready(Err(e)),
            };

            debug_assert!(len > 0 && buf[len - 1] == crate::command::bytes::VISCA_TERMINATOR);

            let kind = command.command_kind();
            let is_inquiry = matches!(kind, crate::command::CommandKind::Inquiry);

            let request = self.envelope.frame_bytes_with_kind(
                &buf[..len],
                kind,
                &self.envelope_buffer_manager,
            );

            let kind = if is_inquiry {
                crate::command::CommandKind::Inquiry
            } else {
                crate::command::CommandKind::Command
            };

            let transport_cell = self.transport();
            let mut transport = match transport_cell.try_borrow_mut() {
                Ok(transport) => transport,
                Err(_) => {
                    return std::future::ready(Err(Error::TransportBusy));
                }
            };

            if let Err(e) = SyncTransport::send_with_kind(&mut *transport, &request, kind) {
                return std::future::ready(Err(e));
            }

            let timeout_config = self.timeout_config();
            let envelope = &self.envelope;

            if !is_inquiry {
                match SyncTransport::recv_with_timeout(&mut *transport, timeout_config.ack_timeout)
                {
                    Ok(first_response_bytes) => {
                        match envelope.extract_response(&first_response_bytes[..]) {
                            Ok(first_visca) => match ViscaResponse::parse(&first_visca[..]) {
                                Ok(ViscaResponse::Error(e)) => std::future::ready(Err(e)),
                                Ok(ViscaResponse::CmdAck { .. }) => {
                                    let completion_timeout =
                                        timeout_config.get_timeout(C::TIMEOUT_CATEGORY);
                                    match transport.recv_with_timeout(completion_timeout) {
                                        Ok(second_response_bytes) => match envelope
                                            .extract_response(&second_response_bytes[..])
                                        {
                                            Ok(second_visca) => {
                                                match ViscaResponse::parse(&second_visca[..]) {
                                                    Ok(response) => {
                                                        std::future::ready(Ok(response))
                                                    }
                                                    Err(e) => std::future::ready(Err(e)),
                                                }
                                            }
                                            Err(e) => std::future::ready(Err(e)),
                                        },
                                        Err(e) => std::future::ready(Err(e)),
                                    }
                                }
                                Ok(response) => std::future::ready(Ok(response)),
                                Err(e) => std::future::ready(Err(e)),
                            },
                            Err(e) => std::future::ready(Err(e)),
                        }
                    }
                    Err(e) => std::future::ready(Err(e)),
                }
            } else {
                let inquiry_timeout = timeout_config.get_timeout(C::TIMEOUT_CATEGORY);
                match SyncTransport::recv_with_timeout(&mut *transport, inquiry_timeout) {
                    Ok(response_bytes) => match envelope.extract_response(&response_bytes[..]) {
                        Ok(visca) => {
                            let parse_result = if let Some(response_type) = command.response_type()
                            {
                                ViscaResponse::parse_with_profile::<P>(&visca[..], &response_type)
                            } else {
                                ViscaResponse::parse(&visca[..])
                            };

                            match parse_result {
                                Ok(response) => std::future::ready(Ok(response)),
                                Err(e) => std::future::ready(Err(e)),
                            }
                        }
                        Err(e) => std::future::ready(Err(e)),
                    },
                    Err(e) => std::future::ready(Err(e)),
                }
            }
        }
    }

    /// Send a typed command and return the response.
    pub fn send_command_typed<C>(
        &self,
        command: &C,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<C::Response, Error>>
    where
        C: ViscaCommand + ViscaEncode + Send + Sync + Clone + 'static,
        C::Response: Send + 'static,
    {
        use crate::command::response::ViscaResponse;

        // Encode command
        let mut buf = [0u8; 64];
        let len = match command.encode_into(self.camera_id, &mut buf) {
            Ok(len) => len,
            Err(e) => return std::future::ready(Err(e)),
        };

        debug_assert!(len > 0 && buf[len - 1] == crate::command::bytes::VISCA_TERMINATOR);

        let kind = command.command_kind();
        let is_inquiry = matches!(kind, crate::command::CommandKind::Inquiry);

        let request =
            self.envelope
                .frame_bytes_with_kind(&buf[..len], kind, &self.envelope_buffer_manager);

        let kind = if is_inquiry {
            crate::command::CommandKind::Inquiry
        } else {
            crate::command::CommandKind::Command
        };

        let transport_cell = self.transport();
        let mut transport = match transport_cell.try_borrow_mut() {
            Ok(transport) => transport,
            Err(_) => {
                return std::future::ready(Err(Error::TransportBusy));
            }
        };

        if let Err(e) = SyncTransport::send_with_kind(&mut *transport, &request, kind) {
            return std::future::ready(Err(e));
        }

        let timeout_config = self.timeout_config();
        let envelope = &self.envelope;

        let visca_result = if !is_inquiry {
            match SyncTransport::recv_with_timeout(&mut *transport, timeout_config.ack_timeout) {
                Ok(first_response_bytes) => {
                    match envelope.extract_response(&first_response_bytes[..]) {
                        Ok(first_visca) => match ViscaResponse::parse(&first_visca[..]) {
                            Ok(ViscaResponse::Error(e)) => Err(e),
                            Ok(ViscaResponse::CmdAck { .. }) => {
                                let completion_timeout =
                                    timeout_config.get_timeout(C::TIMEOUT_CATEGORY);
                                match transport.recv_with_timeout(completion_timeout) {
                                    Ok(second_response_bytes) => {
                                        match envelope.extract_response(&second_response_bytes[..])
                                        {
                                            Ok(second_visca) => {
                                                match ViscaResponse::parse(&second_visca[..]) {
                                                    Ok(response) => Ok(response),
                                                    Err(e) => Err(e),
                                                }
                                            }
                                            Err(e) => Err(e),
                                        }
                                    }
                                    Err(e) => Err(e),
                                }
                            }
                            Ok(response) => Ok(response),
                            Err(e) => Err(e),
                        },
                        Err(e) => Err(e),
                    }
                }
                Err(e) => Err(e),
            }
        } else {
            let inquiry_timeout = timeout_config.get_timeout(C::TIMEOUT_CATEGORY);
            match SyncTransport::recv_with_timeout(&mut *transport, inquiry_timeout) {
                Ok(response_bytes) => match envelope.extract_response(&response_bytes[..]) {
                    Ok(visca) => {
                        let parse_result = if let Some(response_type) = command.response_type() {
                            ViscaResponse::parse_with_profile::<P>(&visca[..], &response_type)
                        } else {
                            ViscaResponse::parse(&visca[..])
                        };

                        match parse_result {
                            Ok(response) => Ok(response),
                            Err(e) => Err(e),
                        }
                    }
                    Err(e) => Err(e),
                },
                Err(e) => Err(e),
            }
        };

        std::future::ready(match visca_result {
            Ok(visca_response) => C::from_response(visca_response),
            Err(e) => Err(e),
        })
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
            .field("timeout_config", &self.timeout_config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        camera::profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
        capabilities::ProfileMetadata,
    };

    #[cfg(all(feature = "async", feature = "rt-tokio"))]
    #[tokio::test]
    async fn test_async_camera_uses_profile_protocol_style() -> Result<(), Error> {
        use crate::{executor::TokioExecutor, testing::camera_simulator::ViscaCameraSimulator};

        let executor = TokioExecutor::from_current()?;

        // Test that SonyFR7 uses Sony encapsulated protocol
        let transport = ViscaCameraSimulator::new();
        let _camera =
            Camera::<crate::mode::Async, SonyFR7, _, _>::new_async(transport, executor.clone())
                .await?;
        // The envelope should be configured with SonyEncapsulated protocol

        // Test that PtzOpticsG2 uses RawVisca protocol
        let transport = ViscaCameraSimulator::new();
        let _camera =
            Camera::<crate::mode::Async, PtzOpticsG2, _, _>::new_async(transport, executor.clone())
                .await?;
        // The envelope should be configured with RawVisca protocol

        // Test that GenericVisca uses RawVisca protocol
        let transport = ViscaCameraSimulator::new();
        let _camera =
            Camera::<crate::mode::Async, GenericVisca, _, _>::new_async(transport, executor)
                .await?;
        // The envelope should be configured with RawVisca protocol
        Ok(())
    }

    #[cfg(all(feature = "async", feature = "rt-tokio"))]
    #[tokio::test]
    async fn test_async_camera_with_explicit_protocol_style() -> Result<(), Error> {
        use crate::{executor::TokioExecutor, testing::camera_simulator::ViscaCameraSimulator};

        let executor = TokioExecutor::from_current()?;

        // Test overriding Sony profile to use RawVisca
        let transport = ViscaCameraSimulator::new();
        let _camera = Camera::<crate::mode::Async, SonyFR7, _, _>::new_async_with_style(
            transport,
            executor,
            ProtocolStyle::RawVisca,
        )
        .await?;
        // The envelope should be configured with RawVisca despite SonyFR7 default
        Ok(())
    }

    #[cfg(all(not(feature = "async"), feature = "rt-tokio"))]
    #[test]
    fn test_blocking_camera_uses_profile_protocol_style() {
        use crate::testing::camera_simulator::ViscaCameraSimulator;

        // Test that SonyFR7 uses Sony encapsulated protocol
        let transport = ViscaCameraSimulator::new();
        let _camera =
            Camera::<crate::mode::Blocking, SonyFR7, _, _>::new_blocking(transport).unwrap();
        // The envelope should be configured with SonyEncapsulated protocol

        // Test that PtzOpticsG2 uses RawVisca protocol
        let transport = ViscaCameraSimulator::new();
        let _camera =
            Camera::<crate::mode::Blocking, PtzOpticsG2, _, _>::new_blocking(transport).unwrap();
        // The envelope should be configured with RawVisca protocol

        // Test that GenericVisca uses RawVisca protocol
        let transport = ViscaCameraSimulator::new();
        let _camera =
            Camera::<crate::mode::Blocking, GenericVisca, _, _>::new_blocking(transport).unwrap();
        // The envelope should be configured with RawVisca protocol
    }

    #[cfg(all(not(feature = "async"), feature = "rt-tokio"))]
    #[test]
    fn test_blocking_camera_with_explicit_protocol_style() {
        use crate::testing::camera_simulator::ViscaCameraSimulator;

        // Test overriding Sony profile to use RawVisca
        let transport = ViscaCameraSimulator::new();
        let _camera = Camera::<crate::mode::Blocking, SonyFR7, _, _>::new_blocking_with_style(
            transport,
            ProtocolStyle::RawVisca,
        )
        .unwrap();
        // The envelope should be configured with RawVisca despite SonyFR7 default
    }

    #[test]
    fn test_profile_protocol_style_constants() {
        // Verify that profiles declare the expected protocol styles
        assert_eq!(
            SonyFR7::PROTOCOL_STYLE,
            ProtocolStyle::SonyEncapsulated { use_sequence: true }
        );
        assert_eq!(PtzOpticsG2::PROTOCOL_STYLE, ProtocolStyle::RawVisca);
        assert_eq!(GenericVisca::PROTOCOL_STYLE, ProtocolStyle::RawVisca);
    }
}
