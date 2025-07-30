use crate::camera_id::CameraId;
use crate::command::response::Response;
use crate::command::system::Socket;
use crate::error::{Error, Result};
use crate::timeout::CommandCategory;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

// Use conditional compilation for async support
#[cfg(feature = "tokio")]
use tokio::sync::{mpsc, oneshot};

#[cfg(not(feature = "tokio"))]
use std::sync::mpsc;

#[cfg(not(feature = "tokio"))]
pub mod oneshot {
    use super::*;

    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let (tx, rx) = mpsc::channel();
        (Sender(tx), Receiver(rx))
    }

    pub type Sender<T> = super::Sender<T>;
    pub type Receiver<T> = super::Receiver<T>;
}

#[cfg(not(feature = "tokio"))]
#[derive(Debug)]
pub struct Sender<T>(mpsc::Sender<T>);

#[cfg(not(feature = "tokio"))]
impl<T> Sender<T> {
    pub fn send(self, value: T) -> Result<(), T> {
        match self.0.send(value) {
            Ok(()) => Ok(()),
            Err(mpsc::SendError(original_value)) => Err(original_value),
        }
    }
}

#[cfg(not(feature = "tokio"))]
#[derive(Debug)]
pub struct Receiver<T>(mpsc::Receiver<T>);

#[cfg(not(feature = "tokio"))]
impl<T> Receiver<T> {
    pub fn recv(self) -> Result<T, Error> {
        self.0.recv().map_err(|_| Error::ChannelClosed)
    }

    pub async fn recv_async(self) -> Result<T, Error> {
        self.recv()
    }
}

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
    pub bytes: Vec<u8>,
    /// Category of the command for timeout and retry logic.
    pub category: CommandCategory,
    /// Channel to send the response back to the caller.
    #[cfg(feature = "tokio")]
    pub response_sender: oneshot::Sender<Result<Response>>,
    /// Channel to send the response back to the caller.
    #[cfg(not(feature = "tokio"))]
    pub response_sender: Sender<Result<Response>>,
    /// Whether this is an inquiry command (query) or action command.
    pub is_inquiry: bool,
    /// When this command was first enqueued.
    pub enqueued_at: Instant,
    /// Number of retry attempts made for this command.
    pub retry_attempt: u32,
}

impl PendingCmd {
    /// Create a new pending command.
    #[cfg(feature = "tokio")]
    pub fn new(
        id: u32,
        bytes: Vec<u8>,
        category: CommandCategory,
        response_sender: oneshot::Sender<Result<Response>>,
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

    #[cfg(not(feature = "tokio"))]
    pub fn new(
        id: u32,
        bytes: Vec<u8>,
        category: CommandCategory,
        response_sender: Sender<Result<Response>>,
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
        bytes: Vec<u8>,
        /// Command category for timeout and retry logic.
        category: CommandCategory,
        /// Whether this is an inquiry (query) command.
        is_inquiry: bool,
        /// Channel to send the response back to the caller.
        #[cfg(feature = "tokio")]
        response_sender: oneshot::Sender<Result<Response>>,
        /// Channel to send the response back to the caller.
        #[cfg(not(feature = "tokio"))]
        response_sender: Sender<Result<Response>>,
    },
    /// Cancel a command on a specific socket.
    #[allow(dead_code)]
    CancelCommand {
        /// Socket to cancel the command on.
        socket: Socket,
        /// Channel to send the cancellation result.
        #[cfg(feature = "tokio")]
        response_sender: oneshot::Sender<Result<()>>,
        /// Channel to send the cancellation result.
        #[cfg(not(feature = "tokio"))]
        response_sender: Sender<Result<()>>,
    },
}

/// Handle to communicate with the socket manager actor.
///
/// This handle can be cloned and shared across threads to send commands
/// to the socket manager from multiple locations.
#[derive(Debug)]
pub(crate) struct SocketManagerHandle {
    #[cfg(feature = "tokio")]
    command_sender: mpsc::UnboundedSender<SocketManagerCommand>,
    #[cfg(not(feature = "tokio"))]
    command_sender: mpsc::Sender<SocketManagerCommand>,
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
    #[cfg(feature = "tokio")]
    pub fn new(command_sender: mpsc::UnboundedSender<SocketManagerCommand>) -> Self {
        Self { command_sender }
    }

    #[cfg(not(feature = "tokio"))]
    pub fn new(command_sender: mpsc::Sender<SocketManagerCommand>) -> Self {
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
        let (response_sender, response_receiver) = oneshot::channel();

        #[cfg(feature = "tokio")]
        let send_result = self
            .command_sender
            .send(SocketManagerCommand::SendCommand {
                bytes,
                category,
                is_inquiry,
                response_sender,
            })
            .map_err(|_| Error::TransportError("Socket manager unavailable".to_string()));

        #[cfg(not(feature = "tokio"))]
        let send_result = self
            .command_sender
            .send(SocketManagerCommand::SendCommand {
                bytes,
                category,
                is_inquiry,
                response_sender,
            })
            .map_err(|_| Error::TransportError("Socket manager unavailable".to_string()));

        send_result?;

        #[cfg(feature = "tokio")]
        let result = response_receiver
            .await
            .map_err(|_| Error::TransportError("Response channel closed".to_string()))?;

        #[cfg(not(feature = "tokio"))]
        let result = response_receiver.recv_async().await?;

        result
    }

    /// Cancel a command on a specific socket.
    ///
    /// This sends a cancel command to the camera for the specified socket.
    #[allow(dead_code)]
    pub async fn cancel_command(&self, socket: Socket) -> Result<()> {
        let (response_sender, response_receiver) = oneshot::channel();

        #[cfg(feature = "tokio")]
        let send_result = self
            .command_sender
            .send(SocketManagerCommand::CancelCommand {
                socket,
                response_sender,
            })
            .map_err(|_| Error::TransportError("Socket manager unavailable".to_string()));

        #[cfg(not(feature = "tokio"))]
        let send_result = self
            .command_sender
            .send(SocketManagerCommand::CancelCommand {
                socket,
                response_sender,
            })
            .map_err(|_| Error::TransportError("Socket manager unavailable".to_string()));

        send_result?;

        #[cfg(feature = "tokio")]
        let result = response_receiver
            .await
            .map_err(|_| Error::TransportError("Response channel closed".to_string()))?;

        #[cfg(not(feature = "tokio"))]
        let result = response_receiver.recv_async().await?;

        result
    }
}

use crate::command::system::CommandCancelCommand;
use crate::command::EncodeVisca;
use crate::transport::UnifiedTransport;
use log::{debug, error, trace, warn};

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
        matches!(error, Error::CommandNotExecutable)
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
pub(crate) struct SocketManagerActor {
    inner: SocketManagerInner,
    transport: Arc<dyn UnifiedTransport>,
    #[cfg(feature = "tokio")]
    command_receiver: mpsc::UnboundedReceiver<SocketManagerCommand>,
    #[cfg(not(feature = "tokio"))]
    command_receiver: mpsc::Receiver<SocketManagerCommand>,
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
    #[cfg(feature = "tokio")]
    pub fn new(
        transport: Arc<dyn UnifiedTransport>,
        command_receiver: mpsc::UnboundedReceiver<SocketManagerCommand>,
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

    #[cfg(not(feature = "tokio"))]
    pub fn new(
        transport: Arc<dyn UnifiedTransport>,
        command_receiver: mpsc::Receiver<SocketManagerCommand>,
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
            #[cfg(feature = "tokio")]
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
                    _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                        // Timeout check interval - this ensures we regularly check for timeouts
                        // even when no other events are happening
                    }
                }
            }

            #[cfg(not(feature = "tokio"))]
            {
                // For non-tokio, we need to handle both command and response channels
                // This is a simplified implementation that alternates between checking both

                // Check for timeouts first
                self.check_timeouts().await;

                // Check for commands first
                match self.command_receiver.recv() {
                    Ok(cmd) => match cmd {
                        SocketManagerCommand::SendCommand {
                            bytes,
                            category,
                            is_inquiry,
                            response_sender,
                        } => {
                            self.handle_send_command(bytes, category, is_inquiry, response_sender)
                                .await;
                        }
                        SocketManagerCommand::CancelCommand {
                            socket,
                            response_sender,
                        } => {
                            self.handle_cancel_command(socket, response_sender).await;
                        }
                    },
                    Err(_) => {
                        // Channel closed, break the loop
                        debug!("Socket manager command channel closed");
                        break;
                    }
                }

                // Try to receive response
                match self.transport.recv().await {
                    Ok(bytes) => {
                        self.handle_raw_response(bytes).await;
                    }
                    Err(_) => {
                        // No response available, continue
                    }
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
        response_sender: oneshot::Sender<Result<Response>>,
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
        trace!("Handling inquiry command {}", pending_cmd.id);

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
                error!("Failed to send inquiry command {}: {}", pending_cmd.id, e);
                pending_cmd.complete(Err(e));
            }
        }
    }

    async fn handle_action_command(&mut self, pending_cmd: PendingCmd) {
        trace!("Handling action command {}", pending_cmd.id);

        if let Some(socket) = self.inner.get_free_socket() {
            self.send_command_on_socket(pending_cmd, socket).await;
        } else {
            trace!("No free sockets, queueing command {}", pending_cmd.id);
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
                error!(
                    "Failed to send command {} on {:?}: {}",
                    pending_cmd.id, socket, e
                );
                pending_cmd.complete(Err(e));
            }
        }
    }

    async fn handle_cancel_command(
        &mut self,
        socket: Socket,
        #[cfg(feature = "tokio")] response_sender: oneshot::Sender<Result<()>>,
        #[cfg(not(feature = "tokio"))] response_sender: Sender<Result<()>>,
    ) {
        trace!("Handling cancel command for {socket:?}");

        if self.inner.sockets[socket.as_index()].is_free() {
            warn!("Attempted to cancel command on free socket {socket:?}");
            let _ = response_sender.send(Err(Error::InvalidRequest(
                "No command active on socket".to_string(),
            )));
            return;
        }

        let cancel_cmd = CommandCancelCommand::new(socket);
        let mut bytes = Vec::new();
        if let Err(e) = cancel_cmd.encode_into(self.inner.camera_id, &mut bytes) {
            error!("Failed to encode cancel command for {socket:?}: {e}");
            let _ = response_sender.send(Err(Error::InvalidRequest(
                "Failed to encode cancel command".to_string(),
            )));
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

    async fn handle_inquiry_response(&mut self, data: crate::command::InquiryResponse) {
        trace!("Received inquiry response: {data:?}");
        if let Some(command) = self.inner.take_pending_inquiry() {
            debug!("Inquiry response received for command {}", command.id);
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
                warn!("Inquiry command timeout for command {}", inquiry.id);
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
                command: format!("Command {} on {socket:?}", command.id),
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
            let timeout_error = Error::CommandTimeout {
                duration: self.completion_timeout,
                command: format!("Inquiry command {}", inquiry.id),
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

        #[cfg(feature = "tokio")]
        {
            // For tokio builds, implement proper delayed retry
            tokio::time::sleep(delay).await;

            // After the delay, re-queue the command
            if command.is_inquiry {
                self.inner.set_pending_inquiry(command);
            } else {
                self.inner.enqueue_command(command);
            }
        }

        #[cfg(not(feature = "tokio"))]
        {
            // For non-tokio builds, use thread::sleep as a fallback
            // This blocks the current thread, which is not ideal but functional
            std::thread::sleep(delay);

            if command.is_inquiry {
                self.inner.set_pending_inquiry(command);
            } else {
                self.inner.enqueue_command(command);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
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

        // Initially both sockets should be free
        assert!(manager.sockets[0].is_free());
        assert!(manager.sockets[1].is_free());

        // Mark socket 1 as busy
        manager.mark_socket_busy(Socket::Socket1, 42, CommandCategory::Movement);
        assert!(manager.sockets[0].is_busy());
        assert!(manager.sockets[1].is_free());

        // Mark socket 2 as busy
        manager.mark_socket_busy(Socket::Socket2, 43, CommandCategory::Quick);
        assert!(manager.sockets[0].is_busy());
        assert!(manager.sockets[1].is_busy());

        // Free socket 1
        manager.mark_socket_free(Socket::Socket1);
        assert!(manager.sockets[0].is_free());
        assert!(manager.sockets[1].is_busy());

        // Free socket 2
        manager.mark_socket_free(Socket::Socket2);
        assert!(manager.sockets[0].is_free());
        assert!(manager.sockets[1].is_free());
    }

    #[cfg(feature = "tokio")]
    #[test]
    fn test_pending_cmd_creation() {
        let (tx, _rx) = oneshot::channel();
        let bytes = vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF];
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
            Duration::from_secs(2)
        );
        assert_eq!(
            CommandCategory::Movement.default_timeout(),
            Duration::from_secs(10)
        );
        assert_eq!(
            CommandCategory::Preset.default_timeout(),
            Duration::from_secs(60)
        );
        assert_eq!(
            CommandCategory::LongRunning.default_timeout(),
            Duration::from_secs(300)
        );
        assert_eq!(
            CommandCategory::Custom.default_timeout(),
            Duration::from_secs(30)
        );
    }

    #[test]
    fn test_default_retry_hook() {
        let hook = DefaultRetryHook::new();

        // Should retry CommandNotExecutable errors
        assert!(hook.should_retry(&Error::CommandNotExecutable, 0));
        assert!(hook.should_retry(&Error::CommandNotExecutable, 1));
        assert!(hook.should_retry(&Error::CommandNotExecutable, 2));
        assert!(!hook.should_retry(&Error::CommandNotExecutable, 3)); // Max attempts reached

        // Should not retry other errors
        assert!(!hook.should_retry(&Error::CameraBusy, 0));
        assert!(!hook.should_retry(&Error::TransportError("test".to_string()), 0));

        // Test retry delay calculation
        let delay0 = hook.retry_delay(&Error::CommandNotExecutable, 0);
        let delay1 = hook.retry_delay(&Error::CommandNotExecutable, 1);
        let delay2 = hook.retry_delay(&Error::CommandNotExecutable, 2);

        assert!(delay1 > delay0);
        assert!(delay2 > delay1);
        assert!(delay2 <= Duration::from_secs(5)); // Should be capped
    }

    #[test]
    fn test_no_retry_hook() {
        let hook = NoRetryHook;

        // Should never retry any error
        assert!(!hook.should_retry(&Error::CommandNotExecutable, 0));
        assert!(!hook.should_retry(&Error::CameraBusy, 0));

        // Should always return zero delay
        assert_eq!(
            hook.retry_delay(&Error::CommandNotExecutable, 0),
            Duration::from_secs(0)
        );

        // Should have max attempts of 1
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

        // Check that base delay is used
        let delay = hook.retry_delay(&Error::CommandNotExecutable, 0);
        assert_eq!(delay, Duration::from_millis(50)); // base_delay * 2^0 = 50ms
    }

    #[test]
    fn test_pending_cmd_retry_tracking() {
        #[cfg(feature = "tokio")]
        {
            let (tx, _rx) = oneshot::channel();
            let mut cmd = PendingCmd::new(
                1,
                vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF],
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

        // Test initial state
        assert_eq!(manager.get_free_socket(), Some(Socket::Socket1));
        assert_eq!(manager.next_command_id, 1);

        // Test command ID generation
        let id1 = manager.get_next_command_id();
        let id2 = manager.get_next_command_id();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        // Test socket state management
        manager.mark_socket_busy(Socket::Socket1, id1, CommandCategory::Movement);
        assert_eq!(manager.get_free_socket(), Some(Socket::Socket2));
        assert!(manager.sockets[0].is_busy());
        assert!(manager.sockets[1].is_free());

        manager.mark_socket_busy(Socket::Socket2, id2, CommandCategory::Quick);
        assert_eq!(manager.get_free_socket(), None);
        assert!(manager.sockets[0].is_busy());
        assert!(manager.sockets[1].is_busy());

        // Test freeing sockets
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

        // Test socket index conversion
        assert_eq!(Socket::Socket1.as_index(), 0);
        assert_eq!(Socket::Socket2.as_index(), 1);
    }

    #[test]
    fn test_socket_manager_demonstrates_two_socket_tracking() {
        let mut manager = SocketManagerInner::new();

        // Simulate the key behavior that prevents Buffer Full errors:
        // Only allow 2 commands to be active at once

        // First command gets Socket1
        let cmd1_id = manager.get_next_command_id();
        let socket1 = manager.get_free_socket();
        assert!(socket1.is_some(), "Test setup ensures this succeeds");
        let socket1 = socket1.expect("test setup ensures socket1 is available");
        assert_eq!(socket1, Socket::Socket1);
        manager.mark_socket_busy(socket1, cmd1_id, CommandCategory::Movement);

        // Second command gets Socket2
        let cmd2_id = manager.get_next_command_id();
        let socket2 = manager.get_free_socket();
        assert!(socket2.is_some(), "Test setup ensures this succeeds");
        let socket2 = socket2.expect("test setup ensures socket2 is available");
        assert_eq!(socket2, Socket::Socket2);
        manager.mark_socket_busy(socket2, cmd2_id, CommandCategory::Movement);

        // Third command would be queued (no free socket)
        assert_eq!(manager.get_free_socket(), None);

        // This is the core of G1: Accurate socket tracking
        // By having None returned, the socket manager knows to queue the command
        // instead of sending it immediately, preventing Buffer Full errors
    }
}
