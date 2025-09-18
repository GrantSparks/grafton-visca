//! Runtime-agnostic scheduler core state machine.
//!
//! This module implements the protocol state machine for VISCA command scheduling,
//! socket allocation, ACK/completion routing, and retry logic without any dependency
//! on async runtimes or channels.

use tracing::{debug, trace, warn};

use std::{
    cmp::Ordering as CmpOrdering,
    collections::{BinaryHeap, HashMap, VecDeque},
    time::{Duration, Instant},
};

use crate::{
    command::{
        response::{parse_inquiry_payload, InquiryKind, Response},
        CommandKind,
    },
    timeout::{CommandCategory, TimeoutConfig},
    visca_socket::ViscaSocket,
    Error,
};

/// Retry budget configuration for command categories.
///
/// This zero-cost POD type replaces the HashMap-based approach with
/// compile-time known budgets for each command category.
#[derive(Debug, Clone, Copy)]
pub struct RetryBudget {
    /// Budget for Quick commands.
    pub quick: u32,
    /// Budget for Movement commands.
    pub movement: u32,
    /// Budget for Preset commands.
    pub preset: u32,
    /// Budget for Network commands.
    pub network: u32,
    /// Budget for LongRunning commands.
    pub long_running: u32,
    /// Budget for Custom commands.
    pub custom: u32,
}

impl RetryBudget {
    /// Create a retry budget from a base retry count.
    ///
    /// This implements the same logic as the previous HashMap-based approach:
    /// - Quick: base + 2 (minimum 1)
    /// - Movement: base
    /// - Preset: base
    /// - Network: base - 1 (minimum 1)
    /// - LongRunning: 1
    /// - Custom: base
    pub const fn from_base(base: u32) -> Self {
        // Quick commands get extra retries
        let quick = if base > u32::MAX - 2 {
            u32::MAX
        } else {
            let sum = base + 2;
            if sum < 1 {
                1
            } else {
                sum
            }
        };

        // Network commands get fewer retries
        let network = if base > 0 {
            let sub = base - 1;
            if sub < 1 {
                1
            } else {
                sub
            }
        } else {
            1
        };

        Self {
            quick,
            movement: base,
            preset: base,
            network,
            long_running: 1,
            custom: base,
        }
    }

    /// Get the retry budget for a specific command category.
    pub const fn for_category(&self, category: CommandCategory) -> u32 {
        match category {
            CommandCategory::Quick => self.quick,
            CommandCategory::Movement => self.movement,
            CommandCategory::Preset => self.preset,
            CommandCategory::Network => self.network,
            CommandCategory::LongRunning => self.long_running,
            CommandCategory::Custom => self.custom,
        }
    }
}

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
#[derive(Clone)]
pub struct RetryCommand {
    /// Command ID.
    pub id: u32,
    /// The pre-encoded command to retry.
    pub command: std::sync::Arc<crate::command::encode::PreparedCommand>,
    /// Command priority.
    pub priority: Priority,
    /// Command category.
    pub category: CommandCategory,
    /// Camera ID used to encode the command.
    pub camera_id: crate::camera_id::CameraId,
    /// Command kind (Command vs Inquiry).
    pub kind: CommandKind,
    /// Retry attempt number.
    pub attempt: u32,
    /// Maximum retries allowed.
    pub max_retries: u32,
    /// When to retry this command (for exponential backoff).
    pub retry_at: Instant,
}

/// Wrapper for RetryCommand that implements Ord for BinaryHeap (min-heap).
#[derive(Clone, Debug)]
struct RetryKey {
    /// The retry command.
    command: RetryCommand,
}

impl PartialEq for RetryKey {
    fn eq(&self, other: &Self) -> bool {
        self.command.retry_at == other.command.retry_at
    }
}

impl Eq for RetryKey {}

impl PartialOrd for RetryKey {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

impl Ord for RetryKey {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        // For min-heap behavior, we want earlier times to have higher priority
        // So we reverse the comparison
        other.command.retry_at.cmp(&self.command.retry_at)
    }
}

impl std::fmt::Debug for RetryCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetryCommand")
            .field("id", &self.id)
            .field("priority", &self.priority)
            .field("category", &self.category)
            .field("camera_id", &self.camera_id)
            .field("kind", &self.kind)
            .field("attempt", &self.attempt)
            .field("max_retries", &self.max_retries)
            .field("retry_at", &self.retry_at)
            .finish()
    }
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
#[derive(Clone)]
pub struct PendingCommand {
    /// Unique identifier for this command.
    pub id: u32,
    /// The pre-encoded command to send.
    pub command: std::sync::Arc<crate::command::encode::PreparedCommand>,
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

impl std::fmt::Debug for PendingCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingCommand")
            .field("id", &self.id)
            .field("priority", &self.priority)
            .field("category", &self.category)
            .field("camera_id", &self.camera_id)
            .field("submitted_at", &self.submitted_at)
            .field("kind", &self.kind)
            .finish()
    }
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
    /// Note: This variant is currently unused but kept for potential future use.
    SendCommand {
        /// Command ID.
        id: u32,
    },
    /// Command completed successfully.
    CommandComplete {
        /// Command ID.
        id: u32,
        /// Command category.
        category: CommandCategory,
        /// Camera ID.
        camera_id: crate::camera_id::CameraId,
        /// Response from the camera.
        response: Response,
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
        /// Delay before retrying.
        delay: Duration,
    },
    /// A command exceeded one of the scheduler's timeouts.
    Timeout {
        /// Command ID that timed out.
        id: u32,
        /// Kind of timeout that occurred.
        kind: TimeoutKind,
        /// 1-based attempt number that timed out.
        attempt: u32,
        /// True if a retry was enqueued.
        will_retry: bool,
    },
}

/// Kind of timeout that can occur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutKind {
    /// Timeout waiting for ACK.
    Ack,
    // Response, // Can be added later for post-ACK response timeouts
}

/// Events that can be fed to the scheduler core.
#[derive(Debug)]
pub enum SchedulerEvent {
    /// ACK received for a socket.
    Ack {
        /// Socket that was acknowledged (None if no socket nibble in frame).
        socket: Option<ViscaSocket>,
        /// Command ID (from sequence mapping when available).
        cmd_id: Option<u32>,
    },
    /// Command completed.
    Completion {
        /// Socket that completed (if known).
        socket: Option<ViscaSocket>,
        /// Command ID (from sequence mapping when available).
        cmd_id: Option<u32>,
        /// Response from the camera.
        response: Response,
    },
    /// Inquiry data reply (no socket allocation).
    InquiryReply {
        /// Command ID (from sequence mapping or order queue).
        cmd_id: Option<u32>,
        /// Response from the camera.
        response: Response,
    },
    /// Error response.
    Error {
        /// Socket that errored (if known).
        socket: Option<ViscaSocket>,
        /// Command ID (from sequence mapping when available).
        cmd_id: Option<u32>,
        /// Error code from the camera.
        code: u8,
    },
    /// Network error (for broadcast recovery).
    NetworkError(Error),
}

/// Sequence list enum for efficient 1:Many command-to-sequences mapping.
/// Uses zero allocation for the common case (no retries).
#[derive(Debug, Clone)]
enum SeqList {
    /// Single sequence (no retries yet) - zero allocation.
    One(u32),
    /// Multiple sequences (with retries) - allocated only when needed.
    Many(Vec<u32>),
}

impl SeqList {
    /// Create a new SeqList with a single sequence.
    fn new(seq: u32) -> Self {
        SeqList::One(seq)
    }

    /// Add a sequence to the list, upgrading from One to Many if needed.
    /// Returns the evicted sequence if the list was at capacity.
    fn push(&mut self, seq: u32) -> Option<u32> {
        match self {
            SeqList::One(existing) => {
                // Upgrade to Many on first retry
                *self = SeqList::Many(vec![*existing, seq]);
                None
            }
            SeqList::Many(vec) => {
                // Optional: Cap at a reasonable limit (e.g., 8 sequences)
                const MAX_SEQUENCES_PER_CMD: usize = 8;
                if vec.len() >= MAX_SEQUENCES_PER_CMD {
                    // Remove oldest sequence to make room
                    let evicted = vec.remove(0);
                    vec.push(seq);
                    Some(evicted)
                } else {
                    vec.push(seq);
                    None
                }
            }
        }
    }

    /// Iterate over all sequences.
    fn iter(&self) -> Box<dyn Iterator<Item = u32> + '_> {
        match self {
            SeqList::One(seq) => Box::new(std::iter::once(*seq)),
            SeqList::Many(vec) => Box::new(vec.iter().copied()),
        }
    }
}

/// Sequence list enum for efficient 1:Many command-to-sequences mapping (16-bit version).
/// Uses zero allocation for the common case (no retries).
#[derive(Debug, Clone)]
enum SeqList16 {
    /// Single sequence (no retries yet) - zero allocation.
    One(u16),
    /// Multiple sequences (with retries) - allocated only when needed.
    Many(Vec<u16>),
}

impl SeqList16 {
    /// Create a new SeqList16 with a single sequence.
    fn new(seq: u16) -> Self {
        SeqList16::One(seq)
    }

    /// Add a sequence to the list, upgrading from One to Many if needed.
    /// Returns the evicted sequence if the list was at capacity.
    fn push(&mut self, seq: u16) -> Option<u16> {
        match self {
            SeqList16::One(existing) => {
                // Upgrade to Many on first retry
                *self = SeqList16::Many(vec![*existing, seq]);
                None
            }
            SeqList16::Many(vec) => {
                // Cap at the same limit as the main sequence list
                const MAX_SEQUENCES_PER_CMD: usize = 8;
                if vec.len() >= MAX_SEQUENCES_PER_CMD {
                    // Remove oldest sequence to make room
                    let evicted = vec.remove(0);
                    vec.push(seq);
                    Some(evicted)
                } else {
                    vec.push(seq);
                    None
                }
            }
        }
    }

    /// Iterate over all sequences.
    fn iter(&self) -> Box<dyn Iterator<Item = u16> + '_> {
        match self {
            SeqList16::One(seq) => Box::new(std::iter::once(*seq)),
            SeqList16::Many(vec) => Box::new(vec.iter().copied()),
        }
    }
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
    /// Maps command ID to (command, priority, category, sent_time, camera_id).
    pending_ack: HashMap<
        u32,
        (
            std::sync::Arc<crate::command::encode::PreparedCommand>,
            Priority,
            CommandCategory,
            Instant,
            crate::camera_id::CameraId,
        ),
    >,
    /// Commands waiting to be retried (after busy response).
    /// Uses a min-heap ordered by retry_at for efficient deadline-driven scheduling.
    retry_queue: BinaryHeap<RetryKey>,
    /// Store command metadata for potential retry.
    command_metadata: HashMap<
        u32,
        (
            std::sync::Arc<crate::command::encode::PreparedCommand>,
            Priority,
            CommandCategory,
            crate::camera_id::CameraId,
            CommandKind,
        ),
    >,
    /// Track retry attempts for commands (command_id -> attempt_count).
    retry_attempts: HashMap<u32, u32>,
    /// Track whether the retry was triggered by a transport error (command_id -> is_transport_error).
    retry_trigger_transport_error: HashMap<u32, bool>,
    /// Priority queue for pending commands.
    command_queue: BinaryHeap<PendingCommand>,
    /// Priority queue for pending inquiries (separate from commands to avoid socket gating).
    inquiry_queue: BinaryHeap<PendingCommand>,
    /// Maximum number of inquiries that can be in flight simultaneously.
    max_inquiries_inflight: usize,
    /// Retry budget for command categories.
    retry_budget: RetryBudget,
    /// Sony sequence tracking: sequence -> command_id.
    seq_to_cmd: HashMap<u32, u32>,
    /// Sony sequence tracking: command_id -> sequences.
    cmd_to_seqs: HashMap<u32, SeqList>,
    /// Sony 16-bit sequence tracking: lower 16 bits -> command_id (for legacy compatibility).
    seq16_to_cmd: HashMap<u16, u32>,
    /// Sony 16-bit sequence tracking: command_id -> 16-bit sequences.
    cmd_to_seq16s: HashMap<u32, SeqList16>,
    /// Inquiries in flight: command_id -> (sent_time, category).
    inquiries_inflight: HashMap<u32, (Instant, CommandCategory)>,
    /// Inquiry order tracking for raw VISCA (no sequence).
    inquiries_order: VecDeque<u32>,
    /// Response types for inquiries (for parsing DataReply).
    inquiry_response_types: HashMap<u32, InquiryKind>,
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
        // Create retry budget from the base retry count
        let retry_budget = RetryBudget::from_base(retry_config.max_retries);

        Self {
            sockets: Default::default(),
            timeout_config,
            retry_config,
            pending_ack: HashMap::new(),
            retry_queue: BinaryHeap::new(),
            command_metadata: HashMap::new(),
            retry_attempts: HashMap::new(),
            retry_trigger_transport_error: HashMap::new(),
            command_queue: BinaryHeap::new(),
            inquiry_queue: BinaryHeap::new(),
            max_inquiries_inflight: 8, // Conservative default to avoid overwhelming devices
            retry_budget,
            seq_to_cmd: HashMap::new(),
            cmd_to_seqs: HashMap::new(),
            seq16_to_cmd: HashMap::new(),
            cmd_to_seq16s: HashMap::new(),
            inquiries_inflight: HashMap::new(),
            inquiries_order: VecDeque::new(),
            inquiry_response_types: HashMap::new(),
        }
    }

    /// Set the timeout configuration.
    pub fn set_timeout_config(&mut self, timeout_config: TimeoutConfig) {
        self.timeout_config = timeout_config;
    }

    /// Set the maximum number of inquiries that can be in flight simultaneously.
    pub fn set_max_inquiries_inflight(&mut self, max: usize) {
        self.max_inquiries_inflight = max;
    }

    /// Queue a command for execution.
    pub fn queue_command(&mut self, command: PendingCommand) {
        // Route based on command kind
        match command.kind {
            CommandKind::Inquiry => {
                self.inquiry_queue.push(command);
            }
            CommandKind::Command => {
                self.command_queue.push(command);
            }
        }
    }

    /// Check if we can send another command (have room for pending ACK).
    pub fn can_send_command(&self) -> bool {
        // Count commands that are either pending ACK or have a socket allocated
        let pending_count = self.pending_ack.len();
        let allocated_count = self.sockets.iter().filter(|s| !s.free).count();
        let total_in_flight = pending_count + allocated_count;

        let can_send = total_in_flight < 2;

        if !can_send && std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
            eprintln!(
                "[SchedulerCore] can_send_command=false: {} pending ACK + {} allocated = {}/2 capacity",
                pending_count, allocated_count, total_in_flight
            );
        }

        debug!(
            "Commands in flight: {} pending ACK + {} allocated = {}/2",
            pending_count, allocated_count, total_in_flight
        );

        // Debug assertion to check invariants
        #[cfg(debug_assertions)]
        {
            // Verify that no command appears in both pending_ack and sockets
            for &cmd_id in self.pending_ack.keys() {
                for socket in &self.sockets {
                    if socket.command_id == Some(cmd_id) {
                        eprintln!(
                            "ERROR: Invariant violation: command {} is both pending ACK and allocated to socket",
                            cmd_id
                        );
                        debug_assert!(false, "Invariant violation detected");
                    }
                }
            }
        }

        can_send
    }

    /// Check if we can send an inquiry (not at max capacity).
    pub fn can_send_inquiry(&self) -> bool {
        self.inquiries_inflight.len() < self.max_inquiries_inflight
    }

    /// Get the next item to send (inquiry or command).
    ///
    /// Inquiries bypass the two-socket gate and are returned immediately if available
    /// and under the inquiry limit. Commands are only returned if sockets are available.
    pub fn next_item_to_send(&mut self) -> Option<PendingCommand> {
        // First check for inquiries - they don't need socket allocation
        if !self.inquiry_queue.is_empty() && self.can_send_inquiry() {
            return self.inquiry_queue.pop();
        }

        // Then check for commands if we have socket capacity
        if self.can_send_command() && !self.command_queue.is_empty() {
            return self.command_queue.pop();
        }

        None
    }

    /// Register that a command was sent and is pending ACK.
    #[allow(clippy::too_many_arguments)]
    pub fn register_pending_ack(
        &mut self,
        id: u32,
        command: std::sync::Arc<crate::command::encode::PreparedCommand>,
        priority: Priority,
        category: CommandCategory,
        camera_id: crate::camera_id::CameraId,
        kind: CommandKind,
        now: Instant,
    ) {
        self.pending_ack
            .insert(id, (command.clone(), priority, category, now, camera_id));
        self.command_metadata
            .insert(id, (command, priority, category, camera_id, kind));
        debug!("Registered command {} as pending ACK", id);
    }

    /// Register a Sony sequence number for a command.
    pub fn register_sequence(&mut self, cmd_id: u32, sequence: u32) {
        debug!(
            "Registering Sony sequence {} for command {}",
            sequence, cmd_id
        );

        // Check for duplicate sequence
        if let Some(existing_cmd) = self.seq_to_cmd.get(&sequence) {
            if *existing_cmd != cmd_id {
                warn!(
                    "Sequence {} already mapped to command {}, overwriting with {}",
                    sequence, existing_cmd, cmd_id
                );
            }
        }

        // Map sequence to command
        self.seq_to_cmd.insert(sequence, cmd_id);

        // Add sequence to command's sequence list
        match self.cmd_to_seqs.get_mut(&cmd_id) {
            Some(seq_list) => {
                // Command already has sequences (this is a retry)
                if let Some(evicted) = seq_list.push(sequence) {
                    // Remove the evicted sequence from seq_to_cmd
                    self.seq_to_cmd.remove(&evicted);
                    debug!(
                        "Added retry sequence {} to command {}, evicted old sequence {}",
                        sequence, cmd_id, evicted
                    );
                } else {
                    debug!("Added retry sequence {} to command {}", sequence, cmd_id);
                }
            }
            None => {
                // First sequence for this command
                self.cmd_to_seqs.insert(cmd_id, SeqList::new(sequence));
            }
        }

        // Also register the lower 16 bits for legacy compatibility
        let seq16 = (sequence & 0xFFFF) as u16;
        debug!(
            "Also registering 16-bit sequence {} (from {}) for command {}",
            seq16, sequence, cmd_id
        );

        // Check for 16-bit collision
        if let Some(existing_cmd) = self.seq16_to_cmd.get(&seq16) {
            if *existing_cmd != cmd_id {
                debug!(
                    "16-bit sequence {} collision: mapped to command {} but also needed for {}",
                    seq16, existing_cmd, cmd_id
                );
            }
        }

        // Map 16-bit sequence to command
        self.seq16_to_cmd.insert(seq16, cmd_id);

        // Add 16-bit sequence to command's 16-bit sequence list
        match self.cmd_to_seq16s.get_mut(&cmd_id) {
            Some(seq16_list) => {
                // Command already has 16-bit sequences (this is a retry)
                if let Some(evicted) = seq16_list.push(seq16) {
                    // Remove the evicted 16-bit sequence from seq16_to_cmd
                    self.seq16_to_cmd.remove(&evicted);
                    debug!(
                        "Added retry 16-bit sequence {} to command {}, evicted old sequence {}",
                        seq16, cmd_id, evicted
                    );
                } else {
                    debug!(
                        "Added retry 16-bit sequence {} to command {}",
                        seq16, cmd_id
                    );
                }
            }
            None => {
                // First 16-bit sequence for this command
                self.cmd_to_seq16s.insert(cmd_id, SeqList16::new(seq16));
            }
        }

        trace!(
            "Sequence mappings: seq_to_cmd has {} entries, cmd_to_seqs has {} entries, seq16_to_cmd has {} entries, cmd_to_seq16s has {} entries",
            self.seq_to_cmd.len(),
            self.cmd_to_seqs.len(),
            self.seq16_to_cmd.len(),
            self.cmd_to_seq16s.len()
        );
    }

    /// Get command ID for a Sony sequence number.
    /// First tries exact 32-bit match, then falls back to 16-bit match if unique.
    pub fn get_command_by_sequence(&self, sequence: u32) -> Option<u32> {
        // 1. Try exact 32-bit match first
        if let Some(cmd_id) = self.seq_to_cmd.get(&sequence).copied() {
            // Extra safety: verify the command is still active
            if self.command_metadata.contains_key(&cmd_id)
                || self.inquiries_inflight.contains_key(&cmd_id)
            {
                debug!(
                    "Found exact 32-bit sequence match for {}: command {}",
                    sequence, cmd_id
                );
                return Some(cmd_id);
            } else {
                debug!(
                    "Ignoring stale 32-bit sequence {} for completed command {}",
                    sequence, cmd_id
                );
            }
        }

        // 2. Try 16-bit fallback (lower 16 bits)
        let seq16 = (sequence & 0xFFFF) as u16;
        if let Some(cmd_id) = self.seq16_to_cmd.get(&seq16).copied() {
            // Verify the command is still active
            if self.command_metadata.contains_key(&cmd_id)
                || self.inquiries_inflight.contains_key(&cmd_id)
            {
                // Check if this 16-bit sequence maps to multiple active commands
                // If so, it's ambiguous and we should return None
                let active_commands_with_seq16: Vec<_> = self
                    .cmd_to_seq16s
                    .iter()
                    .filter(|(cmd_id, seq16_list)| {
                        // Only consider active commands
                        (self.command_metadata.contains_key(cmd_id)
                         || self.inquiries_inflight.contains_key(cmd_id))
                        &&
                        // That contain this 16-bit sequence
                        seq16_list.iter().any(|s| s == seq16)
                    })
                    .map(|(cmd_id, _)| *cmd_id)
                    .collect();

                if active_commands_with_seq16.len() == 1 {
                    debug!(
                        "Found unique 16-bit sequence match for {} (seq16 {}): command {}",
                        sequence, seq16, cmd_id
                    );
                    return Some(cmd_id);
                } else if active_commands_with_seq16.len() > 1 {
                    debug!(
                        "Ambiguous 16-bit sequence {} (from {}) maps to {} active commands: {:?}",
                        seq16,
                        sequence,
                        active_commands_with_seq16.len(),
                        active_commands_with_seq16
                    );
                } else {
                    debug!(
                        "16-bit sequence {} (from {}) found in mapping but no active commands",
                        seq16, sequence
                    );
                }
            } else {
                debug!(
                    "Ignoring stale 16-bit sequence {} (from {}) for completed command {}",
                    seq16, sequence, cmd_id
                );
            }
        }

        // No unique match found
        None
    }

    /// Finish a command by sequence number (Sony protocol).
    pub fn finish_sequence(&mut self, cmd_id: u32) {
        // Remove all 32-bit sequences for this command
        let mut count32 = 0;
        if let Some(seq_list) = self.cmd_to_seqs.remove(&cmd_id) {
            for seq in seq_list.iter() {
                self.seq_to_cmd.remove(&seq);
                count32 += 1;
            }
        }

        // Remove all 16-bit sequences for this command
        let mut count16 = 0;
        if let Some(seq16_list) = self.cmd_to_seq16s.remove(&cmd_id) {
            for seq16 in seq16_list.iter() {
                self.seq16_to_cmd.remove(&seq16);
                count16 += 1;
            }
        }

        debug!(
            "Cleaned up {} Sony sequence(s) and {} 16-bit sequence(s) for command {}",
            count32, count16, cmd_id
        );
    }

    /// Unregister a pending ACK without removing command metadata.
    /// This is used for rollback when a send operation fails.
    /// Returns true if the command was found and removed from pending_ack.
    pub fn unregister_pending_ack(&mut self, id: u32) -> bool {
        self.pending_ack.remove(&id).is_some()
    }

    /// Register the expected response type for an inquiry.
    pub fn register_inquiry_type(&mut self, id: u32, ty: InquiryKind) {
        self.inquiry_response_types.insert(id, ty);
    }

    /// Take the response type for an inquiry (removing it from storage).
    pub fn take_inquiry_type(&mut self, id: u32) -> Option<InquiryKind> {
        self.inquiry_response_types.remove(&id)
    }

    /// Get the response type for an inquiry (without removing it).
    pub fn get_inquiry_type(&self, id: u32) -> Option<&InquiryKind> {
        self.inquiry_response_types.get(&id)
    }

    /// Resolve inquiry ID from a VISCA payload.
    ///
    /// Given optional Sony sequence and a VISCA payload, resolve the cmd_id using:
    /// (a) Sony sequence maps, (b) content-based matcher for Raw VISCA,
    /// else (c) FIFO front of inquiries_order.
    pub fn resolve_inquiry_id(
        &self,
        payload: crate::command::response::payload::Payload<'_>,
        sequence: Option<u32>,
    ) -> Option<u32> {
        // First try sequence-based resolution if available
        if let Some(seq) = sequence {
            if let Some(cmd_id) = self.get_command_by_sequence(seq) {
                // Verify it's an active inquiry
                if self.inquiries_inflight.contains_key(&cmd_id) {
                    return Some(cmd_id);
                }
            }
        }

        // Try content-based matching for raw VISCA
        // Build a map of active inquiries with their types
        let active_inquiries: HashMap<u32, InquiryKind> = self
            .inquiries_inflight
            .keys()
            .filter_map(|&id| self.inquiry_response_types.get(&id).map(|ty| (id, *ty)))
            .collect();

        // Try parsing the payload against each expected response type
        let mut matches: Vec<u32> = active_inquiries
            .iter()
            .filter_map(|(id, response_type)| {
                // Use the existing zero-allocation parser
                // If parsing succeeds, this inquiry type matches the payload
                if parse_inquiry_payload(payload.as_slice(), response_type).is_ok() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        // Remove duplicates (defensive, shouldn't happen with unique IDs)
        matches.dedup();

        match matches.len() {
            0 => {
                // No match - fall back to FIFO as last resort
                debug!("No inquiry match, falling back to FIFO");
                self.inquiries_order.front().copied()
            }
            1 => {
                // Unique match found
                Some(matches[0])
            }
            _ => {
                // Ambiguous - fall back to FIFO
                debug!(
                    "Ambiguous inquiry match ({} candidates), falling back to FIFO",
                    matches.len()
                );
                self.inquiries_order.front().copied()
            }
        }
    }

    /// Process an event and return any actions to take.
    pub fn process_event(&mut self, event: SchedulerEvent, now: Instant) -> Vec<SchedulerAction> {
        let mut actions = Vec::new();

        match event {
            SchedulerEvent::Ack { socket, cmd_id } => {
                if let Some(cmd_id) = self.handle_ack_with_id(socket, cmd_id, now) {
                    debug!("Command {} assigned to socket {:?}", cmd_id, socket);
                }
            }
            SchedulerEvent::Completion {
                socket,
                cmd_id,
                response,
            } => {
                // Prefer cmd_id from sequence mapping
                let resolved_cmd_id = if let Some(id) = cmd_id {
                    Some(id)
                } else if let Some(socket) = socket {
                    self.find_command_on_socket(socket)
                } else {
                    // No longer use find_most_recent_command fallback
                    None
                };

                if let Some(cmd_id) = resolved_cmd_id {
                    if let Some(socket) = socket.or_else(|| self.find_socket_for_command(cmd_id)) {
                        self.free_socket(socket);
                    }
                    self.finish_sequence(cmd_id);
                    // Extract metadata before removing it
                    let (category, camera_id) =
                        if let Some((_, _, cat, cam_id, _)) = self.command_metadata.get(&cmd_id) {
                            (*cat, *cam_id)
                        } else {
                            // Fallback for commands without metadata (shouldn't happen)
                            (CommandCategory::Quick, crate::camera_id::CameraId::CAMERA_1)
                        };
                    self.command_metadata.remove(&cmd_id);
                    self.retry_attempts.remove(&cmd_id);
                    self.retry_trigger_transport_error.remove(&cmd_id);
                    actions.push(SchedulerAction::CommandComplete {
                        id: cmd_id,
                        category,
                        camera_id,
                        response,
                    });
                }
            }
            SchedulerEvent::InquiryReply { cmd_id, response } => {
                // Resolve inquiry ID from sequence or order queue
                let resolved_cmd_id = if let Some(id) = cmd_id {
                    // Remove from order queue if present (for sequence-based reply)
                    self.inquiries_order.retain(|&x| x != id);
                    Some(id)
                } else {
                    // Pop from order queue for raw VISCA
                    self.inquiries_order.pop_front()
                };

                if let Some(cmd_id) = resolved_cmd_id {
                    // Remove from inflight tracking
                    self.inquiries_inflight.remove(&cmd_id);
                    // Clean up inquiry response type
                    self.inquiry_response_types.remove(&cmd_id);
                    // Clean up sequence mappings
                    self.finish_sequence(cmd_id);
                    // Extract metadata before removing it
                    let (category, camera_id) =
                        if let Some((_, _, cat, cam_id, _)) = self.command_metadata.get(&cmd_id) {
                            (*cat, *cam_id)
                        } else {
                            // Fallback for inquiries without metadata
                            (CommandCategory::Quick, crate::camera_id::CameraId::CAMERA_1)
                        };
                    // Remove metadata
                    self.command_metadata.remove(&cmd_id);
                    self.retry_attempts.remove(&cmd_id);
                    self.retry_trigger_transport_error.remove(&cmd_id);
                    // Complete the inquiry
                    actions.push(SchedulerAction::CommandComplete {
                        id: cmd_id,
                        category,
                        camera_id,
                        response,
                    });
                    debug!("Inquiry {} completed with response", cmd_id);
                }
            }
            SchedulerEvent::Error {
                socket,
                cmd_id,
                code,
            } => {
                let error = ViscaError::from_byte(code);

                // Prefer cmd_id from sequence mapping
                let resolved_cmd_id = if let Some(id) = cmd_id {
                    Some(id)
                } else if let Some(socket) = socket {
                    self.find_command_on_socket(socket)
                } else {
                    // For error responses without socket assignment (e.g., immediate syntax errors),
                    // check the pending_ack queue for the most recent command
                    self.pending_ack
                        .iter()
                        .min_by_key(|(_, (_, _, _, sent_time, _))| *sent_time)
                        .map(|(id, _)| *id)
                };

                if let Some(cmd_id) = resolved_cmd_id {
                    // Remove from pending_ack if it's there (for immediate errors without ACK)
                    self.pending_ack.remove(&cmd_id);
                    // Check if this is an inquiry
                    let is_inquiry = self.inquiries_inflight.contains_key(&cmd_id);

                    if is_inquiry {
                        // Remove from inquiry tracking
                        self.inquiries_inflight.remove(&cmd_id);
                        self.inquiries_order.retain(|&id| id != cmd_id);
                        self.inquiry_response_types.remove(&cmd_id);
                    }

                    let should_retry = self.should_retry_command(cmd_id, &error);

                    if should_retry {
                        if let Some(retry_action) = self.queue_retry_for_command(cmd_id, now) {
                            actions.push(retry_action);
                        }
                    } else {
                        if let Some(socket) =
                            socket.or_else(|| self.find_socket_for_command(cmd_id))
                        {
                            self.free_socket(socket);
                        }
                        self.finish_sequence(cmd_id);
                        self.command_metadata.remove(&cmd_id);
                        self.retry_attempts.remove(&cmd_id);
                        self.retry_trigger_transport_error.remove(&cmd_id);
                        actions.push(SchedulerAction::CommandFailed {
                            id: cmd_id,
                            error: Error::from_code(code),
                        });
                    }
                }
            }
            SchedulerEvent::NetworkError(error) => {
                // Network error - retry all pending commands
                let pending_cmds: Vec<_> = self.pending_ack.keys().cloned().collect();
                // Check if this is a transport error
                let is_transport_error = matches!(error, Error::TransportError(_));
                for cmd_id in pending_cmds {
                    // Store whether this retry was triggered by a transport error
                    self.retry_trigger_transport_error
                        .insert(cmd_id, is_transport_error);
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
                        if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                            eprintln!(
                                "[SchedulerCore] Socket timeout: cmd_id={}, socket={:?}, category={:?}, duration={:?}",
                                cmd_id, socket, category, now.duration_since(started_at)
                            );
                        }
                        timed_out.push((socket, cmd_id));
                    }
                }
            }
        }

        // Check inquiry timeouts
        let mut timed_out_inquiries = Vec::new();
        for (&cmd_id, &(started_at, category)) in &self.inquiries_inflight {
            let timeout = self.timeout_config.get_timeout(category);
            if now.duration_since(started_at) > timeout {
                warn!("Inquiry {} timed out after {:?}", cmd_id, timeout);
                timed_out_inquiries.push(cmd_id);
            }
        }

        // Handle timed out inquiries
        for cmd_id in timed_out_inquiries {
            self.inquiries_inflight.remove(&cmd_id);
            // Remove from order queue if present
            self.inquiries_order.retain(|&id| id != cmd_id);
            // Clean up response type if retry fails
            let _should_remove_type = !self
                .command_metadata
                .get(&cmd_id)
                .map(|(_, _, category, _, _)| {
                    let attempts = self.retry_attempts.get(&cmd_id).copied().unwrap_or(0);
                    let max_retries = self.retry_budget.for_category(*category);
                    attempts < max_retries
                })
                .unwrap_or(false);

            // Check if we should retry
            let should_retry = self
                .command_metadata
                .get(&cmd_id)
                .map(|(_, _, category, _, _)| {
                    let attempts = self.retry_attempts.get(&cmd_id).copied().unwrap_or(0);
                    let max_retries = self.retry_budget.for_category(*category);
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
                self.retry_trigger_transport_error.remove(&cmd_id);
                self.inquiry_response_types.remove(&cmd_id);
                actions.push(SchedulerAction::CommandFailed {
                    id: cmd_id,
                    error: Error::Timeout,
                });
            }
        }

        // Check pending-ACK timeouts (commands sent but not yet acknowledged)
        let ack_timeout = self.timeout_config.ack_timeout;
        let mut ack_timed_out = Vec::new();

        for (&cmd_id, &(_, _, _category, sent_at, _)) in &self.pending_ack {
            if now.duration_since(sent_at) > ack_timeout {
                warn!(
                    "Command {} timed out waiting for ACK after {:?}",
                    cmd_id, ack_timeout
                );
                ack_timed_out.push(cmd_id);
            }
        }

        // Process all timed-out ACK commands
        for cmd_id in ack_timed_out {
            // Remove from pending_ack FIRST to free capacity
            if let Some((command, priority, category, _, camera_id)) =
                self.pending_ack.remove(&cmd_id)
            {
                // Get the kind from command_metadata
                let kind = self
                    .command_metadata
                    .get(&cmd_id)
                    .map(|(_, _, _, _, k)| *k)
                    .unwrap_or(CommandKind::Command); // Default to Command if not found
                if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                    eprintln!(
                        "[SchedulerCore] ACK timeout: cmd_id={}, removed from pending_ack (count={})",
                        cmd_id,
                        self.pending_ack.len()
                    );
                }

                // Debug assertion: command should not be in both pending_ack and have a socket
                #[cfg(debug_assertions)]
                {
                    for socket in &self.sockets {
                        if socket.command_id == Some(cmd_id) {
                            eprintln!(
                                "ERROR: Invariant violation: ACK-timed-out command {} still has socket allocated",
                                cmd_id
                            );
                            debug_assert!(false, "ACK timeout invariant violation");
                        }
                    }
                }

                // Get retry count for this command
                let attempts = self.retry_attempts.get(&cmd_id).copied().unwrap_or(0);
                let max_retries = self.retry_budget.for_category(category);

                // Determine if we will retry
                let will_retry = attempts < max_retries;

                // Always emit a timeout action to notify the adapter
                actions.push(SchedulerAction::Timeout {
                    id: cmd_id,
                    kind: TimeoutKind::Ack,
                    attempt: attempts + 1, // 1-based attempt number
                    will_retry,
                });

                if will_retry {
                    // Queue for retry
                    debug!(
                        "Queueing ACK-timed-out command {} for retry (attempt {})",
                        cmd_id,
                        attempts + 1
                    );

                    if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                        eprintln!(
                            "[SchedulerCore] Queueing retry for cmd_id={} (attempt {}/{})",
                            cmd_id,
                            attempts + 1,
                            max_retries
                        );
                    }

                    // Update retry count
                    self.retry_attempts.insert(cmd_id, attempts + 1);

                    // Ensure command metadata is preserved for retry
                    self.command_metadata.insert(
                        cmd_id,
                        (command.clone(), priority, category, camera_id, kind),
                    );

                    // Calculate retry delay using RetryConfig to maintain consistency
                    // For ACK timeouts, we preserve the legacy timing by using a special calculation:
                    // - Base delay is 100ms (same as before)
                    // - Exponent is capped at 5 (2^5 = 32) to match legacy behavior
                    // - We add 1 to attempts because calculate_delay uses 2^(attempt-1)
                    let capped_attempt = (attempts + 1).min(6); // Cap at 6 since calculate_delay uses attempt-1
                    let retry_delay = self.retry_config.calculate_delay(capped_attempt, None);

                    // Create and queue the retry command
                    let retry_cmd = RetryCommand {
                        id: cmd_id,
                        command: command.clone(),
                        priority,
                        category,
                        camera_id,
                        kind,
                        attempt: attempts + 1,
                        max_retries,
                        retry_at: now + retry_delay,
                    };

                    self.retry_queue.push(RetryKey { command: retry_cmd });

                    actions.push(SchedulerAction::RetryCommand {
                        id: cmd_id,
                        delay: retry_delay,
                    });
                } else {
                    // Max retries exceeded - fail the command
                    debug!(
                        "Command {} exceeded max ACK retries, failing with Timeout",
                        cmd_id
                    );

                    if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                        eprintln!(
                            "[SchedulerCore] Command {} exceeded max ACK retries ({}), failing",
                            cmd_id, max_retries
                        );
                    }

                    // Clean up all state for this command
                    self.finish_sequence(cmd_id);
                    self.command_metadata.remove(&cmd_id);
                    self.retry_attempts.remove(&cmd_id);
                    self.retry_trigger_transport_error.remove(&cmd_id);

                    actions.push(SchedulerAction::CommandFailed {
                        id: cmd_id,
                        error: Error::Timeout,
                    });
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
                .map(|(_, _, category, _, _)| {
                    let attempts = self.retry_attempts.get(&cmd_id).copied().unwrap_or(0);
                    let max_retries = self.retry_budget.for_category(*category);
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
                self.retry_trigger_transport_error.remove(&cmd_id);
                actions.push(SchedulerAction::CommandFailed {
                    id: cmd_id,
                    error: Error::Timeout,
                });
            }
        }

        actions
    }

    /// Get the number of commands waiting to be retried.
    pub fn retry_queue_depth(&self) -> usize {
        self.retry_queue.len()
    }

    /// Get commands that are ready to retry.
    pub fn get_ready_retries(&mut self, now: Instant) -> Vec<RetryCommand> {
        let mut ready = Vec::new();

        // Pop commands from the heap while they're ready
        while let Some(retry_key) = self.retry_queue.peek() {
            if retry_key.command.retry_at <= now {
                // Pop the ready command - we know it exists because we just peeked
                if let Some(retry_key) = self.retry_queue.pop() {
                    ready.push(retry_key.command);
                } else {
                    // This shouldn't happen since we just peeked, but handle gracefully
                    debug!("Unexpected: retry queue empty after peek");
                    break;
                }
            } else {
                // Heap is ordered by retry_at, so no more are ready
                break;
            }
        }

        ready
    }

    /// Get the next deadline for time-based operations.
    /// Returns the earliest deadline among ACK timeouts, command timeouts, inquiry timeouts, and retry eligibility.
    pub fn next_deadline(&self, _now: Instant) -> Option<Instant> {
        let mut earliest: Option<Instant> = None;

        // Check ACK timeouts
        for (_, _, _, sent_at, _) in self.pending_ack.values() {
            let deadline = *sent_at + self.timeout_config.ack_timeout;
            earliest = match earliest {
                None => Some(deadline),
                Some(e) if deadline < e => Some(deadline),
                _ => earliest,
            };
        }

        // Check socket command timeouts
        for socket_state in &self.sockets {
            if let (Some(started_at), Some(category)) =
                (socket_state.started_at, socket_state.category)
            {
                let timeout = self.timeout_config.get_timeout(category);
                let deadline = started_at + timeout;
                earliest = match earliest {
                    None => Some(deadline),
                    Some(e) if deadline < e => Some(deadline),
                    _ => earliest,
                };
            }
        }

        // Check inquiry timeouts
        for &(started_at, category) in self.inquiries_inflight.values() {
            let timeout = self.timeout_config.get_timeout(category);
            let deadline = started_at + timeout;
            earliest = match earliest {
                None => Some(deadline),
                Some(e) if deadline < e => Some(deadline),
                _ => earliest,
            };
        }

        // Check retry queue (peek at the earliest retry)
        if let Some(retry_key) = self.retry_queue.peek() {
            let deadline = retry_key.command.retry_at;
            earliest = match earliest {
                None => Some(deadline),
                Some(e) if deadline < e => Some(deadline),
                _ => earliest,
            };
        }

        earliest
    }

    // Private helper methods

    fn handle_ack_with_id(
        &mut self,
        socket: Option<ViscaSocket>,
        cmd_id: Option<u32>,
        now: Instant,
    ) -> Option<u32> {
        // Prefer cmd_id from sequence mapping
        let target_id = if let Some(id) = cmd_id {
            // Verify it's actually pending
            if self.pending_ack.contains_key(&id) {
                Some(id)
            } else {
                debug!("ACK with sequence {} not found in pending commands", id);
                None
            }
        } else {
            // Fall back to oldest pending command (FIFO order for raw VISCA)
            self.pending_ack
                .iter()
                .min_by_key(|(_, (_, _, _, sent_time, _))| *sent_time)
                .map(|(id, _)| *id)
        }?;

        // Remove from pending and assign to socket
        if let Some((bytes, priority, category, _, camera_id)) = self.pending_ack.remove(&target_id)
        {
            // Determine which socket to use with fallback logic
            let assigned_socket = if let Some(s) = socket {
                // Camera specified a socket - try to use it
                let idx = s.as_index();
                if self.sockets[idx].free {
                    // Requested socket is free, use it
                    s
                } else {
                    // Requested socket is busy, try the other one
                    let other = if s == ViscaSocket::S1 {
                        ViscaSocket::S2
                    } else {
                        ViscaSocket::S1
                    };

                    if self.sockets[other.as_index()].free {
                        debug!(
                            "Camera requested {:?} but it's occupied, using {:?} instead",
                            s, other
                        );
                        other
                    } else {
                        // Both sockets are busy
                        warn!("Camera assigned {:?} but both sockets are occupied", s);
                        // Re-insert command into pending_ack since we couldn't assign it
                        self.pending_ack
                            .insert(target_id, (bytes, priority, category, now, camera_id));
                        return None;
                    }
                }
            } else {
                // No socket specified - pick the first free one
                if self.sockets[ViscaSocket::S1.as_index()].free {
                    ViscaSocket::S1
                } else if self.sockets[ViscaSocket::S2.as_index()].free {
                    ViscaSocket::S2
                } else {
                    // Both sockets are busy
                    warn!("ACK received without socket nibble but both sockets are occupied");
                    // Re-insert command into pending_ack since we couldn't assign it
                    self.pending_ack
                        .insert(target_id, (bytes, priority, category, now, camera_id));
                    return None;
                }
            };

            // Allocate the chosen socket
            let idx = assigned_socket.as_index();
            let state = &mut self.sockets[idx];

            state.free = false;
            state.command_id = Some(target_id);
            state.started_at = Some(now);
            state.category = Some(category);

            // Store metadata for potential retry
            self.command_metadata.insert(
                target_id,
                (bytes, priority, category, camera_id, CommandKind::Command),
            );

            debug!(
                "Assigned command {} to {:?} per camera ACK",
                target_id, assigned_socket
            );
            Some(target_id)
        } else {
            warn!("Failed to remove command {} from pending ACK", target_id);
            None
        }
    }

    /// Start tracking an inquiry (no socket allocation).
    #[allow(clippy::too_many_arguments)]
    pub fn start_inquiry(
        &mut self,
        id: u32,
        command: std::sync::Arc<crate::command::encode::PreparedCommand>,
        priority: Priority,
        category: CommandCategory,
        camera_id: crate::camera_id::CameraId,
        kind: CommandKind,
        now: Instant,
    ) {
        // Store metadata for potential retry
        self.command_metadata
            .insert(id, (command, priority, category, camera_id, kind));

        // Track the inquiry as in-flight
        self.inquiries_inflight.insert(id, (now, category));

        // Add to order queue for raw VISCA correlation
        self.inquiries_order.push_back(id);

        debug!("Started inquiry {} (no socket allocation)", id);
    }

    /// Check if a command is pending (either awaiting ACK or has a socket).
    pub fn is_command_pending(&self, cmd_id: u32) -> bool {
        self.pending_ack.contains_key(&cmd_id)
            || self.sockets.iter().any(|s| s.command_id == Some(cmd_id))
    }

    /// Get the count of commands waiting for ACK.
    pub fn pending_ack_count(&self) -> usize {
        self.pending_ack.len()
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

    /// Get the camera ID for a command by its ID.
    pub fn camera_id_for_command(&self, id: u32) -> Option<crate::camera_id::CameraId> {
        self.command_metadata
            .get(&id)
            .map(|(_, _, _, camera_id, _)| *camera_id)
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

    fn should_retry_command(&self, cmd_id: u32, error: &ViscaError) -> bool {
        if let Some((_, _, category, _, _)) = self.command_metadata.get(&cmd_id) {
            if error.is_retryable(Some(*category)) {
                let attempts = self.retry_attempts.get(&cmd_id).copied().unwrap_or(0);
                let max_retries = self.retry_budget.for_category(*category);
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
        // If inquiry is in-flight, remove from inquiries_inflight and inquiries_order
        self.inquiries_inflight.remove(&cmd_id);
        self.inquiries_order.retain(|&x| x != cmd_id);
        self.inquiry_response_types.remove(&cmd_id);

        // For send failures on first attempt, fail immediately with transport error
        self.finish_sequence(cmd_id);
        self.command_metadata.remove(&cmd_id);
        self.retry_attempts.remove(&cmd_id);
        self.retry_trigger_transport_error.remove(&cmd_id);
        Some(SchedulerAction::CommandFailed {
            id: cmd_id,
            error: Error::TransportError("Send failed".into()),
        })
    }

    /// Mark a retry as being triggered by a transport error.
    /// This affects the final error classification when retries are exhausted.
    pub fn mark_retry_as_transport_error(&mut self, cmd_id: u32) {
        self.retry_trigger_transport_error.insert(cmd_id, true);
    }

    /// Queue a command for retry based on the retry configuration.
    pub fn queue_retry_for_command(
        &mut self,
        cmd_id: u32,
        now: Instant,
    ) -> Option<SchedulerAction> {
        if let Some((command, priority, category, camera_id, kind)) =
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
            let max_retries = self.retry_budget.for_category(category);

            if *attempt > max_retries {
                // Command has exceeded retries
                self.finish_sequence(cmd_id);
                self.command_metadata.remove(&cmd_id);
                self.retry_attempts.remove(&cmd_id);
                // Use TransportError if the retry was triggered by a transport error, otherwise Timeout
                let error = if self
                    .retry_trigger_transport_error
                    .remove(&cmd_id)
                    .unwrap_or(false)
                {
                    Error::TransportError("Network error after max retries".into())
                } else {
                    Error::Timeout
                };
                return Some(SchedulerAction::CommandFailed { id: cmd_id, error });
            }

            // Calculate backoff delay using RetryConfig
            let delay = self.retry_config.calculate_delay(*attempt, None);

            let retry_cmd = RetryCommand {
                id: cmd_id,
                command: command.clone(),
                priority,
                category,
                camera_id,
                kind,
                attempt: *attempt,
                max_retries,
                retry_at: now + delay,
            };

            debug!(
                "Queueing retry for command {} (attempt {} of {})",
                cmd_id, attempt, max_retries
            );
            self.retry_queue.push(RetryKey { command: retry_cmd });

            Some(SchedulerAction::RetryCommand { id: cmd_id, delay })
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
    use crate::command::encode::PreparedCommand;
    use crate::transport::RetryConfig;
    use crate::CameraId;
    use bytes::Bytes;
    use std::sync::Arc;

    // Helper structs for different test command categories
    #[derive(Debug, Clone)]
    struct TestCommandQuick {
        bytes: Vec<u8>,
        response_type: Option<InquiryKind>,
    }

    impl crate::command::encode::ViscaCommand for TestCommandQuick {
        type Response = ();
        const MAX_SIZE: usize = 16;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

        fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            let len = self.bytes.len();
            buffer[..len].copy_from_slice(&self.bytes);
            Ok(len)
        }

        fn response_kind(&self) -> Option<InquiryKind> {
            self.response_type
        }
    }

    #[derive(Debug, Clone)]
    struct TestCommandMovement {
        bytes: Vec<u8>,
        response_type: Option<InquiryKind>,
    }

    impl crate::command::encode::ViscaCommand for TestCommandMovement {
        type Response = ();
        const MAX_SIZE: usize = 16;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Movement;

        fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            let len = self.bytes.len();
            buffer[..len].copy_from_slice(&self.bytes);
            Ok(len)
        }

        fn response_kind(&self) -> Option<InquiryKind> {
            self.response_type
        }
    }

    // Helper function to create test commands from byte patterns
    fn create_test_command(
        bytes: Vec<u8>,
        response_type: Option<InquiryKind>,
        category: CommandCategory,
        camera_id: CameraId,
    ) -> Arc<PreparedCommand> {
        let command = match category {
            CommandCategory::Quick => PreparedCommand::new(
                TestCommandQuick {
                    bytes,
                    response_type,
                },
                camera_id,
            )
            .unwrap(),
            CommandCategory::Movement => PreparedCommand::new(
                TestCommandMovement {
                    bytes,
                    response_type,
                },
                camera_id,
            )
            .unwrap(),
            _ => {
                // Default to Quick for other categories in tests
                PreparedCommand::new(
                    TestCommandQuick {
                        bytes,
                        response_type,
                    },
                    camera_id,
                )
                .unwrap()
            }
        };
        Arc::new(command)
    }

    #[test]
    fn test_retry_budget_from_base() {
        // Test with base of 3 (default)
        let budget = RetryBudget::from_base(3);
        assert_eq!(budget.quick, 5); // 3 + 2
        assert_eq!(budget.movement, 3);
        assert_eq!(budget.preset, 3);
        assert_eq!(budget.network, 2); // 3 - 1, min 1
        assert_eq!(budget.long_running, 1);
        assert_eq!(budget.custom, 3);

        // Test with base of 0
        let budget = RetryBudget::from_base(0);
        assert_eq!(budget.quick, 2); // 0 + 2
        assert_eq!(budget.movement, 0);
        assert_eq!(budget.preset, 0);
        assert_eq!(budget.network, 1); // min 1
        assert_eq!(budget.long_running, 1);
        assert_eq!(budget.custom, 0);

        // Test with base of 1
        let budget = RetryBudget::from_base(1);
        assert_eq!(budget.quick, 3); // 1 + 2
        assert_eq!(budget.movement, 1);
        assert_eq!(budget.preset, 1);
        assert_eq!(budget.network, 1); // 1 - 1 = 0, but min 1
        assert_eq!(budget.long_running, 1);
        assert_eq!(budget.custom, 1);

        // Test with large base
        let budget = RetryBudget::from_base(10);
        assert_eq!(budget.quick, 12); // 10 + 2
        assert_eq!(budget.movement, 10);
        assert_eq!(budget.preset, 10);
        assert_eq!(budget.network, 9); // 10 - 1
        assert_eq!(budget.long_running, 1);
        assert_eq!(budget.custom, 10);
    }

    #[test]
    fn test_retry_budget_for_category() {
        let budget = RetryBudget::from_base(3);

        assert_eq!(budget.for_category(CommandCategory::Quick), 5);
        assert_eq!(budget.for_category(CommandCategory::Movement), 3);
        assert_eq!(budget.for_category(CommandCategory::Preset), 3);
        assert_eq!(budget.for_category(CommandCategory::Network), 2);
        assert_eq!(budget.for_category(CommandCategory::LongRunning), 1);
        assert_eq!(budget.for_category(CommandCategory::Custom), 3);
    }

    #[test]
    fn test_ack_backoff_parity() {
        // Test that the new ACK backoff calculation matches the legacy behavior
        let retry_config = RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(10),
            exponential_backoff: true,
        };

        // Legacy calculation: base_delay * 2^attempts.min(5)
        // New calculation: retry_config.calculate_delay((attempts + 1).min(6), None)

        // Test cases matching the legacy behavior
        let test_cases = vec![
            (0, 100),  // 2^0 = 1, 100ms * 1 = 100ms
            (1, 200),  // 2^1 = 2, 100ms * 2 = 200ms
            (2, 400),  // 2^2 = 4, 100ms * 4 = 400ms
            (3, 800),  // 2^3 = 8, 100ms * 8 = 800ms
            (4, 1600), // 2^4 = 16, 100ms * 16 = 1600ms
            (5, 3200), // 2^5 = 32, 100ms * 32 = 3200ms (capped)
            (6, 3200), // Still capped at 2^5
            (7, 3200), // Still capped at 2^5
        ];

        for (attempts, expected_ms) in test_cases {
            // New calculation used in the code
            let capped_attempt = (attempts + 1).min(6);
            let actual_delay = retry_config.calculate_delay(capped_attempt, None);

            assert_eq!(
                actual_delay,
                Duration::from_millis(expected_ms),
                "Mismatch for attempt {}: expected {}ms, got {}ms",
                attempts,
                expected_ms,
                actual_delay.as_millis()
            );
        }
    }

    #[test]
    fn test_inquiry_does_not_consume_sockets() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Quick;
        let camera_id = CameraId::CAMERA_1;

        // Create a test inquiry
        #[derive(Clone)]
        struct TestInquiry;
        impl crate::command::encode::ViscaCommand for TestInquiry {
            type Response = ();
            const MAX_SIZE: usize = 5;
            const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;
            fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                buffer[0] = 0x81;
                buffer[1] = 0x09;
                buffer[2] = 0x00;
                buffer[3] = 0x02;
                buffer[4] = VISCA_TERMINATOR;
                Ok(5)
            }
            fn response_kind(&self) -> Option<InquiryKind> {
                Some(InquiryKind::Power)
            }
        }
        impl std::fmt::Debug for TestInquiry {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("TestInquiry").finish()
            }
        }

        let inquiry_cmd = Arc::new(PreparedCommand::new(TestInquiry, camera_id).unwrap());

        // Helper to create test commands
        #[derive(Clone)]
        struct TestCmd1;
        impl crate::command::encode::ViscaCommand for TestCmd1 {
            type Response = ();
            const MAX_SIZE: usize = 6;
            const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Movement;
            fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x00;
                buffer[4] = 0x02;
                buffer[5] = VISCA_TERMINATOR;
                Ok(6)
            }
            fn response_kind(&self) -> Option<InquiryKind> {
                None
            }
        }
        impl std::fmt::Debug for TestCmd1 {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("TestCmd1").finish()
            }
        }

        #[derive(Clone)]
        struct TestCmd2;
        impl crate::command::encode::ViscaCommand for TestCmd2 {
            type Response = ();
            const MAX_SIZE: usize = 6;
            const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Movement;
            fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x00;
                buffer[4] = 0x03;
                buffer[5] = VISCA_TERMINATOR;
                Ok(6)
            }
            fn response_kind(&self) -> Option<InquiryKind> {
                None
            }
        }
        impl std::fmt::Debug for TestCmd2 {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("TestCmd2").finish()
            }
        }

        // Start two commands to occupy both sockets
        let cmd1 = Arc::new(PreparedCommand::new(TestCmd1, camera_id).unwrap());
        let cmd2 = Arc::new(PreparedCommand::new(TestCmd2, camera_id).unwrap());

        // Register first command on socket 1
        core.register_pending_ack(
            1,
            cmd1.clone(),
            priority,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now,
        );
        // Manually allocate socket 1 (simulating ACK received)
        // When ACK is received, command is removed from pending_ack
        core.pending_ack.remove(&1);
        core.sockets[0].free = false;
        core.sockets[0].command_id = Some(1);
        core.sockets[0].started_at = Some(now);
        core.sockets[0].category = Some(CommandCategory::Movement);

        // Register second command on socket 2
        core.register_pending_ack(
            2,
            cmd2.clone(),
            priority,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now,
        );
        // Manually allocate socket 2 (simulating ACK received)
        // When ACK is received, command is removed from pending_ack
        core.pending_ack.remove(&2);
        core.sockets[1].free = false;
        core.sockets[1].command_id = Some(2);
        core.sockets[1].started_at = Some(now);
        core.sockets[1].category = Some(CommandCategory::Movement);

        // Both sockets are now occupied, but inquiry should still be sendable
        assert!(!core.can_send_command()); // Cannot send more commands

        // Start an inquiry - should not need a socket
        core.start_inquiry(
            3,
            inquiry_cmd.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            now,
        );

        // Verify inquiry is tracked
        assert!(core.inquiries_inflight.contains_key(&3));
        assert!(core.inquiries_order.contains(&3));

        // Sockets should still be occupied by commands
        let state1 = core.socket_state(ViscaSocket::S1);
        assert!(!state1.0); // Socket 1 still occupied
        assert_eq!(state1.1, Some(1)); // By command 1

        let state2 = core.socket_state(ViscaSocket::S2);
        assert!(!state2.0); // Socket 2 still occupied
        assert_eq!(state2.1, Some(2)); // By command 2
    }

    #[test]
    fn test_inquiry_reply_handling() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();

        // Create a test inquiry command
        #[derive(Debug, Clone)]
        struct TestInquiryCmd;
        impl crate::command::encode::ViscaCommand for TestInquiryCmd {
            type Response = ();
            const MAX_SIZE: usize = 5;
            const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;
            fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                buffer[0] = 0x81;
                buffer[1] = 0x09;
                buffer[2] = 0x00;
                buffer[3] = 0x02;
                buffer[4] = VISCA_TERMINATOR;
                Ok(5)
            }
            fn response_kind(&self) -> Option<InquiryKind> {
                Some(InquiryKind::Power)
            }
        }

        let priority = Priority::Normal;
        let category = CommandCategory::Quick;
        let camera_id = CameraId::CAMERA_1;
        let command = Arc::new(PreparedCommand::new(TestInquiryCmd, camera_id).unwrap());

        // Start an inquiry
        core.start_inquiry(
            1,
            command.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            now,
        );

        // Verify inquiry is tracked
        assert!(core.inquiries_inflight.contains_key(&1));
        assert!(core.inquiries_order.contains(&1));

        // Process InquiryReply event
        let response = Response::Inquiry(crate::command::InquiryData::Power { on: true });
        let event = SchedulerEvent::InquiryReply {
            cmd_id: Some(1),
            response,
        };

        let actions = core.process_event(event, now);

        // Should get CommandComplete action
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandComplete {
                id, response: resp, ..
            } => {
                assert_eq!(*id, 1);
                match resp {
                    Response::Inquiry(crate::command::InquiryData::Power { on }) => {
                        assert!(*on);
                    }
                    _ => panic!("Expected Power inquiry response"),
                }
            }
            _ => panic!("Expected CommandComplete action"),
        }

        // Inquiry should be removed from tracking
        assert!(!core.inquiries_inflight.contains_key(&1));
        assert!(!core.inquiries_order.contains(&1));
    }

    #[test]
    fn test_raw_visca_inquiry_ordering() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Quick;
        let camera_id = CameraId::CAMERA_1;

        // Create test inquiry commands
        #[derive(Debug, Clone)]
        struct TestInquiry1;
        impl crate::command::encode::ViscaCommand for TestInquiry1 {
            type Response = ();
            const MAX_SIZE: usize = 5;
            const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;
            fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                buffer[0] = 0x81;
                buffer[1] = 0x09;
                buffer[2] = 0x00;
                buffer[3] = 0x02;
                buffer[4] = VISCA_TERMINATOR;
                Ok(5)
            }
            fn response_kind(&self) -> Option<InquiryKind> {
                Some(InquiryKind::Power)
            }
        }

        #[derive(Debug, Clone)]
        struct TestInquiry2;
        impl crate::command::encode::ViscaCommand for TestInquiry2 {
            type Response = ();
            const MAX_SIZE: usize = 5;
            const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;
            fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                buffer[0] = 0x81;
                buffer[1] = 0x09;
                buffer[2] = 0x04;
                buffer[3] = 0x00;
                buffer[4] = VISCA_TERMINATOR;
                Ok(5)
            }
            fn response_kind(&self) -> Option<InquiryKind> {
                Some(InquiryKind::ZoomPosition)
            }
        }

        #[derive(Debug, Clone)]
        struct TestInquiry3;
        impl crate::command::encode::ViscaCommand for TestInquiry3 {
            type Response = ();
            const MAX_SIZE: usize = 5;
            const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;
            fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                buffer[0] = 0x81;
                buffer[1] = 0x09;
                buffer[2] = 0x06;
                buffer[3] = 0x12;
                buffer[4] = VISCA_TERMINATOR;
                Ok(5)
            }
            fn response_kind(&self) -> Option<InquiryKind> {
                Some(InquiryKind::Power)
            }
        }

        let cmd1 = Arc::new(PreparedCommand::new(TestInquiry1, camera_id).unwrap());
        let cmd2 = Arc::new(PreparedCommand::new(TestInquiry2, camera_id).unwrap());
        let cmd3 = Arc::new(PreparedCommand::new(TestInquiry3, camera_id).unwrap());

        core.start_inquiry(
            1,
            cmd1,
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            now,
        );
        core.start_inquiry(
            2,
            cmd2,
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            now,
        );
        core.start_inquiry(
            3,
            cmd3,
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            now,
        );

        // Verify all inquiries are tracked in order
        assert_eq!(core.inquiries_order.len(), 3);
        assert_eq!(core.inquiries_order[0], 1);
        assert_eq!(core.inquiries_order[1], 2);
        assert_eq!(core.inquiries_order[2], 3);

        // Process InquiryReply events without cmd_id (raw VISCA)
        // First reply should match first inquiry
        let response1 = Response::Inquiry(crate::command::InquiryData::Power { on: true });
        let event1 = SchedulerEvent::InquiryReply {
            cmd_id: None, // No sequence in raw VISCA
            response: response1,
        };

        let actions = core.process_event(event1, now);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandComplete { id, .. } => {
                assert_eq!(*id, 1); // First inquiry completed
            }
            _ => panic!("Expected CommandComplete action"),
        }

        // Order should have inquiry 1 removed
        assert_eq!(core.inquiries_order.len(), 2);
        assert_eq!(core.inquiries_order[0], 2);
        assert_eq!(core.inquiries_order[1], 3);
    }

    #[test]
    fn test_sony_sequence_attribution() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let camera_id = CameraId::CAMERA_1;

        // Create test commands
        #[derive(Debug, Clone)]
        struct TestCmd1;
        impl crate::command::encode::ViscaCommand for TestCmd1 {
            type Response = ();
            const MAX_SIZE: usize = 6;
            const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Movement;
            fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x00;
                buffer[4] = 0x02;
                buffer[5] = VISCA_TERMINATOR;
                Ok(6)
            }
            fn response_kind(&self) -> Option<InquiryKind> {
                None
            }
        }

        #[derive(Debug, Clone)]
        struct TestCmd2;
        impl crate::command::encode::ViscaCommand for TestCmd2 {
            type Response = ();
            const MAX_SIZE: usize = 6;
            const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Movement;
            fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x00;
                buffer[4] = 0x03;
                buffer[5] = VISCA_TERMINATOR;
                Ok(6)
            }
            fn response_kind(&self) -> Option<InquiryKind> {
                None
            }
        }

        let cmd1 = Arc::new(PreparedCommand::new(TestCmd1, camera_id).unwrap());
        let cmd2 = Arc::new(PreparedCommand::new(TestCmd2, camera_id).unwrap());

        core.register_pending_ack(
            1,
            cmd1,
            priority,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(1, 100); // Command 1 has sequence 100

        core.register_pending_ack(
            2,
            cmd2,
            priority,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(2, 101); // Command 2 has sequence 101

        // Process ACK for command 2 first (out of order)
        let cmd_id_2 = core.get_command_by_sequence(101);
        let event = SchedulerEvent::Ack {
            socket: Some(ViscaSocket::S2),
            cmd_id: cmd_id_2, // Using sequence to identify
        };

        let actions = core.process_event(event, now);

        // ACK processing should be silent (no action returned)
        assert!(
            actions.is_empty(),
            "Expected no actions from ACK processing"
        );

        // Verify command 2 got socket 2
        let (free, cmd_id, _) = core.socket_state(ViscaSocket::S2);
        assert!(!free); // Socket occupied
        assert_eq!(cmd_id, Some(2)); // By command 2

        // Command 1 should still be pending
        assert!(core.pending_ack.contains_key(&1));

        // Now process ACK for command 1
        let cmd_id_1 = core.get_command_by_sequence(100);
        let event = SchedulerEvent::Ack {
            socket: Some(ViscaSocket::S1),
            cmd_id: cmd_id_1,
        };

        core.process_event(event, now);

        // Verify command 1 got socket 1
        let (free, cmd_id, _) = core.socket_state(ViscaSocket::S1);
        assert!(!free); // Socket occupied
        assert_eq!(cmd_id, Some(1)); // By command 1
    }

    #[test]
    fn test_inquiry_timeout_handling() {
        let timeout_config = TimeoutConfig {
            quick_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Quick;
        let camera_id = CameraId::CAMERA_1;
        let command = create_test_command(
            vec![0x81, 0x09, 0x00, 0x02, VISCA_TERMINATOR],
            Some(InquiryKind::Power),
            CommandCategory::Quick,
            camera_id,
        );

        // Start an inquiry
        core.start_inquiry(
            1,
            command.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            now,
        );
        assert!(core.inquiries_inflight.contains_key(&1));

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

        // Inquiry should be removed from tracking after timeout
        assert!(!core.inquiries_inflight.contains_key(&1));

        // Exhaust retries by timing out again (simulate max retries reached)
        // For Quick category, we get extra retries, so we need to exhaust them
        // Set retry attempts to max to force failure on next timeout
        core.retry_attempts.insert(1, 10); // Force max retries exceeded

        // Start inquiry again for the retry
        core.start_inquiry(
            1,
            command.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            later,
        );
        assert!(core.inquiries_inflight.contains_key(&1));

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

    #[test]
    fn test_sequence_tracking_with_retries() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Movement;
        let camera_id = CameraId::CAMERA_1;
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );

        // Register command with initial sequence
        core.register_pending_ack(
            1,
            command.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(1, 100);

        // Verify initial sequence is tracked
        assert_eq!(core.get_command_by_sequence(100), Some(1));
        assert_eq!(core.seq_to_cmd.len(), 1);
        assert_eq!(core.cmd_to_seqs.len(), 1);

        // Simulate retry - register new sequence for same command
        core.register_sequence(1, 101);

        // Both sequences should now map to command 1
        assert_eq!(core.get_command_by_sequence(100), Some(1));
        assert_eq!(core.get_command_by_sequence(101), Some(1));
        assert_eq!(core.seq_to_cmd.len(), 2);
        assert_eq!(core.cmd_to_seqs.len(), 1); // Still one command

        // Simulate another retry
        core.register_sequence(1, 102);

        // All three sequences should map to command 1
        assert_eq!(core.get_command_by_sequence(100), Some(1));
        assert_eq!(core.get_command_by_sequence(101), Some(1));
        assert_eq!(core.get_command_by_sequence(102), Some(1));
        assert_eq!(core.seq_to_cmd.len(), 3);

        // Finish the command - all sequences should be cleaned up
        core.finish_sequence(1);
        core.command_metadata.remove(&1);

        // No sequences should remain
        assert_eq!(core.get_command_by_sequence(100), None);
        assert_eq!(core.get_command_by_sequence(101), None);
        assert_eq!(core.get_command_by_sequence(102), None);
        assert_eq!(core.seq_to_cmd.len(), 0);
        assert_eq!(core.cmd_to_seqs.len(), 0);
    }

    #[test]
    fn test_late_reply_after_completion_ignored() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Movement;
        let camera_id = CameraId::CAMERA_1;
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );

        // Register command with sequences from multiple retries
        core.register_pending_ack(
            1,
            command.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(1, 100);
        core.register_sequence(1, 101); // Retry 1
        core.register_sequence(1, 102); // Retry 2

        // Verify all sequences are active
        assert_eq!(core.get_command_by_sequence(100), Some(1));
        assert_eq!(core.get_command_by_sequence(101), Some(1));
        assert_eq!(core.get_command_by_sequence(102), Some(1));

        // Complete the command (simulating success on the third attempt)
        let response = Response::Completion {
            socket: Some(ViscaSocket::S1),
        };
        let event = SchedulerEvent::Completion {
            socket: Some(ViscaSocket::S1),
            cmd_id: Some(1),
            response,
        };

        let actions = core.process_event(event, now);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandComplete { id, .. } => {
                assert_eq!(*id, 1);
            }
            _ => panic!("Expected CommandComplete"),
        }

        // Now simulate a late reply from an earlier sequence
        // This should be ignored since the command is already completed
        assert_eq!(core.get_command_by_sequence(100), None);
        assert_eq!(core.get_command_by_sequence(101), None);
        assert_eq!(core.get_command_by_sequence(102), None);

        // Verify all mappings are cleaned up
        assert_eq!(core.seq_to_cmd.len(), 0);
        assert_eq!(core.cmd_to_seqs.len(), 0);
    }

    #[test]
    fn test_sequence_cap_at_max() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Movement;
        let camera_id = CameraId::CAMERA_1;
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );

        // Register command
        core.register_pending_ack(
            1,
            command.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );

        // Register more than MAX_SEQUENCES_PER_CMD (8) sequences
        for seq in 100..110 {
            core.register_sequence(1, seq);
        }

        // Only the last 8 sequences should be active (102-109)
        // 100 and 101 should have been dropped
        assert_eq!(core.get_command_by_sequence(100), None); // Dropped
        assert_eq!(core.get_command_by_sequence(101), None); // Dropped

        for seq in 102..110 {
            assert_eq!(core.get_command_by_sequence(seq), Some(1));
        }

        // Verify we have exactly 8 sequence mappings
        assert!(core.seq_to_cmd.len() <= 8);
    }

    #[test]
    fn test_multiple_commands_with_sequences() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Movement;
        let camera_id = CameraId::CAMERA_1;

        // Register two different commands
        let cmd1 = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );
        let cmd2 = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );

        core.register_pending_ack(
            1,
            cmd1.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_pending_ack(
            2,
            cmd2.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );

        // Command 1 has sequences 100, 101 (retry)
        core.register_sequence(1, 100);
        core.register_sequence(1, 101);

        // Command 2 has sequences 200, 201, 202 (two retries)
        core.register_sequence(2, 200);
        core.register_sequence(2, 201);
        core.register_sequence(2, 202);

        // Verify all sequences map correctly
        assert_eq!(core.get_command_by_sequence(100), Some(1));
        assert_eq!(core.get_command_by_sequence(101), Some(1));
        assert_eq!(core.get_command_by_sequence(200), Some(2));
        assert_eq!(core.get_command_by_sequence(201), Some(2));
        assert_eq!(core.get_command_by_sequence(202), Some(2));

        // Complete command 1
        core.finish_sequence(1);
        core.command_metadata.remove(&1);

        // Command 1's sequences should be gone, command 2's should remain
        assert_eq!(core.get_command_by_sequence(100), None);
        assert_eq!(core.get_command_by_sequence(101), None);
        assert_eq!(core.get_command_by_sequence(200), Some(2));
        assert_eq!(core.get_command_by_sequence(201), Some(2));
        assert_eq!(core.get_command_by_sequence(202), Some(2));

        // Complete command 2
        core.finish_sequence(2);
        core.command_metadata.remove(&2);

        // All sequences should be cleaned up
        assert_eq!(core.get_command_by_sequence(200), None);
        assert_eq!(core.get_command_by_sequence(201), None);
        assert_eq!(core.get_command_by_sequence(202), None);
        assert_eq!(core.seq_to_cmd.len(), 0);
        assert_eq!(core.cmd_to_seqs.len(), 0);
    }

    #[test]
    fn test_16_bit_sequence_fallback() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Movement;
        let camera_id = CameraId::CAMERA_1;
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );

        // Register a command with a 32-bit sequence that has non-zero high 16 bits
        let full_sequence = 0x12345678u32; // High 16 bits: 0x1234, Low 16 bits: 0x5678
        core.register_pending_ack(
            1,
            command.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(1, full_sequence);

        // Verify that both 32-bit and 16-bit lookups work
        assert_eq!(core.get_command_by_sequence(full_sequence), Some(1));
        assert_eq!(core.get_command_by_sequence(0x5678), Some(1)); // Should find by lower 16 bits

        // Verify that a non-matching 16-bit value doesn't work
        assert_eq!(core.get_command_by_sequence(0x1234), None);
    }

    #[test]
    fn test_16_bit_sequence_ambiguity() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Movement;
        let camera_id = CameraId::CAMERA_1;

        // Register two commands with different 32-bit sequences but same lower 16 bits
        let seq1 = 0x12345678u32;
        let seq2 = 0xABCD5678u32; // Same lower 16 bits (0x5678) but different high bits

        let cmd1 = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );
        let cmd2 = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );

        core.register_pending_ack(
            1,
            cmd1,
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(1, seq1);

        core.register_pending_ack(
            2,
            cmd2,
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(2, seq2);

        // Both 32-bit sequences should work
        assert_eq!(core.get_command_by_sequence(seq1), Some(1));
        assert_eq!(core.get_command_by_sequence(seq2), Some(2));

        // 16-bit lookup should be ambiguous and return None
        assert_eq!(core.get_command_by_sequence(0x5678), None);
    }

    #[test]
    fn test_16_bit_sequence_cleanup() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Movement;
        let camera_id = CameraId::CAMERA_1;
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );

        // Register a command with sequence
        let sequence = 0x12345678u32;
        core.register_pending_ack(
            1,
            command,
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(1, sequence);

        // Verify both mappings exist
        assert_eq!(core.get_command_by_sequence(sequence), Some(1));
        assert_eq!(core.get_command_by_sequence(0x5678), Some(1));

        // Finish the sequence
        core.finish_sequence(1);
        core.command_metadata.remove(&1);

        // Verify both mappings are cleaned up
        assert_eq!(core.get_command_by_sequence(sequence), None);
        assert_eq!(core.get_command_by_sequence(0x5678), None);
    }

    #[test]
    fn test_raw_visca_content_based_matching() {
        // This test verifies that the scheduler core correctly handles
        // out-of-order inquiry replies in raw VISCA mode.
        // The actual content-based matching happens in the adapter layer,
        // but the core should correctly process the events with proper cmd_id.

        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);
        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Quick;
        let camera_id = CameraId::CAMERA_1;

        // Start two different inquiries in raw VISCA mode
        let power_cmd = create_test_command(
            vec![0x81, 0x09, 0x00, 0x02, VISCA_TERMINATOR],
            Some(InquiryKind::Power),
            CommandCategory::Quick,
            camera_id,
        );
        let zoom_cmd = create_test_command(
            vec![0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR],
            Some(InquiryKind::ZoomPosition),
            CommandCategory::Quick,
            camera_id,
        );

        core.start_inquiry(
            1,
            power_cmd,
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            now,
        );
        core.start_inquiry(
            2,
            zoom_cmd,
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            now,
        );

        // Verify both are tracked
        assert_eq!(core.inquiries_order.len(), 2);
        assert!(core.inquiries_inflight.contains_key(&1));
        assert!(core.inquiries_inflight.contains_key(&2));

        // Process replies out of order
        // Second inquiry (zoom) reply arrives first - with explicit cmd_id
        // (This simulates the adapter layer doing content-based matching)
        let zoom_response =
            Response::Inquiry(crate::command::InquiryData::ZoomPosition { position: 0x1234 });
        let event2 = SchedulerEvent::InquiryReply {
            cmd_id: Some(2), // Content-based matching identified this as inquiry 2
            response: zoom_response,
        };

        let actions = core.process_event(event2, now);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandComplete { id, response, .. } => {
                assert_eq!(*id, 2); // Second inquiry completed
                                    // Verify it's a zoom response
                match response {
                    Response::Inquiry(crate::command::InquiryData::ZoomPosition { position }) => {
                        assert_eq!(*position, 0x1234);
                    }
                    _ => panic!("Expected ZoomPosition response"),
                }
            }
            _ => panic!("Expected CommandComplete action"),
        }

        // First inquiry (power) reply arrives second
        let power_response = Response::Inquiry(crate::command::InquiryData::Power { on: true });
        let event1 = SchedulerEvent::InquiryReply {
            cmd_id: Some(1), // Content-based matching identified this as inquiry 1
            response: power_response,
        };

        let actions = core.process_event(event1, now);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandComplete { id, response, .. } => {
                assert_eq!(*id, 1); // First inquiry completed
                                    // Verify it's a power response
                match response {
                    Response::Inquiry(crate::command::InquiryData::Power { on }) => {
                        assert!(*on);
                    }
                    _ => panic!("Expected Power response"),
                }
            }
            _ => panic!("Expected CommandComplete action"),
        }

        // All inquiries should be completed
        assert_eq!(core.inquiries_order.len(), 0);
        assert!(!core.inquiries_inflight.contains_key(&1));
        assert!(!core.inquiries_inflight.contains_key(&2));
    }

    // Tests for issue #362: send-failure retry behavior

    #[test]
    fn test_send_failure_retry_with_transport_error_flag() {
        let mut core = SchedulerCore::with_retry_config(
            TimeoutConfig::default(),
            RetryConfig {
                max_retries: 2,
                base_retry_delay: Duration::from_millis(100),
                max_retry_duration: Duration::from_secs(5),
                exponential_backoff: false,
            },
        );

        let cmd_id = 1;
        let camera_id = CameraId::CAMERA_1;
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );
        let now = Instant::now();

        // Register command metadata (simulating a command that was sent but failed)
        core.register_pending_ack(
            cmd_id,
            command.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );

        // Mark as transport error (simulating send failure)
        core.mark_retry_as_transport_error(cmd_id);

        // Queue retry - should succeed
        let action = core.queue_retry_for_command(cmd_id, now);

        // Should get a retry action
        match action {
            Some(SchedulerAction::RetryCommand { id, delay, .. }) => {
                assert_eq!(id, cmd_id);
                assert!(delay > Duration::ZERO);
            }
            _ => panic!("Expected RetryCommand action, got: {:?}", action),
        }

        // Verify retry is queued
        let retries = core.get_ready_retries(now + Duration::from_millis(200));
        assert_eq!(retries.len(), 1);
        assert_eq!(retries[0].id, cmd_id);
        assert_eq!(retries[0].attempt, 1);
    }

    #[test]
    fn test_transport_error_classification_after_exhausted_retries() {
        let mut core = SchedulerCore::with_retry_config(
            TimeoutConfig::default(),
            RetryConfig {
                max_retries: 1, // Only 1 retry allowed
                base_retry_delay: Duration::from_millis(100),
                max_retry_duration: Duration::from_secs(5),
                exponential_backoff: false,
            },
        );

        let cmd_id = 1;
        let camera_id = CameraId::CAMERA_1;
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );
        let now = Instant::now();

        // Register command metadata
        core.register_pending_ack(
            cmd_id,
            command.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );

        // First retry - should succeed
        core.mark_retry_as_transport_error(cmd_id);
        let action = core.queue_retry_for_command(cmd_id, now);
        assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));

        // Second retry - should fail with TransportError
        core.mark_retry_as_transport_error(cmd_id);
        let action = core.queue_retry_for_command(cmd_id, now);

        match action {
            Some(SchedulerAction::CommandFailed { id, error }) => {
                assert_eq!(id, cmd_id);
                match error {
                    Error::TransportError(msg) => {
                        assert!(
                            msg.contains("Network error after max retries"),
                            "Expected 'Network error after max retries', got: {}",
                            msg
                        );
                    }
                    _ => panic!("Expected TransportError, got: {:?}", error),
                }
            }
            _ => panic!("Expected CommandFailed action, got: {:?}", action),
        }
    }

    #[test]
    fn test_timeout_classification_without_transport_error_flag() {
        let mut core = SchedulerCore::with_retry_config(
            TimeoutConfig::default(),
            RetryConfig {
                max_retries: 1,
                base_retry_delay: Duration::from_millis(100),
                max_retry_duration: Duration::from_secs(5),
                exponential_backoff: false,
            },
        );

        let cmd_id = 1;
        let camera_id = CameraId::CAMERA_1;
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );
        let now = Instant::now();

        // Register command metadata
        core.register_pending_ack(
            cmd_id,
            command.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );

        // First retry WITHOUT marking as transport error
        let action = core.queue_retry_for_command(cmd_id, now);
        assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));

        // Second retry - should fail with Timeout (not TransportError)
        let action = core.queue_retry_for_command(cmd_id, now);

        match action {
            Some(SchedulerAction::CommandFailed { id, error }) => {
                assert_eq!(id, cmd_id);
                match error {
                    Error::Timeout => {
                        // Expected - timeout when not marked as transport error
                    }
                    _ => panic!("Expected Timeout error, got: {:?}", error),
                }
            }
            _ => panic!("Expected CommandFailed action, got: {:?}", action),
        }
    }

    #[test]
    fn test_multiple_commands_with_transport_errors() {
        let mut core = SchedulerCore::with_retry_config(
            TimeoutConfig::default(),
            RetryConfig {
                max_retries: 3,
                base_retry_delay: Duration::from_millis(50),
                max_retry_duration: Duration::from_secs(5),
                exponential_backoff: true,
            },
        );

        let now = Instant::now();
        let camera_id = CameraId::CAMERA_1;

        // Register multiple commands
        for cmd_id in 1..=3 {
            let command = create_test_command(
                vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
                None,
                CommandCategory::Movement,
                camera_id,
            );
            core.register_pending_ack(
                cmd_id,
                command,
                Priority::Normal,
                CommandCategory::Movement,
                CameraId::CAMERA_1,
                CommandKind::Command,
                now,
            );

            // Mark as transport error and queue retry
            core.mark_retry_as_transport_error(cmd_id);
            let action = core.queue_retry_for_command(cmd_id, now);
            assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));
        }

        // Get all ready retries
        let retries = core.get_ready_retries(now + Duration::from_secs(1));
        assert_eq!(retries.len(), 3, "Should have 3 retries queued");

        // Verify all have attempt = 1
        for retry in retries {
            assert_eq!(retry.attempt, 1);
            assert!(retry.id >= 1 && retry.id <= 3);
        }
    }

    #[test]
    fn test_ack_without_socket_nibble_s1_free() {
        // Test: ACK with socket: None, S1 free => assign S1
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();

        // Register a command
        let camera_id = CameraId::CAMERA_1;
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );
        core.register_pending_ack(
            1,
            command,
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );

        // Send ACK without socket nibble
        let event = SchedulerEvent::Ack {
            socket: None,
            cmd_id: Some(1),
        };
        core.process_event(event, now);

        // Verify S1 was assigned
        let (free, cmd_id, _) = core.socket_state(ViscaSocket::S1);
        assert!(!free, "S1 should be occupied");
        assert_eq!(cmd_id, Some(1), "Command 1 should be on S1");

        // Verify S2 is still free
        let (free, _, _) = core.socket_state(ViscaSocket::S2);
        assert!(free, "S2 should still be free");
    }

    #[test]
    fn test_ack_without_socket_nibble_s1_busy_s2_free() {
        // Test: ACK with socket: None, S1 busy, S2 free => assign S2
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();
        let camera_id = CameraId::CAMERA_1;

        // Register two commands
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );
        core.register_pending_ack(
            1,
            command.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );
        core.register_pending_ack(
            2,
            command,
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );

        // First ACK assigns S1
        let event = SchedulerEvent::Ack {
            socket: Some(ViscaSocket::S1),
            cmd_id: Some(1),
        };
        core.process_event(event, now);

        // Second ACK without socket nibble should get S2
        let event = SchedulerEvent::Ack {
            socket: None,
            cmd_id: Some(2),
        };
        core.process_event(event, now);

        // Verify S1 has command 1
        let (free, cmd_id, _) = core.socket_state(ViscaSocket::S1);
        assert!(!free, "S1 should be occupied");
        assert_eq!(cmd_id, Some(1), "Command 1 should be on S1");

        // Verify S2 has command 2
        let (free, cmd_id, _) = core.socket_state(ViscaSocket::S2);
        assert!(!free, "S2 should be occupied");
        assert_eq!(cmd_id, Some(2), "Command 2 should be on S2");
    }

    #[test]
    fn test_ack_without_socket_nibble_both_busy() {
        // Test: ACK with socket: None, both busy => no assignment
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();
        let camera_id = CameraId::CAMERA_1;

        // Register three commands
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );
        for cmd_id in 1..=3 {
            core.register_pending_ack(
                cmd_id,
                command.clone(),
                Priority::Normal,
                CommandCategory::Movement,
                CameraId::CAMERA_1,
                CommandKind::Command,
                now,
            );
        }

        // First two ACKs occupy both sockets
        let event = SchedulerEvent::Ack {
            socket: Some(ViscaSocket::S1),
            cmd_id: Some(1),
        };
        core.process_event(event, now);

        let event = SchedulerEvent::Ack {
            socket: Some(ViscaSocket::S2),
            cmd_id: Some(2),
        };
        core.process_event(event, now);

        // Third ACK without socket nibble should fail
        let event = SchedulerEvent::Ack {
            socket: None,
            cmd_id: Some(3),
        };
        core.process_event(event, now);

        // Verify command 3 is still pending
        assert!(
            core.pending_ack.contains_key(&3),
            "Command 3 should still be pending"
        );

        // Verify sockets are still occupied by commands 1 and 2
        let (free, cmd_id, _) = core.socket_state(ViscaSocket::S1);
        assert!(!free, "S1 should be occupied");
        assert_eq!(cmd_id, Some(1), "Command 1 should be on S1");

        let (free, cmd_id, _) = core.socket_state(ViscaSocket::S2);
        assert!(!free, "S2 should be occupied");
        assert_eq!(cmd_id, Some(2), "Command 2 should be on S2");
    }

    #[test]
    fn test_ack_with_busy_socket_fallback() {
        // Test: ACK requests busy socket, fallback to free socket
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();
        let camera_id = CameraId::CAMERA_1;

        // Register two commands
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );
        core.register_pending_ack(
            1,
            command.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );
        core.register_pending_ack(
            2,
            command,
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );

        // First ACK assigns S1
        let event = SchedulerEvent::Ack {
            socket: Some(ViscaSocket::S1),
            cmd_id: Some(1),
        };
        core.process_event(event, now);

        // Second ACK requests S1 (busy), should fallback to S2
        let event = SchedulerEvent::Ack {
            socket: Some(ViscaSocket::S1), // Request S1 which is busy
            cmd_id: Some(2),
        };
        core.process_event(event, now);

        // Verify S1 still has command 1
        let (free, cmd_id, _) = core.socket_state(ViscaSocket::S1);
        assert!(!free, "S1 should be occupied");
        assert_eq!(cmd_id, Some(1), "Command 1 should be on S1");

        // Verify S2 has command 2 (fallback allocation)
        let (free, cmd_id, _) = core.socket_state(ViscaSocket::S2);
        assert!(!free, "S2 should be occupied");
        assert_eq!(cmd_id, Some(2), "Command 2 should be on S2 (fallback)");
    }

    #[test]
    fn test_command_kind_preserved_through_retries() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();

        // Test case: A Command with bytes[1] == 0x09 (synthetic vendor command)
        // This would have been misclassified as Inquiry by the old heuristic

        // Create a dummy command for testing
        #[derive(Debug, Clone)]
        struct TestCommand;
        impl crate::command::encode::ViscaCommand for TestCommand {
            type Response = ();
            const MAX_SIZE: usize = 6;
            const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Movement;

            fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                // Encode a command with bytes[1] == 0x09 that would be misclassified
                buffer[0] = 0x81;
                buffer[1] = 0x09;
                buffer[2] = 0x04;
                buffer[3] = 0x00;
                buffer[4] = 0x01;
                buffer[5] = VISCA_TERMINATOR;
                Ok(6)
            }

            fn response_kind(&self) -> Option<InquiryKind> {
                None // This is a command, not an inquiry
            }
        }

        let test_command = Arc::new(PreparedCommand::new(TestCommand, CameraId::CAMERA_1).unwrap());
        let cmd_id = 1;

        // Register as Command explicitly
        core.register_pending_ack(
            cmd_id,
            test_command.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command, // Explicitly a Command despite bytes[1] == 0x09
            now,
        );

        // Mark as transport error and queue retry
        core.mark_retry_as_transport_error(cmd_id);
        let action = core.queue_retry_for_command(cmd_id, now);

        // Should get a retry action
        assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));

        // Get the retry and verify the kind is preserved
        let retries = core.get_ready_retries(now + Duration::from_millis(200));
        assert_eq!(retries.len(), 1);
        let retry = &retries[0];

        // The key assertion: kind should be Command, not Inquiry
        // This verifies that we no longer use the bytes[1] == 0x09 heuristic
        assert_eq!(retry.kind, CommandKind::Command);
        assert_eq!(retry.id, cmd_id);
        // Verify the command is preserved (same Arc)
        assert!(Arc::ptr_eq(&retry.command, &test_command));

        // Test case 2: A normal Inquiry to ensure it also preserves correctly
        let mut core2 = SchedulerCore::new(TimeoutConfig::default());

        // Create a dummy inquiry for testing
        #[derive(Debug, Clone)]
        struct TestInquiry;
        impl crate::command::encode::ViscaCommand for TestInquiry {
            type Response = ();
            const MAX_SIZE: usize = 6;
            const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

            fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                buffer[0] = 0x81;
                buffer[1] = 0x09;
                buffer[2] = 0x04;
                buffer[3] = 0x00;
                buffer[4] = 0x02;
                buffer[5] = VISCA_TERMINATOR;
                Ok(6)
            }

            fn response_kind(&self) -> Option<InquiryKind> {
                Some(InquiryKind::Power) // This is an inquiry
            }
        }

        let test_inquiry = Arc::new(PreparedCommand::new(TestInquiry, CameraId::CAMERA_1).unwrap());
        let inquiry_id = 2;

        // Start as inquiry
        core2.start_inquiry(
            inquiry_id,
            test_inquiry.clone(),
            Priority::Normal,
            CommandCategory::Quick,
            CameraId::CAMERA_1,
            CommandKind::Inquiry,
            now,
        );

        // Mark as transport error and queue retry
        core2.mark_retry_as_transport_error(inquiry_id);
        let inquiry_action = core2.queue_retry_for_command(inquiry_id, now);

        // Should get a retry action
        assert!(matches!(
            inquiry_action,
            Some(SchedulerAction::RetryCommand { .. })
        ));

        // Get the retry and verify inquiry kind is preserved
        let inquiry_retries = core2.get_ready_retries(now + Duration::from_millis(200));
        assert_eq!(inquiry_retries.len(), 1);
        let inquiry_retry = &inquiry_retries[0];

        // Verify Inquiry kind is preserved
        assert_eq!(inquiry_retry.kind, CommandKind::Inquiry);
        assert_eq!(inquiry_retry.id, inquiry_id);
        // Verify the inquiry is preserved (same Arc)
        assert!(Arc::ptr_eq(&inquiry_retry.command, &test_inquiry));
    }

    #[test]
    fn test_inquiry_bypasses_socket_gate() {
        // This test verifies that inquiries can be sent even when both command sockets are occupied
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();

        // Create two commands and one inquiry
        let command1 = Arc::new(PreparedCommand {
            payload: Bytes::from(vec![0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]),
            kind: CommandKind::Command,
            category: CommandCategory::Movement,
            response_type: None,
        });
        let command2 = Arc::new(PreparedCommand {
            payload: Bytes::from(vec![0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR]),
            kind: CommandKind::Command,
            category: CommandCategory::Movement,
            response_type: None,
        });
        let inquiry = Arc::new(PreparedCommand {
            payload: Bytes::from(vec![0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
            kind: CommandKind::Inquiry,
            category: CommandCategory::Quick,
            response_type: Some(InquiryKind::ZoomPosition),
        });

        // Queue both commands
        core.queue_command(PendingCommand {
            id: 1,
            command: command1.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Movement,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Command,
        });
        core.queue_command(PendingCommand {
            id: 2,
            command: command2.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Movement,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Command,
        });

        // Send and register both commands as pending ACK
        let cmd1 = core.next_item_to_send().unwrap();
        assert_eq!(cmd1.id, 1);
        core.register_pending_ack(
            1,
            command1.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );

        let cmd2 = core.next_item_to_send().unwrap();
        assert_eq!(cmd2.id, 2);
        core.register_pending_ack(
            2,
            command2.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );

        // Now both commands are pending ACK - sockets are at capacity
        assert!(!core.can_send_command());
        assert_eq!(core.next_item_to_send(), None); // No commands can be sent

        // Queue an inquiry
        core.queue_command(PendingCommand {
            id: 3,
            command: inquiry.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Quick,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Inquiry,
        });

        // The inquiry should be sendable even though command sockets are full
        let inq = core.next_item_to_send();
        assert!(inq.is_some());
        let inq = inq.unwrap();
        assert_eq!(inq.id, 3);
        assert_eq!(inq.kind, CommandKind::Inquiry);

        // Start the inquiry (track it in flight)
        core.start_inquiry(
            3,
            inquiry.clone(),
            Priority::Normal,
            CommandCategory::Quick,
            CameraId::CAMERA_1,
            CommandKind::Inquiry,
            now,
        );

        // Queue another command - should not be sendable
        core.queue_command(PendingCommand {
            id: 4,
            command: command1.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Movement,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Command,
        });

        // No more commands should be sendable (sockets still full)
        assert_eq!(core.next_item_to_send(), None);

        // Queue another inquiry - should be sendable
        core.queue_command(PendingCommand {
            id: 5,
            command: inquiry.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Quick,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Inquiry,
        });

        let inq2 = core.next_item_to_send();
        assert!(inq2.is_some());
        assert_eq!(inq2.unwrap().id, 5);
    }

    #[test]
    fn test_inquiry_pipeline_limit() {
        // Test that inquiries respect the max_inquiries_inflight limit
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        core.set_max_inquiries_inflight(2); // Set a low limit for testing
        let now = Instant::now();

        let inquiry = Arc::new(PreparedCommand {
            payload: Bytes::from(vec![0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
            kind: CommandKind::Inquiry,
            category: CommandCategory::Quick,
            response_type: Some(InquiryKind::ZoomPosition),
        });

        // Queue 3 inquiries
        for id in 1..=3 {
            core.queue_command(PendingCommand {
                id,
                command: inquiry.clone(),
                priority: Priority::Normal,
                category: CommandCategory::Quick,
                camera_id: CameraId::CAMERA_1,
                submitted_at: now,
                kind: CommandKind::Inquiry,
            });
        }

        // First inquiry should be sendable
        let inq1 = core.next_item_to_send();
        assert!(inq1.is_some());
        assert_eq!(inq1.unwrap().id, 1);
        core.start_inquiry(
            1,
            inquiry.clone(),
            Priority::Normal,
            CommandCategory::Quick,
            CameraId::CAMERA_1,
            CommandKind::Inquiry,
            now,
        );

        // Second inquiry should be sendable
        let inq2 = core.next_item_to_send();
        assert!(inq2.is_some());
        assert_eq!(inq2.unwrap().id, 2);
        core.start_inquiry(
            2,
            inquiry.clone(),
            Priority::Normal,
            CommandCategory::Quick,
            CameraId::CAMERA_1,
            CommandKind::Inquiry,
            now,
        );

        // Third inquiry should NOT be sendable (limit reached)
        assert!(!core.can_send_inquiry());
        let inq3 = core.next_item_to_send();
        assert!(inq3.is_none());

        // Complete one inquiry by removing it from inflight
        core.inquiries_inflight.remove(&1);

        // Now the third inquiry should be sendable
        assert!(core.can_send_inquiry());
        let inq3 = core.next_item_to_send();
        assert!(inq3.is_some());
        assert_eq!(inq3.unwrap().id, 3);
    }

    #[test]
    fn test_mixed_priority_queue_ordering() {
        // Test that priority works correctly across both queues
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();

        let command = Arc::new(PreparedCommand {
            payload: Bytes::from(vec![0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]),
            kind: CommandKind::Command,
            category: CommandCategory::Movement,
            response_type: None,
        });
        let inquiry = Arc::new(PreparedCommand {
            payload: Bytes::from(vec![0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
            kind: CommandKind::Inquiry,
            category: CommandCategory::Quick,
            response_type: Some(InquiryKind::ZoomPosition),
        });

        // Queue items with different priorities
        core.queue_command(PendingCommand {
            id: 1,
            command: command.clone(),
            priority: Priority::Low,
            category: CommandCategory::Movement,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Command,
        });

        core.queue_command(PendingCommand {
            id: 2,
            command: inquiry.clone(),
            priority: Priority::High,
            category: CommandCategory::Quick,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Inquiry,
        });

        core.queue_command(PendingCommand {
            id: 3,
            command: command.clone(),
            priority: Priority::Critical,
            category: CommandCategory::Movement,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Command,
        });

        core.queue_command(PendingCommand {
            id: 4,
            command: inquiry.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Quick,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Inquiry,
        });

        // High priority inquiry should come first
        let item1 = core.next_item_to_send().unwrap();
        assert_eq!(item1.id, 2);
        assert_eq!(item1.priority, Priority::High);

        // Normal priority inquiry next (inquiries not gated by sockets)
        let item2 = core.next_item_to_send().unwrap();
        assert_eq!(item2.id, 4);
        assert_eq!(item2.priority, Priority::Normal);

        // Critical priority command
        let item3 = core.next_item_to_send().unwrap();
        assert_eq!(item3.id, 3);
        assert_eq!(item3.priority, Priority::Critical);

        // Low priority command last
        let item4 = core.next_item_to_send().unwrap();
        assert_eq!(item4.id, 1);
        assert_eq!(item4.priority, Priority::Low);
    }
}
