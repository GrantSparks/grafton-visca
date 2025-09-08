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
        response::{parse_inquiry_payload, ViscaResponse, ViscaResponseType},
        CommandKind,
    },
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
        response: ViscaResponse,
    },
    /// Inquiry data reply (no socket allocation).
    InquiryReply {
        /// Command ID (from sequence mapping or order queue).
        cmd_id: Option<u32>,
        /// Response from the camera.
        response: ViscaResponse,
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
    NetworkError,
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
    inquiry_response_types: HashMap<u32, ViscaResponseType>,
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
            seq_to_cmd: HashMap::new(),
            cmd_to_seqs: HashMap::new(),
            seq16_to_cmd: HashMap::new(),
            cmd_to_seq16s: HashMap::new(),
            inquiries_inflight: HashMap::new(),
            inquiries_order: VecDeque::new(),
            inquiry_response_types: HashMap::new(),
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
    pub fn register_inquiry_type(&mut self, id: u32, ty: ViscaResponseType) {
        self.inquiry_response_types.insert(id, ty);
    }

    /// Take the response type for an inquiry (removing it from storage).
    pub fn take_inquiry_type(&mut self, id: u32) -> Option<ViscaResponseType> {
        self.inquiry_response_types.remove(&id)
    }

    /// Get the response type for an inquiry (without removing it).
    pub fn get_inquiry_type(&self, id: u32) -> Option<&ViscaResponseType> {
        self.inquiry_response_types.get(&id)
    }

    /// Resolve inquiry ID from a VISCA payload.
    ///
    /// Given optional Sony sequence and a VISCA payload, resolve the cmd_id using:
    /// (a) Sony sequence maps, (b) content-based matcher for Raw VISCA,
    /// else (c) FIFO front of inquiries_order.
    pub fn resolve_inquiry_id(&self, payload: &[u8], sequence: Option<u32>) -> Option<u32> {
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
        let active_inquiries: HashMap<u32, ViscaResponseType> = self
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
                if parse_inquiry_payload(payload, response_type).is_ok() {
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
                    self.command_metadata.remove(&cmd_id);
                    self.retry_attempts.remove(&cmd_id);
                    actions.push(SchedulerAction::CommandComplete {
                        id: cmd_id,
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
                    // Remove metadata
                    self.command_metadata.remove(&cmd_id);
                    self.retry_attempts.remove(&cmd_id);
                    // Complete the inquiry
                    actions.push(SchedulerAction::CommandComplete {
                        id: cmd_id,
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
                    None
                };

                if let Some(cmd_id) = resolved_cmd_id {
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
                        actions.push(SchedulerAction::CommandFailed {
                            id: cmd_id,
                            error: Error::from_code(code),
                        });
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
                self.inquiry_response_types.remove(&cmd_id);
                actions.push(SchedulerAction::CommandFailed {
                    id: cmd_id,
                    error: Error::Timeout,
                });
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

    fn handle_ack_with_id(
        &mut self,
        socket: ViscaSocket,
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
            state.command_id = Some(target_id);
            state.started_at = Some(now);
            state.category = Some(category);

            // Store metadata for potential retry
            self.command_metadata
                .insert(target_id, (bytes, priority, category, camera_id));

            debug!(
                "Assigned command {} to {:?} per camera ACK",
                target_id, socket
            );
            Some(target_id)
        } else {
            warn!("Failed to remove command {} from pending ACK", target_id);
            None
        }
    }

    /// Start tracking an inquiry (no socket allocation).
    pub fn start_inquiry(
        &mut self,
        id: u32,
        bytes: bytes::Bytes,
        priority: Priority,
        category: CommandCategory,
        camera_id: crate::camera_id::CameraId,
        now: Instant,
    ) {
        // Store metadata for potential retry
        self.command_metadata
            .insert(id, (bytes, priority, category, camera_id));

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
        // If inquiry is in-flight, remove from inquiries_inflight and inquiries_order
        self.inquiries_inflight.remove(&cmd_id);
        self.inquiries_order.retain(|&x| x != cmd_id);
        self.inquiry_response_types.remove(&cmd_id);

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
    fn test_inquiry_does_not_consume_sockets() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = crate::transport::RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let bytes = bytes::Bytes::from(vec![0x81, 0x09, 0x00, 0x02, VISCA_TERMINATOR]);
        let priority = Priority::Normal;
        let category = CommandCategory::Quick;
        let camera_id = crate::camera_id::CameraId::CAMERA_1;

        // Start two commands to occupy both sockets
        let cmd1_bytes = bytes::Bytes::from(vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]);
        let cmd2_bytes = bytes::Bytes::from(vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR]);

        // Register first command on socket 1
        core.register_pending_ack(
            1,
            cmd1_bytes.clone(),
            priority,
            CommandCategory::Movement,
            camera_id,
            now,
        );
        // Manually allocate socket 1 (simulating ACK received)
        core.sockets[0].free = false;
        core.sockets[0].command_id = Some(1);
        core.sockets[0].started_at = Some(now);
        core.sockets[0].category = Some(CommandCategory::Movement);

        // Register second command on socket 2
        core.register_pending_ack(
            2,
            cmd2_bytes.clone(),
            priority,
            CommandCategory::Movement,
            camera_id,
            now,
        );
        // Manually allocate socket 2 (simulating ACK received)
        core.sockets[1].free = false;
        core.sockets[1].command_id = Some(2);
        core.sockets[1].started_at = Some(now);
        core.sockets[1].category = Some(CommandCategory::Movement);

        // Both sockets are now occupied, but inquiry should still be sendable
        assert!(!core.can_send_command()); // Cannot send more commands

        // Start an inquiry - should not need a socket
        core.start_inquiry(3, bytes.clone(), priority, category, camera_id, now);

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
        let retry_config = crate::transport::RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let bytes = bytes::Bytes::from(vec![0x81, 0x09, 0x00, 0x02, VISCA_TERMINATOR]);
        let priority = Priority::Normal;
        let category = CommandCategory::Quick;
        let camera_id = crate::camera_id::CameraId::CAMERA_1;

        // Start an inquiry
        core.start_inquiry(1, bytes.clone(), priority, category, camera_id, now);

        // Verify inquiry is tracked
        assert!(core.inquiries_inflight.contains_key(&1));
        assert!(core.inquiries_order.contains(&1));

        // Process InquiryReply event
        let response = ViscaResponse::Inquiry(crate::command::InquiryResponse::Power { on: true });
        let event = SchedulerEvent::InquiryReply {
            cmd_id: Some(1),
            response,
        };

        let actions = core.process_event(event, now);

        // Should get CommandComplete action
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandComplete { id, response: resp } => {
                assert_eq!(*id, 1);
                match resp {
                    ViscaResponse::Inquiry(crate::command::InquiryResponse::Power { on }) => {
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
        let retry_config = crate::transport::RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Quick;
        let camera_id = crate::camera_id::CameraId::CAMERA_1;

        // Start multiple inquiries in raw VISCA mode (no sequence)
        let bytes1 = bytes::Bytes::from(vec![0x81, 0x09, 0x00, 0x02, VISCA_TERMINATOR]);
        let bytes2 = bytes::Bytes::from(vec![0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR]);
        let bytes3 = bytes::Bytes::from(vec![0x81, 0x09, 0x06, 0x12, VISCA_TERMINATOR]);

        core.start_inquiry(1, bytes1, priority, category, camera_id, now);
        core.start_inquiry(2, bytes2, priority, category, camera_id, now);
        core.start_inquiry(3, bytes3, priority, category, camera_id, now);

        // Verify all inquiries are tracked in order
        assert_eq!(core.inquiries_order.len(), 3);
        assert_eq!(core.inquiries_order[0], 1);
        assert_eq!(core.inquiries_order[1], 2);
        assert_eq!(core.inquiries_order[2], 3);

        // Process InquiryReply events without cmd_id (raw VISCA)
        // First reply should match first inquiry
        let response1 = ViscaResponse::Inquiry(crate::command::InquiryResponse::Power { on: true });
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
        let retry_config = crate::transport::RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let camera_id = crate::camera_id::CameraId::CAMERA_1;

        // Register two commands with sequences (simulating Sony protocol)
        let bytes1 = bytes::Bytes::from(vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]);
        let bytes2 = bytes::Bytes::from(vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR]);

        core.register_pending_ack(
            1,
            bytes1,
            priority,
            CommandCategory::Movement,
            camera_id,
            now,
        );
        core.register_sequence(1, 100); // Command 1 has sequence 100

        core.register_pending_ack(
            2,
            bytes2,
            priority,
            CommandCategory::Movement,
            camera_id,
            now,
        );
        core.register_sequence(2, 101); // Command 2 has sequence 101

        // Process ACK for command 2 first (out of order)
        let cmd_id_2 = core.get_command_by_sequence(101);
        let event = SchedulerEvent::Ack {
            socket: ViscaSocket::S2,
            cmd_id: cmd_id_2, // Using sequence to identify
        };

        let actions = core.process_event(event, now);

        // ACK processing should be silent (no action returned)
        assert!(
            actions.is_empty(),
            "Expected no actions from ACK processing"
        );

        // Verify command 2 got socket 2
        let state = core.socket_state(ViscaSocket::S2);
        assert!(!state.0); // Socket occupied
        assert_eq!(state.1, Some(2)); // By command 2

        // Command 1 should still be pending
        assert!(core.pending_ack.contains_key(&1));

        // Now process ACK for command 1
        let cmd_id_1 = core.get_command_by_sequence(100);
        let event = SchedulerEvent::Ack {
            socket: ViscaSocket::S1,
            cmd_id: cmd_id_1,
        };

        core.process_event(event, now);

        // Verify command 1 got socket 1
        let state = core.socket_state(ViscaSocket::S1);
        assert!(!state.0); // Socket occupied
        assert_eq!(state.1, Some(1)); // By command 1
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

        // Start an inquiry
        core.start_inquiry(1, bytes.clone(), priority, category, camera_id, now);
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
        core.start_inquiry(1, bytes.clone(), priority, category, camera_id, later);
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
        let retry_config = crate::transport::RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let bytes = bytes::Bytes::from(vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]);
        let priority = Priority::Normal;
        let category = CommandCategory::Movement;
        let camera_id = crate::camera_id::CameraId::CAMERA_1;

        // Register command with initial sequence
        core.register_pending_ack(1, bytes.clone(), priority, category, camera_id, now);
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
        let retry_config = crate::transport::RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let bytes = bytes::Bytes::from(vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]);
        let priority = Priority::Normal;
        let category = CommandCategory::Movement;
        let camera_id = crate::camera_id::CameraId::CAMERA_1;

        // Register command with sequences from multiple retries
        core.register_pending_ack(1, bytes.clone(), priority, category, camera_id, now);
        core.register_sequence(1, 100);
        core.register_sequence(1, 101); // Retry 1
        core.register_sequence(1, 102); // Retry 2

        // Verify all sequences are active
        assert_eq!(core.get_command_by_sequence(100), Some(1));
        assert_eq!(core.get_command_by_sequence(101), Some(1));
        assert_eq!(core.get_command_by_sequence(102), Some(1));

        // Complete the command (simulating success on the third attempt)
        let response = ViscaResponse::Completion {
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
        let retry_config = crate::transport::RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let bytes = bytes::Bytes::from(vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]);
        let priority = Priority::Normal;
        let category = CommandCategory::Movement;
        let camera_id = crate::camera_id::CameraId::CAMERA_1;

        // Register command
        core.register_pending_ack(1, bytes.clone(), priority, category, camera_id, now);

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
        let retry_config = crate::transport::RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Movement;
        let camera_id = crate::camera_id::CameraId::CAMERA_1;

        // Register two different commands
        let bytes1 = bytes::Bytes::from(vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]);
        let bytes2 = bytes::Bytes::from(vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR]);

        core.register_pending_ack(1, bytes1.clone(), priority, category, camera_id, now);
        core.register_pending_ack(2, bytes2.clone(), priority, category, camera_id, now);

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
        let retry_config = crate::transport::RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let bytes = bytes::Bytes::from(vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]);
        let priority = Priority::Normal;
        let category = CommandCategory::Movement;
        let camera_id = crate::camera_id::CameraId::CAMERA_1;

        // Register a command with a 32-bit sequence that has non-zero high 16 bits
        let full_sequence = 0x12345678u32; // High 16 bits: 0x1234, Low 16 bits: 0x5678
        core.register_pending_ack(1, bytes.clone(), priority, category, camera_id, now);
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
        let retry_config = crate::transport::RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Movement;
        let camera_id = crate::camera_id::CameraId::CAMERA_1;

        // Register two commands with different 32-bit sequences but same lower 16 bits
        let seq1 = 0x12345678u32;
        let seq2 = 0xABCD5678u32; // Same lower 16 bits (0x5678) but different high bits

        let bytes1 = bytes::Bytes::from(vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]);
        let bytes2 = bytes::Bytes::from(vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR]);

        core.register_pending_ack(1, bytes1, priority, category, camera_id, now);
        core.register_sequence(1, seq1);

        core.register_pending_ack(2, bytes2, priority, category, camera_id, now);
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
        let retry_config = crate::transport::RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let bytes = bytes::Bytes::from(vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]);
        let priority = Priority::Normal;
        let category = CommandCategory::Movement;
        let camera_id = crate::camera_id::CameraId::CAMERA_1;

        // Register a command with sequence
        let sequence = 0x12345678u32;
        core.register_pending_ack(1, bytes, priority, category, camera_id, now);
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
        let retry_config = crate::transport::RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);
        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Quick;
        let camera_id = crate::camera_id::CameraId::CAMERA_1;

        // Start two different inquiries in raw VISCA mode
        let power_bytes = bytes::Bytes::from(vec![0x81, 0x09, 0x00, 0x02, VISCA_TERMINATOR]);
        let zoom_bytes = bytes::Bytes::from(vec![0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR]);

        core.start_inquiry(1, power_bytes, priority, category, camera_id, now);
        core.start_inquiry(2, zoom_bytes, priority, category, camera_id, now);

        // Verify both are tracked
        assert_eq!(core.inquiries_order.len(), 2);
        assert!(core.inquiries_inflight.contains_key(&1));
        assert!(core.inquiries_inflight.contains_key(&2));

        // Process replies out of order
        // Second inquiry (zoom) reply arrives first - with explicit cmd_id
        // (This simulates the adapter layer doing content-based matching)
        let zoom_response = ViscaResponse::Inquiry(crate::command::InquiryResponse::ZoomPosition {
            position: 0x1234,
        });
        let event2 = SchedulerEvent::InquiryReply {
            cmd_id: Some(2), // Content-based matching identified this as inquiry 2
            response: zoom_response,
        };

        let actions = core.process_event(event2, now);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandComplete { id, response } => {
                assert_eq!(*id, 2); // Second inquiry completed
                                    // Verify it's a zoom response
                match response {
                    ViscaResponse::Inquiry(crate::command::InquiryResponse::ZoomPosition {
                        position,
                    }) => {
                        assert_eq!(*position, 0x1234);
                    }
                    _ => panic!("Expected ZoomPosition response"),
                }
            }
            _ => panic!("Expected CommandComplete action"),
        }

        // First inquiry (power) reply arrives second
        let power_response =
            ViscaResponse::Inquiry(crate::command::InquiryResponse::Power { on: true });
        let event1 = SchedulerEvent::InquiryReply {
            cmd_id: Some(1), // Content-based matching identified this as inquiry 1
            response: power_response,
        };

        let actions = core.process_event(event1, now);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandComplete { id, response } => {
                assert_eq!(*id, 1); // First inquiry completed
                                    // Verify it's a power response
                match response {
                    ViscaResponse::Inquiry(crate::command::InquiryResponse::Power { on }) => {
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
}
