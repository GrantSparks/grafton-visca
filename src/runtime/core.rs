//! Runtime-agnostic scheduler core state machine.
//!
//! This module implements the protocol state machine for VISCA command scheduling,
//! socket allocation, ACK/completion routing, and retry logic without any dependency
//! on async runtimes or channels.

use tracing::{debug, trace, warn};

use std::{
    cmp::Ordering as CmpOrdering,
    collections::{BinaryHeap, HashMap},
    time::{Duration, Instant},
};

use crate::{
    command::{response::ViscaResponse, CommandKind},
    timeout::{CommandCategory, TimeoutConfig},
    visca_socket::ViscaSocket,
    Error,
};

/// Command priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Low priority - normal operations.
    Low = 0,
    /// Normal priority - default.
    Normal = 1,
    /// High priority - user-initiated actions.
    High = 2,
    /// Critical priority - emergency/safety operations.
    Critical = 3,
}

/// VISCA protocol error codes wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ViscaError(u8);

impl ViscaError {
    /// Create from VISCA error byte.
    pub fn from_byte(byte: u8) -> Self {
        ViscaError(byte)
    }

    /// Check if this error should trigger a retry.
    pub fn is_retryable(&self, category: Option<CommandCategory>) -> bool {
        // Convert to public error type to check retryability
        let error = Error::from_code(self.0);

        // Check base retryability from Error type
        if error.is_retryable() {
            return true;
        }

        // Special case: 0x41 (CommandNotExecutable) may be retryable for movement/preset
        if self.0 == 0x41 {
            matches!(
                category,
                Some(CommandCategory::Movement | CommandCategory::Preset)
            )
        } else {
            false
        }
    }
}

/// Command waiting to be retried.
#[derive(Debug, Clone)]
pub struct RetryCommand {
    /// Command ID.
    pub id: u32,
    /// Command bytes.
    pub bytes: bytes::Bytes,
    /// Command priority.
    pub priority: Priority,
    /// Command category.
    pub category: CommandCategory,
    /// Camera ID used to encode the command.
    pub camera_id: crate::camera_id::CameraId,
    /// Retry attempt number.
    pub attempt: u32,
    /// Maximum retries allowed.
    pub max_retries: u32,
    /// When to retry this command (for exponential backoff).
    pub retry_at: Instant,
}

/// Socket state tracking.
#[derive(Debug, Clone)]
struct SocketState {
    /// Whether socket is free.
    free: bool,
    /// Current command ID if busy.
    command_id: Option<u32>,
    /// When command started.
    started_at: Option<Instant>,
    /// Command category for timeout.
    category: Option<CommandCategory>,
}

impl Default for SocketState {
    fn default() -> Self {
        Self {
            free: true,
            command_id: None,
            started_at: None,
            category: None,
        }
    }
}

/// Priority queue item wrapper for commands.
#[derive(Debug, Clone)]
pub struct PendingCommand {
    /// Unique identifier for this command.
    pub id: u32,
    /// Raw VISCA bytes to send.
    pub bytes: bytes::Bytes,
    /// Priority level for scheduling.
    pub priority: Priority,
    /// Category for timeout calculation.
    pub category: CommandCategory,
    /// Camera ID used to encode the command.
    pub camera_id: crate::camera_id::CameraId,
    /// When the command was submitted.
    pub submitted_at: Instant,
    /// Command kind (Command or Inquiry).
    pub kind: CommandKind,
}

impl PartialEq for PendingCommand {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.submitted_at == other.submitted_at
    }
}

impl Eq for PendingCommand {}

impl PartialOrd for PendingCommand {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

impl Ord for PendingCommand {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        // First compare by priority (higher priority first)
        match self.priority.cmp(&other.priority) {
            CmpOrdering::Equal => {
                // For same priority, use FIFO (earlier submission time first)
                // Note: reverse comparison for submission time
                other.submitted_at.cmp(&self.submitted_at)
            }
            other => other,
        }
    }
}

/// Actions that the scheduler core can request.
#[derive(Debug)]
pub enum SchedulerAction {
    /// Send a command.
    SendCommand {
        /// Command ID.
        id: u32,
        /// Command bytes to send.
        bytes: bytes::Bytes,
    },
    /// Command completed successfully.
    CommandComplete {
        /// Command ID.
        id: u32,
        /// Response from the camera.
        response: ViscaResponse,
    },
    /// Command failed with error.
    CommandFailed {
        /// Command ID.
        id: u32,
        /// Error that occurred.
        error: Error,
    },
    /// Retry a command.
    RetryCommand {
        /// Command ID.
        id: u32,
        /// Command bytes to send.
        bytes: bytes::Bytes,
        /// Delay before retrying.
        delay: Duration,
    },
}

/// Events that can be fed to the scheduler core.
#[derive(Debug)]
pub enum SchedulerEvent {
    /// ACK received for a socket.
    Ack {
        /// Socket that was acknowledged.
        socket: ViscaSocket,
    },
    /// Command completed.
    Completion {
        /// Socket that completed (if known).
        socket: Option<ViscaSocket>,
        /// Response from the camera.
        response: ViscaResponse,
    },
    /// Error response.
    Error {
        /// Socket that errored (if known).
        socket: Option<ViscaSocket>,
        /// Error code from the camera.
        code: u8,
    },
    /// Network error (for broadcast recovery).
    NetworkError,
}

/// Runtime-agnostic scheduler core.
///
/// This contains all the state machine logic without any async dependencies.
#[derive(Debug)]
pub struct SchedulerCore {
    /// Socket states.
    sockets: [SocketState; 2],
    /// Timeout configuration.
    timeout_config: TimeoutConfig,
    /// Retry configuration.
    retry_config: crate::transport::RetryConfig,
    /// Commands that have been sent but not yet acknowledged.
    /// Maps command ID to (bytes, priority, category, sent_time, camera_id).
    pending_ack: HashMap<
        u32,
        (
            bytes::Bytes,
            Priority,
            CommandCategory,
            Instant,
            crate::camera_id::CameraId,
        ),
    >,
    /// Commands waiting to be retried (after busy response).
    pub retry_queue: Vec<RetryCommand>,
    /// Store command metadata for potential retry.
    command_metadata: HashMap<
        u32,
        (
            bytes::Bytes,
            Priority,
            CommandCategory,
            crate::camera_id::CameraId,
        ),
    >,
    /// Track retry attempts for commands (command_id -> attempt_count).
    retry_attempts: HashMap<u32, u32>,
    /// Priority queue for pending commands.
    command_queue: BinaryHeap<PendingCommand>,
    /// Maximum retries per command category.
    max_retries_per_category: HashMap<CommandCategory, u32>,
    /// Sony sequence tracking: sequence -> command_id.
    pending_by_sequence: HashMap<u32, u32>,
    /// Sony sequence tracking: command_id -> sequence.
    sequence_by_command: HashMap<u32, u32>,
}

impl SchedulerCore {
    /// Create a new scheduler core with the given timeout configuration.
    pub fn new(timeout_config: TimeoutConfig) -> Self {
        // Use default retry config
        Self::with_retry_config(timeout_config, crate::transport::RetryConfig::default())
    }

    /// Create a new scheduler core with the given timeout and retry configuration.
    pub fn with_retry_config(
        timeout_config: TimeoutConfig,
        retry_config: crate::transport::RetryConfig,
    ) -> Self {
        // Set max retries per category based on retry config
        let mut max_retries = HashMap::new();
        // Scale the category-specific retries based on the overall max_retries setting
        let base_retries = retry_config.max_retries;
        max_retries.insert(CommandCategory::Quick, base_retries.max(1) + 2); // Quick commands get extra retries
        max_retries.insert(CommandCategory::Movement, base_retries);
        max_retries.insert(CommandCategory::Preset, base_retries);
        max_retries.insert(
            CommandCategory::Network,
            base_retries.saturating_sub(1).max(1),
        );
        max_retries.insert(CommandCategory::LongRunning, 1); // Long running always get minimal retries
        max_retries.insert(CommandCategory::Custom, base_retries);

        Self {
            sockets: Default::default(),
            timeout_config,
            retry_config,
            pending_ack: HashMap::new(),
            retry_queue: Vec::new(),
            command_metadata: HashMap::new(),
            retry_attempts: HashMap::new(),
            command_queue: BinaryHeap::new(),
            max_retries_per_category: max_retries,
            pending_by_sequence: HashMap::new(),
            sequence_by_command: HashMap::new(),
        }
    }

    /// Queue a command for execution.
    pub fn queue_command(&mut self, command: PendingCommand) {
        self.command_queue.push(command);
    }

    /// Check if we can send another command (have room for pending ACK).
    pub fn can_send_command(&self) -> bool {
        // Count commands that are either pending ACK or have a socket allocated
        let pending_count = self.pending_ack.len();
        let allocated_count = self.sockets.iter().filter(|s| !s.free).count();
        let total_in_flight = pending_count + allocated_count;

        debug!(
            "Commands in flight: {} pending ACK + {} allocated = {}/2",
            pending_count, allocated_count, total_in_flight
        );

        total_in_flight < 2
    }

    /// Get the next command to send if any.
    pub fn next_command_to_send(&mut self) -> Option<PendingCommand> {
        if self.can_send_command() && !self.command_queue.is_empty() {
            self.command_queue.pop()
        } else {
            None
        }
    }

    /// Register that a command was sent and is pending ACK.
    pub fn register_pending_ack(
        &mut self,
        id: u32,
        bytes: bytes::Bytes,
        priority: Priority,
        category: CommandCategory,
        camera_id: crate::camera_id::CameraId,
        now: Instant,
    ) {
        self.pending_ack
            .insert(id, (bytes.clone(), priority, category, now, camera_id));
        self.command_metadata
            .insert(id, (bytes, priority, category, camera_id));
        debug!("Registered command {} as pending ACK", id);
    }

    /// Register a Sony sequence number for a command.
    pub fn register_sequence(&mut self, cmd_id: u32, sequence: u32) {
        debug!(
            "Registering Sony sequence {} for command {}",
            sequence, cmd_id
        );

        // Check for duplicate sequence
        if let Some(existing_cmd) = self.pending_by_sequence.get(&sequence) {
            if *existing_cmd != cmd_id {
                warn!(
                    "Sequence {} already mapped to command {}, overwriting with {}",
                    sequence, existing_cmd, cmd_id
                );
            }
        }

        self.pending_by_sequence.insert(sequence, cmd_id);
        self.sequence_by_command.insert(cmd_id, sequence);

        trace!(
            "Sequence mappings: pending_by_seq has {} entries, seq_by_cmd has {} entries",
            self.pending_by_sequence.len(),
            self.sequence_by_command.len()
        );
    }

    /// Get command ID for a Sony sequence number.
    pub fn get_command_by_sequence(&self, sequence: u32) -> Option<u32> {
        self.pending_by_sequence.get(&sequence).copied()
    }

    /// Finish a command by sequence number (Sony protocol).
    pub fn finish_sequence(&mut self, cmd_id: u32) {
        if let Some(sequence) = self.sequence_by_command.remove(&cmd_id) {
            self.pending_by_sequence.remove(&sequence);
            debug!(
                "Cleaned up Sony sequence {} for command {}",
                sequence, cmd_id
            );
        }
    }

    /// Unregister a pending ACK without removing command metadata.
    /// This is used for rollback when a send operation fails.
    /// Returns true if the command was found and removed from pending_ack.
    pub fn unregister_pending_ack(&mut self, id: u32) -> bool {
        self.pending_ack.remove(&id).is_some()
    }

    /// Process an event and return any actions to take.
    pub fn process_event(&mut self, event: SchedulerEvent, now: Instant) -> Vec<SchedulerAction> {
        let mut actions = Vec::new();

        match event {
            SchedulerEvent::Ack { socket } => {
                if let Some(cmd_id) = self.handle_ack(socket, now) {
                    debug!("Command {} assigned to socket {:?}", cmd_id, socket);
                }
            }
            SchedulerEvent::Completion { socket, response } => {
                if let Some(socket) = socket {
                    if let Some(cmd_id) = self.find_command_on_socket(socket) {
                        self.free_socket(socket);
                        self.finish_sequence(cmd_id);
                        self.command_metadata.remove(&cmd_id);
                        self.retry_attempts.remove(&cmd_id);
                        actions.push(SchedulerAction::CommandComplete {
                            id: cmd_id,
                            response,
                        });
                    }
                } else {
                    // Broadcast completion - route to most recent command
                    if let Some(cmd_id) = self.find_most_recent_command() {
                        if let Some(socket) = self.find_socket_for_command(cmd_id) {
                            self.free_socket(socket);
                        }
                        self.finish_sequence(cmd_id);
                        self.command_metadata.remove(&cmd_id);
                        self.retry_attempts.remove(&cmd_id);
                        actions.push(SchedulerAction::CommandComplete {
                            id: cmd_id,
                            response,
                        });
                    }
                }
            }
            SchedulerEvent::Error { socket, code } => {
                let error = ViscaError::from_byte(code);

                if let Some(socket) = socket {
                    if let Some(cmd_id) = self.find_command_on_socket(socket) {
                        let should_retry = self.should_retry_command(cmd_id, &error);

                        if should_retry {
                            if let Some(retry_action) = self.queue_retry_for_command(cmd_id, now) {
                                actions.push(retry_action);
                            }
                        } else {
                            self.free_socket(socket);
                            self.finish_sequence(cmd_id);
                            self.command_metadata.remove(&cmd_id);
                            self.retry_attempts.remove(&cmd_id);
                            actions.push(SchedulerAction::CommandFailed {
                                id: cmd_id,
                                error: Error::from_code(code),
                            });
                        }
                    }
                } else {
                    // Broadcast error - route to most recent command
                    if let Some(cmd_id) = self.find_most_recent_command() {
                        let should_retry = self.should_retry_command(cmd_id, &error);

                        if should_retry {
                            if let Some(retry_action) = self.queue_retry_for_command(cmd_id, now) {
                                actions.push(retry_action);
                            }
                        } else {
                            if let Some(socket) = self.find_socket_for_command(cmd_id) {
                                self.free_socket(socket);
                            }
                            self.finish_sequence(cmd_id);
                            self.command_metadata.remove(&cmd_id);
                            self.retry_attempts.remove(&cmd_id);
                            actions.push(SchedulerAction::CommandFailed {
                                id: cmd_id,
                                error: Error::from_code(code),
                            });
                        }
                    }
                }
            }
            SchedulerEvent::NetworkError => {
                // Network error - retry all pending commands
                let pending_cmds: Vec<_> = self.pending_ack.keys().cloned().collect();
                for cmd_id in pending_cmds {
                    if let Some(retry_action) = self.queue_retry_for_command(cmd_id, now) {
                        actions.push(retry_action);
                    }
                }
            }
        }

        actions
    }

    /// Check for timeouts and return commands that need action.
    pub fn check_timeouts(&mut self, now: Instant) -> Vec<SchedulerAction> {
        let mut actions = Vec::new();
        let mut timed_out = Vec::new();

        // Check socket timeouts
        for socket_idx in 0..2 {
            let socket = if socket_idx == 0 {
                ViscaSocket::S1
            } else {
                ViscaSocket::S2
            };

            if let Some(cmd_id) = self.sockets[socket_idx].command_id {
                if let Some(started_at) = self.sockets[socket_idx].started_at {
                    let category = self.sockets[socket_idx]
                        .category
                        .unwrap_or(CommandCategory::Custom);
                    let timeout = self.timeout_config.get_timeout(category);

                    if now.duration_since(started_at) > timeout {
                        warn!(
                            "Command {} on socket {:?} timed out after {:?}",
                            cmd_id, socket, timeout
                        );
                        timed_out.push((socket, cmd_id));
                    }
                }
            }
        }

        // Handle timed out commands
        for (socket, cmd_id) in timed_out {
            self.free_socket(socket);

            // Check if we should retry
            let should_retry = self
                .command_metadata
                .get(&cmd_id)
                .map(|(_, _, category, _)| {
                    let attempts = self.retry_attempts.get(&cmd_id).copied().unwrap_or(0);
                    let max_retries = self
                        .max_retries_per_category
                        .get(category)
                        .copied()
                        .unwrap_or(3);
                    attempts < max_retries
                })
                .unwrap_or(false);

            if should_retry {
                if let Some(retry_action) = self.queue_retry_for_command(cmd_id, now) {
                    actions.push(retry_action);
                }
            } else {
                self.finish_sequence(cmd_id);
                self.command_metadata.remove(&cmd_id);
                self.retry_attempts.remove(&cmd_id);
                actions.push(SchedulerAction::CommandFailed {
                    id: cmd_id,
                    error: Error::Timeout,
                });
            }
        }

        actions
    }

    /// Get commands that are ready to retry.
    pub fn get_ready_retries(&mut self, now: Instant) -> Vec<RetryCommand> {
        let mut ready = Vec::new();
        let mut remaining = Vec::new();

        for retry_cmd in self.retry_queue.drain(..) {
            if retry_cmd.retry_at <= now {
                ready.push(retry_cmd);
            } else {
                remaining.push(retry_cmd);
            }
        }

        self.retry_queue = remaining;
        ready
    }

    // Private helper methods

    fn handle_ack(&mut self, socket: ViscaSocket, now: Instant) -> Option<u32> {
        // Find oldest pending command (FIFO order for ACKs)
        let oldest_id = self
            .pending_ack
            .iter()
            .min_by_key(|(_, (_, _, _, sent_time, _))| *sent_time)
            .map(|(id, _)| *id)?;

        // Remove from pending and assign to socket
        if let Some((bytes, priority, category, _, camera_id)) = self.pending_ack.remove(&oldest_id)
        {
            // Allocate the specific socket the camera assigned
            let idx = socket.as_index();
            let state = &mut self.sockets[idx];

            if !state.free {
                warn!(
                    "Camera assigned {:?} but it's already occupied by command {:?}",
                    socket, state.command_id
                );
                return None;
            }

            state.free = false;
            state.command_id = Some(oldest_id);
            state.started_at = Some(now);
            state.category = Some(category);

            // Store metadata for potential retry
            self.command_metadata
                .insert(oldest_id, (bytes, priority, category, camera_id));

            debug!(
                "Assigned command {} to {:?} per camera ACK",
                oldest_id, socket
            );
            Some(oldest_id)
        } else {
            warn!("Failed to remove command {} from pending ACK", oldest_id);
            None
        }
    }

    /// Reserve a socket for an inquiry without waiting for ACK.
    ///
    /// Inquiries don't receive ACK responses, so we need to allocate a socket
    /// immediately when sending them. This method follows the same fairness
    /// and availability rules as ACK-based allocation.
    pub fn reserve_socket_for_inquiry(
        &mut self,
        id: u32,
        bytes: bytes::Bytes,
        priority: Priority,
        category: CommandCategory,
        camera_id: crate::camera_id::CameraId,
        now: Instant,
    ) -> Option<ViscaSocket> {
        // Check if we can send (respects 2-in-flight limit)
        if !self.can_send_command() {
            debug!("Cannot reserve socket for inquiry {}: no free sockets", id);
            return None;
        }

        // Find a free socket
        let socket = if self.sockets[0].free {
            ViscaSocket::S1
        } else if self.sockets[1].free {
            ViscaSocket::S2
        } else {
            debug!("No free socket available for inquiry {}", id);
            return None;
        };

        let idx = socket.as_index();
        let state = &mut self.sockets[idx];

        // Allocate the socket
        state.free = false;
        state.command_id = Some(id);
        state.started_at = Some(now);
        state.category = Some(category);

        // Store metadata for potential retry
        self.command_metadata
            .insert(id, (bytes, priority, category, camera_id));

        debug!("Reserved {:?} for inquiry {}", socket, id);
        Some(socket)
    }

    /// Check if a command is pending (either awaiting ACK or has a socket).
    pub fn is_command_pending(&self, cmd_id: u32) -> bool {
        self.pending_ack.contains_key(&cmd_id)
            || self.sockets.iter().any(|s| s.command_id == Some(cmd_id))
    }

    /// Free a previously reserved socket (used for rollback on inquiry send failure).
    pub fn free_socket(&mut self, socket: ViscaSocket) {
        let idx = socket.as_index();
        let state = &mut self.sockets[idx];

        if let Some(cmd_id) = state.command_id {
            debug!("Freeing {:?} from command {}", socket, cmd_id);
        }

        state.free = true;
        state.command_id = None;
        state.started_at = None;
        state.category = None;
    }

    /// Find the command ID currently assigned to a socket.
    pub fn find_command_on_socket(&self, socket: ViscaSocket) -> Option<u32> {
        self.sockets[socket.as_index()].command_id
    }

    pub(crate) fn find_socket_for_command(&self, cmd_id: u32) -> Option<ViscaSocket> {
        for (idx, state) in self.sockets.iter().enumerate() {
            if state.command_id == Some(cmd_id) {
                return Some(if idx == 0 {
                    ViscaSocket::S1
                } else {
                    ViscaSocket::S2
                });
            }
        }
        None
    }

    /// Get socket state for testing.
    #[cfg(test)]
    pub fn socket_state(
        &self,
        socket: ViscaSocket,
    ) -> (bool, Option<u32>, Option<CommandCategory>) {
        let state = &self.sockets[socket.as_index()];
        (state.free, state.command_id, state.category)
    }

    fn find_most_recent_command(&self) -> Option<u32> {
        // First check sockets for active commands
        let mut candidates = Vec::new();

        for state in &self.sockets {
            if let (Some(cmd_id), Some(started_at)) = (state.command_id, state.started_at) {
                candidates.push((cmd_id, started_at));
            }
        }

        // Also check pending ACK commands
        for (cmd_id, (_, _, _, sent_at, _)) in &self.pending_ack {
            candidates.push((*cmd_id, *sent_at));
        }

        // Return the most recently sent command
        candidates
            .into_iter()
            .max_by_key(|(_, time)| *time)
            .map(|(cmd_id, _)| cmd_id)
    }

    fn should_retry_command(&self, cmd_id: u32, error: &ViscaError) -> bool {
        if let Some((_, _, category, _)) = self.command_metadata.get(&cmd_id) {
            if error.is_retryable(Some(*category)) {
                let attempts = self.retry_attempts.get(&cmd_id).copied().unwrap_or(0);
                let max_retries = self
                    .max_retries_per_category
                    .get(category)
                    .copied()
                    .unwrap_or(3);
                attempts < max_retries
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Handle a send failure by immediately failing the command.
    ///
    /// This method is called when a command fails to send over the transport.
    /// The command is immediately failed without retry since we cannot know if
    /// it reached the camera.
    pub fn fail_after_send_error(&mut self, cmd_id: u32) -> Option<SchedulerAction> {
        // For send failures on first attempt, fail immediately with transport error
        self.finish_sequence(cmd_id);
        self.command_metadata.remove(&cmd_id);
        self.retry_attempts.remove(&cmd_id);
        Some(SchedulerAction::CommandFailed {
            id: cmd_id,
            error: Error::TransportError("Send failed".into()),
        })
    }

    /// Queue a command for retry based on the retry configuration.
    pub fn queue_retry_for_command(
        &mut self,
        cmd_id: u32,
        now: Instant,
    ) -> Option<SchedulerAction> {
        if let Some((bytes, priority, category, camera_id)) =
            self.command_metadata.get(&cmd_id).cloned()
        {
            // Free the socket if allocated
            if let Some(socket) = self.find_socket_for_command(cmd_id) {
                self.free_socket(socket);
            }

            // Increment retry count
            let attempt = self.retry_attempts.entry(cmd_id).or_insert(0);
            *attempt += 1;

            // Check if we've exceeded max retries
            let max_retries = self
                .max_retries_per_category
                .get(&category)
                .copied()
                .unwrap_or(3);

            if *attempt > max_retries {
                // Command has exceeded retries
                self.finish_sequence(cmd_id);
                self.command_metadata.remove(&cmd_id);
                self.retry_attempts.remove(&cmd_id);
                return Some(SchedulerAction::CommandFailed {
                    id: cmd_id,
                    error: Error::Timeout,
                });
            }

            // Calculate backoff delay using RetryConfig
            let delay = self.retry_config.calculate_delay(*attempt, None);

            let retry_cmd = RetryCommand {
                id: cmd_id,
                bytes: bytes.clone(),
                priority,
                category,
                camera_id,
                attempt: *attempt,
                max_retries,
                retry_at: now + delay,
            };

            debug!(
                "Queueing retry for command {} (attempt {} of {})",
                cmd_id, attempt, max_retries
            );
            self.retry_queue.push(retry_cmd);

            Some(SchedulerAction::RetryCommand {
                id: cmd_id,
                bytes,
                delay,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;

    #[test]
    fn test_reserve_socket_for_inquiry_when_free() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = crate::transport::RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let bytes = bytes::Bytes::from(vec![0x81, 0x09, 0x00, 0x02, VISCA_TERMINATOR]);
        let priority = Priority::Normal;
        let category = CommandCategory::Quick;
        let camera_id = crate::camera_id::CameraId::CAMERA_1;

        // Should be able to reserve when sockets are free
        let socket =
            core.reserve_socket_for_inquiry(1, bytes.clone(), priority, category, camera_id, now);

        assert!(socket.is_some());
        let socket = socket.unwrap();

        // Verify socket is now occupied
        let state = core.socket_state(socket);
        assert!(!state.0); // free = false
        assert_eq!(state.1, Some(1)); // command_id
        assert_eq!(state.2, Some(category)); // category
    }

    #[test]
    fn test_reserve_socket_for_inquiry_when_one_busy() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = crate::transport::RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let bytes = bytes::Bytes::from(vec![0x81, 0x09, 0x00, 0x02, VISCA_TERMINATOR]);
        let priority = Priority::Normal;
        let category = CommandCategory::Quick;
        let camera_id = crate::camera_id::CameraId::CAMERA_1;

        // Reserve first socket
        let socket1 =
            core.reserve_socket_for_inquiry(1, bytes.clone(), priority, category, camera_id, now);
        assert!(socket1.is_some());

        // Should be able to reserve second socket
        let socket2 =
            core.reserve_socket_for_inquiry(2, bytes.clone(), priority, category, camera_id, now);
        assert!(socket2.is_some());

        // Sockets should be different
        assert_ne!(socket1, socket2);
    }

    #[test]
    fn test_reserve_socket_for_inquiry_when_both_busy() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = crate::transport::RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let bytes = bytes::Bytes::from(vec![0x81, 0x09, 0x00, 0x02, VISCA_TERMINATOR]);
        let priority = Priority::Normal;
        let category = CommandCategory::Quick;
        let camera_id = crate::camera_id::CameraId::CAMERA_1;

        // Reserve both sockets
        let socket1 =
            core.reserve_socket_for_inquiry(1, bytes.clone(), priority, category, camera_id, now);
        assert!(socket1.is_some());

        let socket2 =
            core.reserve_socket_for_inquiry(2, bytes.clone(), priority, category, camera_id, now);
        assert!(socket2.is_some());

        // Third inquiry should fail to reserve
        let socket3 =
            core.reserve_socket_for_inquiry(3, bytes.clone(), priority, category, camera_id, now);
        assert!(socket3.is_none());
    }

    #[test]
    fn test_inquiry_completion_frees_socket() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = crate::transport::RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let bytes = bytes::Bytes::from(vec![0x81, 0x09, 0x00, 0x02, VISCA_TERMINATOR]);
        let priority = Priority::Normal;
        let category = CommandCategory::Quick;
        let camera_id = crate::camera_id::CameraId::CAMERA_1;

        // Reserve socket for inquiry
        let socket =
            core.reserve_socket_for_inquiry(1, bytes.clone(), priority, category, camera_id, now);
        assert!(socket.is_some());
        let socket = socket.unwrap();

        // Process completion event
        let response = ViscaResponse::Inquiry(crate::command::InquiryResponse::Power { on: true });
        let event = SchedulerEvent::Completion {
            socket: Some(socket),
            response,
        };

        let actions = core.process_event(event, now);

        // Should get CommandComplete action
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandComplete { id, response: resp } => {
                assert_eq!(*id, 1);
                // Verify it's a power inquiry response
                match resp {
                    ViscaResponse::Inquiry(crate::command::InquiryResponse::Power { on }) => {
                        assert!(*on);
                    }
                    _ => panic!("Expected Power inquiry response"),
                }
            }
            _ => panic!("Expected CommandComplete action"),
        }

        // Socket should be free again
        let state = core.socket_state(socket);
        assert!(state.0); // free = true
        assert_eq!(state.1, None); // command_id
    }

    #[test]
    fn test_inquiry_timeout_handling() {
        let timeout_config = TimeoutConfig {
            quick_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        let retry_config = crate::transport::RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let bytes = bytes::Bytes::from(vec![0x81, 0x09, 0x00, 0x02, VISCA_TERMINATOR]);
        let priority = Priority::Normal;
        let category = CommandCategory::Quick;
        let camera_id = crate::camera_id::CameraId::CAMERA_1;

        // Reserve socket for inquiry
        let socket =
            core.reserve_socket_for_inquiry(1, bytes.clone(), priority, category, camera_id, now);
        assert!(socket.is_some());

        // Check timeout immediately - should not timeout
        let actions = core.check_timeouts(now);
        assert!(actions.is_empty());

        // Check timeout after the timeout period
        let later = now + Duration::from_millis(200);
        let actions = core.check_timeouts(later);

        // Quick commands get retries, so first timeout triggers a retry
        assert!(!actions.is_empty(), "Expected timeout action but got none");
        assert_eq!(
            actions.len(),
            1,
            "Expected exactly one action, got: {:?}",
            actions
        );
        match &actions[0] {
            SchedulerAction::RetryCommand { id, .. } => {
                assert_eq!(*id, 1);
            }
            other => panic!("Expected RetryCommand action, got: {:?}", other),
        }

        // Socket should be free after retry scheduling
        let state = core.socket_state(socket.unwrap());
        assert!(state.0); // free = true

        // Exhaust retries by timing out again (simulate max retries reached)
        // For Quick category, we get extra retries, so we need to exhaust them
        // Set retry attempts to max to force failure on next timeout
        core.retry_attempts.insert(1, 10); // Force max retries exceeded

        // Allocate socket again for the retry
        let socket2 =
            core.reserve_socket_for_inquiry(1, bytes.clone(), priority, category, camera_id, later);
        assert!(socket2.is_some());

        // Now timeout should fail
        let later2 = later + Duration::from_millis(200);
        let actions2 = core.check_timeouts(later2);

        assert_eq!(actions2.len(), 1, "Expected exactly one action");
        match &actions2[0] {
            SchedulerAction::CommandFailed { id, error } => {
                assert_eq!(*id, 1);
                assert!(matches!(error, Error::Timeout));
            }
            other => panic!(
                "Expected CommandFailed action after max retries, got: {:?}",
                other
            ),
        }
    }
}
