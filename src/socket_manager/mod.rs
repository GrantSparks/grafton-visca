//! Socket manager module for managing VISCA command execution over two sockets.
//!
//! This module provides a socket manager that handles the VISCA two-socket protocol,
//! managing command queueing, timeouts, retries, and responses.

use crate::camera_id::CameraId;
use crate::channels::OneshotSender;
use crate::command::response::Response;
use crate::command::system::Socket;
use crate::error::Result;
use crate::timeout::CommandCategory;
use std::collections::VecDeque;
use std::time::Instant;

// Re-export the appropriate implementation based on features
#[cfg(feature = "tokio")]
mod tokio;
#[cfg(feature = "tokio")]
pub(crate) use self::tokio::*;

// Common types that are shared between implementations

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
        bytes: Vec<u8>,
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
        response_sender: OneshotSender<Result<Response>>,
    },
    /// Cancel a command on a specific socket.
    #[allow(dead_code)] // Future feature for command cancellation
    CancelCommand {
        /// Socket to cancel the command on.
        socket: Socket,
        /// Channel to send the cancellation result.
        response_sender: OneshotSender<Result<()>>,
    },
}

use crate::error::Error;

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
        let (tx, _rx) = crate::channels::oneshot();
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
        assert!(!hook.should_retry(
            &Error::TransportError(std::borrow::Cow::Borrowed("test")),
            0
        ));

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
            let (tx, _rx) = crate::channels::oneshot();
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
