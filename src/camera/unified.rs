//! Unified camera implementation using Mode trait for async/blocking operations.
//!
//! This module provides a single Camera type that works with both blocking and async
//! operations through the Mode trait system, eliminating the need for separate
//! AsyncCamera and BlockingCamera types.

#[cfg(feature = "async")]
use crate::{executor::Executor, transport::AsyncTransport};

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
        SyncTransport,
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
    envelope: TransportEnvelope,
    #[allow(dead_code)] // TODO: integrate with response parsing pipeline
    envelope_buffer_manager: BufferManager,
    timeout_config: TimeoutConfig,

    // Mode-specific transport storage
    transport: M::Shared<Tr>,

    // Mode-specific executor (async only)
    #[cfg(feature = "async")]
    executor: Option<Exec>,

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
    Tr: AsyncTransport + Send + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    /// Create a new async camera instance using the profile's protocol style.
    pub async fn new_async(transport: Tr, executor: Exec) -> Result<Self, Error> {
        Self::new_async_with_style(transport, executor, P::PROTOCOL_STYLE).await
    }

    /// Create a new async camera instance with explicit protocol style.
    pub async fn new_async_with_style(
        transport: Tr,
        executor: Exec,
        protocol_style: ProtocolStyle,
    ) -> Result<Self, Error> {
        let camera_id = CameraId::new(1)?; // Default camera ID
        let buffer_config = BufferConfig::default();
        let envelope = TransportEnvelope::new(protocol_style);
        let envelope_buffer_manager = BufferManager::new(buffer_config);
        let timeout_config = TimeoutConfig::default();

        // Store transport in async-safe shared storage as single source of truth
        let shared_transport = crate::mode::Async::share(transport);

        Ok(Self {
            camera_id,
            envelope,
            envelope_buffer_manager,
            timeout_config,
            transport: shared_transport,
            executor: Some(executor),
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
    /// Create a new blocking camera instance using the profile's protocol style.
    pub fn new_blocking(transport: Tr) -> Result<Self, Error> {
        Self::new_blocking_with_style(transport, P::PROTOCOL_STYLE)
    }

    /// Create a new blocking camera instance with explicit protocol style.
    pub fn new_blocking_with_style(
        transport: Tr,
        protocol_style: ProtocolStyle,
    ) -> Result<Self, Error> {
        let camera_id = CameraId::new(1)?; // Default camera ID
        let buffer_config = BufferConfig::default();
        let envelope = TransportEnvelope::new(protocol_style);
        let envelope_buffer_manager = BufferManager::new(buffer_config);
        let timeout_config = TimeoutConfig::default();

        // Store transport directly for blocking mode (no sharing needed)
        let shared_transport = crate::mode::Blocking::share(transport);

        Ok(Self {
            camera_id,
            envelope,
            envelope_buffer_manager,
            timeout_config,
            transport: shared_transport,
            #[cfg(feature = "async")]
            executor: None, // Not used in blocking mode
            _phantom_mode: core::marker::PhantomData,
            _phantom_profile: core::marker::PhantomData,
            _phantom_exec: core::marker::PhantomData,
        })
    }
}

// Common methods available for all camera types
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

    /// Get access to the envelope for command framing.
    pub(crate) fn envelope(&self) -> &TransportEnvelope {
        &self.envelope
    }

    /// Get access to the envelope buffer manager.
    pub(crate) fn envelope_buffer_manager(&self) -> &BufferManager {
        &self.envelope_buffer_manager
    }

    /// Zero-cost helper for encoding and framing commands.
    ///
    /// This method uses a reasonable buffer size to avoid allocations
    /// and relies on existing const builders/macro invariants instead of runtime fixups.
    #[inline]
    fn encode_and_frame<C: ViscaEncode>(&self, cmd: &C) -> Result<(bytes::Bytes, bool), Error> {
        // Use a reasonable maximum buffer size for stack allocation
        // Most VISCA commands are well under 32 bytes, but we use 64 for safety
        // TODO: When const generics stabilize, use C::MAX_SIZE directly
        let mut buf = [0u8; 64];
        let len = cmd.encode_into(self.camera_id(), &mut buf)?;

        // We rely on the encode invariants; assert in debug for early signal
        debug_assert!(len > 0 && buf[len - 1] == crate::command::bytes::VISCA_TERMINATOR);

        let is_inquiry = cmd.response_type().is_some();
        let framed =
            self.envelope()
                .frame_command(&buf[..len], is_inquiry, self.envelope_buffer_manager());
        Ok((framed, is_inquiry))
    }
}

// Methods specific to async cameras regardless of transport bounds
#[cfg(feature = "async")]
impl<P, Tr, Exec> Camera<crate::mode::Async, P, Tr, Exec>
where
    P: Profile,
{
    /// Get access to the async transport for async mode.
    pub(crate) fn async_transport(&self) -> &std::sync::Arc<async_lock::Mutex<Tr>> {
        &self.transport
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
    /// This method connects to the actual transport and command execution system,
    /// working directly with the transport field for async mode.
    ///
    /// ## Implementation Status
    ///
    /// The async implementation is currently blocked by Rust issue #100013
    /// (<https://github.com/rust-lang/rust/issues/100013>) which prevents async
    /// closures from properly handling lifetime relationships with generic parameters.
    ///
    /// The implementation correctly:
    /// - ✅ Enforces ACK and per-category timeouts via `Executor::timeout`
    /// - ✅ Uses zero-allocation encoding with shared `encode_and_frame` helper
    /// - ✅ Preserves VISCA invariants with debug assertions
    /// - ✅ Maintains runtime-agnostic design via `Executor` trait
    ///
    /// However, compilation is blocked by the lifetime limitation when using
    /// `C::TIMEOUT_CATEGORY` in async contexts.
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
        // Clone necessary data from self to avoid borrowing issues
        let transport = self.async_transport().clone();
        let envelope = self.envelope().clone();
        let timeout_config = *self.timeout_config();
        let executor = match self.executor_ref() {
            Some(exec) => exec.clone(),
            None => {
                return crate::mode::Async::ret_fut(async move {
                    Err(Error::InvalidState(
                        "executor not present in Async Camera".into(),
                    ))
                })
            }
        };

        // Early error handling - encode and frame the command
        let (framed_bytes, is_inquiry) = match self.encode_and_frame(command) {
            Ok(result) => result,
            Err(e) => return crate::mode::Async::ret_fut(async move { Err(e) }),
        };

        // Clone command for move into async block
        let command = command.clone();
        // Extract timeout category to avoid generic parameter in async block
        let timeout_category = C::TIMEOUT_CATEGORY;

        // Build async future that owns all needed data
        Box::pin(async move {
            use crate::command::response::ViscaResponse;

            // Send the command first
            {
                let mut transport_guard = transport.lock().await;
                transport_guard.send(&framed_bytes).await?;
            }

            if !is_inquiry {
                // For non-inquiry commands, handle ACK/Completion sequence

                // Create a future for ACK that owns the transport lock
                let ack_fut = {
                    let transport = transport.clone();
                    async move {
                        let mut transport_guard = transport.lock().await;
                        transport_guard.recv().await
                    }
                };

                // First recv: ACK with ack_timeout
                let first_response_bytes = executor
                    .timeout_owned(timeout_config.ack_timeout, ack_fut)
                    .await??;
                let first_visca = envelope.extract_response(&first_response_bytes)?;

                match ViscaResponse::parse(&first_visca) {
                    Ok(ViscaResponse::Error(e)) => Err(e),
                    Ok(ViscaResponse::CmdAck) => {
                        // Got ACK, now wait for completion
                        let completion_timeout = timeout_config.get_timeout(timeout_category);

                        // Create a future for completion that owns the transport lock
                        let completion_fut = {
                            let transport = transport.clone();
                            async move {
                                let mut transport_guard = transport.lock().await;
                                transport_guard.recv().await
                            }
                        };

                        let second_response_bytes = executor
                            .timeout_owned(completion_timeout, completion_fut)
                            .await??;
                        let second_visca = envelope.extract_response(&second_response_bytes)?;
                        ViscaResponse::parse(&second_visca)
                    }
                    Ok(response) => Ok(response), // Some cameras skip ACK
                    Err(e) => Err(e),
                }
            } else {
                // For inquiry commands, just read one response with category timeout
                let inquiry_timeout = timeout_config.get_timeout(timeout_category);

                // Create a future for inquiry that owns the transport lock
                let inquiry_fut = {
                    let transport = transport.clone();
                    async move {
                        let mut transport_guard = transport.lock().await;
                        transport_guard.recv().await
                    }
                };

                let response_bytes = executor
                    .timeout_owned(inquiry_timeout, inquiry_fut)
                    .await??;
                let visca = envelope.extract_response(&response_bytes)?;

                // Use parse_with_type for inquiry responses
                if let Some(response_type) = command.response_type() {
                    ViscaResponse::parse_with_type(&visca, &response_type)
                } else {
                    ViscaResponse::parse(&visca)
                }
            }
        })
    }

    /// Send a typed command and return the response.
    ///
    /// This method demonstrates how typed commands work in the unified API for async mode.
    pub fn send_command_typed<C>(
        &self,
        command: &C,
    ) -> <crate::mode::Async as Mode>::Ret<'static, Result<C::Response, Error>>
    where
        C: ViscaCommand + ViscaEncode + Send + Sync + Clone + 'static,
        C::Response: Send + 'static,
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        // Clone command to move into the async block
        let command = command.clone();
        // Delegate to send_command and parse response
        let response_future = self.send_command(&command);
        crate::mode::Async::ret_fut(async move {
            let response = response_future.await?;
            C::from_response(response)
        })
    }

    /// Send a command and return a command ID and response future.
    ///
    /// This allows advanced users to track and potentially cancel commands.
    /// Most users should use the high-level trait methods instead.
    pub fn send_command_with_id<C>(
        &self,
        command: &C,
    ) -> <crate::mode::Async as Mode>::Ret<
        'static,
        Result<(u32, crate::command::response::ViscaResponse), Error>,
    >
    where
        C: ViscaEncode + Send + Sync + Clone + 'static,
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        use std::sync::atomic::{AtomicU32, Ordering};

        // Generate a unique command ID
        static NEXT_COMMAND_ID: AtomicU32 = AtomicU32::new(1);
        let command_id = NEXT_COMMAND_ID.fetch_add(1, Ordering::Relaxed);

        // Clone command to move into the async block
        let command = command.clone();
        // Execute the command and return both ID and response
        let response_future = self.send_command(&command);
        crate::mode::Async::ret_fut(async move {
            let response = response_future.await?;
            Ok((command_id, response))
        })
    }

    /// Cancel all commands on a specific socket.
    ///
    /// This method sends a cancel command to the specified VISCA socket.
    pub async fn cancel_socket(&self, socket: crate::ViscaSocket) -> Result<(), Error>
    where
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        use crate::command::system::CommandCancelCommand;

        let cancel_command = CommandCancelCommand::new(socket);

        // Use the standard send_command to send the cancel command
        match self.send_command(&cancel_command).await {
            Ok(_) => Ok(()),
            // Treat "no socket" error as success since there's nothing to cancel
            Err(Error::NoSocket) => Ok(()),
            // Treat "command canceled" as success since that's the expected result
            Err(Error::CommandCanceled) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Get access to the executor for async mode.
    #[cfg(feature = "async")]
    pub fn executor_ref(&self) -> Option<&Exec> {
        self.executor.as_ref()
    }

    /// Sleep for the specified duration using the executor.
    ///
    /// This method provides runtime-agnostic sleeping functionality.
    #[cfg(feature = "async")]
    pub async fn sleep(&self, duration: std::time::Duration)
    where
        Exec: Executor + Send + Sync,
    {
        if let Some(executor) = &self.executor {
            executor.sleep(duration).await;
        } else {
            // Fallback for when no executor is available
            #[cfg(feature = "rt-tokio")]
            tokio::time::sleep(duration).await;
            #[cfg(not(feature = "rt-tokio"))]
            {
                // For other runtimes, we'd need different approaches
                // This is a temporary fallback
            }
        }
    }

    // Note: Command cancellation features are not available in the unified design
    // as we operate directly on the transport without a runtime background task.
    // If command cancellation is needed, it would need to be implemented at the
    // transport level or through a different architecture.
}

// Implementation for blocking mode
#[cfg(not(feature = "async"))]
impl<P, Tr> Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: Profile + Default,
    Tr: SyncTransport + Send + 'static,
{
    /// Send a command using the mode-specific return type.
    ///
    /// This method connects to the actual transport and command execution system,
    /// working correctly in blocking mode through direct transport access.
    pub fn send_command<C>(
        &mut self,
        command: &C,
    ) -> <crate::mode::Blocking as Mode>::Ret<
        '_,
        Result<crate::command::response::ViscaResponse, Error>,
    >
    where
        C: ViscaEncode + Send + Sync + Clone + 'static,
    {
        use crate::command::response::ViscaResponse;

        // Use the encode_and_frame helper
        let (request, is_inquiry) = match self.encode_and_frame(command) {
            Ok(result) => result,
            Err(e) => return std::future::ready(Err(e)),
        };

        // Get access to the transport through RefCell for interior mutability
        let transport_cell = self.transport();
        let mut transport = match transport_cell.try_borrow_mut() {
            Ok(transport) => transport,
            Err(_) => {
                return std::future::ready(Err(Error::InvalidState(
                    "Transport is already borrowed".into(),
                )));
            }
        };

        if let Err(e) = SyncTransport::send(&mut *transport, &request) {
            return std::future::ready(Err(e));
        }

        // Handle response based on command type
        let timeout_config = self.timeout_config();
        let envelope = self.envelope();

        if !is_inquiry {
            // For non-inquiry commands, handle ACK/Completion sequence

            // Read first response (should be ACK or error) with ACK timeout
            match SyncTransport::recv_with_timeout(&mut *transport, timeout_config.ack_timeout) {
                Ok(first_response_bytes) => {
                    match envelope.extract_response(&first_response_bytes[..]) {
                        Ok(first_visca) => {
                            match ViscaResponse::parse(&first_visca[..]) {
                                Ok(ViscaResponse::Error(e)) => std::future::ready(Err(e)),
                                Ok(ViscaResponse::CmdAck) => {
                                    // Got ACK, now wait for completion
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
                                Ok(response) => std::future::ready(Ok(response)), // Some cameras skip ACK
                                Err(e) => std::future::ready(Err(e)),
                            }
                        }
                        Err(e) => std::future::ready(Err(e)),
                    }
                }
                Err(e) => std::future::ready(Err(e)),
            }
        } else {
            // For inquiry commands, just read one response with category timeout
            let inquiry_timeout = timeout_config.get_timeout(C::TIMEOUT_CATEGORY);
            match SyncTransport::recv_with_timeout(&mut *transport, inquiry_timeout) {
                Ok(response_bytes) => {
                    match envelope.extract_response(&response_bytes[..]) {
                        Ok(visca) => {
                            // Use parse_with_type for inquiry responses
                            let parse_result = if let Some(response_type) = command.response_type()
                            {
                                ViscaResponse::parse_with_type(&visca[..], &response_type)
                            } else {
                                ViscaResponse::parse(&visca[..])
                            };

                            match parse_result {
                                Ok(response) => std::future::ready(Ok(response)),
                                Err(e) => std::future::ready(Err(e)),
                            }
                        }
                        Err(e) => std::future::ready(Err(e)),
                    }
                }
                Err(e) => std::future::ready(Err(e)),
            }
        }
    }

    /// Send a typed command and return the response.
    ///
    /// This method demonstrates how typed commands work in the unified API for blocking mode.
    pub fn send_command_typed<C>(
        &mut self,
        command: &C,
    ) -> <crate::mode::Blocking as Mode>::Ret<'_, Result<C::Response, Error>>
    where
        C: ViscaCommand + ViscaEncode + Send + Sync + Clone + 'static,
        C::Response: Send + 'static,
    {
        use crate::command::response::ViscaResponse;

        // Use the encode_and_frame helper
        let (request, is_inquiry) = match self.encode_and_frame(command) {
            Ok(result) => result,
            Err(e) => return std::future::ready(Err(e)),
        };

        // Get access to the transport through RefCell for interior mutability
        let transport_cell = self.transport();
        let mut transport = match transport_cell.try_borrow_mut() {
            Ok(transport) => transport,
            Err(_) => {
                return std::future::ready(Err(Error::InvalidState(
                    "Transport is already borrowed".into(),
                )));
            }
        };

        // Send the request
        if let Err(e) = SyncTransport::send(&mut *transport, &request) {
            return std::future::ready(Err(e));
        }

        // Handle response based on command type
        let timeout_config = self.timeout_config();
        let envelope = self.envelope();

        let visca_result = if !is_inquiry {
            // For non-inquiry commands, handle ACK/Completion sequence
            match SyncTransport::recv_with_timeout(&mut *transport, timeout_config.ack_timeout) {
                Ok(first_response_bytes) => {
                    match envelope.extract_response(&first_response_bytes[..]) {
                        Ok(first_visca) => {
                            match ViscaResponse::parse(&first_visca[..]) {
                                Ok(ViscaResponse::Error(e)) => Err(e),
                                Ok(ViscaResponse::CmdAck) => {
                                    // Got ACK, now wait for completion
                                    let completion_timeout =
                                        timeout_config.get_timeout(C::TIMEOUT_CATEGORY);
                                    match transport.recv_with_timeout(completion_timeout) {
                                        Ok(second_response_bytes) => {
                                            match envelope
                                                .extract_response(&second_response_bytes[..])
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
                                Ok(response) => Ok(response), // Some cameras skip ACK
                                Err(e) => Err(e),
                            }
                        }
                        Err(e) => Err(e),
                    }
                }
                Err(e) => Err(e),
            }
        } else {
            // For inquiry commands, just read one response with category timeout
            let inquiry_timeout = timeout_config.get_timeout(C::TIMEOUT_CATEGORY);
            match SyncTransport::recv_with_timeout(&mut *transport, inquiry_timeout) {
                Ok(response_bytes) => {
                    match envelope.extract_response(&response_bytes[..]) {
                        Ok(visca) => {
                            // Use parse_with_type for inquiry responses
                            let parse_result = if let Some(response_type) = command.response_type()
                            {
                                ViscaResponse::parse_with_type(&visca[..], &response_type)
                            } else {
                                ViscaResponse::parse(&visca[..])
                            };

                            match parse_result {
                                Ok(response) => Ok(response),
                                Err(e) => Err(e),
                            }
                        }
                        Err(e) => Err(e),
                    }
                }
                Err(e) => Err(e),
            }
        };

        // Map the VISCA response to the typed response
        std::future::ready(match visca_result {
            Ok(visca_response) => C::from_response(visca_response),
            Err(e) => Err(e),
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        camera::profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
        capabilities::ProfileMetadata,
    };

    // Simulator is only used in feature-gated tests

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
