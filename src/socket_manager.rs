use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use crate::command::response::Response;
use crate::command::system::Socket;
use crate::error::{Error, Result};
use crate::timeout::CommandCategory;

// Use conditional compilation for async support
#[cfg(feature = "tokio")]
use tokio::sync::{mpsc, oneshot, Mutex};

#[cfg(not(feature = "tokio"))]
use std::sync::Mutex;

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
    /// Convert a VISCA response byte to a Socket enum.
    ///
    /// This function maps the socket identifier bytes used in VISCA responses
    /// (0x90 for Socket1, 0x91 for Socket2) to the corresponding Socket enum variants.
    ///
    /// # Arguments
    /// * `byte` - The response byte from a VISCA message
    ///
    /// # Returns
    /// * `Some(Socket)` if the byte corresponds to a valid socket
    /// * `None` if the byte is not a recognized socket identifier
    pub fn from_response_byte(byte: u8) -> Option<Socket> {
        match byte {
            0x90 => Some(Socket::Socket1),
            0x91 => Some(Socket::Socket2),
            _ => None,
        }
    }
    
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
#[derive(Debug, Clone)]
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
    pub fn is_free(&self) -> bool {
        matches!(self, SocketState::Free)
    }

    pub fn is_busy(&self) -> bool {
        matches!(self, SocketState::Busy { .. })
    }

    pub fn started_at(&self) -> Option<Instant> {
        match self {
            SocketState::Free => None,
            SocketState::Busy { started_at, .. } => Some(*started_at),
        }
    }

    pub fn command_id(&self) -> Option<u32> {
        match self {
            SocketState::Free => None,
            SocketState::Busy { command_id, .. } => Some(*command_id),
        }
    }

    pub fn category(&self) -> Option<CommandCategory> {
        match self {
            SocketState::Free => None,
            SocketState::Busy { category, .. } => Some(*category),
        }
    }
}

#[derive(Debug)]
pub struct PendingCmd {
    pub id: u32,
    pub bytes: Vec<u8>,
    pub category: CommandCategory,
    #[cfg(feature = "tokio")]
    pub response_sender: oneshot::Sender<Result<Response>>,
    #[cfg(not(feature = "tokio"))]
    pub response_sender: Sender<Result<Response>>,
    pub is_inquiry: bool,
    pub enqueued_at: Instant,
    pub retry_attempt: u32,
}

impl PendingCmd {
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

    pub fn complete(self, result: Result<Response>) {
        let _ = self.response_sender.send(result);
    }
}

#[derive(Debug)]
pub struct SocketManagerInner {
    pub sockets: [SocketState; 2],
    pub command_queue: VecDeque<PendingCmd>,
    pub next_command_id: u32,
    pub active_commands: [Option<PendingCmd>; 2],
    pub pending_inquiry: Option<PendingCmd>,
}

impl SocketManagerInner {
    pub fn new() -> Self {
        Self {
            sockets: [SocketState::Free, SocketState::Free],
            command_queue: VecDeque::new(),
            next_command_id: 1,
            active_commands: [None, None],
            pending_inquiry: None,
        }
    }

    pub fn get_free_socket(&self) -> Option<Socket> {
        if self.sockets[0].is_free() {
            Some(Socket::Socket1)
        } else if self.sockets[1].is_free() {
            Some(Socket::Socket2)
        } else {
            None
        }
    }

    pub fn mark_socket_busy(&mut self, socket: Socket, command_id: u32, category: CommandCategory) {
        let index = socket.as_index();
        self.sockets[index] = SocketState::Busy {
            command_id,
            started_at: Instant::now(),
            category,
        };
    }

    pub fn mark_socket_free(&mut self, socket: Socket) {
        let index = socket.as_index();
        self.sockets[index] = SocketState::Free;
        self.active_commands[index] = None;
    }

    pub fn get_next_command_id(&mut self) -> u32 {
        let id = self.next_command_id;
        self.next_command_id = self.next_command_id.wrapping_add(1);
        id
    }

    pub fn enqueue_command(&mut self, command: PendingCmd) {
        self.command_queue.push_back(command);
    }

    pub fn dequeue_command(&mut self) -> Option<PendingCmd> {
        self.command_queue.pop_front()
    }

    pub fn set_active_command(&mut self, socket: Socket, command: PendingCmd) {
        let index = socket.as_index();
        self.active_commands[index] = Some(command);
    }

    pub fn take_active_command(&mut self, socket: Socket) -> Option<PendingCmd> {
        let index = socket.as_index();
        self.active_commands[index].take()
    }

    pub fn get_active_command(&self, socket: Socket) -> Option<&PendingCmd> {
        let index = socket.as_index();
        self.active_commands[index].as_ref()
    }

    pub fn set_pending_inquiry(&mut self, command: PendingCmd) {
        self.pending_inquiry = Some(command);
    }

    pub fn take_pending_inquiry(&mut self) -> Option<PendingCmd> {
        self.pending_inquiry.take()
    }
}


pub enum SocketManagerCommand {
    SendCommand {
        bytes: Vec<u8>,
        category: CommandCategory,
        is_inquiry: bool,
        #[cfg(feature = "tokio")]
        response_sender: oneshot::Sender<Result<Response>>,
        #[cfg(not(feature = "tokio"))]
        response_sender: Sender<Result<Response>>,
    },
    CancelCommand {
        socket: Socket,
        #[cfg(feature = "tokio")]
        response_sender: oneshot::Sender<Result<()>>,
        #[cfg(not(feature = "tokio"))]
        response_sender: Sender<Result<()>>,
    },
    HandleResponse {
        response: Response,
    },
    Shutdown,
}

pub struct SocketManagerHandle {
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
    #[cfg(feature = "tokio")]
    pub fn new(command_sender: mpsc::UnboundedSender<SocketManagerCommand>) -> Self {
        Self { command_sender }
    }
    
    #[cfg(not(feature = "tokio"))]
    pub fn new(command_sender: mpsc::Sender<SocketManagerCommand>) -> Self {
        Self { command_sender }
    }

    pub async fn send_command(
        &self,
        bytes: Vec<u8>,
        category: CommandCategory,
        is_inquiry: bool,
    ) -> Result<Response> {
        let (response_sender, response_receiver) = oneshot::channel();
        
        #[cfg(feature = "tokio")]
        let send_result = self.command_sender
            .send(SocketManagerCommand::SendCommand {
                bytes,
                category,
                is_inquiry,
                response_sender,
            })
            .map_err(|_| Error::TransportError("Socket manager unavailable".to_string()));
            
        #[cfg(not(feature = "tokio"))]
        let send_result = self.command_sender
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
        let result = response_receiver
            .recv_async()
            .await?;
            
        result
    }

    pub async fn cancel_command(&self, socket: Socket) -> Result<()> {
        let (response_sender, response_receiver) = oneshot::channel();
        
        #[cfg(feature = "tokio")]
        let send_result = self.command_sender
            .send(SocketManagerCommand::CancelCommand {
                socket,
                response_sender,
            })
            .map_err(|_| Error::TransportError("Socket manager unavailable".to_string()));
            
        #[cfg(not(feature = "tokio"))]
        let send_result = self.command_sender
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
        let result = response_receiver
            .recv_async()
            .await?;
            
        result
    }

    pub fn handle_response(&self, response: Response) -> Result<()> {
        #[cfg(feature = "tokio")]
        let send_result = self.command_sender
            .send(SocketManagerCommand::HandleResponse { response })
            .map_err(|_| Error::TransportError("Socket manager unavailable".to_string()));
            
        #[cfg(not(feature = "tokio"))]
        let send_result = self.command_sender
            .send(SocketManagerCommand::HandleResponse { response })
            .map_err(|_| Error::TransportError("Socket manager unavailable".to_string()));
            
        send_result?;
        Ok(())
    }

    pub fn shutdown(&self) -> Result<()> {
        #[cfg(feature = "tokio")]
        let send_result = self.command_sender
            .send(SocketManagerCommand::Shutdown)
            .map_err(|_| Error::TransportError("Socket manager unavailable".to_string()));
            
        #[cfg(not(feature = "tokio"))]
        let send_result = self.command_sender
            .send(SocketManagerCommand::Shutdown)
            .map_err(|_| Error::TransportError("Socket manager unavailable".to_string()));
            
        send_result?;
        Ok(())
    }
}

use crate::command::system::CommandCancelCommand;
use crate::command::EncodeVisca;
use crate::camera::UnifiedTransport;
use log::{debug, error, trace, warn};

/// Trait for handling retry decisions for commands.
/// This provides extensibility for automatic retry logic.
pub trait RetryHook {
    /// Called when a command fails with a potentially retryable error.
    /// Returns whether the command should be retried.
    fn should_retry(&self, error: &Error, attempt: u32) -> bool;
    
    /// Returns the delay before retrying the command.
    fn retry_delay(&self, error: &Error, attempt: u32) -> std::time::Duration;
    
    /// Returns the maximum number of retry attempts.
    fn max_attempts(&self) -> u32;
}

/// Default retry hook implementation that handles 0x41 "Not Executable" errors.
pub struct DefaultRetryHook {
    max_attempts: u32,
    base_delay: std::time::Duration,
}

impl DefaultRetryHook {
    pub fn new() -> Self {
        Self {
            max_attempts: 3,
            base_delay: std::time::Duration::from_millis(100),
        }
    }
    
    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = max_attempts;
        self
    }
    
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
pub struct NoRetryHook;

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

pub struct SocketManagerActor {
    inner: SocketManagerInner,
    transport: Arc<dyn UnifiedTransport>,
    #[cfg(feature = "tokio")]
    command_receiver: mpsc::UnboundedReceiver<SocketManagerCommand>,
    #[cfg(not(feature = "tokio"))]
    command_receiver: mpsc::Receiver<SocketManagerCommand>,
    profile: crate::camera::DynamicProfile,
    retry_hook: Box<dyn RetryHook + Send + Sync>,
}

impl SocketManagerActor {
    #[cfg(feature = "tokio")]
    pub fn new(
        transport: Arc<dyn UnifiedTransport>,
        command_receiver: mpsc::UnboundedReceiver<SocketManagerCommand>,
        profile: crate::camera::DynamicProfile,
    ) -> Self {
        Self {
            inner: SocketManagerInner::new(),
            transport,
            command_receiver,
            profile,
            retry_hook: Box::new(DefaultRetryHook::new()),
        }
    }
    
    #[cfg(not(feature = "tokio"))]
    pub fn new(
        transport: Arc<dyn UnifiedTransport>,
        command_receiver: mpsc::Receiver<SocketManagerCommand>,
        profile: crate::camera::DynamicProfile,
    ) -> Self {
        Self {
            inner: SocketManagerInner::new(),
            transport,
            command_receiver,
            profile,
            retry_hook: Box::new(DefaultRetryHook::new()),
        }
    }
    
    /// Set a custom retry hook for this socket manager.
    pub fn with_retry_hook(mut self, retry_hook: Box<dyn RetryHook + Send + Sync>) -> Self {
        self.retry_hook = retry_hook;
        self
    }

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
                            Some(SocketManagerCommand::HandleResponse { response }) => {
                                self.handle_response(response).await;
                            }
                            Some(SocketManagerCommand::Shutdown) => {
                                debug!("Socket manager shutting down");
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
                                warn!("Failed to receive response from transport: {}", e);
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
                let command = self.command_receiver.recv().ok();
                if let Some(cmd) = command {
                    match cmd {
                        SocketManagerCommand::SendCommand {
                            bytes,
                            category,
                            is_inquiry,
                            response_sender,
                        } => {
                            self.handle_send_command(bytes, category, is_inquiry, response_sender).await;
                        }
                        SocketManagerCommand::CancelCommand {
                            socket,
                            response_sender,
                        } => {
                            self.handle_cancel_command(socket, response_sender).await;
                        }
                        SocketManagerCommand::HandleResponse { response } => {
                            self.handle_response(response).await;
                        }
                        SocketManagerCommand::Shutdown => {
                            debug!("Socket manager shutting down");
                            break;
                        }
                    }
                } else {
                    // If no command, try to receive response
                    // Note: This is a simplified approach and may not be optimal
                    // In a real implementation, we'd want proper event-driven handling
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
        trace!("Sending command {} on {:?}", pending_cmd.id, socket);
        
        match self.transport.send(&pending_cmd.bytes).await {
            Ok(()) => {
                trace!("Command {} sent successfully on {:?}", pending_cmd.id, socket);
                self.inner.mark_socket_busy(socket, pending_cmd.id, pending_cmd.category);
                self.inner.set_active_command(socket, pending_cmd);
            }
            Err(e) => {
                error!("Failed to send command {} on {:?}: {}", pending_cmd.id, socket, e);
                pending_cmd.complete(Err(e));
            }
        }
    }

    async fn handle_cancel_command(
        &mut self,
        socket: Socket,
        #[cfg(feature = "tokio")]
        response_sender: oneshot::Sender<Result<()>>,
        #[cfg(not(feature = "tokio"))]
        response_sender: Sender<Result<()>>,
    ) {
        trace!("Handling cancel command for {:?}", socket);
        
        if self.inner.sockets[socket.as_index()].is_free() {
            warn!("Attempted to cancel command on free socket {:?}", socket);
            let _ = response_sender.send(Err(Error::InvalidRequest("No command active on socket".to_string())));
            return;
        }

        let cancel_cmd = CommandCancelCommand::new(socket);
        let mut bytes = Vec::new();
        if let Err(e) = cancel_cmd.encode_into(&mut bytes) {
            error!("Failed to encode cancel command for {:?}: {}", socket, e);
            let _ = response_sender.send(Err(Error::InvalidRequest("Failed to encode cancel command".to_string())));
            return;
        }

        match self.transport.send(&bytes).await {
            Ok(()) => {
                trace!("Cancel command sent for {:?}", socket);
                let _ = response_sender.send(Ok(()));
            }
            Err(e) => {
                error!("Failed to send cancel command for {:?}: {}", socket, e);
                let _ = response_sender.send(Err(e));
            }
        }
    }

    async fn handle_response(&mut self, response: Response) {
        trace!("Handling response: {:?}", response);
        
        match response {
            Response::CmdAck => {
                warn!("Received CmdAck without socket information");
            }
            Response::Completion => {
                warn!("Received Completion without socket information");
            }
            Response::Error(error) => {
                warn!("Received Error without socket information: {:?}", error);
            }
            Response::InquiryResponse(data) => {
                self.handle_inquiry_response(data).await;
            }
            _ => {
                warn!("Unhandled response type: {:?}", response);
            }
        }
    }
    
    async fn handle_raw_response(&mut self, bytes: bytes::Bytes) {
        trace!("Handling raw response: {:02X?}", bytes);
        
        if bytes.is_empty() {
            warn!("Empty response received");
            return;
        }
        
        // Extract socket information from raw bytes
        let socket_byte = bytes[0];
        let socket = Socket::from_response_byte(socket_byte);
        
        // Parse the response
        match Response::parse(&bytes) {
            Ok(Response::CmdAck) => {
                if let Some(socket) = socket {
                    self.handle_ack_response(socket).await;
                } else {
                    warn!("Invalid socket byte in ACK response: 0x{:02x}", socket_byte);
                }
            }
            Ok(Response::Completion) => {
                if let Some(socket) = socket {
                    self.handle_completion_response(socket).await;
                } else {
                    warn!("Invalid socket byte in completion response: 0x{:02x}", socket_byte);
                }
            }
            Ok(Response::Error(error)) => {
                if let Some(socket) = socket {
                    self.handle_error_response(socket, error).await;
                } else {
                    warn!("Invalid socket byte in error response: 0x{:02x}", socket_byte);
                }
            }
            Ok(Response::InquiryResponse(data)) => {
                self.handle_inquiry_response(data).await;
            }
            Ok(other) => {
                warn!("Unhandled response type: {:?}", other);
            }
            Err(e) => {
                // Try to parse as inquiry response
                warn!("Failed to parse response: {:?}", e);
            }
        }
    }

    async fn handle_ack_response(&mut self, socket: Socket) {
        trace!("Received ACK for {:?}", socket);
        if let Some(command) = self.inner.get_active_command(socket) {
            debug!("ACK received for command {} on {:?}", command.id, socket);
        } else {
            warn!("Received ACK for {:?} but no active command", socket);
        }
    }

    async fn handle_completion_response(&mut self, socket: Socket) {
        trace!("Received completion for {:?}", socket);
        if let Some(command) = self.inner.take_active_command(socket) {
            debug!("Completion received for command {} on {:?}", command.id, socket);
            command.complete(Ok(Response::Completion));
            self.inner.mark_socket_free(socket);
            self.try_dispatch_next_command().await;
        } else {
            warn!("Received completion for {:?} but no active command", socket);
        }
    }

    async fn handle_error_response(&mut self, socket: Socket, error: Error) {
        trace!("Received error for {:?}: {:?}", socket, error);
        if let Some(mut command) = self.inner.take_active_command(socket) {
            debug!("Error received for command {} on {:?}: {:?}", command.id, socket, error);
            
            // Check if we should retry this command
            if self.retry_hook.should_retry(&error, command.retry_attempt) {
                command.retry_attempt += 1;
                let delay = self.retry_hook.retry_delay(&error, command.retry_attempt - 1);
                let max_attempts = self.retry_hook.max_attempts();
                
                debug!("Retrying command {} (attempt {}/{}) after {:?}", 
                       command.id, command.retry_attempt, max_attempts, delay);
                
                // Schedule the retry
                self.schedule_retry(command, delay).await;
            } else {
                // No retry, complete with error
                debug!("Command {} exceeded max retry attempts, failing with error: {:?}", 
                       command.id, error);
                command.complete(Err(error));
            }
            
            self.inner.mark_socket_free(socket);
            self.try_dispatch_next_command().await;
        } else {
            warn!("Received error for {:?} but no active command", socket);
        }
    }

    async fn handle_inquiry_response(&mut self, data: crate::command::InquiryResponse) {
        trace!("Received inquiry response: {:?}", data);
        if let Some(command) = self.inner.take_pending_inquiry() {
            debug!("Inquiry response received for command {}", command.id);
            command.complete(Ok(Response::InquiryResponse(data)));
        } else {
            warn!("Received inquiry response but no pending inquiry");
        }
    }

    async fn try_dispatch_next_command(&mut self) {
        if let Some(socket) = self.inner.get_free_socket() {
            if let Some(command) = self.inner.dequeue_command() {
                trace!("Dispatching queued command {} on {:?}", command.id, socket);
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
                if let (Some(started_at), Some(category)) = (socket_state.started_at(), socket_state.category()) {
                    let timeout_duration = match category {
                        CommandCategory::Quick => self.profile.ack_timeout(),
                        CommandCategory::Movement => self.profile.completion_timeout(),
                        CommandCategory::Preset => self.profile.completion_timeout(),
                        CommandCategory::LongRunning => self.profile.completion_timeout(),
                        CommandCategory::Custom => self.profile.completion_timeout(),
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
                warn!("Command {} timeout on {:?}, sending cancel command", command_id, socket);
            } else {
                warn!("Command timeout on {:?}, sending cancel command", socket);
            }
            self.handle_command_timeout(socket).await;
        }
        
        // Check for inquiry timeout
        if let Some(ref inquiry) = self.inner.pending_inquiry {
            let timeout_duration = self.profile.completion_timeout();
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
            debug!("Sending cancel command for timed out command {} on socket {:?}", cmd_id, socket);
        } else {
            debug!("Sending cancel command for timed out socket {:?}", socket);
        }
        match cancel_command.encode_into(&mut cancel_bytes) {
            Ok(size) => {
                cancel_bytes.truncate(size);
                if let Err(e) = self.transport.send(&cancel_bytes).await {
                    error!("Failed to send cancel command: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to encode cancel command: {}", e);
            }
        }
        
        // Complete the active command with a timeout error
        if let Some(command) = self.inner.take_active_command(socket) {
            let timeout_error = Error::CommandTimeout {
                duration: self.profile.completion_timeout(),
                command: format!("Command {} on {:?}", command.id, socket),
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
                duration: self.profile.completion_timeout(),
                command: format!("Inquiry command {}", inquiry.id),
            };
            inquiry.complete(Err(timeout_error));
        }
    }
    
    async fn schedule_retry(&mut self, command: PendingCmd, delay: std::time::Duration) {
        log::debug!(
            "Scheduling retry for command {} (attempt {}) with delay {:?}",
            command.id, command.retry_attempt, delay
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
    fn test_socket_from_response_byte() {
        assert_eq!(Socket::from_response_byte(0x90), Some(Socket::Socket1));
        assert_eq!(Socket::from_response_byte(0x91), Some(Socket::Socket2));
        assert_eq!(Socket::from_response_byte(0x92), None);
        assert_eq!(Socket::from_response_byte(0x80), None);
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
        let cmd = PendingCmd::new(
            1,
            bytes.clone(),
            CommandCategory::Movement,
            tx,
            false,
        );
        
        assert_eq!(cmd.id, 1);
        assert_eq!(cmd.bytes, bytes);
        assert_eq!(cmd.category, CommandCategory::Movement);
        assert!(!cmd.is_inquiry);
    }

    #[test]
    fn test_command_category_default_timeouts() {
        assert_eq!(CommandCategory::Quick.default_timeout(), Duration::from_secs(2));
        assert_eq!(CommandCategory::Movement.default_timeout(), Duration::from_secs(10));
        assert_eq!(CommandCategory::Preset.default_timeout(), Duration::from_secs(60));
        assert_eq!(CommandCategory::LongRunning.default_timeout(), Duration::from_secs(300));
        assert_eq!(CommandCategory::Custom.default_timeout(), Duration::from_secs(30));
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
        assert_eq!(hook.retry_delay(&Error::CommandNotExecutable, 0), Duration::from_secs(0));
        
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
}