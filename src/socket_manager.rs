use bytes::Bytes;
use log::{debug, error, trace, warn};

use std::{borrow::Cow, collections::VecDeque, sync::Arc, time::Instant};

use crate::{
    camera_id::CameraId,
    channels::{self, OneshotSender, UnboundedReceiver, UnboundedSender},
    command::{
        response::Response,
        system::{CommandCancelCommand, Socket},
        EncodeVisca,
    },
    error::{Error, Result},
    timeout::{CommandCategory, TimeoutConfig},
    transport::AsyncTransport,
};

impl Socket {
    /// Get the socket as a zero-based array index.
    ///
    /// This is useful for indexing into arrays where sockets are tracked by position.
    ///
    /// # Returns
    /// * `0` for Socket1
    /// * `1` for Socket2
    pub fn as_index(&self) -> usize {
        match self {
            Socket::Socket1 => 0,
            Socket::Socket2 => 1,
        }
    }
}

/// Represents the state of a socket in the VISCA communication system.
///
/// Sockets can either be free (available for new commands) or busy
/// (currently executing a command).
#[derive(Debug, Clone, Copy)]
pub enum SocketState {
    /// Socket is free and available for new commands.
    Free,
    /// Socket is busy executing a command.
    Busy {
        /// Unique identifier of the command being executed on this socket.
        command_id: u32,
        /// When the command execution started.
        started_at: Instant,
        /// Category of the command for timeout calculation.
        category: CommandCategory,
    },
}

impl SocketState {
    /// Check if the socket is free and available for new commands.
    pub fn is_free(&self) -> bool {
        matches!(self, SocketState::Free)
    }

    /// Check if the socket is busy executing a command.
    pub fn is_busy(&self) -> bool {
        matches!(self, SocketState::Busy { .. })
    }

    /// Get the time when the current command execution started, if the socket is busy.
    pub fn started_at(&self) -> Option<Instant> {
        match self {
            SocketState::Free => None,
            SocketState::Busy { started_at, .. } => Some(*started_at),
        }
    }

    /// Get the ID of the command currently being executed, if the socket is busy.
    pub fn command_id(&self) -> Option<u32> {
        match self {
            SocketState::Free => None,
            SocketState::Busy { command_id, .. } => Some(*command_id),
        }
    }

    /// Get the category of the command currently being executed, if the socket is busy.
    pub fn category(&self) -> Option<CommandCategory> {
        match self {
            SocketState::Free => None,
            SocketState::Busy { category, .. } => Some(*category),
        }
    }
}

/// Represents a command that is waiting to be sent or is currently being executed.
#[derive(Debug)]
pub struct PendingCmd {
    /// Unique identifier for this command instance.
    pub id: u32,
    /// The raw VISCA command bytes to send.
    pub bytes: Bytes,
    /// Category of the command for timeout and retry logic.
    pub category: CommandCategory,
    /// Channel to send the response back to the caller.
    pub response_sender: OneshotSender<Result<Response>>,
    /// Whether this is an inquiry command (query) or action command.
    pub is_inquiry: bool,
    /// When this command was first enqueued.
    pub enqueued_at: Instant,
    /// Number of retry attempts made for this command.
    pub retry_attempt: u32,
}

impl PendingCmd {
    /// Create a new pending command.
    pub fn new(
        id: u32,
        bytes: Bytes,
        category: CommandCategory,
        response_sender: OneshotSender<Result<Response>>,
        is_inquiry: bool,
    ) -> Self {
        Self {
            id,
            bytes,
            category,
            response_sender,
            is_inquiry,
            enqueued_at: Instant::now(),
            retry_attempt: 0,
        }
    }

    /// Complete this command by sending the result through the response channel.
    pub fn complete(self, result: Result<Response>) {
        let _ = self.response_sender.send(result);
    }
}

/// Internal state of the socket manager.
///
/// This struct maintains the state of both sockets, the command queue,
/// and tracking information for active commands.
#[derive(Debug)]
pub struct SocketManagerInner {
    /// State of each socket (Socket1 and Socket2).
    pub sockets: [SocketState; 2],
    /// Queue of commands waiting to be sent.
    pub command_queue: VecDeque<PendingCmd>,
    /// Counter for generating unique command IDs.
    pub next_command_id: u32,
    /// Commands currently being executed on each socket.
    pub active_commands: [Option<PendingCmd>; 2],
    /// Inquiry command waiting for response (only one allowed at a time).
    pub pending_inquiry: Option<PendingCmd>,
    /// Camera ID for addressing commands.
    pub camera_id: CameraId,
    /// List of waiters for completion messages.
    pub completion_waiters: VecDeque<OneshotSender<Result<()>>>,
}

impl Default for SocketManagerInner {
    fn default() -> Self {
        Self {
            sockets: [SocketState::Free, SocketState::Free],
            command_queue: VecDeque::new(),
            next_command_id: 1,
            active_commands: [None, None],
            pending_inquiry: None,
            camera_id: CameraId::CAMERA_1,
            completion_waiters: VecDeque::new(),
        }
    }
}

impl SocketManagerInner {
    /// Create a new socket manager inner state with both sockets free.
    pub fn new() -> Self {
        Self::default()
    }

    /// Find a free socket available for sending a command.
    pub fn get_free_socket(&self) -> Option<Socket> {
        if self.sockets[0].is_free() {
            Some(Socket::Socket1)
        } else if self.sockets[1].is_free() {
            Some(Socket::Socket2)
        } else {
            None
        }
    }

    /// Mark a socket as busy with the given command.
    pub fn mark_socket_busy(&mut self, socket: Socket, command_id: u32, category: CommandCategory) {
        let index = socket.as_index();
        self.sockets[index] = SocketState::Busy {
            command_id,
            started_at: Instant::now(),
            category,
        };
    }

    /// Mark a socket as free and clear its active command.
    pub fn mark_socket_free(&mut self, socket: Socket) {
        let index = socket.as_index();
        self.sockets[index] = SocketState::Free;
        self.active_commands[index] = None;
    }

    /// Get the next unique command ID.
    pub fn get_next_command_id(&mut self) -> u32 {
        let id = self.next_command_id;
        self.next_command_id = self.next_command_id.wrapping_add(1);
        id
    }

    /// Add a command to the queue.
    pub fn enqueue_command(&mut self, command: PendingCmd) {
        self.command_queue.push_back(command);
    }

    /// Remove and return the next command from the queue.
    pub fn dequeue_command(&mut self) -> Option<PendingCmd> {
        self.command_queue.pop_front()
    }

    /// Set the active command for a socket.
    pub fn set_active_command(&mut self, socket: Socket, command: PendingCmd) {
        let index = socket.as_index();
        self.active_commands[index] = Some(command);
    }

    /// Take the active command for a socket, leaving None in its place.
    pub fn take_active_command(&mut self, socket: Socket) -> Option<PendingCmd> {
        let index = socket.as_index();
        self.active_commands[index].take()
    }

    /// Get a reference to the active command for a socket.
    pub fn get_active_command(&self, socket: Socket) -> Option<&PendingCmd> {
        let index = socket.as_index();
        self.active_commands[index].as_ref()
    }

    /// Set the pending inquiry command.
    pub fn set_pending_inquiry(&mut self, command: PendingCmd) {
        self.pending_inquiry = Some(command);
    }

    /// Take the pending inquiry command, leaving None in its place.
    pub fn take_pending_inquiry(&mut self) -> Option<PendingCmd> {
        self.pending_inquiry.take()
    }
}

/// Commands that can be sent to the socket manager actor.
#[derive(Debug)]
pub(crate) enum SocketManagerCommand {
    /// Send a VISCA command through the socket manager.
    SendCommand {
        /// Raw command bytes to send.
        bytes: Bytes,
        /// Command category for timeout and retry logic.
        category: CommandCategory,
        /// Whether this is an inquiry (query) command.
        is_inquiry: bool,
        /// Channel to send the response back to the caller.
        response_sender: OneshotSender<Result<Response>>,
    },
    /// Wait for a completion message (0x51) on any socket.
    WaitForCompletion {
        /// Channel to send the result when a completion message is received.
        response_sender: OneshotSender<Result<()>>,
    },
    /// Shutdown the socket manager gracefully.
    Shutdown,
}

/// Handle to communicate with the socket manager actor.
///
/// This handle can be cloned and shared across threads to send commands
/// to the socket manager from multiple locations.
#[derive(Debug)]
pub(crate) struct SocketManagerHandle {
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
        bytes: Bytes,
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
            .map_err(|_| Error::SocketManagerUnavailable);

        send_result?;

        response_receiver.recv().await?
    }

    /// Wait for a completion message from any socket.
    ///
    /// This is used for event-driven movement detection to wait for
    /// operation complete (0x51) messages.
    pub async fn wait_for_completion(&self) -> Result<()> {
        let (response_sender, response_receiver) = channels::oneshot();

        #[cfg(feature = "rt-tokio")]
        let send_result = self
            .command_sender
            .send(SocketManagerCommand::WaitForCompletion { response_sender });

        #[cfg(not(feature = "rt-tokio"))]
        let send_result = self
            .command_sender
            .send(SocketManagerCommand::WaitForCompletion { response_sender });

        send_result.map_err(|_| Error::SocketManagerChannelClosed)?;

        response_receiver.recv().await?
    }

    /// Send a WaitForCompletion command to the socket manager (for blocking mode).
    ///
    /// This returns the receiver that can be used to wait for the completion
    /// message with a timeout.
    #[allow(dead_code)]
    pub fn send_wait_for_completion(&self) -> Result<channels::OneshotReceiver<Result<()>>> {
        let (response_sender, response_receiver) = channels::oneshot();

        self.command_sender
            .send(SocketManagerCommand::WaitForCompletion { response_sender })
            .map_err(|_| Error::SocketManagerChannelClosed)?;

        Ok(response_receiver)
    }

    /// Shutdown the socket manager gracefully.
    /// This method sends a shutdown command to the actor and returns immediately.
    /// The actor will complete any in-flight commands before shutting down.
    pub async fn shutdown(&self) -> Result<()> {
        self.command_sender
            .send(SocketManagerCommand::Shutdown)
            .map_err(|_| Error::SocketManagerChannelClosed)
    }
}

/// Trait for handling retry decisions for commands.
/// This provides extensibility for automatic retry logic.
pub(crate) trait RetryHook {
    /// Called when a command fails with a potentially retryable error.
    /// Returns whether the command should be retried.
    fn should_retry(&self, error: &Error, attempt: u32) -> bool;

    /// Returns the delay before retrying the command.
    fn retry_delay(&self, error: &Error, attempt: u32) -> std::time::Duration;

    /// Returns the maximum number of retry attempts.
    fn max_attempts(&self) -> u32;
}

/// Default retry hook implementation that handles 0x41 "Not Executable" errors.
#[derive(Debug, Copy, Clone)]
pub(crate) struct DefaultRetryHook {
    max_attempts: u32,
    base_delay: std::time::Duration,
}

impl Default for DefaultRetryHook {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: std::time::Duration::from_millis(100),
        }
    }
}

impl DefaultRetryHook {
    /// Create a new default retry hook with standard settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of retry attempts.
    #[cfg(test)]
    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Set the base delay for exponential backoff.
    #[cfg(test)]
    pub fn with_base_delay(mut self, delay: std::time::Duration) -> Self {
        self.base_delay = delay;
        self
    }
}

impl RetryHook for DefaultRetryHook {
    fn should_retry(&self, error: &Error, attempt: u32) -> bool {
        if attempt >= self.max_attempts {
            return false;
        }

        // Check if this is a retryable error
        matches!(
            error,
            Error::CommandNotExecutable | Error::CommandBufferFull
        )
    }

    fn retry_delay(&self, _error: &Error, attempt: u32) -> std::time::Duration {
        // Exponential backoff with jitter
        let multiplier = 2_u32.pow(attempt);
        let delay = self.base_delay * multiplier;

        // Cap the delay at 5 seconds
        if delay > std::time::Duration::from_secs(5) {
            std::time::Duration::from_secs(5)
        } else {
            delay
        }
    }

    fn max_attempts(&self) -> u32 {
        self.max_attempts
    }
}

/// No-op retry hook that never retries.
#[cfg(test)]
#[derive(Debug, Copy, Clone)]
pub struct NoRetryHook;

#[cfg(test)]
impl RetryHook for NoRetryHook {
    fn should_retry(&self, _error: &Error, _attempt: u32) -> bool {
        false
    }

    fn retry_delay(&self, _error: &Error, _attempt: u32) -> std::time::Duration {
        std::time::Duration::from_secs(0)
    }

    fn max_attempts(&self) -> u32 {
        1
    }
}

/// The socket manager actor that runs the main event loop.
///
/// This actor manages the two-socket state machine, processes commands,
/// handles responses, and manages timeouts and retries.
///
/// The actor is generic over the transport type to allow both concrete
/// types (for performance) and boxed types (for flexibility).
pub(crate) struct SocketManagerActor<T> {
    inner: SocketManagerInner,
    transport: Arc<T>,
    command_receiver: UnboundedReceiver<SocketManagerCommand>,
    timeout_config: TimeoutConfig,
    runtime: crate::runtime::SharedRuntime,
    retry_hook: Box<dyn RetryHook + Send + Sync>,
}

impl<T> std::fmt::Debug for SocketManagerActor<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut builder = f.debug_struct("SocketManagerActor");
        builder
            .field("inner", &self.inner)
            .field("transport", &"Arc<T>")
            .field("timeout_config", &self.timeout_config)
            .field("runtime", &"SharedRuntime")
            .field("retry_hook", &"Box<dyn RetryHook>")
            .finish()
    }
}

impl<T> SocketManagerActor<T>
where
    T: AsyncTransport + Send + Sync + 'static,
{
    /// Create a new socket manager actor.
    pub fn new(
        transport: Arc<T>,
        command_receiver: UnboundedReceiver<SocketManagerCommand>,
        timeout_config: TimeoutConfig,
        camera_id: CameraId,
        runtime: crate::runtime::SharedRuntime,
    ) -> Self {
        let mut inner = SocketManagerInner::new();
        inner.camera_id = camera_id;
        Self {
            inner,
            transport,
            command_receiver,
            timeout_config,
            runtime,
            retry_hook: Box::new(DefaultRetryHook::new()),
        }
    }

    /// Run the socket manager actor event loop.
    ///
    /// This method runs until shutdown is requested or an error occurs.
    pub async fn run(mut self) -> Result<()> {
        debug!("Socket manager starting");

        #[cfg(not(feature = "rt-tokio"))]
        let mut empty_iterations = 0;

        loop {
            #[cfg(feature = "rt-tokio")]
            {
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
                            Some(SocketManagerCommand::WaitForCompletion {
                                response_sender,
                            }) => {
                                self.inner.completion_waiters.push_back(response_sender);
                                debug!("Added completion waiter, {} waiters now", self.inner.completion_waiters.len());
                            }
                            Some(SocketManagerCommand::Shutdown) => {
                                debug!("Socket manager received shutdown command");
                                break;
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
                                let err: Error = e;
                                warn!("Failed to receive response from transport: {err}");
                                // Continue processing other commands
                            }
                        }
                    }
                    _ = self.runtime.sleep(std::time::Duration::from_millis(100)) => {}
                }
            }

            #[cfg(not(feature = "rt-tokio"))]
            {
                self.check_timeouts().await;

                let mut activity = false;
                if let Some(cmd) = self.command_receiver.try_recv() {
                    activity = true;
                    empty_iterations = 0;
                    match cmd {
                        SocketManagerCommand::SendCommand {
                            bytes,
                            category,
                            is_inquiry,
                            response_sender,
                        } => {
                            self.handle_send_command(bytes, category, is_inquiry, response_sender)
                                .await;
                        }
                        SocketManagerCommand::WaitForCompletion { response_sender } => {
                            self.inner.completion_waiters.push_back(response_sender);
                            debug!(
                                "Added completion waiter, {} waiters now",
                                self.inner.completion_waiters.len()
                            );
                        }
                        SocketManagerCommand::Shutdown => {
                            debug!("Socket manager received shutdown command");
                            return Ok(());
                        }
                    }
                }

                if let Ok(bytes) = self.transport.recv().await {
                    activity = true;
                    empty_iterations = 0;
                    self.handle_raw_response(bytes).await;
                }

                if !activity {
                    empty_iterations += 1;
                    if empty_iterations > 1000 {
                        match self.command_receiver.recv().await {
                            Some(cmd) => {
                                empty_iterations = 0;
                                match cmd {
                                    SocketManagerCommand::SendCommand {
                                        bytes,
                                        category,
                                        is_inquiry,
                                        response_sender,
                                    } => {
                                        self.handle_send_command(
                                            bytes,
                                            category,
                                            is_inquiry,
                                            response_sender,
                                        )
                                        .await;
                                    }
                                    SocketManagerCommand::WaitForCompletion { response_sender } => {
                                        self.inner.completion_waiters.push_back(response_sender);
                                        debug!(
                                            "Added completion waiter, {} waiters now",
                                            self.inner.completion_waiters.len()
                                        );
                                    }
                                    SocketManagerCommand::Shutdown => {
                                        debug!("Socket manager received shutdown command");
                                        return Ok(());
                                    }
                                }
                            }
                            None => {
                                debug!("Socket manager command channel closed");
                                break;
                            }
                        }
                    }
                }

                // In non-tokio async mode, we should not be using thread::sleep
                // This should be handled differently in the async context
                // For now, we'll keep this as-is since this is the non-tokio branch
            }
        }

        debug!("Socket manager stopped");
        Ok(())
    }

    async fn handle_send_command(
        &mut self,
        bytes: Bytes,
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
                let err: Error = e;
                error!("Failed to send inquiry command {id}: {err}");
                pending_cmd.complete(Err(err));
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
                let err: Error = e;
                error!("Failed to send command {id} on {socket:?}: {err}");
                pending_cmd.complete(Err(err));
            }
        }
    }

    async fn handle_raw_response(&mut self, bytes: Bytes) {
        trace!("Handling raw response: {bytes:02X?}");

        if bytes.is_empty() {
            warn!("Empty response received");
            return;
        }

        match Response::parse(&bytes) {
            Ok(Response::CmdAck) => {
                // For ACK: 90 4y FF, extract socket from second byte
                if bytes.len() >= 2 {
                    let socket_num = bytes[1] & 0x0F;
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
                    let socket_num = bytes[1] & 0x0F;
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
                    let socket_num = bytes[1] & 0x0F;
                    if socket_num == 1 || socket_num == 2 {
                        let socket = if socket_num == 1 {
                            Socket::Socket1
                        } else {
                            Socket::Socket2
                        };
                        self.handle_error_response(socket, error).await;
                    } else if socket_num == 0 {
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
                warn!("Failed to parse response: {e:?}");
            }
        }
    }

    async fn handle_ack_response(&mut self, socket: Socket) {
        trace!("Received ACK for {socket:?}");

        // Check if socket is still busy with a command (not already timed out)
        if !self.inner.sockets[socket.as_index()].is_busy() {
            warn!("Received ACK for {socket:?} but socket is not busy (likely timed out)");
            return;
        }

        if let Some(command) = self.inner.get_active_command(socket) {
            debug!("ACK received for command {} on {socket:?}", command.id);
        } else {
            warn!("Received ACK for {socket:?} but no active command");
        }
    }

    async fn handle_completion_response(&mut self, socket: Socket) {
        trace!("Received completion for {socket:?}");

        // Notify any waiters for completion messages
        if let Some(waiter) = self.inner.completion_waiters.pop_front() {
            debug!("Notifying completion waiter");
            let _ = waiter.send(Ok(()));
        }

        // Check if socket is still busy with a command (not already timed out)
        if !self.inner.sockets[socket.as_index()].is_busy() {
            warn!("Received completion for {socket:?} but socket is not busy (likely timed out)");
            return;
        }

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
            // Mark socket free even if no command found
            self.inner.mark_socket_free(socket);
        }
    }

    async fn handle_error_response(&mut self, socket: Socket, error: Error) {
        trace!("Received error for {socket:?}: {error:?}");

        // Check if socket is still busy with a command (not already timed out)
        if !self.inner.sockets[socket.as_index()].is_busy() {
            warn!("Received error for {socket:?} but socket is not busy (likely timed out)");
            return;
        }

        if let Some(mut command) = self.inner.take_active_command(socket) {
            debug!(
                "Error received for command {} on {:?}: {:?}",
                command.id, socket, error
            );

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

                self.schedule_retry(command, delay).await;
            } else {
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
            // Mark socket free even if no command found
            self.inner.mark_socket_free(socket);
        }
    }

    async fn handle_inquiry_response(&mut self, data: crate::command::InquiryResponse) {
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

        for (socket_index, socket_state) in self.inner.sockets.iter().enumerate() {
            if socket_state.is_busy() {
                if let (Some(started_at), Some(category)) =
                    (socket_state.started_at(), socket_state.category())
                {
                    let timeout_duration = self.timeout_config.get_timeout(category);

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

        for socket in timed_out_sockets {
            let socket_state = &self.inner.sockets[socket.as_index()];
            if let Some(command_id) = socket_state.command_id() {
                warn!("Command {command_id} timeout on {socket:?}, sending cancel command");
            } else {
                warn!("Command timeout on {socket:?}, sending cancel command");
            }
            self.handle_command_timeout(socket).await;
        }

        if let Some(ref inquiry) = self.inner.pending_inquiry {
            // Use the appropriate timeout for inquiry commands
            let timeout_duration = self.timeout_config.get_timeout(inquiry.category);
            if now.duration_since(inquiry.enqueued_at) > timeout_duration {
                let id = inquiry.id;
                warn!("Inquiry command timeout for command {id}");
                self.handle_inquiry_timeout().await;
            }
        }
    }

    async fn handle_command_timeout(&mut self, socket: Socket) {
        let command_id = self.inner.sockets[socket.as_index()].command_id();

        // First, take the active command to prevent race conditions
        let command = self.inner.take_active_command(socket);

        // Mark socket as free immediately to prevent further operations
        self.inner.mark_socket_free(socket);

        // Send cancel command to ensure camera state is consistent
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
                // Send cancel command but don't wait for response to avoid further delays
                if let Err(e) = self.transport.send(&cancel_bytes).await {
                    let err: Error = e;
                    error!("Failed to send cancel command: {err}");
                }
            }
            Err(e) => {
                error!("Failed to encode cancel command: {e}");
            }
        }

        // Complete the command with timeout error
        if let Some(command) = command {
            let category = command.category;
            let timeout_duration = self.timeout_config.get_timeout(category);
            let timeout_error = Error::CommandTimeout {
                duration: timeout_duration,
                command: Cow::Owned(format!("Command {} on {socket:?}", command.id)),
            };
            command.complete(Err(timeout_error));
        }

        // Try to dispatch the next queued command
        self.try_dispatch_next_command().await;
    }

    async fn handle_inquiry_timeout(&mut self) {
        if let Some(inquiry) = self.inner.take_pending_inquiry() {
            let id = inquiry.id;
            let category = inquiry.category;
            let timeout_duration = self.timeout_config.get_timeout(category);
            let timeout_error = Error::CommandTimeout {
                duration: timeout_duration,
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

        self.runtime.sleep(delay).await;

        if command.is_inquiry {
            self.inner.set_pending_inquiry(command);
        } else {
            self.inner.enqueue_command(command);
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    #[cfg(feature = "rt-tokio")]
    use crate::command::const_encoding::VISCA_TERMINATOR;
    use crate::command::system::Socket;
    use crate::timeout::CommandCategory;
    use std::time::Duration;

    #[test]
    fn test_socket_state_creation() {
        let state = SocketState::Free;
        assert!(state.is_free());
        assert!(!state.is_busy());
        assert_eq!(state.started_at(), None);
        assert_eq!(state.command_id(), None);
        assert_eq!(state.category(), None);
    }

    #[test]
    fn test_socket_state_busy() {
        let now = Instant::now();
        let state = SocketState::Busy {
            command_id: 42,
            started_at: now,
            category: CommandCategory::Movement,
        };

        assert!(!state.is_free());
        assert!(state.is_busy());
        assert_eq!(state.started_at(), Some(now));
        assert_eq!(state.command_id(), Some(42));
        assert_eq!(state.category(), Some(CommandCategory::Movement));
    }

    #[test]
    fn test_socket_manager_inner_initialization() {
        let manager = SocketManagerInner::new();
        assert_eq!(manager.next_command_id, 1);
        assert!(manager.command_queue.is_empty());
        assert_eq!(manager.active_commands.len(), 2);
        assert!(manager.active_commands[0].is_none());
        assert!(manager.active_commands[1].is_none());
        assert!(manager.pending_inquiry.is_none());
    }

    #[test]
    fn test_socket_manager_free_socket_detection() {
        let manager = SocketManagerInner::new();
        assert_eq!(manager.get_free_socket(), Some(Socket::Socket1));

        let mut manager = SocketManagerInner::new();
        manager.mark_socket_busy(Socket::Socket1, 1, CommandCategory::Movement);
        assert_eq!(manager.get_free_socket(), Some(Socket::Socket2));

        manager.mark_socket_busy(Socket::Socket2, 2, CommandCategory::Movement);
        assert_eq!(manager.get_free_socket(), None);
    }

    #[test]
    fn test_socket_manager_command_id_generation() {
        let mut manager = SocketManagerInner::new();
        assert_eq!(manager.get_next_command_id(), 1);
        assert_eq!(manager.get_next_command_id(), 2);
        assert_eq!(manager.get_next_command_id(), 3);
    }

    #[test]
    fn test_socket_as_index() {
        assert_eq!(Socket::Socket1.as_index(), 0);
        assert_eq!(Socket::Socket2.as_index(), 1);
    }

    #[test]
    fn test_socket_manager_socket_state_tracking() {
        let mut manager = SocketManagerInner::new();

        assert!(manager.sockets[0].is_free());
        assert!(manager.sockets[1].is_free());

        manager.mark_socket_busy(Socket::Socket1, 42, CommandCategory::Movement);
        assert!(manager.sockets[0].is_busy());
        assert!(manager.sockets[1].is_free());

        manager.mark_socket_busy(Socket::Socket2, 43, CommandCategory::Quick);
        assert!(manager.sockets[0].is_busy());
        assert!(manager.sockets[1].is_busy());

        manager.mark_socket_free(Socket::Socket1);
        assert!(manager.sockets[0].is_free());
        assert!(manager.sockets[1].is_busy());

        manager.mark_socket_free(Socket::Socket2);
        assert!(manager.sockets[0].is_free());
        assert!(manager.sockets[1].is_free());
    }

    #[cfg(feature = "rt-tokio")]
    #[test]
    fn test_pending_cmd_creation() {
        let (tx, _rx) = channels::oneshot();
        let bytes = Bytes::from(vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]);
        let cmd = PendingCmd::new(1, bytes.clone(), CommandCategory::Movement, tx, false);

        assert_eq!(cmd.id, 1);
        assert_eq!(cmd.bytes, bytes);
        assert_eq!(cmd.category, CommandCategory::Movement);
        assert!(!cmd.is_inquiry);
    }

    #[test]
    fn test_command_category_default_timeouts() {
        assert_eq!(
            CommandCategory::Quick.default_timeout(),
            Duration::from_secs(5)
        );
        assert_eq!(
            CommandCategory::Movement.default_timeout(),
            Duration::from_secs(30)
        );
        assert_eq!(
            CommandCategory::Preset.default_timeout(),
            Duration::from_secs(90)
        );
        assert_eq!(
            CommandCategory::LongRunning.default_timeout(),
            Duration::from_secs(300)
        );
        assert_eq!(
            CommandCategory::Network.default_timeout(),
            Duration::from_secs(5)
        );
        assert_eq!(
            CommandCategory::Custom.default_timeout(),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn test_default_retry_hook() {
        let hook = DefaultRetryHook::new();

        // Should retry CommandNotExecutable errors
        assert!(hook.should_retry(&Error::CommandNotExecutable, 0));
        assert!(hook.should_retry(&Error::CommandNotExecutable, 1));
        assert!(hook.should_retry(&Error::CommandNotExecutable, 2));
        assert!(!hook.should_retry(&Error::CommandNotExecutable, 3));

        // Should retry CommandBufferFull errors
        assert!(hook.should_retry(&Error::CommandBufferFull, 0));
        assert!(hook.should_retry(&Error::CommandBufferFull, 2));
        assert!(!hook.should_retry(&Error::CommandBufferFull, 3));

        assert!(!hook.should_retry(&Error::CameraBusy, 0));
        assert!(!hook.should_retry(&Error::SocketManagerUnavailable, 0));

        let delay0 = hook.retry_delay(&Error::CommandNotExecutable, 0);
        let delay1 = hook.retry_delay(&Error::CommandNotExecutable, 1);
        let delay2 = hook.retry_delay(&Error::CommandNotExecutable, 2);

        assert!(delay1 > delay0);
        assert!(delay2 > delay1);
        assert!(delay2 <= Duration::from_secs(5));
    }

    #[test]
    fn test_no_retry_hook() {
        let hook = NoRetryHook;

        assert!(!hook.should_retry(&Error::CommandNotExecutable, 0));
        assert!(!hook.should_retry(&Error::CameraBusy, 0));

        assert_eq!(
            hook.retry_delay(&Error::CommandNotExecutable, 0),
            Duration::from_secs(0)
        );

        assert_eq!(hook.max_attempts(), 1);
    }

    #[test]
    fn test_default_retry_hook_configuration() {
        let hook = DefaultRetryHook::new()
            .with_max_attempts(5)
            .with_base_delay(Duration::from_millis(50));

        assert_eq!(hook.max_attempts(), 5);
        assert!(hook.should_retry(&Error::CommandNotExecutable, 4));
        assert!(!hook.should_retry(&Error::CommandNotExecutable, 5));

        let delay = hook.retry_delay(&Error::CommandNotExecutable, 0);
        assert_eq!(delay, Duration::from_millis(50));
    }

    #[test]
    fn test_pending_cmd_retry_tracking() {
        #[cfg(feature = "rt-tokio")]
        {
            let (tx, _rx) = channels::oneshot();
            let mut cmd = PendingCmd::new(
                1,
                Bytes::from(vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]),
                CommandCategory::Movement,
                tx,
                false,
            );

            assert_eq!(cmd.retry_attempt, 0);

            cmd.retry_attempt += 1;
            assert_eq!(cmd.retry_attempt, 1);
        }
    }

    #[test]
    fn test_socket_manager_basic_functionality() {
        let mut manager = SocketManagerInner::new();

        assert_eq!(manager.get_free_socket(), Some(Socket::Socket1));
        assert_eq!(manager.next_command_id, 1);

        let id1 = manager.get_next_command_id();
        let id2 = manager.get_next_command_id();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        manager.mark_socket_busy(Socket::Socket1, id1, CommandCategory::Movement);
        assert_eq!(manager.get_free_socket(), Some(Socket::Socket2));
        assert!(manager.sockets[0].is_busy());
        assert!(manager.sockets[1].is_free());

        manager.mark_socket_busy(Socket::Socket2, id2, CommandCategory::Quick);
        assert_eq!(manager.get_free_socket(), None);
        assert!(manager.sockets[0].is_busy());
        assert!(manager.sockets[1].is_busy());

        manager.mark_socket_free(Socket::Socket1);
        assert_eq!(manager.get_free_socket(), Some(Socket::Socket1));
        assert!(manager.sockets[0].is_free());
        assert!(manager.sockets[1].is_busy());
    }

    #[test]
    fn test_socket_state_properties() {
        let free_state = SocketState::Free;
        assert!(free_state.is_free());
        assert!(!free_state.is_busy());
        assert_eq!(free_state.command_id(), None);
        assert_eq!(free_state.started_at(), None);
        assert_eq!(free_state.category(), None);

        let busy_state = SocketState::Busy {
            command_id: 42,
            started_at: Instant::now(),
            category: CommandCategory::Movement,
        };
        assert!(!busy_state.is_free());
        assert!(busy_state.is_busy());
        assert_eq!(busy_state.command_id(), Some(42));
        assert!(busy_state.started_at().is_some());
        assert_eq!(busy_state.category(), Some(CommandCategory::Movement));
    }

    #[test]
    fn test_socket_response_byte_parsing() {
        use crate::command::system::Socket;

        assert_eq!(Socket::Socket1.as_index(), 0);
        assert_eq!(Socket::Socket2.as_index(), 1);
    }

    #[test]
    fn test_socket_manager_demonstrates_two_socket_tracking() {
        let mut manager = SocketManagerInner::new();

        let cmd1_id = manager.get_next_command_id();
        let socket1 = manager.get_free_socket();
        assert!(socket1.is_some(), "Test setup ensures this succeeds");
        let socket1 = socket1.expect("test setup ensures socket1 is available");
        assert_eq!(socket1, Socket::Socket1);
        manager.mark_socket_busy(socket1, cmd1_id, CommandCategory::Movement);

        let cmd2_id = manager.get_next_command_id();
        let socket2 = manager.get_free_socket();
        assert!(socket2.is_some(), "Test setup ensures this succeeds");
        let socket2 = socket2.expect("test setup ensures socket2 is available");
        assert_eq!(socket2, Socket::Socket2);
        manager.mark_socket_busy(socket2, cmd2_id, CommandCategory::Movement);

        assert_eq!(manager.get_free_socket(), None);
    }

    // ===== New Unit Tests for Socket Allocation Logic =====

    #[test]
    fn test_socket_allocation_prefers_socket1() {
        let manager = SocketManagerInner::new();

        // When both sockets are free, Socket1 should be preferred
        assert_eq!(manager.get_free_socket(), Some(Socket::Socket1));
    }

    #[test]
    fn test_socket_allocation_falls_back_to_socket2() {
        let mut manager = SocketManagerInner::new();

        // Mark Socket1 as busy
        manager.mark_socket_busy(Socket::Socket1, 1, CommandCategory::Quick);

        // Should allocate Socket2 when Socket1 is busy
        assert_eq!(manager.get_free_socket(), Some(Socket::Socket2));
    }

    #[test]
    fn test_socket_allocation_returns_none_when_all_busy() {
        let mut manager = SocketManagerInner::new();

        // Mark both sockets as busy
        manager.mark_socket_busy(Socket::Socket1, 1, CommandCategory::Quick);
        manager.mark_socket_busy(Socket::Socket2, 2, CommandCategory::Movement);

        // Should return None when both sockets are busy
        assert_eq!(manager.get_free_socket(), None);
    }

    #[test]
    fn test_socket_allocation_reuses_freed_sockets() {
        let mut manager = SocketManagerInner::new();

        // Allocate both sockets
        manager.mark_socket_busy(Socket::Socket1, 1, CommandCategory::Quick);
        manager.mark_socket_busy(Socket::Socket2, 2, CommandCategory::Movement);
        assert_eq!(manager.get_free_socket(), None);

        // Free Socket2
        manager.mark_socket_free(Socket::Socket2);

        // Socket1 is still busy, Socket2 should be available
        assert_eq!(manager.get_free_socket(), Some(Socket::Socket2));

        // Free Socket1
        manager.mark_socket_free(Socket::Socket1);

        // Now Socket1 should be preferred again
        assert_eq!(manager.get_free_socket(), Some(Socket::Socket1));
    }

    // ===== Unit Tests for Command Queue Management =====

    #[test]
    fn test_command_queue_enqueue_dequeue() {
        #[cfg(feature = "rt-tokio")]
        {
            let mut manager = SocketManagerInner::new();

            // Create test commands
            let (tx1, _rx1) = channels::oneshot();
            let cmd1 = PendingCmd::new(
                1,
                Bytes::from(vec![0x81, 0x01]),
                CommandCategory::Quick,
                tx1,
                false,
            );

            let (tx2, _rx2) = channels::oneshot();
            let cmd2 = PendingCmd::new(
                2,
                Bytes::from(vec![0x81, 0x02]),
                CommandCategory::Movement,
                tx2,
                false,
            );

            // Queue should start empty
            assert!(manager.command_queue.is_empty());
            assert!(manager.dequeue_command().is_none());

            // Enqueue commands
            manager.enqueue_command(cmd1);
            manager.enqueue_command(cmd2);

            // Queue should have 2 commands
            assert_eq!(manager.command_queue.len(), 2);

            // Dequeue should return commands in FIFO order
            let dequeued1 = manager.dequeue_command();
            assert!(dequeued1.is_some());
            assert_eq!(dequeued1.expect("should have command 1").id, 1);

            let dequeued2 = manager.dequeue_command();
            assert!(dequeued2.is_some());
            assert_eq!(dequeued2.expect("should have command 2").id, 2);

            // Queue should be empty again
            assert!(manager.command_queue.is_empty());
            assert!(manager.dequeue_command().is_none());
        }
    }

    #[test]
    fn test_command_queue_maintains_order() {
        #[cfg(feature = "rt-tokio")]
        {
            let mut manager = SocketManagerInner::new();

            // Enqueue multiple commands
            for i in 1..=10 {
                let (tx, _rx) = channels::oneshot();
                let cmd = PendingCmd::new(
                    i,
                    Bytes::from(vec![0x81, i as u8]),
                    CommandCategory::Quick,
                    tx,
                    false,
                );
                manager.enqueue_command(cmd);
            }

            // Verify FIFO order
            for expected_id in 1..=10 {
                let cmd = manager.dequeue_command();
                assert!(cmd.is_some());
                assert_eq!(cmd.expect("should have command in order").id, expected_id);
            }

            assert!(manager.dequeue_command().is_none());
        }
    }

    #[test]
    fn test_pending_inquiry_management() {
        #[cfg(feature = "rt-tokio")]
        {
            let mut manager = SocketManagerInner::new();

            // Initially no pending inquiry
            assert!(manager.pending_inquiry.is_none());

            // Set pending inquiry
            let (tx, _rx) = channels::oneshot();
            let inquiry = PendingCmd::new(
                1,
                Bytes::from(vec![0x81, 0x09]),
                CommandCategory::Quick,
                tx,
                true,
            );
            manager.set_pending_inquiry(inquiry);

            assert!(manager.pending_inquiry.is_some());

            // Take pending inquiry
            let taken = manager.take_pending_inquiry();
            assert!(taken.is_some());
            assert_eq!(taken.expect("should take pending inquiry").id, 1);
            assert!(manager.pending_inquiry.is_none());
        }
    }

    // ===== Unit Tests for Retry Logic =====

    #[test]
    fn test_retry_logic_with_exponential_backoff() {
        let hook = DefaultRetryHook::new();

        // Test exponential backoff calculation
        let delay0 = hook.retry_delay(&Error::CommandNotExecutable, 0);
        let delay1 = hook.retry_delay(&Error::CommandNotExecutable, 1);
        let delay2 = hook.retry_delay(&Error::CommandNotExecutable, 2);

        // Each delay should be exponentially larger
        assert_eq!(delay0, Duration::from_millis(100)); // base_delay
        assert_eq!(delay1, Duration::from_millis(200)); // base_delay * 2
        assert_eq!(delay2, Duration::from_millis(400)); // base_delay * 4
    }

    #[test]
    fn test_retry_logic_respects_max_attempts() {
        let hook = DefaultRetryHook::new(); // Default max_attempts is 3

        // Should retry for attempts 0, 1, 2
        assert!(hook.should_retry(&Error::CommandNotExecutable, 0));
        assert!(hook.should_retry(&Error::CommandNotExecutable, 1));
        assert!(hook.should_retry(&Error::CommandNotExecutable, 2));

        // Should not retry after max_attempts
        assert!(!hook.should_retry(&Error::CommandNotExecutable, 3));
        assert!(!hook.should_retry(&Error::CommandNotExecutable, 4));
    }

    #[test]
    fn test_retry_logic_selective_error_handling() {
        let hook = DefaultRetryHook::new();

        // Should retry these specific errors
        assert!(hook.should_retry(&Error::CommandNotExecutable, 0));
        assert!(hook.should_retry(&Error::CommandBufferFull, 0));

        // Should NOT retry these errors
        assert!(!hook.should_retry(&Error::CameraBusy, 0));
        assert!(!hook.should_retry(&Error::Timeout, 0));
        assert!(!hook.should_retry(&Error::SocketManagerUnavailable, 0));
        assert!(!hook.should_retry(&Error::InvalidResponseFormat, 0));
    }

    #[test]
    fn test_retry_logic_with_custom_configuration() {
        let hook = DefaultRetryHook::new()
            .with_max_attempts(5)
            .with_base_delay(Duration::from_millis(50));

        // Should allow more attempts
        assert!(hook.should_retry(&Error::CommandNotExecutable, 3));
        assert!(hook.should_retry(&Error::CommandNotExecutable, 4));
        assert!(!hook.should_retry(&Error::CommandNotExecutable, 5));

        // Should use custom base delay
        let delay0 = hook.retry_delay(&Error::CommandNotExecutable, 0);
        assert_eq!(delay0, Duration::from_millis(50));

        let delay1 = hook.retry_delay(&Error::CommandNotExecutable, 1);
        assert_eq!(delay1, Duration::from_millis(100)); // 50 * 2
    }

    #[test]
    fn test_retry_logic_max_delay_cap() {
        let hook = DefaultRetryHook::new();

        // Even with high attempt numbers, delay should be capped
        let delay10 = hook.retry_delay(&Error::CommandNotExecutable, 10);
        assert!(delay10 <= Duration::from_secs(5)); // max_delay is 5 seconds
    }

    #[test]
    fn test_command_id_wrapping() {
        let mut manager = SocketManagerInner::new();
        manager.next_command_id = u32::MAX;

        assert_eq!(manager.get_next_command_id(), u32::MAX);
        assert_eq!(manager.get_next_command_id(), 0); // Should wrap around
        assert_eq!(manager.get_next_command_id(), 1);
    }

    #[test]
    fn test_socket_state_transitions() {
        let mut manager = SocketManagerInner::new();

        // Initial state: both sockets free
        assert!(manager.sockets[0].is_free());
        assert!(manager.sockets[1].is_free());

        // Transition Socket1 to busy
        manager.mark_socket_busy(Socket::Socket1, 42, CommandCategory::Movement);
        assert!(manager.sockets[0].is_busy());
        assert_eq!(manager.sockets[0].command_id(), Some(42));

        // Transition Socket2 to busy
        manager.mark_socket_busy(Socket::Socket2, 43, CommandCategory::Quick);
        assert!(manager.sockets[1].is_busy());
        assert_eq!(manager.sockets[1].command_id(), Some(43));

        // Free Socket1
        manager.mark_socket_free(Socket::Socket1);
        assert!(manager.sockets[0].is_free());
        assert!(manager.active_commands[0].is_none());

        // Socket2 should still be busy
        assert!(manager.sockets[1].is_busy());
    }

    #[test]
    fn test_active_command_tracking() {
        #[cfg(feature = "rt-tokio")]
        {
            let mut manager = SocketManagerInner::new();

            // Create test commands
            let (tx1, _rx1) = channels::oneshot();
            let cmd1 = PendingCmd::new(
                1,
                Bytes::from(vec![0x81, 0x01]),
                CommandCategory::Quick,
                tx1,
                false,
            );

            let (tx2, _rx2) = channels::oneshot();
            let cmd2 = PendingCmd::new(
                2,
                Bytes::from(vec![0x81, 0x02]),
                CommandCategory::Movement,
                tx2,
                false,
            );

            // Set active commands
            manager.set_active_command(Socket::Socket1, cmd1);
            manager.set_active_command(Socket::Socket2, cmd2);

            // Verify commands are tracked
            assert!(manager.get_active_command(Socket::Socket1).is_some());
            assert_eq!(
                manager
                    .get_active_command(Socket::Socket1)
                    .expect("Socket1 should have active command")
                    .id,
                1
            );

            assert!(manager.get_active_command(Socket::Socket2).is_some());
            assert_eq!(
                manager
                    .get_active_command(Socket::Socket2)
                    .expect("Socket2 should have active command")
                    .id,
                2
            );

            // Take command from Socket1
            let taken = manager.take_active_command(Socket::Socket1);
            assert!(taken.is_some());
            assert_eq!(taken.expect("should take active command").id, 1);
            assert!(manager.get_active_command(Socket::Socket1).is_none());

            // Socket2 should still have its command
            assert!(manager.get_active_command(Socket::Socket2).is_some());
        }
    }
}
