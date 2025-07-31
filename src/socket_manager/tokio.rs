//! Tokio-specific socket manager implementation.

use crate::camera_id::CameraId;
use crate::channels::{self, OneshotSender, UnboundedReceiver, UnboundedSender};
use crate::command::response::Response;
use crate::command::system::{CommandCancelCommand, Socket};
use crate::command::{EncodeVisca, InquiryResponse};
use crate::error::{Error, Result};
use crate::socket_manager::{
    DefaultRetryHook, PendingCmd, RetryHook, SocketManagerCommand, SocketManagerInner,
};
use crate::timeout::CommandCategory;
use crate::transport::UnifiedTransport;
use log::{debug, error, trace, warn};
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Instant;
use tokio::time;

/// Handle to communicate with the socket manager actor.
///
/// This handle can be cloned and shared across threads to send commands
/// to the socket manager from multiple locations.
#[derive(Debug)]
pub struct SocketManagerHandle {
    command_sender: UnboundedSender<SocketManagerCommand>,
}

impl Clone for SocketManagerHandle {
    fn clone(&self) -> Self {
        Self {
            command_sender: self.command_sender.clone(),
        }
    }
}

impl SocketManagerHandle {
    /// Create a new socket manager handle.
    pub fn new(command_sender: UnboundedSender<SocketManagerCommand>) -> Self {
        Self { command_sender }
    }

    /// Send a command through the socket manager.
    ///
    /// Commands are queued and executed on available sockets. The method
    /// returns when the command completes or times out.
    pub async fn send_command(
        &self,
        bytes: Vec<u8>,
        category: CommandCategory,
        is_inquiry: bool,
    ) -> Result<Response> {
        let (response_sender, response_receiver) = channels::oneshot();

        let send_result = self
            .command_sender
            .send(SocketManagerCommand::SendCommand {
                bytes,
                category,
                is_inquiry,
                response_sender,
            })
            .map_err(|_| Error::TransportError(Cow::Borrowed("Socket manager unavailable")));

        send_result?;

        let result = response_receiver.recv().await?;
        result
    }

    /// Cancel a command on a specific socket.
    ///
    /// This sends a cancel command to the camera for the specified socket.
    #[allow(dead_code)] // Future feature for command cancellation
    pub async fn cancel_command(&self, socket: Socket) -> Result<()> {
        let (response_sender, response_receiver) = channels::oneshot();

        let send_result = self
            .command_sender
            .send(SocketManagerCommand::CancelCommand {
                socket,
                response_sender,
            })
            .map_err(|_| Error::TransportError(Cow::Borrowed("Socket manager unavailable")));

        send_result?;

        let result = response_receiver.recv().await?;
        result
    }
}

/// The socket manager actor that runs the main event loop.
///
/// This actor manages the two-socket state machine, processes commands,
/// handles responses, and manages timeouts and retries.
pub struct SocketManagerActor {
    inner: SocketManagerInner,
    transport: Arc<dyn UnifiedTransport>,
    command_receiver: UnboundedReceiver<SocketManagerCommand>,
    ack_timeout: std::time::Duration,
    completion_timeout: std::time::Duration,
    retry_hook: Box<dyn RetryHook + Send + Sync>,
}

impl std::fmt::Debug for SocketManagerActor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SocketManagerActor")
            .field("inner", &self.inner)
            .field("transport", &"Arc<dyn UnifiedTransport>")
            .field("ack_timeout", &self.ack_timeout)
            .field("completion_timeout", &self.completion_timeout)
            .field("retry_hook", &"Box<dyn RetryHook>")
            .finish()
    }
}

impl SocketManagerActor {
    /// Create a new socket manager actor.
    pub fn new(
        transport: Arc<dyn UnifiedTransport>,
        command_receiver: UnboundedReceiver<SocketManagerCommand>,
        ack_timeout: std::time::Duration,
        completion_timeout: std::time::Duration,
        camera_id: CameraId,
    ) -> Self {
        let mut inner = SocketManagerInner::new();
        inner.camera_id = camera_id;
        Self {
            inner,
            transport,
            command_receiver,
            ack_timeout,
            completion_timeout,
            retry_hook: Box::new(DefaultRetryHook::new()),
        }
    }

    /// Run the socket manager actor event loop.
    ///
    /// This method runs until shutdown is requested or an error occurs.
    pub async fn run(mut self) -> Result<()> {
        debug!("Socket manager starting");

        loop {
            // Check for timeouts before processing new commands
            self.check_timeouts().await;

            tokio::select! {
                command = self.command_receiver.recv() => {
                    match command {
                        Some(SocketManagerCommand::SendCommand {
                            bytes,
                            category,
                            is_inquiry,
                            response_sender,
                        }) => {
                            self.handle_send_command(bytes, category, is_inquiry, response_sender).await;
                        }
                        Some(SocketManagerCommand::CancelCommand {
                            socket,
                            response_sender,
                        }) => {
                            self.handle_cancel_command(socket, response_sender).await;
                        }
                        None => {
                            debug!("Socket manager command channel closed");
                            break;
                        }
                    }
                }
                response_result = self.transport.recv() => {
                    match response_result {
                        Ok(bytes) => {
                            self.handle_raw_response(bytes).await;
                        }
                        Err(e) => {
                            warn!("Failed to receive response from transport: {e}");
                            // Continue processing other commands
                        }
                    }
                }
                _ = time::sleep(std::time::Duration::from_millis(100)) => {
                    // Timeout check interval - this ensures we regularly check for timeouts
                    // even when no other events are happening
                }
            }
        }

        debug!("Socket manager stopped");
        Ok(())
    }

    async fn handle_send_command(
        &mut self,
        bytes: Vec<u8>,
        category: CommandCategory,
        is_inquiry: bool,
        response_sender: OneshotSender<Result<Response>>,
    ) {
        let command_id = self.inner.get_next_command_id();
        let pending_cmd = PendingCmd::new(command_id, bytes, category, response_sender, is_inquiry);

        if is_inquiry {
            self.handle_inquiry_command(pending_cmd).await;
        } else {
            self.handle_action_command(pending_cmd).await;
        }
    }

    async fn handle_inquiry_command(&mut self, pending_cmd: PendingCmd) {
        let id = pending_cmd.id;
        trace!("Handling inquiry command {id}");

        if self.inner.pending_inquiry.is_some() {
            warn!("Inquiry already in progress, completing with error");
            pending_cmd.complete(Err(Error::CommandBufferFull));
            return;
        }

        match self.transport.send(&pending_cmd.bytes).await {
            Ok(()) => {
                trace!("Inquiry command {} sent successfully", pending_cmd.id);
                self.inner.set_pending_inquiry(pending_cmd);
            }
            Err(e) => {
                let id = pending_cmd.id;
                error!("Failed to send inquiry command {id}: {e}");
                pending_cmd.complete(Err(e));
            }
        }
    }

    async fn handle_action_command(&mut self, pending_cmd: PendingCmd) {
        let id = pending_cmd.id;
        trace!("Handling action command {id}");

        if let Some(socket) = self.inner.get_free_socket() {
            self.send_command_on_socket(pending_cmd, socket).await;
        } else {
            let id = pending_cmd.id;
            trace!("No free sockets, queueing command {id}");
            self.inner.enqueue_command(pending_cmd);
        }
    }

    async fn send_command_on_socket(&mut self, pending_cmd: PendingCmd, socket: Socket) {
        trace!("Sending command {} on {socket:?}", pending_cmd.id);

        match self.transport.send(&pending_cmd.bytes).await {
            Ok(()) => {
                trace!(
                    "Command {} sent successfully on {:?}",
                    pending_cmd.id,
                    socket
                );
                self.inner
                    .mark_socket_busy(socket, pending_cmd.id, pending_cmd.category);
                self.inner.set_active_command(socket, pending_cmd);
            }
            Err(e) => {
                let id = pending_cmd.id;
                error!("Failed to send command {id} on {socket:?}: {e}");
                pending_cmd.complete(Err(e));
            }
        }
    }

    async fn handle_cancel_command(
        &mut self,
        socket: Socket,
        response_sender: OneshotSender<Result<()>>,
    ) {
        trace!("Handling cancel command for {socket:?}");

        if self.inner.sockets[socket.as_index()].is_free() {
            warn!("Attempted to cancel command on free socket {socket:?}");
            let _ = response_sender.send(Err(Error::InvalidRequest(Cow::Borrowed(
                "No command active on socket",
            ))));
            return;
        }

        let cancel_cmd = CommandCancelCommand::new(socket);
        let mut bytes = Vec::new();
        if let Err(e) = cancel_cmd.encode_into(self.inner.camera_id, &mut bytes) {
            error!("Failed to encode cancel command for {socket:?}: {e}");
            let _ = response_sender.send(Err(Error::InvalidRequest(Cow::Borrowed(
                "Failed to encode cancel command",
            ))));
            return;
        }

        match self.transport.send(&bytes).await {
            Ok(()) => {
                trace!("Cancel command sent for {socket:?}");
                let _ = response_sender.send(Ok(()));
            }
            Err(e) => {
                error!("Failed to send cancel command for {socket:?}: {e}");
                let _ = response_sender.send(Err(e));
            }
        }
    }

    async fn handle_raw_response(&mut self, bytes: bytes::Bytes) {
        trace!("Handling raw response: {bytes:02X?}");

        if bytes.is_empty() {
            warn!("Empty response received");
            return;
        }

        // Parse the response first to determine its type
        match Response::parse(&bytes) {
            Ok(Response::CmdAck) => {
                // For ACK: 90 4y FF, extract socket from second byte
                if bytes.len() >= 2 {
                    let socket_num = bytes[1] & 0x0F; // Extract y from 4y
                    if socket_num == 1 || socket_num == 2 {
                        let socket = if socket_num == 1 {
                            Socket::Socket1
                        } else {
                            Socket::Socket2
                        };
                        self.handle_ack_response(socket).await;
                    } else {
                        warn!("Invalid socket number in ACK response: {socket_num}");
                    }
                } else {
                    warn!("ACK response too short to extract socket");
                }
            }
            Ok(Response::Completion) => {
                // For Completion: 90 5y FF, extract socket from second byte
                if bytes.len() >= 2 {
                    let socket_num = bytes[1] & 0x0F; // Extract y from 5y
                    if socket_num == 1 || socket_num == 2 {
                        let socket = if socket_num == 1 {
                            Socket::Socket1
                        } else {
                            Socket::Socket2
                        };
                        self.handle_completion_response(socket).await;
                    } else {
                        warn!("Invalid socket number in completion response: {socket_num}");
                    }
                } else {
                    warn!("Completion response too short to extract socket");
                }
            }
            Ok(Response::Error(error)) => {
                // For Error: 90 6y EE FF, extract socket from second byte
                if bytes.len() >= 2 {
                    let socket_num = bytes[1] & 0x0F; // Extract y from 6y
                    if socket_num == 1 || socket_num == 2 {
                        let socket = if socket_num == 1 {
                            Socket::Socket1
                        } else {
                            Socket::Socket2
                        };
                        self.handle_error_response(socket, error).await;
                    } else if socket_num == 0 {
                        // Socket 0 errors are for inquiries or general errors
                        warn!("Error response for inquiry or general error: {error:?}");
                    } else {
                        warn!("Invalid socket number in error response: {socket_num}");
                    }
                } else {
                    warn!("Error response too short to extract socket");
                }
            }
            Ok(Response::Inquiry(data)) => {
                self.handle_inquiry_response(data).await;
            }
            Ok(other) => {
                warn!("Unhandled response type: {other:?}");
            }
            Err(e) => {
                // Try to parse as inquiry response
                warn!("Failed to parse response: {e:?}");
            }
        }
    }

    async fn handle_ack_response(&mut self, socket: Socket) {
        trace!("Received ACK for {socket:?}");
        if let Some(command) = self.inner.get_active_command(socket) {
            debug!("ACK received for command {} on {socket:?}", command.id);
        } else {
            warn!("Received ACK for {socket:?} but no active command");
        }
    }

    async fn handle_completion_response(&mut self, socket: Socket) {
        trace!("Received completion for {socket:?}");
        if let Some(command) = self.inner.take_active_command(socket) {
            debug!(
                "Completion received for command {} on {:?}",
                command.id, socket
            );
            command.complete(Ok(Response::Completion));
            self.inner.mark_socket_free(socket);
            self.try_dispatch_next_command().await;
        } else {
            warn!("Received completion for {socket:?} but no active command");
        }
    }

    async fn handle_error_response(&mut self, socket: Socket, error: Error) {
        trace!("Received error for {socket:?}: {error:?}");
        if let Some(mut command) = self.inner.take_active_command(socket) {
            debug!(
                "Error received for command {} on {:?}: {:?}",
                command.id, socket, error
            );

            // Check if we should retry this command
            if self.retry_hook.should_retry(&error, command.retry_attempt) {
                command.retry_attempt += 1;
                let delay = self
                    .retry_hook
                    .retry_delay(&error, command.retry_attempt - 1);
                let max_attempts = self.retry_hook.max_attempts();

                debug!(
                    "Retrying command {} (attempt {}/{}) after {:?}",
                    command.id, command.retry_attempt, max_attempts, delay
                );

                // Schedule the retry
                self.schedule_retry(command, delay).await;
            } else {
                // No retry, complete with error
                debug!(
                    "Command {} exceeded max retry attempts, failing with error: {:?}",
                    command.id, error
                );
                command.complete(Err(error));
            }

            self.inner.mark_socket_free(socket);
            self.try_dispatch_next_command().await;
        } else {
            warn!("Received error for {socket:?} but no active command");
        }
    }

    async fn handle_inquiry_response(&mut self, data: InquiryResponse) {
        trace!("Received inquiry response: {data:?}");
        if let Some(command) = self.inner.take_pending_inquiry() {
            let id = command.id;
            debug!("Inquiry response received for command {id}");
            command.complete(Ok(Response::Inquiry(data)));
        } else {
            warn!("Received inquiry response but no pending inquiry");
        }
    }

    async fn try_dispatch_next_command(&mut self) {
        if let Some(socket) = self.inner.get_free_socket() {
            if let Some(command) = self.inner.dequeue_command() {
                trace!("Dispatching queued command {} on {socket:?}", command.id);
                self.send_command_on_socket(command, socket).await;
            }
        }
    }

    async fn check_timeouts(&mut self) {
        let now = Instant::now();
        let mut timed_out_sockets = Vec::new();

        // Check for socket timeouts
        for (socket_index, socket_state) in self.inner.sockets.iter().enumerate() {
            if socket_state.is_busy() {
                if let (Some(started_at), Some(category)) =
                    (socket_state.started_at(), socket_state.category())
                {
                    let timeout_duration = match category {
                        CommandCategory::Quick => self.ack_timeout,
                        CommandCategory::Movement => self.completion_timeout,
                        CommandCategory::Preset => self.completion_timeout,
                        CommandCategory::LongRunning => self.completion_timeout,
                        CommandCategory::Network => self.ack_timeout,
                        CommandCategory::Custom => self.completion_timeout,
                    };

                    if now.duration_since(started_at) > timeout_duration {
                        let socket = match socket_index {
                            0 => Socket::Socket1,
                            1 => Socket::Socket2,
                            _ => continue,
                        };
                        timed_out_sockets.push(socket);
                    }
                }
            }
        }

        // Handle timed out sockets
        for socket in timed_out_sockets {
            let socket_state = &self.inner.sockets[socket.as_index()];
            if let Some(command_id) = socket_state.command_id() {
                warn!("Command {command_id} timeout on {socket:?}, sending cancel command");
            } else {
                warn!("Command timeout on {socket:?}, sending cancel command");
            }
            self.handle_command_timeout(socket).await;
        }

        // Check for inquiry timeout
        if let Some(ref inquiry) = self.inner.pending_inquiry {
            let timeout_duration = self.completion_timeout;
            if now.duration_since(inquiry.enqueued_at) > timeout_duration {
                let id = inquiry.id;
                warn!("Inquiry command timeout for command {id}");
                self.handle_inquiry_timeout().await;
            }
        }
    }

    async fn handle_command_timeout(&mut self, socket: Socket) {
        // Get command ID for better debugging
        let command_id = self.inner.sockets[socket.as_index()].command_id();

        // Send a cancel command to the camera
        let cancel_command = CommandCancelCommand::new(socket);
        let mut cancel_bytes = vec![0u8; CommandCancelCommand::MAX_SIZE];

        if let Some(cmd_id) = command_id {
            debug!("Sending cancel command for timed out command {cmd_id} on socket {socket:?}");
        } else {
            debug!("Sending cancel command for timed out socket {socket:?}");
        }
        match cancel_command.encode_into(self.inner.camera_id, &mut cancel_bytes) {
            Ok(size) => {
                cancel_bytes.truncate(size);
                if let Err(e) = self.transport.send(&cancel_bytes).await {
                    error!("Failed to send cancel command: {e}");
                }
            }
            Err(e) => {
                error!("Failed to encode cancel command: {e}");
            }
        }

        // Complete the active command with a timeout error
        if let Some(command) = self.inner.take_active_command(socket) {
            let timeout_error = Error::CommandTimeout {
                duration: self.completion_timeout,
                command: Cow::Owned(format!("Command {} on {socket:?}", command.id)),
            };
            command.complete(Err(timeout_error));
        }

        // Mark the socket as free
        self.inner.mark_socket_free(socket);

        // Try to dispatch the next command
        self.try_dispatch_next_command().await;
    }

    async fn handle_inquiry_timeout(&mut self) {
        if let Some(inquiry) = self.inner.take_pending_inquiry() {
            let id = inquiry.id;
            let timeout_error = Error::CommandTimeout {
                duration: self.completion_timeout,
                command: Cow::Owned(format!("Inquiry command {id}")),
            };
            inquiry.complete(Err(timeout_error));
        }
    }

    async fn schedule_retry(&mut self, command: PendingCmd, delay: std::time::Duration) {
        log::debug!(
            "Scheduling retry for command {} (attempt {}) with delay {:?}",
            command.id,
            command.retry_attempt,
            delay
        );

        // For tokio builds, implement proper delayed retry
        time::sleep(delay).await;

        // After the delay, re-queue the command
        if command.is_inquiry {
            self.inner.set_pending_inquiry(command);
        } else {
            self.inner.enqueue_command(command);
        }
    }
}
