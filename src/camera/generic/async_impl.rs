//! Async-specific implementations for the generic Camera.

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use crate::{
    capabilities::Profile,
    command::{encode_visca::EncodeVisca, Response, ResponseType},
    error::Error,
    executor::Spawner,
    socket_manager::{SocketManagerActor, SocketManagerHandle},
    transport::{core::Transport, AsyncTransportWrapper, UnifiedTransport},
};

use super::Camera;

// Impl block for convenience constructor with Transport types
impl<P, AT> Camera<P, AsyncTransportWrapper<AT>>
where
    P: Profile,
    AT: Transport + Send + Sync + 'static,
    for<'a> AT::SendFut<'a>: Send,
    for<'a> AT::RecvFut<'a>: Send,
{
    /// Create a new camera with a transport implementing the Transport trait.
    ///
    /// This is a convenience method for async mode.
    pub fn new(transport: AT) -> Self {
        let wrapped = AsyncTransportWrapper { transport };
        Self::from_transport(wrapped)
    }

    /// Create a new camera with custom spawner for async operations.
    ///
    /// This constructor allows you to provide a custom spawner for the socket manager.
    /// The socket manager will be initialized automatically.
    pub fn new_with_spawner<S: Spawner + 'static>(transport: AT, spawner: S) -> Self {
        let wrapped = AsyncTransportWrapper { transport };
        let mut camera = Self::from_transport(wrapped);
        camera.spawner = Some(Arc::new(spawner));

        // Initialize socket manager automatically for better reliability
        if let Err(e) = camera.initialize_socket_manager() {
            log::warn!("Failed to initialize socket manager: {e}");
        }

        camera
    }
}

impl<P, T> Camera<P, T>
where
    P: Profile,
    T: UnifiedTransport + 'static,
{
    /// Initialize the socket manager for this camera.
    pub fn initialize_socket_manager(&mut self) -> Result<(), Error> {
        if self.socket_manager.is_some() {
            return Ok(()); // Already initialized
        }

        // Create socket manager components
        let (command_sender, command_receiver) = crate::channels::unbounded();

        // Store the handle
        let handle = SocketManagerHandle::new(command_sender);
        self.socket_manager = Some(handle);

        // Start the socket manager actor
        let transport = Arc::clone(&self.transport);
        let actor = SocketManagerActor::new(
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
            Ok(())
        } else {
            // No spawner provided - this is expected when using standard constructors
            log::error!("Cannot initialize socket manager without a spawner");
            Err(Error::InvalidState(Cow::Borrowed(
                "Socket manager requires a spawner. Use Camera::new_with_spawner() to provide one.",
            )))
        }
    }

    /// Get a reference to the socket manager handle if initialized.
    #[must_use]
    pub fn socket_manager(&self) -> Option<&SocketManagerHandle> {
        self.socket_manager.as_ref()
    }

    /// Check if the socket manager is initialized.
    #[must_use]
    pub fn has_socket_manager(&self) -> bool {
        self.socket_manager.is_some()
    }

    /// Send a command asynchronously.
    pub async fn send_command<C>(&self, command: &C) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Prefer socket manager if available
        if let Some(socket_manager) = &self.socket_manager {
            self.send_command_via_socket_manager(command, socket_manager)
                .await
        } else {
            // Fall back to direct send
            self.send_command_direct(command).await
        }
    }

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
}
