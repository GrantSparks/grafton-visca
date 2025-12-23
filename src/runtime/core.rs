//! Runtime-agnostic scheduler core state machine.
//!
//! This module implements the protocol state machine for VISCA command scheduling,
//! socket allocation, ACK/completion routing, and retry logic without any dependency
//! on async runtimes or channels.

use smallvec::SmallVec;
use tracing::{debug, trace, warn};

use std::{
    cell::Cell,
    cmp::Ordering as CmpOrdering,
    collections::{BinaryHeap, HashMap, HashSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    camera::inflight::CommandId,
    command::{
        encode::EncodedCommand,
        response::{parse_inquiry_payload, InquiryKind, Response},
        CommandKind,
    },
    timeout::{CommandCategory, TimeoutConfig},
    visca_socket::ViscaSocket,
    Error,
};

/// Complete lifecycle state for a single command.
///
/// This struct consolidates all per-command state that was previously scattered
/// across multiple HashMaps (pending_ack, command_metadata, retry_attempts,
/// retry_trigger_transport_error, inquiries_inflight, inquiry_response_types).
///
/// # Fields
/// - Core identity: `command`, `priority`, `category`, `camera_id`, `kind`
/// - Timing: `submitted_at`, `sent_at`
/// - Retry tracking: `attempt`, `transport_error`
/// - Inquiry-specific: `response_type`
#[derive(Debug, Clone)]
pub struct CommandState {
    /// Pre-encoded command bytes.
    pub command: Arc<EncodedCommand>,
    /// Scheduling priority.
    pub priority: Priority,
    /// Timeout category for calculating timeouts.
    pub category: CommandCategory,
    /// Target camera.
    pub camera_id: crate::camera_id::CameraId,
    /// Command vs Inquiry.
    pub kind: CommandKind,
    /// When first submitted to the scheduler.
    pub submitted_at: Instant,

    // --- Lifecycle tracking ---
    /// When sent over the wire (None if not yet sent).
    pub sent_at: Option<Instant>,
    /// Current retry attempt (0 = first try).
    pub attempt: u32,
    /// Whether last failure was a transport error.
    pub transport_error: bool,

    // --- Inquiry-specific (if kind == Inquiry) ---
    /// Expected response type for DataReply parsing.
    pub response_type: Option<InquiryKind>,
}

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
    /// Command ID (type-safe, non-zero).
    pub id: CommandId,
    /// The pre-encoded command to retry.
    pub command: Arc<EncodedCommand>,
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

/// Socket state tracking using typestate pattern.
///
/// This enum encodes the socket state invariants at compile-time:
/// - A socket is either `Free` or `Busy`
/// - A `Busy` socket always has a command_id, started_at, and category
/// - Invalid states (like free=true with command_id=Some) are unrepresentable
#[derive(Debug, Clone, Default)]
enum SocketState {
    /// Socket is available for allocation.
    #[default]
    Free,
    /// Socket is allocated to a command.
    Busy {
        /// The command ID currently using this socket.
        command_id: CommandId,
        /// When the command was assigned to this socket.
        started_at: Instant,
        /// Command category for timeout calculation.
        category: CommandCategory,
    },
}

impl SocketState {
    /// Returns true if the socket is free.
    #[inline]
    fn is_free(&self) -> bool {
        matches!(self, Self::Free)
    }

    /// Returns the command ID if the socket is busy, None if free.
    #[inline]
    fn command_id(&self) -> Option<CommandId> {
        match self {
            Self::Busy { command_id, .. } => Some(*command_id),
            Self::Free => None,
        }
    }
}

/// Priority queue item wrapper for commands.
#[derive(Clone)]
pub struct PendingCommand {
    /// Unique identifier for this command (type-safe, non-zero).
    pub id: CommandId,
    /// The pre-encoded command to send.
    pub command: Arc<EncodedCommand>,
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
        /// Command ID (type-safe).
        id: CommandId,
    },
    /// Command completed successfully.
    CommandComplete {
        /// Command ID (type-safe).
        id: CommandId,
        /// Command category.
        category: CommandCategory,
        /// Camera ID.
        camera_id: crate::camera_id::CameraId,
        /// Response from the camera.
        response: Response,
    },
    /// Command failed with error.
    CommandFailed {
        /// Command ID (type-safe).
        id: CommandId,
        /// Error that occurred.
        error: Error,
    },
    /// Retry a command.
    RetryCommand {
        /// Command ID (type-safe).
        id: CommandId,
        /// Delay before retrying.
        delay: Duration,
    },
    /// A command exceeded one of the scheduler's timeouts.
    Timeout {
        /// Command ID that timed out (type-safe).
        id: CommandId,
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
        /// Command ID (from sequence mapping when available, type-safe).
        cmd_id: Option<CommandId>,
        /// Sony sequence number from frame metadata (None for raw VISCA).
        ///
        /// When `Some(_)`, sequence correlation is authoritative: if `cmd_id` is `None`,
        /// this is a stale/unmatched sequenced reply that must be ignored (not re-attributed
        /// via socket or pending-ACK heuristics). When `None`, heuristic fallback is allowed.
        sequence: Option<u32>,
    },
    /// Command completed.
    Completion {
        /// Socket that completed (if known).
        socket: Option<ViscaSocket>,
        /// Command ID (from sequence mapping when available, type-safe).
        cmd_id: Option<CommandId>,
        /// Sony sequence number from frame metadata (None for raw VISCA).
        ///
        /// When `Some(_)`, sequence correlation is authoritative: if `cmd_id` is `None`,
        /// this is a stale/unmatched sequenced reply that must be ignored (not re-attributed
        /// via socket or pending-ACK heuristics). When `None`, heuristic fallback is allowed.
        sequence: Option<u32>,
        /// Response from the camera.
        response: Response,
    },
    /// Inquiry data reply (no socket allocation).
    InquiryReply {
        /// Command ID (from sequence mapping or order queue, type-safe).
        cmd_id: Option<CommandId>,
        /// Sony sequence number from frame metadata (None for raw VISCA).
        ///
        /// When `Some(_)`, sequence correlation is authoritative: if `cmd_id` is `None`,
        /// this is a stale/unmatched sequenced reply that must be ignored (not re-attributed
        /// via socket or FIFO heuristics). When `None`, heuristic fallback is allowed.
        sequence: Option<u32>,
        /// Response from the camera.
        response: Response,
    },
    /// Error response.
    Error {
        /// Socket that errored (if known).
        socket: Option<ViscaSocket>,
        /// Command ID (from sequence mapping when available, type-safe).
        cmd_id: Option<CommandId>,
        /// Sony sequence number from frame metadata (None for raw VISCA).
        ///
        /// When `Some(_)`, sequence correlation is authoritative: if `cmd_id` is `None`,
        /// this is a stale/unmatched sequenced reply that must be ignored (not re-attributed
        /// via socket or pending-ACK heuristics). When `None`, heuristic fallback is allowed.
        sequence: Option<u32>,
        /// Error code from the camera.
        code: u8,
    },
    /// Network error (for broadcast recovery).
    NetworkError(Error),
}

/// Maximum number of sequences to track per command.
///
/// This constant defines the cap for both 32-bit and 16-bit sequence histories.
/// When a command accumulates more sequences than this (due to retries), the oldest
/// sequence is evicted using FIFO ordering.
const MAX_SEQUENCES_PER_CMD: usize = 8;

/// Macro to generate sequence history types with allocation-free inline storage.
///
/// This avoids code duplication between SeqHistory32 and SeqHistory16 while
/// maintaining type safety and avoiding the limitations of const generics with SmallVec.
macro_rules! define_seq_history {
    ($name:ident, $elem_ty:ty) => {
        /// Sequence history backed by `SmallVec` for allocation-free storage.
        ///
        /// Stores up to 8 sequences inline. Uses FIFO eviction when full.
        #[derive(Debug, Clone)]
        struct $name {
            sequences: SmallVec<[$elem_ty; MAX_SEQUENCES_PER_CMD]>,
        }

        impl $name {
            /// Create a new history with a single initial sequence.
            fn new(seq: $elem_ty) -> Self {
                let mut sequences = SmallVec::new();
                sequences.push(seq);
                Self { sequences }
            }

            /// Add a sequence to the history.
            ///
            /// Returns the evicted (oldest) sequence if the history was at capacity.
            fn push(&mut self, seq: $elem_ty) -> Option<$elem_ty> {
                if self.sequences.len() >= MAX_SEQUENCES_PER_CMD {
                    let evicted = self.sequences.remove(0);
                    self.sequences.push(seq);
                    Some(evicted)
                } else {
                    self.sequences.push(seq);
                    None
                }
            }

            /// Iterate over all sequences in this history.
            fn iter(&self) -> impl Iterator<Item = $elem_ty> + '_ {
                self.sequences.iter().copied()
            }
        }
    };
}

define_seq_history!(SeqHistory32, u32);
define_seq_history!(SeqHistory16, u16);

/// Multi-owner tracking for 16-bit sequences.
///
/// A single 16-bit sequence value can map to multiple active commands when collisions
/// occur (two different 32-bit sequences with the same lower 16 bits). This type
/// tracks all command IDs that own a particular 16-bit sequence.
///
/// Uses `SmallVec<[CommandId; 2]>` since collisions are rare and typically involve only
/// 2 commands when they do occur.
#[derive(Debug, Clone)]
struct Seq16Owners {
    /// Command IDs that currently own this 16-bit sequence (type-safe).
    cmd_ids: SmallVec<[CommandId; 2]>,
}

impl Seq16Owners {
    /// Create a new Seq16Owners with a single owner.
    fn new(cmd_id: CommandId) -> Self {
        let mut cmd_ids = SmallVec::new();
        cmd_ids.push(cmd_id);
        Self { cmd_ids }
    }

    /// Add an owner to this sequence. Returns true if the owner was added
    /// (i.e., was not already present).
    fn add_owner(&mut self, cmd_id: CommandId) -> bool {
        if !self.cmd_ids.contains(&cmd_id) {
            self.cmd_ids.push(cmd_id);
            true
        } else {
            false
        }
    }

    /// Remove an owner from this sequence. Returns true if the owner was present
    /// and removed.
    fn remove_owner(&mut self, cmd_id: CommandId) -> bool {
        if let Some(pos) = self.cmd_ids.iter().position(|&id| id == cmd_id) {
            self.cmd_ids.remove(pos);
            true
        } else {
            false
        }
    }

    /// Check if this sequence has no remaining owners.
    fn is_empty(&self) -> bool {
        self.cmd_ids.is_empty()
    }

    /// Get the number of owners.
    fn len(&self) -> usize {
        self.cmd_ids.len()
    }

    /// Iterate over all owners.
    fn iter(&self) -> impl Iterator<Item = CommandId> + '_ {
        self.cmd_ids.iter().copied()
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
    /// Tracks whether we've logged the idle state (zero commands in flight).
    last_logged_idle: Cell<bool>,

    // === Unified command state ===
    /// All active commands indexed by ID.
    /// This consolidates what was previously scattered across multiple HashMaps.
    commands: HashMap<CommandId, CommandState>,
    /// Command IDs that have been sent but not yet acknowledged.
    /// Lightweight index set for O(1) membership queries.
    pending_ack_ids: HashSet<CommandId>,
    /// Inquiry IDs that are currently in flight.
    /// Lightweight index set for O(1) membership queries.
    inflight_inquiry_ids: HashSet<CommandId>,

    /// Commands waiting to be retried (after busy response).
    /// Uses a min-heap ordered by retry_at for efficient deadline-driven scheduling.
    retry_queue: BinaryHeap<RetryKey>,
    /// Priority queue for pending commands.
    command_queue: BinaryHeap<PendingCommand>,
    /// Priority queue for pending inquiries (separate from commands to avoid socket gating).
    inquiry_queue: BinaryHeap<PendingCommand>,
    /// Maximum number of inquiries that can be in flight simultaneously.
    max_inquiries_inflight: usize,
    /// Retry budget for command categories.
    retry_budget: RetryBudget,
    /// Sony sequence tracking: sequence -> command_id (sequence stays u32, ID is type-safe).
    seq_to_cmd: HashMap<u32, CommandId>,
    /// Sony sequence tracking: command_id -> sequences.
    cmd_to_seqs: HashMap<CommandId, SeqHistory32>,
    /// Sony 16-bit sequence tracking: lower 16 bits -> owning command(s).
    ///
    /// Multi-owner: A 16-bit sequence can map to multiple active commands when
    /// collisions occur (two 32-bit sequences with the same lower 16 bits).
    /// When one command finishes, the remaining command(s) become uniquely resolvable.
    seq16_to_cmds: HashMap<u16, Seq16Owners>,
    /// Sony 16-bit sequence tracking: command_id -> 16-bit sequences.
    cmd_to_seq16s: HashMap<CommandId, SeqHistory16>,
    /// Inquiry order tracking for raw VISCA (no sequence).
    inquiries_order: VecDeque<CommandId>,
    /// Minimum time spacing between consecutive inquiry sends.
    min_inquiry_spacing: Duration,
    /// When the last inquiry was sent (for spacing enforcement).
    last_inquiry_sent: Option<Instant>,
    /// Pending inquiry response types for commands that haven't been started yet.
    /// This is needed because response types are registered before the command state exists.
    pending_inquiry_types: HashMap<CommandId, InquiryKind>,
    /// Counter for ignored unmatched sequenced replies.
    ///
    /// Tracks the number of times a sequenced reply (ACK, Completion, Error, InquiryReply)
    /// was received with a sequence number that did not resolve to an active command.
    /// This is expected for stale/duplicate UDP packets and indicates the sequence
    /// correlation safety mechanism is working correctly.
    ignored_unmatched_sequenced_replies: u64,
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
            last_logged_idle: Cell::new(false),
            commands: HashMap::new(),
            pending_ack_ids: HashSet::new(),
            inflight_inquiry_ids: HashSet::new(),
            retry_queue: BinaryHeap::new(),
            command_queue: BinaryHeap::new(),
            inquiry_queue: BinaryHeap::new(),
            max_inquiries_inflight: 8, // Conservative default to avoid overwhelming devices
            retry_budget,
            seq_to_cmd: HashMap::new(),
            cmd_to_seqs: HashMap::new(),
            seq16_to_cmds: HashMap::new(),
            cmd_to_seq16s: HashMap::new(),
            inquiries_order: VecDeque::new(),
            min_inquiry_spacing: Duration::ZERO,
            last_inquiry_sent: None,
            pending_inquiry_types: HashMap::new(),
            ignored_unmatched_sequenced_replies: 0,
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

    /// Set the minimum spacing between consecutive inquiry sends.
    ///
    /// Some cameras (e.g., PTZOptics) cannot process inquiries faster than
    /// ~125-150ms apart. Setting this enforces a minimum delay between sends.
    pub fn set_min_inquiry_spacing(&mut self, spacing: Duration) {
        self.min_inquiry_spacing = spacing;
    }

    /// Get the minimum spacing between consecutive inquiry sends.
    pub fn min_inquiry_spacing(&self) -> Duration {
        self.min_inquiry_spacing
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
        let pending_count = self.pending_ack_ids.len();
        let allocated_count = self.sockets.iter().filter(|s| !s.is_free()).count();
        let total_in_flight = pending_count + allocated_count;

        let can_send = total_in_flight < 2;

        if !can_send && std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
            eprintln!(
                "[SchedulerCore] can_send_command=false: {} pending ACK + {} allocated = {}/2 capacity",
                pending_count, allocated_count, total_in_flight
            );
        }

        // Log state transitions: always log once when going to idle, otherwise only when busy
        if total_in_flight > 0 {
            trace!(
                "Commands in flight: {} pending ACK + {} allocated = {}/2",
                pending_count,
                allocated_count,
                total_in_flight
            );
            self.last_logged_idle.set(false);
        } else if !self.last_logged_idle.get() {
            trace!("Commands in flight: 0 pending ACK + 0 allocated = 0/2");
            self.last_logged_idle.set(true);
        }

        can_send
    }

    /// Check if we can send an inquiry (not at max capacity and spacing satisfied).
    ///
    /// Returns true if:
    /// 1. The number of in-flight inquiries is below the maximum limit
    /// 2. The minimum spacing requirement since the last inquiry has been satisfied
    pub fn can_send_inquiry(&self, now: Instant) -> bool {
        // Check concurrency limit
        if self.inflight_inquiry_ids.len() >= self.max_inquiries_inflight {
            return false;
        }

        // Check spacing requirement
        if let Some(last_sent) = self.last_inquiry_sent {
            if now.duration_since(last_sent) < self.min_inquiry_spacing {
                return false;
            }
        }

        true
    }

    /// Get the next item to send (inquiry or command).
    ///
    /// Selects the highest-priority item across both queues, respecting capacity limits:
    /// - Inquiries bypass socket allocation but respect max_inquiries_inflight
    /// - Commands require socket capacity (2-socket limit)
    ///
    /// When priorities are equal, inquiries are preferred for backwards compatibility
    /// (they're typically faster and don't hold sockets).
    ///
    /// # Priority-Aware Selection
    ///
    /// This method prevents command starvation by comparing priorities across queues.
    /// A High-priority command will be selected over a Normal-priority inquiry,
    /// ensuring user-initiated actions (preset save, etc.) aren't blocked by background
    /// polling inquiries.
    ///
    /// # Arguments
    ///
    /// * `now` - Current instant, used to check inquiry spacing requirements
    pub fn next_item_to_send(&mut self, now: Instant) -> Option<PendingCommand> {
        // Check what's available in each queue
        let inquiry_available = !self.inquiry_queue.is_empty() && self.can_send_inquiry(now);
        let command_available = self.can_send_command() && !self.command_queue.is_empty();

        match (inquiry_available, command_available) {
            (false, false) => None,
            (true, false) => self.inquiry_queue.pop(),
            (false, true) => self.command_queue.pop(),
            (true, true) => {
                // Both queues have items - compare priorities
                // peek() is safe here because we already checked is_empty()
                let inquiry_priority = self
                    .inquiry_queue
                    .peek()
                    .map(|c| c.priority)
                    .unwrap_or(Priority::Low);
                let command_priority = self
                    .command_queue
                    .peek()
                    .map(|c| c.priority)
                    .unwrap_or(Priority::Low);

                // If command has strictly higher priority, prefer it
                // Otherwise (equal or lower), prefer inquiry for backwards compatibility
                if command_priority > inquiry_priority {
                    self.command_queue.pop()
                } else {
                    self.inquiry_queue.pop()
                }
            }
        }
    }

    /// Register that a command was sent and is pending ACK.
    #[allow(clippy::too_many_arguments)]
    pub fn register_pending_ack(
        &mut self,
        id: CommandId,
        command: Arc<EncodedCommand>,
        priority: Priority,
        category: CommandCategory,
        camera_id: crate::camera_id::CameraId,
        kind: CommandKind,
        now: Instant,
    ) {
        // Update existing command state if it exists (preserves retry state),
        // otherwise create a new state
        if let Some(existing) = self.commands.get_mut(&id) {
            // Update send-related fields, preserve retry tracking
            existing.command = command;
            existing.priority = priority;
            existing.category = category;
            existing.camera_id = camera_id;
            existing.kind = kind;
            existing.sent_at = Some(now);
            // Note: preserve attempt, transport_error, response_type, and submitted_at
        } else {
            // New command - create fresh state
            let state = CommandState {
                command,
                priority,
                category,
                camera_id,
                kind,
                submitted_at: now,
                sent_at: Some(now),
                attempt: 0,
                transport_error: false,
                response_type: None,
            };
            self.commands.insert(id, state);
        }
        self.pending_ack_ids.insert(id);
        trace!(%id, "Registered command as pending ACK");
    }

    /// Register a Sony sequence number for a command.
    ///
    /// The `cmd_id` parameter is type-safe (CommandId), while `sequence` remains a raw
    /// protocol u32 since it comes from the wire.
    pub fn register_sequence(&mut self, cmd_id: CommandId, sequence: u32) {
        trace!(
            "Registering Sony sequence {} for command {}",
            sequence,
            cmd_id
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
            Some(seq_history) => {
                // Command already has sequences (this is a retry)
                if let Some(evicted) = seq_history.push(sequence) {
                    // Remove the evicted sequence from seq_to_cmd
                    self.seq_to_cmd.remove(&evicted);
                    trace!(
                        "Added retry sequence {} to command {}, evicted old sequence {}",
                        sequence,
                        cmd_id,
                        evicted
                    );
                } else {
                    trace!("Added retry sequence {sequence} to command {cmd_id}");
                }
            }
            None => {
                // First sequence for this command
                self.cmd_to_seqs.insert(cmd_id, SeqHistory32::new(sequence));
            }
        }

        // Also register the lower 16 bits for truncated-sequence compatibility
        let seq16 = (sequence & 0xFFFF) as u16;
        trace!(
            "Also registering 16-bit sequence {} (from {}) for command {}",
            seq16,
            sequence,
            cmd_id
        );

        // Add cmd_id as an owner of this 16-bit sequence (multi-owner for collision safety)
        match self.seq16_to_cmds.get_mut(&seq16) {
            Some(owners) => {
                if owners.add_owner(cmd_id) && owners.len() > 1 {
                    trace!(
                        "16-bit sequence {} collision: now owned by {} commands",
                        seq16,
                        owners.len()
                    );
                }
            }
            None => {
                self.seq16_to_cmds.insert(seq16, Seq16Owners::new(cmd_id));
            }
        }

        // Add 16-bit sequence to command's 16-bit sequence list
        match self.cmd_to_seq16s.get_mut(&cmd_id) {
            Some(seq16_history) => {
                // Command already has 16-bit sequences (this is a retry)
                if let Some(evicted) = seq16_history.push(seq16) {
                    // Remove this cmd_id from the evicted sequence's owners
                    self.remove_seq16_owner(evicted, cmd_id);
                    trace!(
                        "Added retry 16-bit sequence {} to command {}, evicted old sequence {}",
                        seq16,
                        cmd_id,
                        evicted
                    );
                } else {
                    trace!(
                        "Added retry 16-bit sequence {} to command {}",
                        seq16,
                        cmd_id
                    );
                }
            }
            None => {
                // First 16-bit sequence for this command
                self.cmd_to_seq16s.insert(cmd_id, SeqHistory16::new(seq16));
            }
        }

        trace!(
            "Sequence mappings: seq_to_cmd has {} entries, cmd_to_seqs has {} entries, seq16_to_cmds has {} entries, cmd_to_seq16s has {} entries",
            self.seq_to_cmd.len(),
            self.cmd_to_seqs.len(),
            self.seq16_to_cmds.len(),
            self.cmd_to_seq16s.len()
        );
    }

    /// Remove a command from a 16-bit sequence's owner list.
    ///
    /// If the owner list becomes empty after removal, the entry is deleted from
    /// `seq16_to_cmds`. This ensures proper cleanup when commands finish or are evicted.
    fn remove_seq16_owner(&mut self, seq16: u16, cmd_id: CommandId) {
        if let Some(owners) = self.seq16_to_cmds.get_mut(&seq16) {
            owners.remove_owner(cmd_id);
            if owners.is_empty() {
                self.seq16_to_cmds.remove(&seq16);
            }
        }
    }

    /// Get command ID for a Sony sequence number.
    ///
    /// First tries exact 32-bit match, then falls back to 16-bit match if unique.
    /// With the multi-owner 16-bit tracking, this method:
    /// - Returns `Some(cmd_id)` if exactly one active command owns the 16-bit sequence
    /// - Returns `None` if multiple active commands own it (ambiguous)
    /// - Returns `None` if no active commands own it
    pub fn get_command_by_sequence(&self, sequence: u32) -> Option<CommandId> {
        // 1. Try exact 32-bit match first
        if let Some(cmd_id) = self.seq_to_cmd.get(&sequence).copied() {
            // Extra safety: verify the command is still active
            if self.commands.contains_key(&cmd_id) {
                trace!(
                    "Found exact 32-bit sequence match for {}: command {}",
                    sequence,
                    cmd_id
                );
                return Some(cmd_id);
            } else {
                trace!(
                    "Ignoring stale 32-bit sequence {} for completed command {}",
                    sequence,
                    cmd_id
                );
            }
        }

        // 2. Try 16-bit fallback (lower 16 bits) using multi-owner tracking
        let seq16 = (sequence & 0xFFFF) as u16;
        if let Some(owners) = self.seq16_to_cmds.get(&seq16) {
            // Filter to only active owners
            let active_owners: SmallVec<[CommandId; 2]> = owners
                .iter()
                .filter(|&cmd_id| self.commands.contains_key(&cmd_id))
                .collect();

            match active_owners.len() {
                1 => {
                    let cmd_id = active_owners[0];
                    trace!(
                        "Found unique 16-bit sequence match for {} (seq16 {}): command {}",
                        sequence,
                        seq16,
                        cmd_id
                    );
                    return Some(cmd_id);
                }
                n if n > 1 => {
                    debug!(
                        "Ambiguous 16-bit sequence {} (from {}) maps to {} active commands: {:?}",
                        seq16,
                        sequence,
                        n,
                        active_owners.as_slice()
                    );
                }
                _ => {
                    trace!(
                        "16-bit sequence {} (from {}) has no active owners",
                        seq16,
                        sequence
                    );
                }
            }
        }

        // No unique match found
        None
    }

    /// Finish a command by sequence number (Sony protocol).
    ///
    /// Cleans up all sequence mappings for the command:
    /// - Removes 32-bit sequence -> cmd mappings
    /// - Removes cmd_id from 16-bit sequence owner lists (multi-owner safe)
    /// - Removes cmd -> sequences history entries
    pub fn finish_sequence(&mut self, cmd_id: CommandId) {
        // Remove all 32-bit sequences for this command
        let mut count32 = 0;
        if let Some(seq_history) = self.cmd_to_seqs.remove(&cmd_id) {
            for seq in seq_history.iter() {
                self.seq_to_cmd.remove(&seq);
                count32 += 1;
            }
        }

        // Remove cmd_id from all 16-bit sequence owner lists (collision-safe)
        let mut count16 = 0;
        if let Some(seq16_history) = self.cmd_to_seq16s.remove(&cmd_id) {
            for seq16 in seq16_history.iter() {
                self.remove_seq16_owner(seq16, cmd_id);
                count16 += 1;
            }
        }

        trace!(
            "Cleaned up {} Sony sequence(s) and {} 16-bit sequence(s) for command {}",
            count32,
            count16,
            cmd_id
        );
    }

    /// Check if a command is still active (tracked by the scheduler).
    ///
    /// A command is considered active if it exists in `command_metadata`
    /// (for regular commands) or in `inquiries_inflight` (for inquiries).
    /// This matches the staleness checks used in `get_command_by_sequence`.
    ///
    /// This is used to filter out stale retries in `get_ready_retries`.
    fn is_command_active(&self, cmd_id: CommandId) -> bool {
        self.commands.contains_key(&cmd_id)
    }

    /// Cancel a command, cleaning up all associated state.
    ///
    /// This is used when a command needs to be abandoned due to an external
    /// deadline being exceeded. It cleans up:
    /// - Sequence mappings
    /// - Pending ACK state
    /// - Command metadata
    /// - Retry state
    /// - Inquiry state
    /// - Socket allocations
    pub fn cancel_command(&mut self, cmd_id: CommandId) {
        trace!("Cancelling command {cmd_id}");

        // Free any allocated socket
        for socket_idx in 0..2 {
            if self.sockets[socket_idx].command_id() == Some(cmd_id) {
                let socket = if socket_idx == 0 {
                    ViscaSocket::S1
                } else {
                    ViscaSocket::S2
                };
                self.free_socket(socket);
            }
        }

        // Clean up sequence mappings
        self.finish_sequence(cmd_id);

        // Remove from index sets
        self.pending_ack_ids.remove(&cmd_id);
        self.inflight_inquiry_ids.remove(&cmd_id);
        self.inquiries_order.retain(|&id| id != cmd_id);

        // Remove unified command state
        self.commands.remove(&cmd_id);

        // Remove from command/inquiry queues if still there
        // Note: BinaryHeap doesn't support removal by value, so we drain and rebuild
        let old_cmd_queue = std::mem::take(&mut self.command_queue);
        for cmd in old_cmd_queue.into_iter() {
            if cmd.id != cmd_id {
                self.command_queue.push(cmd);
            }
        }

        let old_inq_queue = std::mem::take(&mut self.inquiry_queue);
        for cmd in old_inq_queue.into_iter() {
            if cmd.id != cmd_id {
                self.inquiry_queue.push(cmd);
            }
        }

        // Remove from retry queue if present
        let old_retries = std::mem::take(&mut self.retry_queue);
        for retry_key in old_retries.into_iter() {
            if retry_key.command.id != cmd_id {
                self.retry_queue.push(retry_key);
            }
        }

        trace!("Command {cmd_id} cancelled and all state cleaned up");
    }

    /// Complete an inquiry, cleaning up tracking state.
    ///
    /// This should be called when an inquiry response is received and the
    /// caller is returning early (not going through `process_event`).
    /// It removes the inquiry from inflight tracking to prevent stale
    /// command IDs from causing response misrouting.
    pub fn complete_inquiry(&mut self, cmd_id: CommandId) {
        trace!("Completing inquiry {cmd_id}");

        // Remove from index sets
        self.inflight_inquiry_ids.remove(&cmd_id);
        self.pending_ack_ids.remove(&cmd_id);
        self.inquiries_order.retain(|&id| id != cmd_id);

        // Clean up sequence mappings
        self.finish_sequence(cmd_id);

        // Remove unified command state
        self.commands.remove(&cmd_id);
    }

    /// Complete a command, cleaning up tracking state.
    ///
    /// This should be called when a command completion is received and the
    /// caller is returning early (not going through `process_event`).
    pub fn complete_command(&mut self, cmd_id: CommandId) {
        trace!("Completing command {cmd_id}");

        // Clean up sequence mappings
        self.finish_sequence(cmd_id);

        // Remove from index sets
        self.pending_ack_ids.remove(&cmd_id);
        self.inflight_inquiry_ids.remove(&cmd_id);

        // Remove unified command state
        self.commands.remove(&cmd_id);
    }

    /// Unregister a pending ACK without removing command metadata.
    /// This is used for rollback when a send operation fails.
    /// Returns true if the command was found and removed from pending_ack_ids.
    pub fn unregister_pending_ack(&mut self, id: CommandId) -> bool {
        self.pending_ack_ids.remove(&id)
    }

    /// Register the expected response type for an inquiry.
    pub fn register_inquiry_type(&mut self, id: CommandId, ty: InquiryKind) {
        // Update the response_type in the command state if it exists
        if let Some(state) = self.commands.get_mut(&id) {
            state.response_type = Some(ty);
        } else {
            // Command state doesn't exist yet - store in pending map
            // This will be merged when the command is started
            self.pending_inquiry_types.insert(id, ty);
        }
    }

    /// Take the response type for an inquiry (removing it from storage).
    pub fn take_inquiry_type(&mut self, id: CommandId) -> Option<InquiryKind> {
        self.commands
            .get_mut(&id)
            .and_then(|state| state.response_type.take())
            .or_else(|| self.pending_inquiry_types.remove(&id))
    }

    /// Get the response type for an inquiry (without removing it).
    pub fn get_inquiry_type(&self, id: CommandId) -> Option<InquiryKind> {
        self.commands
            .get(&id)
            .and_then(|state| state.response_type)
            .or_else(|| self.pending_inquiry_types.get(&id).copied())
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
    ) -> Option<CommandId> {
        use tracing::{debug, trace};

        // Format payload as hex for debugging (lazy evaluation)
        let payload_hex = || {
            payload
                .as_slice()
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ")
        };

        // First try sequence-based resolution if available
        if let Some(seq) = sequence {
            if let Some(cmd_id) = self.get_command_by_sequence(seq) {
                // Verify it's an active inquiry
                if self.inflight_inquiry_ids.contains(&cmd_id) {
                    trace!(%cmd_id, sequence = seq, "Resolved inquiry via sequence");
                    return Some(cmd_id);
                } else {
                    // This is unusual - sequence maps to a command but it's not active
                    debug!(
                        sequence = seq,
                        %cmd_id, "Sequence maps to inactive inquiry - will try content matching"
                    );
                }
            } else {
                trace!(sequence = seq, "Sequence not found in mappings");
            }
        }

        // Try content-based matching for raw VISCA
        // Build a map of active inquiries with their types
        let active_inquiries: HashMap<CommandId, InquiryKind> = self
            .inflight_inquiry_ids
            .iter()
            .filter_map(|&id| {
                self.commands
                    .get(&id)
                    .and_then(|state| state.response_type)
                    .map(|ty| (id, ty))
            })
            .collect();

        trace!(
            count = active_inquiries.len(),
            payload = payload_hex(),
            "Testing payload against active inquiries"
        );

        // Try parsing the payload against each expected response type
        let mut matches: Vec<CommandId> = Vec::new();

        for (id, response_type) in active_inquiries.iter() {
            // Use the existing zero-allocation parser
            // If parsing succeeds, this inquiry type matches the payload
            match parse_inquiry_payload(payload.as_slice(), response_type) {
                Ok(_) => {
                    trace!(cmd_id = %id, inquiry_type = ?response_type, "Matched");
                    matches.push(*id);
                }
                Err(_e) => {
                    trace!(cmd_id = %id, inquiry_type = ?response_type, "No match");
                }
            }
        }

        // Remove duplicates (defensive, shouldn't happen with unique IDs)
        matches.dedup();

        match matches.len() {
            0 => {
                // No match - fall back to FIFO as last resort
                let fifo_front = self.inquiries_order.front().copied();

                // Only log at DEBUG when FIFO fallback actually happens (indicates potential issue)
                if fifo_front.is_some() {
                    debug!(
                        cmd_id = ?fifo_front,
                        payload = payload_hex(),
                        active_count = active_inquiries.len(),
                        "Content matching failed, using FIFO fallback"
                    );
                } else if !active_inquiries.is_empty() {
                    // This is unexpected - we have active inquiries but none matched and FIFO is empty
                    debug!(
                        payload = payload_hex(),
                        active_count = active_inquiries.len(),
                        "No inquiry match and FIFO empty - inquiry may be orphaned"
                    );
                }
                fifo_front
            }
            1 => {
                // Unique match found - this is the expected path, only log at TRACE
                trace!(cmd_id = %matches[0], "Content match successful");
                Some(matches[0])
            }
            _ => {
                // Ambiguous - multiple inquiries match the same payload type
                // This is unusual and worth logging at DEBUG
                let fifo_front = self.inquiries_order.front().copied();
                debug!(
                    candidates = ?matches,
                    fifo_fallback = ?fifo_front,
                    payload = payload_hex(),
                    "Ambiguous inquiry match - using FIFO to disambiguate"
                );
                fifo_front
            }
        }
    }

    /// Process an event and return any actions to take.
    pub fn process_event(&mut self, event: SchedulerEvent, now: Instant) -> Vec<SchedulerAction> {
        let mut actions = Vec::new();

        match event {
            SchedulerEvent::Ack {
                socket,
                cmd_id,
                sequence,
            } => {
                // If sequence is present but cmd_id is None, this is an unmatched sequenced reply.
                // For sequenced transports, sequence correlation is authoritative - ignore stale replies.
                if sequence.is_some() && cmd_id.is_none() {
                    debug!(
                        "Ignoring unmatched sequenced ACK (sequence={:?}, socket={:?})",
                        sequence, socket
                    );
                    self.ignored_unmatched_sequenced_replies += 1;
                    return actions;
                }

                if let Some(cmd_id) = self.handle_ack_with_id(socket, cmd_id, now) {
                    trace!("Command {cmd_id} assigned to socket {socket:?}");
                }
            }
            SchedulerEvent::Completion {
                socket,
                cmd_id,
                sequence,
                response,
            } => {
                // Resolve command ID: prefer sequence-based cmd_id, then socket fallback (raw VISCA only).
                let resolved_cmd_id = if let Some(id) = cmd_id {
                    Some(id)
                } else if sequence.is_some() {
                    // Sequenced reply with no cmd_id match: this is a stale/duplicate reply.
                    // Do NOT fall back to socket-based heuristics - that would misattribute.
                    debug!(
                        "Ignoring unmatched sequenced Completion (sequence={:?}, socket={:?})",
                        sequence, socket
                    );
                    self.ignored_unmatched_sequenced_replies += 1;
                    None
                } else if let Some(socket) = socket {
                    // Raw VISCA (no sequence): fall back to socket-based correlation.
                    self.find_command_on_socket(socket)
                } else {
                    // No sequence, no socket: cannot attribute.
                    None
                };

                if let Some(cmd_id) = resolved_cmd_id {
                    if let Some(socket) = socket.or_else(|| self.find_socket_for_command(cmd_id)) {
                        self.free_socket(socket);
                    }
                    self.finish_sequence(cmd_id);
                    // Extract metadata before removing it
                    let (category, camera_id) = if let Some(state) = self.commands.get(&cmd_id) {
                        (state.category, state.camera_id)
                    } else {
                        // Fallback for commands without metadata (shouldn't happen)
                        (CommandCategory::Quick, crate::camera_id::CameraId::CAMERA_1)
                    };
                    // Remove from index sets and unified state
                    self.pending_ack_ids.remove(&cmd_id);
                    self.inflight_inquiry_ids.remove(&cmd_id);
                    self.commands.remove(&cmd_id);
                    actions.push(SchedulerAction::CommandComplete {
                        id: cmd_id,
                        category,
                        camera_id,
                        response,
                    });
                }
            }
            SchedulerEvent::InquiryReply {
                cmd_id,
                sequence,
                response,
            } => {
                // Resolve inquiry ID from sequence or order queue
                let resolved_cmd_id = if let Some(id) = cmd_id {
                    // Remove from order queue if present (for sequence-based reply)
                    self.inquiries_order.retain(|&x| x != id);
                    Some(id)
                } else if sequence.is_some() {
                    // Sequenced reply with no cmd_id match: stale/duplicate inquiry reply.
                    // Do NOT fall back to FIFO order queue - that would misattribute.
                    debug!(
                        "Ignoring unmatched sequenced InquiryReply (sequence={:?})",
                        sequence
                    );
                    self.ignored_unmatched_sequenced_replies += 1;
                    None
                } else {
                    // Raw VISCA (no sequence): pop from order queue.
                    self.inquiries_order.pop_front()
                };

                if let Some(cmd_id) = resolved_cmd_id {
                    // Remove from inflight tracking
                    self.inflight_inquiry_ids.remove(&cmd_id);
                    // Clean up sequence mappings
                    self.finish_sequence(cmd_id);
                    // Extract metadata before removing it
                    let (category, camera_id) = if let Some(state) = self.commands.get(&cmd_id) {
                        (state.category, state.camera_id)
                    } else {
                        // Fallback for inquiries without metadata
                        (CommandCategory::Quick, crate::camera_id::CameraId::CAMERA_1)
                    };
                    // Remove unified state
                    self.pending_ack_ids.remove(&cmd_id);
                    self.commands.remove(&cmd_id);
                    // Complete the inquiry
                    actions.push(SchedulerAction::CommandComplete {
                        id: cmd_id,
                        category,
                        camera_id,
                        response,
                    });
                    trace!("Inquiry {cmd_id} completed with response");
                }
            }
            SchedulerEvent::Error {
                socket,
                cmd_id,
                sequence,
                code,
            } => {
                let error = ViscaError::from_byte(code);

                // Resolve which command this error belongs to.
                // For sequenced transports: sequence correlation is authoritative.
                // For raw VISCA: use socket/FIFO/temporal heuristics.
                let resolved_cmd_id = if let Some(id) = cmd_id {
                    Some(id)
                } else if sequence.is_some() {
                    // Sequenced reply with no cmd_id match: stale/duplicate error.
                    // Do NOT fall back to socket or temporal heuristics - that would misattribute.
                    debug!(
                        "Ignoring unmatched sequenced Error (sequence={:?}, socket={:?}, code=0x{:02X})",
                        sequence, socket, code
                    );
                    self.ignored_unmatched_sequenced_replies += 1;
                    None
                } else if let Some(sock) = socket {
                    // Raw VISCA: socket nibble present. Try socket mapping, then temporal fallback.
                    // Some cameras emit 90 6y EE without a prior ACK; the socket nibble may not be reliable.
                    self.find_command_on_socket(sock).or_else(|| {
                        // Fall back to most-recent pending ACK when no command is actually allocated to that socket.
                        self.pending_ack_ids
                            .iter()
                            .filter_map(|&id| {
                                self.commands
                                    .get(&id)
                                    .and_then(|s| s.sent_at)
                                    .map(|t| (id, t))
                            })
                            .max_by_key(|(_, sent_time)| *sent_time)
                            .map(|(id, _)| id)
                    })
                } else {
                    // Raw VISCA: y == 0 case (inquiry errors and some syntax errors).
                    // If an inquiry is in flight, attribute to the oldest. Otherwise, use temporal correlation.
                    self.inquiries_order.front().copied().or_else(|| {
                        self.pending_ack_ids
                            .iter()
                            .filter_map(|&id| {
                                self.commands
                                    .get(&id)
                                    .and_then(|s| s.sent_at)
                                    .map(|t| (id, t))
                            })
                            .max_by_key(|(_, sent_time)| *sent_time)
                            .map(|(id, _)| id)
                    })
                };

                if let Some(cmd_id) = resolved_cmd_id {
                    // Remove from pending_ack_ids if it's there (for immediate errors without ACK)
                    self.pending_ack_ids.remove(&cmd_id);
                    // Check if this is an inquiry
                    let is_inquiry = self.inflight_inquiry_ids.contains(&cmd_id);

                    if is_inquiry {
                        // Remove from inquiry tracking
                        self.inflight_inquiry_ids.remove(&cmd_id);
                        self.inquiries_order.retain(|&id| id != cmd_id);
                    }

                    let should_retry = self.should_retry_command(cmd_id, &error, now);

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
                        self.commands.remove(&cmd_id);
                        actions.push(SchedulerAction::CommandFailed {
                            id: cmd_id,
                            error: Error::from_code(code),
                        });
                    }
                } else if sequence.is_none() {
                    // Raw VISCA: as a last resort, never drop protocol errors on the floor.
                    // If nothing is pending, still surface the error for visibility.
                    // (No state to clean in this rare path.)
                    warn!("Unattributed VISCA error 0x{:02X} received; no pending commands or inquiries to fail", code);
                }
                // For sequenced but unmatched errors, we already logged and incremented the counter above.
            }
            SchedulerEvent::NetworkError(error) => {
                // Network error - retry all pending commands
                let pending_cmds: Vec<_> = self.pending_ack_ids.iter().copied().collect();
                // Check if this is a transport error
                let is_transport_error = matches!(error, Error::TransportError(_));
                for cmd_id in pending_cmds {
                    // Store whether this retry was triggered by a transport error
                    if let Some(state) = self.commands.get_mut(&cmd_id) {
                        state.transport_error = is_transport_error;
                    }
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

            if let SocketState::Busy {
                command_id,
                started_at,
                category,
            } = &self.sockets[socket_idx]
            {
                let timeout = self.timeout_config.get_timeout(*category);

                if now.duration_since(*started_at) > timeout {
                    warn!(
                        "Command {} on socket {:?} timed out after {:?}",
                        command_id, socket, timeout
                    );
                    if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                        eprintln!(
                            "[SchedulerCore] Socket timeout: cmd_id={}, socket={:?}, category={:?}, duration={:?}",
                            command_id, socket, category, now.duration_since(*started_at)
                        );
                    }
                    timed_out.push((socket, *command_id));
                }
            }
        }

        // Check inquiry timeouts
        let mut timed_out_inquiries = Vec::new();
        for &cmd_id in &self.inflight_inquiry_ids {
            if let Some(state) = self.commands.get(&cmd_id) {
                if let Some(sent_at) = state.sent_at {
                    let timeout = self.timeout_config.get_timeout(state.category);
                    let elapsed = now.duration_since(sent_at);
                    if elapsed > timeout {
                        warn!(
                            %cmd_id,
                            inquiry_type = ?state.response_type,
                            timeout = ?timeout,
                            elapsed = ?elapsed,
                            "Inquiry timed out"
                        );
                        timed_out_inquiries.push(cmd_id);
                    }
                }
            }
        }

        // Handle timed out inquiries
        for cmd_id in timed_out_inquiries {
            self.inflight_inquiry_ids.remove(&cmd_id);
            // Remove from order queue if present
            self.inquiries_order.retain(|&id| id != cmd_id);
            // Check if we should retry (budget and duration checks)
            let should_retry = self
                .commands
                .get(&cmd_id)
                .map(|state| {
                    let max_retries = self.retry_budget.for_category(state.category);
                    let within_duration = now.duration_since(state.submitted_at)
                        < self.retry_config.max_retry_duration;
                    state.attempt < max_retries && within_duration
                })
                .unwrap_or(false);

            if should_retry {
                if let Some(retry_action) = self.queue_retry_for_command(cmd_id, now) {
                    actions.push(retry_action);
                }
            } else {
                self.finish_sequence(cmd_id);
                self.commands.remove(&cmd_id);
                actions.push(SchedulerAction::CommandFailed {
                    id: cmd_id,
                    error: Error::Timeout,
                });
            }
        }

        // Check pending-ACK timeouts (commands sent but not yet acknowledged)
        let ack_timeout = self.timeout_config.ack_timeout;
        let mut ack_timed_out = Vec::new();

        for &cmd_id in &self.pending_ack_ids {
            if let Some(state) = self.commands.get(&cmd_id) {
                if let Some(sent_at) = state.sent_at {
                    let elapsed = now.duration_since(sent_at);
                    if elapsed > ack_timeout {
                        warn!(
                            "Command {} timed out waiting for ACK after {:?}",
                            cmd_id, ack_timeout
                        );
                        ack_timed_out.push(cmd_id);
                    }
                }
            }
        }

        // Process all timed-out ACK commands
        for cmd_id in ack_timed_out {
            // Get state before modification
            let state_copy = self.commands.get(&cmd_id).cloned();

            if let Some(state) = state_copy {
                // Remove from pending_ack_ids FIRST to free capacity
                self.pending_ack_ids.remove(&cmd_id);

                if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                    eprintln!(
                        "[SchedulerCore] ACK timeout: cmd_id={}, removed from pending_ack_ids (count={})",
                        cmd_id,
                        self.pending_ack_ids.len()
                    );
                }

                // Debug assertion: command should not be in both pending_ack_ids and have a socket
                #[cfg(debug_assertions)]
                {
                    for socket in &self.sockets {
                        if socket.command_id() == Some(cmd_id) {
                            eprintln!(
                                "ERROR: Invariant violation: ACK-timed-out command {} still has socket allocated",
                                cmd_id
                            );
                            debug_assert!(false, "ACK timeout invariant violation");
                        }
                    }
                }

                // Get retry count for this command
                let attempts = state.attempt;
                let max_retries = self.retry_budget.for_category(state.category);

                // Check if within max_retry_duration
                let within_duration =
                    now.duration_since(state.submitted_at) < self.retry_config.max_retry_duration;

                // Determine if we will retry (both budget and duration must allow it)
                let will_retry = attempts < max_retries && within_duration;

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

                    // Update retry count in command state
                    if let Some(cmd_state) = self.commands.get_mut(&cmd_id) {
                        cmd_state.attempt = attempts + 1;
                    }

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
                        command: state.command.clone(),
                        priority: state.priority,
                        category: state.category,
                        camera_id: state.camera_id,
                        kind: state.kind,
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
                    self.commands.remove(&cmd_id);

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

            // Check if we should retry (budget and duration checks)
            let should_retry = self
                .commands
                .get(&cmd_id)
                .map(|state| {
                    let max_retries = self.retry_budget.for_category(state.category);
                    let within_duration = now.duration_since(state.submitted_at)
                        < self.retry_config.max_retry_duration;
                    state.attempt < max_retries && within_duration
                })
                .unwrap_or(false);

            if should_retry {
                if let Some(retry_action) = self.queue_retry_for_command(cmd_id, now) {
                    actions.push(retry_action);
                }
            } else {
                self.finish_sequence(cmd_id);
                self.commands.remove(&cmd_id);
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

    /// Get the total pending queue depth (commands + inquiries waiting to be sent).
    ///
    /// This is the primary metric for admission control and queue depth reporting.
    /// It includes all items queued but not yet sent to the transport:
    /// - Commands waiting in the command queue
    /// - Inquiries waiting in the inquiry queue
    ///
    /// Note: This does NOT include commands that have been sent but are awaiting
    /// response (pending_ack, inquiries_inflight), as those have already been
    /// accepted and are tracked separately.
    pub fn pending_queue_depth(&self) -> usize {
        self.command_queue.len() + self.inquiry_queue.len()
    }

    /// Get commands that are ready to retry.
    ///
    /// This method filters out stale retries by validating that:
    /// 1. The command is still active (exists in `command_metadata` or `inquiries_inflight`)
    /// 2. The retry attempt matches the current `retry_attempts` count
    ///
    /// This prevents re-sending commands that have already completed or failed,
    /// and ensures that only the latest retry entry for a command is executed
    /// (dropping any superseded entries from overlapping timeout/error paths).
    pub fn get_ready_retries(&mut self, now: Instant) -> Vec<RetryCommand> {
        let mut ready = Vec::new();

        // Pop commands from the heap while they're ready
        while let Some(retry_key) = self.retry_queue.peek() {
            if retry_key.command.retry_at <= now {
                // Pop the ready command - we know it exists because we just peeked
                if let Some(retry_key) = self.retry_queue.pop() {
                    let cmd_id = retry_key.command.id;
                    let retry_attempt = retry_key.command.attempt;

                    // Check if command is still active
                    if !self.is_command_active(cmd_id) {
                        trace!(
                            %cmd_id,
                            attempt = retry_attempt,
                            "Dropping stale retry: command no longer active"
                        );
                        continue;
                    }

                    // Check if this retry attempt is still current
                    // (prevents executing superseded retries from overlapping timeout paths)
                    let current_attempt =
                        self.commands.get(&cmd_id).map(|s| s.attempt).unwrap_or(0);
                    if retry_attempt != current_attempt {
                        trace!(
                            %cmd_id,
                            queued_attempt = retry_attempt,
                            current_attempt,
                            "Dropping stale retry: attempt count mismatch"
                        );
                        continue;
                    }

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
        for &cmd_id in &self.pending_ack_ids {
            if let Some(state) = self.commands.get(&cmd_id) {
                if let Some(sent_at) = state.sent_at {
                    let deadline = sent_at + self.timeout_config.ack_timeout;
                    earliest = match earliest {
                        None => Some(deadline),
                        Some(e) if deadline < e => Some(deadline),
                        _ => earliest,
                    };
                }
            }
        }

        // Check socket command timeouts
        for socket_state in &self.sockets {
            if let SocketState::Busy {
                started_at,
                category,
                ..
            } = socket_state
            {
                let timeout = self.timeout_config.get_timeout(*category);
                let deadline = *started_at + timeout;
                earliest = match earliest {
                    None => Some(deadline),
                    Some(e) if deadline < e => Some(deadline),
                    _ => earliest,
                };
            }
        }

        // Check inquiry timeouts
        for &cmd_id in &self.inflight_inquiry_ids {
            if let Some(state) = self.commands.get(&cmd_id) {
                if let Some(sent_at) = state.sent_at {
                    let timeout = self.timeout_config.get_timeout(state.category);
                    let deadline = sent_at + timeout;
                    earliest = match earliest {
                        None => Some(deadline),
                        Some(e) if deadline < e => Some(deadline),
                        _ => earliest,
                    };
                }
            }
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

        // Check inquiry spacing deadline (when queued inquiries can be sent)
        if !self.inquiry_queue.is_empty() && !self.min_inquiry_spacing.is_zero() {
            if let Some(last_sent) = self.last_inquiry_sent {
                let next_inquiry_eligible = last_sent + self.min_inquiry_spacing;
                earliest = match earliest {
                    None => Some(next_inquiry_eligible),
                    Some(e) if next_inquiry_eligible < e => Some(next_inquiry_eligible),
                    _ => earliest,
                };
            }
        }

        earliest
    }

    // Private helper methods

    fn handle_ack_with_id(
        &mut self,
        socket: Option<ViscaSocket>,
        cmd_id: Option<CommandId>,
        now: Instant,
    ) -> Option<CommandId> {
        // Prefer cmd_id from sequence mapping
        let target_id = if let Some(id) = cmd_id {
            // Verify it's actually pending
            if self.pending_ack_ids.contains(&id) {
                Some(id)
            } else {
                debug!("ACK with sequence {} not found in pending commands", id);
                None
            }
        } else {
            // Fall back to oldest pending command (FIFO order for raw VISCA)
            self.pending_ack_ids
                .iter()
                .filter_map(|&id| {
                    self.commands
                        .get(&id)
                        .and_then(|s| s.sent_at)
                        .map(|t| (id, t))
                })
                .min_by_key(|(_, sent_time)| *sent_time)
                .map(|(id, _)| id)
        }?;

        // Get command state
        let state_copy = self.commands.get(&target_id).cloned();
        if let Some(cmd_state) = state_copy {
            // Remove from pending_ack_ids
            self.pending_ack_ids.remove(&target_id);

            // Determine which socket to use with fallback logic
            let assigned_socket = if let Some(s) = socket {
                // Camera specified a socket - try to use it
                let idx = s.as_index();
                if self.sockets[idx].is_free() {
                    // Requested socket is free, use it
                    s
                } else {
                    // Requested socket is busy, try the other one
                    let other = if s == ViscaSocket::S1 {
                        ViscaSocket::S2
                    } else {
                        ViscaSocket::S1
                    };

                    if self.sockets[other.as_index()].is_free() {
                        debug!(
                            "Camera requested {:?} but it's occupied, using {:?} instead",
                            s, other
                        );
                        other
                    } else {
                        // Both sockets are busy
                        warn!("Camera assigned {:?} but both sockets are occupied", s);
                        // Re-insert command into pending_ack_ids since we couldn't assign it
                        self.pending_ack_ids.insert(target_id);
                        return None;
                    }
                }
            } else {
                // No socket specified - pick the first free one
                if self.sockets[ViscaSocket::S1.as_index()].is_free() {
                    ViscaSocket::S1
                } else if self.sockets[ViscaSocket::S2.as_index()].is_free() {
                    ViscaSocket::S2
                } else {
                    // Both sockets are busy
                    warn!("ACK received without socket nibble but both sockets are occupied");
                    // Re-insert command into pending_ack_ids since we couldn't assign it
                    self.pending_ack_ids.insert(target_id);
                    return None;
                }
            };

            // Allocate the chosen socket
            let idx = assigned_socket.as_index();
            self.sockets[idx] = SocketState::Busy {
                command_id: target_id,
                started_at: now,
                category: cmd_state.category,
            };

            trace!(
                "Assigned command {} to {:?} per camera ACK",
                target_id,
                assigned_socket
            );
            Some(target_id)
        } else {
            warn!("Failed to get command state for {target_id}");
            None
        }
    }

    /// Start tracking an inquiry (no socket allocation).
    #[allow(clippy::too_many_arguments)]
    pub fn start_inquiry(
        &mut self,
        id: CommandId,
        command: Arc<EncodedCommand>,
        priority: Priority,
        category: CommandCategory,
        camera_id: crate::camera_id::CameraId,
        kind: CommandKind,
        now: Instant,
    ) {
        // Get existing response_type - check both commands and pending_inquiry_types
        let response_type = self
            .commands
            .get(&id)
            .and_then(|s| s.response_type)
            .or_else(|| self.pending_inquiry_types.remove(&id));

        // Track whether this was already in-flight
        let was_already_tracked = self.inflight_inquiry_ids.contains(&id);

        // Create or update command state
        let state = CommandState {
            command,
            priority,
            category,
            camera_id,
            kind,
            submitted_at: now,
            sent_at: Some(now),
            attempt: 0,
            transport_error: false,
            response_type,
        };
        self.commands.insert(id, state);
        self.inflight_inquiry_ids.insert(id);

        // Add to order queue for raw VISCA correlation
        self.inquiries_order.push_back(id);

        // Update last inquiry sent time for spacing enforcement
        self.last_inquiry_sent = Some(now);

        if was_already_tracked {
            warn!(
                %id,
                inquiry_type = ?response_type,
                "Inquiry started but was already in inflight_inquiry_ids - timestamp overwritten"
            );
        }

        let inflight_count = self.inflight_inquiry_ids.len();
        trace!(
            %id,
            inquiry_type = ?response_type,
            inflight_count,
            "Started inquiry (no socket allocation)"
        );
    }

    /// Check if a command is pending (either awaiting ACK or has a socket).
    pub fn is_command_pending(&self, cmd_id: CommandId) -> bool {
        self.pending_ack_ids.contains(&cmd_id)
            || self.sockets.iter().any(|s| s.command_id() == Some(cmd_id))
    }

    /// Get the count of commands waiting for ACK.
    pub fn pending_ack_count(&self) -> usize {
        self.pending_ack_ids.len()
    }

    /// Get the count of ignored unmatched sequenced replies.
    ///
    /// This counter tracks the number of times a sequenced reply (ACK, Completion,
    /// Error, InquiryReply) was received with a sequence number that did not resolve
    /// to an active command. This is expected for stale/duplicate UDP packets and
    /// indicates the sequence correlation safety mechanism is working correctly.
    pub fn ignored_unmatched_sequenced_replies(&self) -> u64 {
        self.ignored_unmatched_sequenced_replies
    }

    /// Free a previously reserved socket (used for rollback on inquiry send failure).
    pub fn free_socket(&mut self, socket: ViscaSocket) {
        let idx = socket.as_index();

        if let SocketState::Busy { command_id, .. } = &self.sockets[idx] {
            trace!("Freeing {socket:?} from command {command_id}");
        }

        self.sockets[idx] = SocketState::Free;
    }

    /// Find the command ID currently assigned to a socket.
    pub fn find_command_on_socket(&self, socket: ViscaSocket) -> Option<CommandId> {
        self.sockets[socket.as_index()].command_id()
    }

    /// Get the camera ID for a command by its ID.
    pub fn camera_id_for_command(&self, id: CommandId) -> Option<crate::camera_id::CameraId> {
        self.commands.get(&id).map(|state| state.camera_id)
    }

    pub(crate) fn find_socket_for_command(&self, cmd_id: CommandId) -> Option<ViscaSocket> {
        for (idx, state) in self.sockets.iter().enumerate() {
            if state.command_id() == Some(cmd_id) {
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
    ) -> (bool, Option<CommandId>, Option<CommandCategory>) {
        let state = &self.sockets[socket.as_index()];
        match state {
            SocketState::Free => (true, None, None),
            SocketState::Busy {
                command_id,
                category,
                ..
            } => (false, Some(*command_id), Some(*category)),
        }
    }

    fn should_retry_command(&self, cmd_id: CommandId, error: &ViscaError, now: Instant) -> bool {
        if let Some(state) = self.commands.get(&cmd_id) {
            if error.is_retryable(Some(state.category)) {
                let max_retries = self.retry_budget.for_category(state.category);
                let within_duration =
                    now.duration_since(state.submitted_at) < self.retry_config.max_retry_duration;
                state.attempt < max_retries && within_duration
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
    ///
    /// The `cause` parameter preserves the original error (including timeout semantics)
    /// while wrapping it with "Send failed" context.
    pub fn fail_after_send_error(
        &mut self,
        cmd_id: CommandId,
        cause: Error,
    ) -> Option<SchedulerAction> {
        // If inquiry is in-flight, remove from index sets and order
        self.inflight_inquiry_ids.remove(&cmd_id);
        self.pending_ack_ids.remove(&cmd_id);
        self.inquiries_order.retain(|&x| x != cmd_id);

        // For send failures on first attempt, fail immediately with the original error
        // wrapped with "Send failed" context. This preserves the error kind (e.g., Timeout)
        // while adding context about when the failure occurred.
        self.finish_sequence(cmd_id);
        self.commands.remove(&cmd_id);
        Some(SchedulerAction::CommandFailed {
            id: cmd_id,
            error: cause.with_context("Send failed"),
        })
    }

    /// Fail a command due to a receive-side error (e.g., decode failure, protocol error).
    ///
    /// Similar to `fail_after_send_error`, this immediately fails a command without
    /// retry since the response was received but contained an error. The error is
    /// passed through without additional context wrapping.
    ///
    /// Unlike send errors, receive errors indicate the camera did receive and process
    /// the command, but the response was malformed or indicated an error condition.
    /// No "Send failed" context is added since the send succeeded.
    pub fn fail_after_receive_error(
        &mut self,
        cmd_id: CommandId,
        error: Error,
    ) -> Option<SchedulerAction> {
        // Remove from all tracking sets
        self.inflight_inquiry_ids.remove(&cmd_id);
        self.pending_ack_ids.remove(&cmd_id);
        self.inquiries_order.retain(|&x| x != cmd_id);

        // Free any socket allocated to this command.
        // Commands (unlike inquiries) may have sockets; freeing them allows
        // subsequent commands to proceed without waiting for a timeout.
        if let Some(socket) = self.find_socket_for_command(cmd_id) {
            self.free_socket(socket);
        }

        // Clean up sequence mappings and command state
        self.finish_sequence(cmd_id);
        self.commands.remove(&cmd_id);

        Some(SchedulerAction::CommandFailed { id: cmd_id, error })
    }

    /// Mark a retry as being triggered by a transport error.
    /// This affects the final error classification when retries are exhausted.
    pub fn mark_retry_as_transport_error(&mut self, cmd_id: CommandId) {
        if let Some(state) = self.commands.get_mut(&cmd_id) {
            state.transport_error = true;
        }
    }

    /// Queue a command for retry based on the retry configuration.
    pub fn queue_retry_for_command(
        &mut self,
        cmd_id: CommandId,
        now: Instant,
    ) -> Option<SchedulerAction> {
        // Get current state
        let state_copy = self.commands.get(&cmd_id).cloned();
        if let Some(state) = state_copy {
            // Free the socket if allocated
            if let Some(socket) = self.find_socket_for_command(cmd_id) {
                self.free_socket(socket);
            }

            // Increment retry count
            let new_attempt = state.attempt + 1;

            // Check if we've exceeded max retries
            let max_retries = self.retry_budget.for_category(state.category);

            // Check if we've exceeded max_retry_duration
            let elapsed = now.duration_since(state.submitted_at);
            let exceeded_duration = elapsed >= self.retry_config.max_retry_duration;

            if new_attempt > max_retries || exceeded_duration {
                // Command has exceeded retries or duration limit
                self.finish_sequence(cmd_id);
                // Use TransportError if the retry was triggered by a transport error, otherwise Timeout
                let error = if state.transport_error {
                    Error::TransportError("Network error after max retries".into())
                } else {
                    Error::Timeout
                };
                self.commands.remove(&cmd_id);
                return Some(SchedulerAction::CommandFailed { id: cmd_id, error });
            }

            // Update retry count in command state
            if let Some(cmd_state) = self.commands.get_mut(&cmd_id) {
                cmd_state.attempt = new_attempt;
            }

            // Calculate backoff delay using RetryConfig
            let delay = self.retry_config.calculate_delay(new_attempt, None);

            let retry_cmd = RetryCommand {
                id: cmd_id,
                command: state.command.clone(),
                priority: state.priority,
                category: state.category,
                camera_id: state.camera_id,
                kind: state.kind,
                attempt: new_attempt,
                max_retries,
                retry_at: now + delay,
            };

            debug!(
                "Queueing retry for command {} (attempt {} of {})",
                cmd_id, new_attempt, max_retries
            );
            self.retry_queue.push(RetryKey { command: retry_cmd });

            Some(SchedulerAction::RetryCommand { id: cmd_id, delay })
        } else {
            None
        }
    }

    /// Clear all scheduler state.
    ///
    /// This is used when the transport is poisoned and all commands must be failed.
    /// It clears all internal state but does not send errors to response channels -
    /// that is handled by the caller.
    pub fn clear_all(&mut self) {
        self.commands.clear();
        self.pending_ack_ids.clear();
        self.inflight_inquiry_ids.clear();
        self.retry_queue.clear();
        self.command_queue.clear();
        self.inquiry_queue.clear();
        self.seq_to_cmd.clear();
        self.cmd_to_seqs.clear();
        self.seq16_to_cmds.clear();
        self.cmd_to_seq16s.clear();
        self.inquiries_order.clear();
        self.pending_inquiry_types.clear();
        self.sockets = Default::default();
        self.last_logged_idle.set(false);
        self.last_inquiry_sent = None;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::command::encode::EncodedCommand;
    use crate::transport::RetryConfig;
    use crate::CameraId;
    use smallvec::SmallVec;
    use std::sync::Arc;
    use std::time::Duration;

    /// Helper function to create CommandId from u32 in tests.
    /// Panics if value is 0 (invalid for CommandId).
    fn cmd_id(value: u32) -> CommandId {
        CommandId::from_raw(value).expect("test command ID must be non-zero")
    }

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
    ) -> Arc<EncodedCommand> {
        let command = match category {
            CommandCategory::Quick => EncodedCommand::new(
                TestCommandQuick {
                    bytes,
                    response_type,
                },
                camera_id,
            )
            .unwrap(),
            CommandCategory::Movement => EncodedCommand::new(
                TestCommandMovement {
                    bytes,
                    response_type,
                },
                camera_id,
            )
            .unwrap(),
            _ => {
                // Default to Quick for other categories in tests
                EncodedCommand::new(
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

        let inquiry_cmd = Arc::new(EncodedCommand::new(TestInquiry, camera_id).unwrap());

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
        let cmd1 = Arc::new(EncodedCommand::new(TestCmd1, camera_id).unwrap());
        let cmd2 = Arc::new(EncodedCommand::new(TestCmd2, camera_id).unwrap());

        // Register first command on socket 1
        core.register_pending_ack(
            cmd_id(1),
            cmd1.clone(),
            priority,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now,
        );
        // Manually allocate socket 1 (simulating ACK received)
        // When ACK is received, command is removed from pending_ack_ids
        core.pending_ack_ids.remove(&cmd_id(1));
        core.sockets[0] = SocketState::Busy {
            command_id: cmd_id(1),
            started_at: now,
            category: CommandCategory::Movement,
        };

        // Register second command on socket 2
        core.register_pending_ack(
            cmd_id(2),
            cmd2.clone(),
            priority,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now,
        );
        // Manually allocate socket 2 (simulating ACK received)
        // When ACK is received, command is removed from pending_ack_ids
        core.pending_ack_ids.remove(&cmd_id(2));
        core.sockets[1] = SocketState::Busy {
            command_id: cmd_id(2),
            started_at: now,
            category: CommandCategory::Movement,
        };

        // Both sockets are now occupied, but inquiry should still be sendable
        assert!(!core.can_send_command()); // Cannot send more commands

        // Start an inquiry - should not need a socket
        core.start_inquiry(
            cmd_id(3),
            inquiry_cmd.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            now,
        );

        // Verify inquiry is tracked
        assert!(core.inflight_inquiry_ids.contains(&cmd_id(3)));
        assert!(core.inquiries_order.contains(&cmd_id(3)));

        // Sockets should still be occupied by commands
        let state1 = core.socket_state(ViscaSocket::S1);
        assert!(!state1.0); // Socket 1 still occupied
        assert_eq!(state1.1, Some(cmd_id(1))); // By command 1

        let state2 = core.socket_state(ViscaSocket::S2);
        assert!(!state2.0); // Socket 2 still occupied
        assert_eq!(state2.1, Some(cmd_id(2))); // By command 2
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
        let command = Arc::new(EncodedCommand::new(TestInquiryCmd, camera_id).unwrap());

        // Start an inquiry
        core.start_inquiry(
            cmd_id(1),
            command.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            now,
        );

        // Verify inquiry is tracked
        assert!(core.inflight_inquiry_ids.contains(&cmd_id(1)));
        assert!(core.inquiries_order.contains(&cmd_id(1)));

        // Process InquiryReply event
        let response = Response::Inquiry(crate::command::InquiryData::Power { on: true });
        let event = SchedulerEvent::InquiryReply {
            cmd_id: Some(cmd_id(1)),
            sequence: None,
            response,
        };

        let actions = core.process_event(event, now);

        // Should get CommandComplete action
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandComplete {
                id, response: resp, ..
            } => {
                assert_eq!(*id, cmd_id(1));
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
        assert!(!core.inflight_inquiry_ids.contains(&cmd_id(1)));
        assert!(!core.inquiries_order.contains(&cmd_id(1)));
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

        let cmd1 = Arc::new(EncodedCommand::new(TestInquiry1, camera_id).unwrap());
        let cmd2 = Arc::new(EncodedCommand::new(TestInquiry2, camera_id).unwrap());
        let cmd3 = Arc::new(EncodedCommand::new(TestInquiry3, camera_id).unwrap());

        core.start_inquiry(
            cmd_id(1),
            cmd1,
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            now,
        );
        core.start_inquiry(
            cmd_id(2),
            cmd2,
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            now,
        );
        core.start_inquiry(
            cmd_id(3),
            cmd3,
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            now,
        );

        // Verify all inquiries are tracked in order
        assert_eq!(core.inquiries_order.len(), 3);
        assert_eq!(core.inquiries_order[0], cmd_id(1));
        assert_eq!(core.inquiries_order[1], cmd_id(2));
        assert_eq!(core.inquiries_order[2], cmd_id(3));

        // Process InquiryReply events without cmd_id (raw VISCA)
        // First reply should match first inquiry
        let response1 = Response::Inquiry(crate::command::InquiryData::Power { on: true });
        let event1 = SchedulerEvent::InquiryReply {
            cmd_id: None, // No sequence in raw VISCA
            sequence: None,
            response: response1,
        };

        let actions = core.process_event(event1, now);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandComplete { id, .. } => {
                assert_eq!(*id, cmd_id(1)); // First inquiry completed
            }
            _ => panic!("Expected CommandComplete action"),
        }

        // Order should have inquiry 1 removed
        assert_eq!(core.inquiries_order.len(), 2);
        assert_eq!(core.inquiries_order[0], cmd_id(2));
        assert_eq!(core.inquiries_order[1], cmd_id(3));
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

        let cmd1 = Arc::new(EncodedCommand::new(TestCmd1, camera_id).unwrap());
        let cmd2 = Arc::new(EncodedCommand::new(TestCmd2, camera_id).unwrap());

        core.register_pending_ack(
            cmd_id(1),
            cmd1,
            priority,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(cmd_id(1), 100); // Command 1 has sequence 100

        core.register_pending_ack(
            cmd_id(2),
            cmd2,
            priority,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(cmd_id(2), 101); // Command 2 has sequence 101

        // Process ACK for command 2 first (out of order)
        let cmd_id_2 = core.get_command_by_sequence(101);
        let event = SchedulerEvent::Ack {
            socket: Some(ViscaSocket::S2),
            cmd_id: cmd_id_2, // Using sequence to identify
            sequence: Some(101),
        };

        let actions = core.process_event(event, now);

        // ACK processing should be silent (no action returned)
        assert!(
            actions.is_empty(),
            "Expected no actions from ACK processing"
        );

        // Verify command 2 got socket 2
        let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S2);
        assert!(!free); // Socket occupied
        assert_eq!(socket_cmd_id, Some(cmd_id(2))); // By command 2

        // Command 1 should still be pending
        assert!(core.pending_ack_ids.contains(&cmd_id(1)));

        // Now process ACK for command 1
        let cmd_id_1 = core.get_command_by_sequence(100);
        let event = SchedulerEvent::Ack {
            socket: Some(ViscaSocket::S1),
            cmd_id: cmd_id_1,
            sequence: Some(100),
        };

        core.process_event(event, now);

        // Verify command 1 got socket 1
        let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
        assert!(!free); // Socket occupied
        assert_eq!(socket_cmd_id, Some(cmd_id(1))); // By command 1
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
            cmd_id(1),
            command.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            now,
        );
        assert!(core.inflight_inquiry_ids.contains(&cmd_id(1)));

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
                assert_eq!(*id, cmd_id(1));
            }
            other => panic!("Expected RetryCommand action, got: {:?}", other),
        }

        // Inquiry should be removed from tracking after timeout
        assert!(!core.inflight_inquiry_ids.contains(&cmd_id(1)));

        // Start inquiry again for the retry
        core.start_inquiry(
            cmd_id(1),
            command.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            later,
        );
        assert!(core.inflight_inquiry_ids.contains(&cmd_id(1)));

        // Exhaust retries by timing out again (simulate max retries reached)
        // For Quick category, we get extra retries, so we need to exhaust them
        // Set retry attempts to max to force failure on next timeout
        // Must be done AFTER start_inquiry since it creates a new CommandState
        if let Some(state) = core.commands.get_mut(&cmd_id(1)) {
            state.attempt = 10; // Force max retries exceeded
        }

        // Now timeout should fail
        let later2 = later + Duration::from_millis(200);
        let actions2 = core.check_timeouts(later2);

        assert_eq!(actions2.len(), 1, "Expected exactly one action");
        match &actions2[0] {
            SchedulerAction::CommandFailed { id, error } => {
                assert_eq!(*id, cmd_id(1));
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
            cmd_id(1),
            command.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(cmd_id(1), 100);

        // Verify initial sequence is tracked
        assert_eq!(core.get_command_by_sequence(100), Some(cmd_id(1)));
        assert_eq!(core.seq_to_cmd.len(), 1);
        assert_eq!(core.cmd_to_seqs.len(), 1);

        // Simulate retry - register new sequence for same command
        core.register_sequence(cmd_id(1), 101);

        // Both sequences should now map to command 1
        assert_eq!(core.get_command_by_sequence(100), Some(cmd_id(1)));
        assert_eq!(core.get_command_by_sequence(101), Some(cmd_id(1)));
        assert_eq!(core.seq_to_cmd.len(), 2);
        assert_eq!(core.cmd_to_seqs.len(), 1); // Still one command

        // Simulate another retry
        core.register_sequence(cmd_id(1), 102);

        // All three sequences should map to command 1
        assert_eq!(core.get_command_by_sequence(100), Some(cmd_id(1)));
        assert_eq!(core.get_command_by_sequence(101), Some(cmd_id(1)));
        assert_eq!(core.get_command_by_sequence(102), Some(cmd_id(1)));
        assert_eq!(core.seq_to_cmd.len(), 3);

        // Finish the command - all sequences should be cleaned up
        core.finish_sequence(cmd_id(1));
        core.commands.remove(&cmd_id(1));

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
            cmd_id(1),
            command.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(cmd_id(1), 100);
        core.register_sequence(cmd_id(1), 101); // Retry 1
        core.register_sequence(cmd_id(1), 102); // Retry 2

        // Verify all sequences are active
        assert_eq!(core.get_command_by_sequence(100), Some(cmd_id(1)));
        assert_eq!(core.get_command_by_sequence(101), Some(cmd_id(1)));
        assert_eq!(core.get_command_by_sequence(102), Some(cmd_id(1)));

        // Complete the command (simulating success on the third attempt)
        let response = Response::Completion {
            socket: Some(ViscaSocket::S1),
        };
        let event = SchedulerEvent::Completion {
            socket: Some(ViscaSocket::S1),
            cmd_id: Some(cmd_id(1)),
            sequence: Some(102), // Third retry sequence
            response,
        };

        let actions = core.process_event(event, now);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandComplete { id, .. } => {
                assert_eq!(*id, cmd_id(1));
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
            cmd_id(1),
            command.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );

        // Register more than MAX_SEQUENCES_PER_CMD (8) sequences
        for seq in 100..110 {
            core.register_sequence(cmd_id(1), seq);
        }

        // Only the last 8 sequences should be active (102-109)
        // 100 and 101 should have been dropped
        assert_eq!(core.get_command_by_sequence(100), None); // Dropped
        assert_eq!(core.get_command_by_sequence(101), None); // Dropped

        for seq in 102..110 {
            assert_eq!(core.get_command_by_sequence(seq), Some(cmd_id(1)));
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
            cmd_id(1),
            cmd1.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_pending_ack(
            cmd_id(2),
            cmd2.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );

        // Command 1 has sequences 100, 101 (retry)
        core.register_sequence(cmd_id(1), 100);
        core.register_sequence(cmd_id(1), 101);

        // Command 2 has sequences 200, 201, 202 (two retries)
        core.register_sequence(cmd_id(2), 200);
        core.register_sequence(cmd_id(2), 201);
        core.register_sequence(cmd_id(2), 202);

        // Verify all sequences map correctly
        assert_eq!(core.get_command_by_sequence(100), Some(cmd_id(1)));
        assert_eq!(core.get_command_by_sequence(101), Some(cmd_id(1)));
        assert_eq!(core.get_command_by_sequence(200), Some(cmd_id(2)));
        assert_eq!(core.get_command_by_sequence(201), Some(cmd_id(2)));
        assert_eq!(core.get_command_by_sequence(202), Some(cmd_id(2)));

        // Complete command 1
        core.finish_sequence(cmd_id(1));
        core.commands.remove(&cmd_id(1));

        // Command 1's sequences should be gone, command 2's should remain
        assert_eq!(core.get_command_by_sequence(100), None);
        assert_eq!(core.get_command_by_sequence(101), None);
        assert_eq!(core.get_command_by_sequence(200), Some(cmd_id(2)));
        assert_eq!(core.get_command_by_sequence(201), Some(cmd_id(2)));
        assert_eq!(core.get_command_by_sequence(202), Some(cmd_id(2)));

        // Complete command 2
        core.finish_sequence(cmd_id(2));
        core.commands.remove(&cmd_id(2));

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
            cmd_id(1),
            command.clone(),
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(cmd_id(1), full_sequence);

        // Verify that both 32-bit and 16-bit lookups work
        assert_eq!(core.get_command_by_sequence(full_sequence), Some(cmd_id(1)));
        assert_eq!(core.get_command_by_sequence(0x5678), Some(cmd_id(1))); // Should find by lower 16 bits

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
            cmd_id(1),
            cmd1,
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(cmd_id(1), seq1);

        core.register_pending_ack(
            cmd_id(2),
            cmd2,
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(cmd_id(2), seq2);

        // Both 32-bit sequences should work
        assert_eq!(core.get_command_by_sequence(seq1), Some(cmd_id(1)));
        assert_eq!(core.get_command_by_sequence(seq2), Some(cmd_id(2)));

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
            cmd_id(1),
            command,
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(cmd_id(1), sequence);

        // Verify both mappings exist
        assert_eq!(core.get_command_by_sequence(sequence), Some(cmd_id(1)));
        assert_eq!(core.get_command_by_sequence(0x5678), Some(cmd_id(1)));

        // Finish the sequence
        core.finish_sequence(cmd_id(1));
        core.commands.remove(&cmd_id(1));

        // Verify both mappings are cleaned up
        assert_eq!(core.get_command_by_sequence(sequence), None);
        assert_eq!(core.get_command_by_sequence(0x5678), None);
    }

    #[test]
    fn test_16_bit_sequence_collision_recovers_after_finish() {
        // This test verifies the key fix from issue #457:
        // When two commands collide on a 16-bit sequence and one finishes,
        // the remaining command should become uniquely resolvable.
        //
        // Previously, this would fail because finish_sequence would delete the
        // seq16_to_cmd entry entirely, making the remaining command unreachable.
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Movement;
        let camera_id = CameraId::CAMERA_1;

        // Create two commands with different 32-bit sequences but same lower 16 bits
        let seq1 = 0x12345678u32; // Lower 16 bits: 0x5678
        let seq2 = 0xABCD5678u32; // Lower 16 bits: 0x5678 (collision!)

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

        // Register both commands
        core.register_pending_ack(
            cmd_id(1),
            cmd1,
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(cmd_id(1), seq1);

        core.register_pending_ack(
            cmd_id(2),
            cmd2,
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(cmd_id(2), seq2);

        // Both 32-bit sequences should work
        assert_eq!(core.get_command_by_sequence(seq1), Some(cmd_id(1)));
        assert_eq!(core.get_command_by_sequence(seq2), Some(cmd_id(2)));

        // 16-bit lookup should be ambiguous and return None (both commands active)
        assert_eq!(
            core.get_command_by_sequence(0x5678),
            None,
            "16-bit lookup should be ambiguous while both commands are active"
        );

        // Now finish command 1
        core.finish_sequence(cmd_id(1));
        core.commands.remove(&cmd_id(1));

        // Command 2's 32-bit sequence should still work
        assert_eq!(core.get_command_by_sequence(seq2), Some(cmd_id(2)));

        // KEY FIX: 16-bit lookup should NOW return command 2 (no longer ambiguous!)
        // This works for both explicit 16-bit values and full 32-bit values that
        // share the same lower 16 bits
        assert_eq!(
            core.get_command_by_sequence(0x5678),
            Some(cmd_id(2)),
            "After finishing command 1, 16-bit lookup should uniquely resolve to command 2"
        );

        // Note: Looking up seq1 (0x12345678) will also find command 2 via 16-bit fallback
        // because the 32-bit exact match fails and the 16-bit (0x5678) uniquely matches
        // command 2. This is expected behavior for truncated sequence resolution.
        assert_eq!(
            core.get_command_by_sequence(seq1),
            Some(cmd_id(2)),
            "seq1's lower 16 bits match seq2, so 16-bit fallback finds command 2"
        );
    }

    #[test]
    fn test_16_bit_sequence_collision_recovers_finish_order_reversed() {
        // Same as above, but finish command 2 first to ensure symmetry
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Movement;
        let camera_id = CameraId::CAMERA_1;

        let seq1 = 0x12345678u32;
        let seq2 = 0xABCD5678u32;

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
            cmd_id(1),
            cmd1,
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(cmd_id(1), seq1);

        core.register_pending_ack(
            cmd_id(2),
            cmd2,
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(cmd_id(2), seq2);

        // Both active: 16-bit should be ambiguous
        assert_eq!(core.get_command_by_sequence(0x5678), None);

        // Finish command 2 first (reversed order from previous test)
        core.finish_sequence(cmd_id(2));
        core.commands.remove(&cmd_id(2));

        // Command 1's sequence should still work
        assert_eq!(core.get_command_by_sequence(seq1), Some(cmd_id(1)));

        // 16-bit lookup should now return command 1
        assert_eq!(
            core.get_command_by_sequence(0x5678),
            Some(cmd_id(1)),
            "After finishing command 2, 16-bit lookup should uniquely resolve to command 1"
        );

        // Note: seq2 (0xABCD5678) will also find command 1 via 16-bit fallback
        // since its 32-bit exact match fails but 16-bit (0x5678) uniquely matches command 1
        assert_eq!(
            core.get_command_by_sequence(seq2),
            Some(cmd_id(1)),
            "seq2's lower 16 bits match seq1, so 16-bit fallback finds command 1"
        );
    }

    #[test]
    fn test_16_bit_eviction_under_collision() {
        // This test verifies that when a command's 16-bit sequence is evicted due to
        // the 8-sequence cap, it doesn't break another command's ownership of that sequence.
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let now = Instant::now();
        let priority = Priority::Normal;
        let category = CommandCategory::Movement;
        let camera_id = CameraId::CAMERA_1;

        // Command 1: starts with seq16 = 0x5678
        let seq1_initial = 0x12345678u32;

        // Command 2: also uses seq16 = 0x5678 (collision)
        let seq2 = 0xABCD5678u32;

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

        // Register command 1 with its initial sequence
        core.register_pending_ack(
            cmd_id(1),
            cmd1,
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(cmd_id(1), seq1_initial);

        // Register command 2 (collision on 0x5678)
        core.register_pending_ack(
            cmd_id(2),
            cmd2,
            priority,
            category,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(cmd_id(2), seq2);

        // Both commands own 0x5678 - ambiguous
        assert_eq!(core.get_command_by_sequence(0x5678), None);

        // Now simulate retries for command 1 that eventually evict its 0x5678 entry
        // Add 8 more sequences with DIFFERENT 16-bit values to command 1
        for i in 0u32..8 {
            // Generate sequences with different lower 16 bits
            let retry_seq = 0x1234_0000 + i + 1; // 0x12340001, 0x12340002, ..., 0x12340008
            core.register_sequence(cmd_id(1), retry_seq);
        }

        // Command 1's original 0x5678 should be evicted from its history
        // But command 2 should still own 0x5678!
        assert_eq!(
            core.get_command_by_sequence(0x5678),
            Some(cmd_id(2)),
            "After command 1's 0x5678 is evicted, command 2 should uniquely own it"
        );

        // Command 2's full sequence should still work
        assert_eq!(core.get_command_by_sequence(seq2), Some(cmd_id(2)));
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
            cmd_id(1),
            power_cmd,
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            now,
        );
        core.start_inquiry(
            cmd_id(2),
            zoom_cmd,
            priority,
            category,
            camera_id,
            CommandKind::Inquiry,
            now,
        );

        // Verify both are tracked
        assert_eq!(core.inquiries_order.len(), 2);
        assert!(core.inflight_inquiry_ids.contains(&cmd_id(1)));
        assert!(core.inflight_inquiry_ids.contains(&cmd_id(2)));

        // Process replies out of order
        // Second inquiry (zoom) reply arrives first - with explicit cmd_id
        // (This simulates the adapter layer doing content-based matching)
        let zoom_response =
            Response::Inquiry(crate::command::InquiryData::ZoomPosition { position: 0x1234 });
        let event2 = SchedulerEvent::InquiryReply {
            cmd_id: Some(cmd_id(2)), // Content-based matching identified this as inquiry 2
            sequence: None,          // Raw VISCA, no sequence
            response: zoom_response,
        };

        let actions = core.process_event(event2, now);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandComplete { id, response, .. } => {
                assert_eq!(*id, cmd_id(2)); // Second inquiry completed
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
            cmd_id: Some(cmd_id(1)), // Content-based matching identified this as inquiry 1
            sequence: None,          // Raw VISCA, no sequence
            response: power_response,
        };

        let actions = core.process_event(event1, now);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandComplete { id, response, .. } => {
                assert_eq!(*id, cmd_id(1)); // First inquiry completed
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
        assert!(!core.inflight_inquiry_ids.contains(&cmd_id(1)));
        assert!(!core.inflight_inquiry_ids.contains(&cmd_id(2)));
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

        let cmd_id = cmd_id(1);
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

        let cmd_id = cmd_id(1);
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

        let cmd_id = cmd_id(1);
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
        for i in 1..=3 {
            let command = create_test_command(
                vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
                None,
                CommandCategory::Movement,
                camera_id,
            );
            core.register_pending_ack(
                cmd_id(i),
                command,
                Priority::Normal,
                CommandCategory::Movement,
                CameraId::CAMERA_1,
                CommandKind::Command,
                now,
            );

            // Mark as transport error and queue retry
            core.mark_retry_as_transport_error(cmd_id(i));
            let action = core.queue_retry_for_command(cmd_id(i), now);
            assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));
        }

        // Get all ready retries
        let retries = core.get_ready_retries(now + Duration::from_secs(1));
        assert_eq!(retries.len(), 3, "Should have 3 retries queued");

        // Verify all have attempt = 1
        for retry in retries {
            assert_eq!(retry.attempt, 1);
            assert!([cmd_id(1), cmd_id(2), cmd_id(3)].contains(&retry.id));
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
            cmd_id(1),
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
            cmd_id: Some(cmd_id(1)),
            sequence: None, // Raw VISCA
        };
        core.process_event(event, now);

        // Verify S1 was assigned
        let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
        assert!(!free, "S1 should be occupied");
        assert_eq!(socket_cmd_id, Some(cmd_id(1)), "Command 1 should be on S1");

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
            cmd_id(1),
            command.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );
        core.register_pending_ack(
            cmd_id(2),
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
            cmd_id: Some(cmd_id(1)),
            sequence: None, // Raw VISCA
        };
        core.process_event(event, now);

        // Second ACK without socket nibble should get S2
        let event = SchedulerEvent::Ack {
            socket: None,
            cmd_id: Some(cmd_id(2)),
            sequence: None, // Raw VISCA
        };
        core.process_event(event, now);

        // Verify S1 has command 1
        let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
        assert!(!free, "S1 should be occupied");
        assert_eq!(socket_cmd_id, Some(cmd_id(1)), "Command 1 should be on S1");

        // Verify S2 has command 2
        let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S2);
        assert!(!free, "S2 should be occupied");
        assert_eq!(socket_cmd_id, Some(cmd_id(2)), "Command 2 should be on S2");
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
        for i in 1..=3 {
            core.register_pending_ack(
                cmd_id(i),
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
            cmd_id: Some(cmd_id(1)),
            sequence: None, // Raw VISCA
        };
        core.process_event(event, now);

        let event = SchedulerEvent::Ack {
            socket: Some(ViscaSocket::S2),
            cmd_id: Some(cmd_id(2)),
            sequence: None, // Raw VISCA
        };
        core.process_event(event, now);

        // Third ACK without socket nibble should fail
        let event = SchedulerEvent::Ack {
            socket: None,
            cmd_id: Some(cmd_id(3)),
            sequence: None, // Raw VISCA
        };
        core.process_event(event, now);

        // Verify command 3 is still pending
        assert!(
            core.pending_ack_ids.contains(&cmd_id(3)),
            "Command 3 should still be pending"
        );

        // Verify sockets are still occupied by commands 1 and 2
        let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
        assert!(!free, "S1 should be occupied");
        assert_eq!(socket_cmd_id, Some(cmd_id(1)), "Command 1 should be on S1");

        let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S2);
        assert!(!free, "S2 should be occupied");
        assert_eq!(socket_cmd_id, Some(cmd_id(2)), "Command 2 should be on S2");
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
            cmd_id(1),
            command.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );
        core.register_pending_ack(
            cmd_id(2),
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
            cmd_id: Some(cmd_id(1)),
            sequence: None, // Raw VISCA
        };
        core.process_event(event, now);

        // Second ACK requests S1 (busy), should fallback to S2
        let event = SchedulerEvent::Ack {
            socket: Some(ViscaSocket::S1), // Request S1 which is busy
            cmd_id: Some(cmd_id(2)),
            sequence: None, // Raw VISCA
        };
        core.process_event(event, now);

        // Verify S1 still has command 1
        let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
        assert!(!free, "S1 should be occupied");
        assert_eq!(socket_cmd_id, Some(cmd_id(1)), "Command 1 should be on S1");

        // Verify S2 has command 2 (fallback allocation)
        let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S2);
        assert!(!free, "S2 should be occupied");
        assert_eq!(
            socket_cmd_id,
            Some(cmd_id(2)),
            "Command 2 should be on S2 (fallback)"
        );
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

        let test_command = Arc::new(EncodedCommand::new(TestCommand, CameraId::CAMERA_1).unwrap());
        let test_cmd_id = cmd_id(1);

        // Register as Command explicitly
        core.register_pending_ack(
            test_cmd_id,
            test_command.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command, // Explicitly a Command despite bytes[1] == 0x09
            now,
        );

        // Mark as transport error and queue retry
        core.mark_retry_as_transport_error(test_cmd_id);
        let action = core.queue_retry_for_command(test_cmd_id, now);

        // Should get a retry action
        assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));

        // Get the retry and verify the kind is preserved
        let retries = core.get_ready_retries(now + Duration::from_millis(200));
        assert_eq!(retries.len(), 1);
        let retry = &retries[0];

        // The key assertion: kind should be Command, not Inquiry
        // This verifies that we no longer use the bytes[1] == 0x09 heuristic
        assert_eq!(retry.kind, CommandKind::Command);
        assert_eq!(retry.id, test_cmd_id);
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

        let test_inquiry = Arc::new(EncodedCommand::new(TestInquiry, CameraId::CAMERA_1).unwrap());
        let inquiry_id = cmd_id(2);

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
        let command1 = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]),
            kind: CommandKind::Command,
            category: CommandCategory::Movement,
            response_type: None,
        });
        let command2 = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR]),
            kind: CommandKind::Command,
            category: CommandCategory::Movement,
            response_type: None,
        });
        let inquiry = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
            kind: CommandKind::Inquiry,
            category: CommandCategory::Quick,
            response_type: Some(InquiryKind::ZoomPosition),
        });

        // Queue both commands
        core.queue_command(PendingCommand {
            id: cmd_id(1),
            command: command1.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Movement,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Command,
        });
        core.queue_command(PendingCommand {
            id: cmd_id(2),
            command: command2.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Movement,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Command,
        });

        // Send and register both commands as pending ACK
        let cmd1 = core.next_item_to_send(now).unwrap();
        assert_eq!(cmd1.id, cmd_id(1));
        core.register_pending_ack(
            cmd_id(1),
            command1.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );

        let cmd2 = core.next_item_to_send(now).unwrap();
        assert_eq!(cmd2.id, cmd_id(2));
        core.register_pending_ack(
            cmd_id(2),
            command2.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );

        // Now both commands are pending ACK - sockets are at capacity
        assert!(!core.can_send_command());
        assert_eq!(core.next_item_to_send(now), None); // No commands can be sent

        // Queue an inquiry
        core.queue_command(PendingCommand {
            id: cmd_id(3),
            command: inquiry.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Quick,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Inquiry,
        });

        // The inquiry should be sendable even though command sockets are full
        let inq = core.next_item_to_send(now);
        assert!(inq.is_some());
        let inq = inq.unwrap();
        assert_eq!(inq.id, cmd_id(3));
        assert_eq!(inq.kind, CommandKind::Inquiry);

        // Start the inquiry (track it in flight)
        core.start_inquiry(
            cmd_id(3),
            inquiry.clone(),
            Priority::Normal,
            CommandCategory::Quick,
            CameraId::CAMERA_1,
            CommandKind::Inquiry,
            now,
        );

        // Queue another command - should not be sendable
        core.queue_command(PendingCommand {
            id: cmd_id(4),
            command: command1.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Movement,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Command,
        });

        // No more commands should be sendable (sockets still full)
        assert_eq!(core.next_item_to_send(now), None);

        // Queue another inquiry - should be sendable
        core.queue_command(PendingCommand {
            id: cmd_id(5),
            command: inquiry.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Quick,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Inquiry,
        });

        let inq2 = core.next_item_to_send(now);
        assert!(inq2.is_some());
        assert_eq!(inq2.unwrap().id, cmd_id(5));
    }

    #[test]
    fn test_inquiry_pipeline_limit() {
        // Test that inquiries respect the max_inquiries_inflight limit
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        core.set_max_inquiries_inflight(2); // Set a low limit for testing
        let now = Instant::now();

        let inquiry = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
            kind: CommandKind::Inquiry,
            category: CommandCategory::Quick,
            response_type: Some(InquiryKind::ZoomPosition),
        });

        // Queue 3 inquiries
        for id in 1..=3 {
            core.queue_command(PendingCommand {
                id: cmd_id(id),
                command: inquiry.clone(),
                priority: Priority::Normal,
                category: CommandCategory::Quick,
                camera_id: CameraId::CAMERA_1,
                submitted_at: now,
                kind: CommandKind::Inquiry,
            });
        }

        // First inquiry should be sendable
        let inq1 = core.next_item_to_send(now);
        assert!(inq1.is_some());
        assert_eq!(inq1.unwrap().id, cmd_id(1));
        core.start_inquiry(
            cmd_id(1),
            inquiry.clone(),
            Priority::Normal,
            CommandCategory::Quick,
            CameraId::CAMERA_1,
            CommandKind::Inquiry,
            now,
        );

        // Second inquiry should be sendable
        let inq2 = core.next_item_to_send(now);
        assert!(inq2.is_some());
        assert_eq!(inq2.unwrap().id, cmd_id(2));
        core.start_inquiry(
            cmd_id(2),
            inquiry.clone(),
            Priority::Normal,
            CommandCategory::Quick,
            CameraId::CAMERA_1,
            CommandKind::Inquiry,
            now,
        );

        // Third inquiry should NOT be sendable (limit reached)
        assert!(!core.can_send_inquiry(now));
        let inq3 = core.next_item_to_send(now);
        assert!(inq3.is_none());

        // Complete one inquiry by removing it from inflight
        core.inflight_inquiry_ids.remove(&cmd_id(1));
        core.commands.remove(&cmd_id(1));

        // Now the third inquiry should be sendable
        assert!(core.can_send_inquiry(now));
        let inq3 = core.next_item_to_send(now);
        assert!(inq3.is_some());
        assert_eq!(inq3.unwrap().id, cmd_id(3));
    }

    #[test]
    fn test_mixed_priority_queue_ordering() {
        // Test that priority-aware selection works correctly across both queues.
        // Higher priority commands should preempt lower priority inquiries,
        // preventing command starvation from background polling.
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();

        let command = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]),
            kind: CommandKind::Command,
            category: CommandCategory::Movement,
            response_type: None,
        });
        let inquiry = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
            kind: CommandKind::Inquiry,
            category: CommandCategory::Quick,
            response_type: Some(InquiryKind::ZoomPosition),
        });

        // Queue items with different priorities
        core.queue_command(PendingCommand {
            id: cmd_id(1),
            command: command.clone(),
            priority: Priority::Low,
            category: CommandCategory::Movement,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Command,
        });

        core.queue_command(PendingCommand {
            id: cmd_id(2),
            command: inquiry.clone(),
            priority: Priority::High,
            category: CommandCategory::Quick,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Inquiry,
        });

        core.queue_command(PendingCommand {
            id: cmd_id(3),
            command: command.clone(),
            priority: Priority::Critical,
            category: CommandCategory::Movement,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Command,
        });

        core.queue_command(PendingCommand {
            id: cmd_id(4),
            command: inquiry.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Quick,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Inquiry,
        });

        // Critical priority command should come first (Critical > High > Normal > Low)
        let item1 = core.next_item_to_send(now).unwrap();
        assert_eq!(item1.id, cmd_id(3));
        assert_eq!(item1.priority, Priority::Critical);

        // High priority inquiry second (preempts Normal priority inquiry)
        let item2 = core.next_item_to_send(now).unwrap();
        assert_eq!(item2.id, cmd_id(2));
        assert_eq!(item2.priority, Priority::High);

        // Normal priority inquiry (no higher priority items remaining)
        let item3 = core.next_item_to_send(now).unwrap();
        assert_eq!(item3.id, cmd_id(4));
        assert_eq!(item3.priority, Priority::Normal);

        // Low priority command last
        let item4 = core.next_item_to_send(now).unwrap();
        assert_eq!(item4.id, cmd_id(1));
        assert_eq!(item4.priority, Priority::Low);
    }

    #[test]
    fn test_high_priority_command_not_starved_by_normal_inquiries() {
        // Regression test for command starvation issue (GitHub issue #381):
        // When background polling generates many Normal-priority inquiries,
        // a High-priority command (like preset save) should not be blocked.
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();

        let command = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x01, 0x04, 0x3F, 0x01, 0x05, VISCA_TERMINATOR]),
            kind: CommandKind::Command,
            category: CommandCategory::Preset,
            response_type: None,
        });
        let inquiry = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
            kind: CommandKind::Inquiry,
            category: CommandCategory::Quick,
            response_type: Some(InquiryKind::ZoomPosition),
        });

        // Simulate a burst of Normal-priority polling inquiries (typical background load)
        for i in 1..=10 {
            core.queue_command(PendingCommand {
                id: cmd_id(i),
                command: inquiry.clone(),
                priority: Priority::Normal,
                category: CommandCategory::Quick,
                camera_id: CameraId::CAMERA_1,
                submitted_at: now,
                kind: CommandKind::Inquiry,
            });
        }

        // High-priority preset save command arrives (user action)
        core.queue_command(PendingCommand {
            id: cmd_id(100),
            command: command.clone(),
            priority: Priority::High,
            category: CommandCategory::Preset,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Command,
        });

        // The High-priority command should be returned BEFORE the Normal-priority inquiries
        // This prevents command starvation from background polling
        let item = core.next_item_to_send(now).unwrap();
        assert_eq!(
            item.id,
            cmd_id(100),
            "High-priority command should not be starved by Normal-priority inquiries"
        );
        assert_eq!(item.priority, Priority::High);
        assert_eq!(item.kind, CommandKind::Command);

        // Subsequent calls should return the Normal-priority inquiries
        let item2 = core.next_item_to_send(now).unwrap();
        assert_eq!(item2.priority, Priority::Normal);
        assert_eq!(item2.kind, CommandKind::Inquiry);
    }

    #[test]
    fn test_equal_priority_prefers_inquiry_for_backwards_compat() {
        // When command and inquiry have equal priority, prefer inquiry for backwards
        // compatibility (inquiries don't hold sockets and are typically faster).
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();

        let command = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]),
            kind: CommandKind::Command,
            category: CommandCategory::Movement,
            response_type: None,
        });
        let inquiry = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
            kind: CommandKind::Inquiry,
            category: CommandCategory::Quick,
            response_type: Some(InquiryKind::ZoomPosition),
        });

        // Queue command first, then inquiry (both Normal priority)
        core.queue_command(PendingCommand {
            id: cmd_id(1),
            command: command.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Movement,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Command,
        });
        core.queue_command(PendingCommand {
            id: cmd_id(2),
            command: inquiry.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Quick,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Inquiry,
        });

        // Inquiry should be preferred at equal priority
        let item = core.next_item_to_send(now).unwrap();
        assert_eq!(
            item.id,
            cmd_id(2),
            "Inquiry should be preferred at equal priority"
        );
        assert_eq!(item.kind, CommandKind::Inquiry);

        // Then the command
        let item2 = core.next_item_to_send(now).unwrap();
        assert_eq!(item2.id, cmd_id(1));
        assert_eq!(item2.kind, CommandKind::Command);
    }

    #[test]
    fn test_immediate_error_without_ack_maps_to_most_recent_pending_command() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);
        let now = Instant::now();

        // Create two pending ACK commands; the second one is the most recent
        let camera_id = CameraId::CAMERA_1;
        let cmd1 = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]),
            kind: CommandKind::Command,
            category: CommandCategory::Movement,
            response_type: None,
        });
        let cmd2 = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x10, 0x05, VISCA_TERMINATOR]), // One Push Trigger
            kind: CommandKind::Command,
            category: CommandCategory::Quick,
            response_type: None,
        });

        core.register_pending_ack(
            cmd_id(1),
            cmd1,
            Priority::Normal,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_pending_ack(
            cmd_id(2),
            cmd2,
            Priority::Normal,
            CommandCategory::Quick,
            camera_id,
            CommandKind::Command,
            now + Duration::from_millis(1),
        );

        // Simulate: camera returns 90 6y 41 FF (Not Executable) without a prior ACK
        // Raw VISCA (no sequence), so heuristic fallback is allowed
        let event = SchedulerEvent::Error {
            socket: Some(ViscaSocket::S2),
            cmd_id: None,
            sequence: None, // Raw VISCA
            code: 0x41,
        };
        let actions = core.process_event(event, now + Duration::from_millis(2));

        // The *most recent* pending command (id=2) should be failed immediately with 0x41
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandFailed { id, error } => {
                assert_eq!(*id, cmd_id(2), "Newest pending command must be attributed");
                assert!(matches!(error, Error::CommandNotExecutable));
            }
            _ => panic!("Expected CommandFailed for id=2"),
        }
        // And it must be removed from pending_ack
        assert!(!core.is_command_pending(cmd_id(2)));
        assert!(core.is_command_pending(cmd_id(1)));
    }

    #[test]
    fn test_error_without_socket_prefers_inflight_inquiry() {
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);
        let now = Instant::now();
        let camera_id = CameraId::CAMERA_1;

        // Start an inquiry (front of FIFO)
        let inq = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x81, 0x09, 0x04, 0x35, VISCA_TERMINATOR]), // WB Mode Inquiry
            kind: CommandKind::Inquiry,
            category: CommandCategory::Quick,
            response_type: Some(InquiryKind::Power), // any kind
        });
        core.start_inquiry(
            cmd_id(42),
            inq,
            Priority::Normal,
            CommandCategory::Quick,
            camera_id,
            CommandKind::Inquiry,
            now,
        );

        // Also have a pending ACK command in the background
        let cmd = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR]),
            kind: CommandKind::Command,
            category: CommandCategory::Movement,
            response_type: None,
        });
        core.register_pending_ack(
            cmd_id(99),
            cmd,
            Priority::Normal,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now,
        );

        // Simulate an inquiry-style error: 90 60 EE FF (y=0 -> no socket field)
        // Raw VISCA (no sequence), so heuristic fallback is allowed
        let event = SchedulerEvent::Error {
            socket: None,
            cmd_id: None,
            sequence: None, // Raw VISCA
            code: 0x41,
        };
        let actions = core.process_event(event, now + Duration::from_millis(1));

        // It must fail the inflight inquiry (id=42), not the command
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandFailed { id, error } => {
                assert_eq!(*id, cmd_id(42));
                assert!(matches!(error, Error::CommandNotExecutable));
            }
            _ => panic!("Expected CommandFailed for inquiry id=42"),
        }
        // Confirm the command is still pending
        assert!(core.is_command_pending(cmd_id(99)));
    }

    // =========================================================================
    // Tests for issue #434: max_retry_duration enforcement in SchedulerCore
    // =========================================================================

    // Helper to create a simple test command for duration tests
    fn make_duration_test_cmd(category: CommandCategory) -> Arc<EncodedCommand> {
        create_test_command(
            vec![0x81, 0x01, 0x00, VISCA_TERMINATOR],
            None,
            category,
            CameraId::CAMERA_1,
        )
    }

    #[test]
    fn test_max_retry_duration_ack_timeout() {
        // Test that ACK timeout retries respect max_retry_duration
        // Use short timeouts for testing
        let timeout_config = TimeoutConfig::uniform(Duration::from_millis(50));
        let retry_config = RetryConfig {
            max_retries: 10, // High retry count to ensure duration is the limiting factor
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_millis(200), // Short duration for testing
            exponential_backoff: false,
        };
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let cmd = make_duration_test_cmd(CommandCategory::Quick);
        let start = Instant::now();

        // Register a command
        core.register_pending_ack(
            cmd_id(1),
            cmd,
            Priority::Normal,
            CommandCategory::Quick,
            CameraId::CAMERA_1,
            CommandKind::Command,
            start,
        );

        // Simulate ACK timeout at t+60ms (ack_timeout=50ms + 10ms margin, within duration 200ms)
        let actions = core.check_timeouts(start + Duration::from_millis(60));

        // Should retry because we're within max_retry_duration (200ms)
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, SchedulerAction::RetryCommand { id, .. } if *id == cmd_id(1))),
            "Expected retry within max_retry_duration"
        );

        // Re-register for next retry simulation with the original start time
        let cmd = make_duration_test_cmd(CommandCategory::Quick);
        core.register_pending_ack(
            cmd_id(1),
            cmd,
            Priority::Normal,
            CommandCategory::Quick,
            CameraId::CAMERA_1,
            CommandKind::Command,
            start,
        );

        // Simulate ACK timeout at t+250ms (exceeds duration of 200ms)
        let actions = core.check_timeouts(start + Duration::from_millis(250));

        // Should NOT retry because max_retry_duration (200ms) exceeded
        // Instead, should fail the command
        let has_retry = actions
            .iter()
            .any(|a| matches!(a, SchedulerAction::RetryCommand { id, .. } if *id == cmd_id(1)));
        let has_timeout_no_retry = actions.iter().any(|a| {
            matches!(
                a,
                SchedulerAction::Timeout {
                    id,
                    will_retry: false,
                    ..
                } if *id == cmd_id(1)
            )
        });
        let has_failed = actions
            .iter()
            .any(|a| matches!(a, SchedulerAction::CommandFailed { id, .. } if *id == cmd_id(1)));

        assert!(
            !has_retry || has_timeout_no_retry || has_failed,
            "Expected no retry or failure after max_retry_duration exceeded, got: {:?}",
            actions
        );
    }

    #[test]
    fn test_max_retry_duration_inquiry_timeout() {
        // Test that inquiry timeout retries respect max_retry_duration
        // Use short timeouts for testing
        let timeout_config = TimeoutConfig::uniform(Duration::from_millis(50));
        let retry_config = RetryConfig {
            max_retries: 10,
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_millis(200),
            exponential_backoff: false,
        };
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let cmd = make_duration_test_cmd(CommandCategory::Quick);
        let start = Instant::now();

        // Start an inquiry
        core.start_inquiry(
            cmd_id(1),
            cmd,
            Priority::Normal,
            CommandCategory::Quick,
            CameraId::CAMERA_1,
            CommandKind::Inquiry,
            start,
        );

        // Check timeout at t+60ms (quick_timeout=50ms + 10ms margin, within duration 200ms)
        let actions = core.check_timeouts(start + Duration::from_millis(60));

        // Should have retry action within duration
        let has_retry = actions
            .iter()
            .any(|a| matches!(a, SchedulerAction::RetryCommand { id, .. } if *id == cmd_id(1)));
        assert!(
            has_retry,
            "Expected retry within max_retry_duration for inquiry"
        );
    }

    #[test]
    fn test_max_retry_duration_queue_retry_for_command() {
        // Test that queue_retry_for_command respects max_retry_duration
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig {
            max_retries: 10,
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_millis(100),
            exponential_backoff: false,
        };
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let cmd = make_duration_test_cmd(CommandCategory::Quick);
        let start = Instant::now();

        // Register a command
        core.register_pending_ack(
            cmd_id(1),
            cmd,
            Priority::Normal,
            CommandCategory::Quick,
            CameraId::CAMERA_1,
            CommandKind::Command,
            start,
        );

        // Queue retry within duration - should succeed
        let action = core.queue_retry_for_command(cmd_id(1), start + Duration::from_millis(50));
        assert!(
            matches!(action, Some(SchedulerAction::RetryCommand { id, .. }) if id == cmd_id(1)),
            "Expected retry command within duration"
        );

        // Clear retry state for next test
        if let Some(state) = core.commands.get_mut(&cmd_id(1)) {
            state.attempt = 0;
        }

        // Queue retry after duration exceeded - should fail
        let action = core.queue_retry_for_command(cmd_id(1), start + Duration::from_millis(150));
        assert!(
            matches!(
                action,
                Some(SchedulerAction::CommandFailed {
                    id,
                    error: Error::Timeout
                }) if id == cmd_id(1)
            ),
            "Expected failure after duration exceeded"
        );
    }

    #[test]
    fn test_max_retry_duration_should_retry_command() {
        // Test that should_retry_command respects max_retry_duration
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig {
            max_retries: 10,
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_millis(100),
            exponential_backoff: false,
        };
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let cmd = make_duration_test_cmd(CommandCategory::Movement);
        let start = Instant::now();

        // Register a command
        core.register_pending_ack(
            cmd_id(1),
            cmd,
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            start,
        );

        // Create a retryable error (0x41 = CommandNotExecutable, retryable for Movement)
        let error = ViscaError::from_byte(0x41);

        // Should retry within duration
        assert!(
            core.should_retry_command(cmd_id(1), &error, start + Duration::from_millis(50)),
            "Expected should_retry_command=true within duration"
        );

        // Should NOT retry after duration exceeded
        assert!(
            !core.should_retry_command(cmd_id(1), &error, start + Duration::from_millis(150)),
            "Expected should_retry_command=false after duration exceeded"
        );
    }

    #[test]
    fn test_max_retry_duration_socket_timeout() {
        // Test that socket timeout retries respect max_retry_duration
        // Use short timeouts for testing
        let timeout_config = TimeoutConfig::uniform(Duration::from_millis(50));
        let retry_config = RetryConfig {
            max_retries: 10,
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_millis(200),
            exponential_backoff: false,
        };
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let cmd = make_duration_test_cmd(CommandCategory::Quick);
        let start = Instant::now();

        // Register and assign to socket
        core.register_pending_ack(
            cmd_id(1),
            cmd.clone(),
            Priority::Normal,
            CommandCategory::Quick,
            CameraId::CAMERA_1,
            CommandKind::Command,
            start,
        );

        // Simulate ACK received, assign to socket
        core.handle_ack_with_id(Some(ViscaSocket::S1), Some(cmd_id(1)), start);

        // Check socket timeout at t+60ms (quick_timeout=50ms + 10ms margin, within duration 200ms)
        let actions = core.check_timeouts(start + Duration::from_millis(60));

        // Should have retry action
        let has_retry = actions
            .iter()
            .any(|a| matches!(a, SchedulerAction::RetryCommand { id, .. } if *id == cmd_id(1)));
        assert!(
            has_retry,
            "Expected retry for socket timeout within duration"
        );
    }

    #[test]
    fn test_max_retry_duration_preserved_across_retries() {
        // Test that submitted_at is preserved when metadata is re-inserted during retries
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig {
            max_retries: 10,
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_millis(500),
            exponential_backoff: false,
        };
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let cmd = make_duration_test_cmd(CommandCategory::Quick);
        let start = Instant::now();

        // Register initial command
        core.register_pending_ack(
            cmd_id(1),
            cmd.clone(),
            Priority::Normal,
            CommandCategory::Quick,
            CameraId::CAMERA_1,
            CommandKind::Command,
            start,
        );

        // Get the initial submitted_at
        let initial_submitted_at = core
            .commands
            .get(&cmd_id(1))
            .map(|state| state.submitted_at);
        assert!(initial_submitted_at.is_some());

        // Simulate ACK received and socket assignment
        core.handle_ack_with_id(
            Some(ViscaSocket::S1),
            Some(cmd_id(1)),
            start + Duration::from_millis(50),
        );

        // Verify submitted_at is preserved after socket assignment
        let after_ack_submitted_at = core
            .commands
            .get(&cmd_id(1))
            .map(|state| state.submitted_at);
        assert_eq!(
            initial_submitted_at, after_ack_submitted_at,
            "submitted_at should be preserved after ACK"
        );

        // Queue a retry
        core.queue_retry_for_command(cmd_id(1), start + Duration::from_millis(100));

        // The command should still be trackable with original submitted_at
        // Note: After retry, the command may be in retry_queue not commands
        // But if it's still in commands, the timestamp should match
        if let Some(state) = core.commands.get(&cmd_id(1)) {
            assert_eq!(
                state.submitted_at,
                initial_submitted_at.unwrap(),
                "submitted_at should be preserved across retries"
            );
        }
    }

    #[test]
    fn test_max_retry_duration_with_high_retry_budget() {
        // Ensure that even with high retry budget, duration limit is enforced
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig {
            max_retries: 100, // Very high retry count
            base_retry_delay: Duration::from_millis(1),
            max_retry_duration: Duration::from_millis(50), // Very short duration
            exponential_backoff: false,
        };
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

        let cmd = make_duration_test_cmd(CommandCategory::Quick);
        let start = Instant::now();

        core.register_pending_ack(
            cmd_id(1),
            cmd,
            Priority::Normal,
            CommandCategory::Quick,
            CameraId::CAMERA_1,
            CommandKind::Command,
            start,
        );

        // Even with 100 max_retries, should fail after 50ms duration
        let action = core.queue_retry_for_command(cmd_id(1), start + Duration::from_millis(60));

        assert!(
            matches!(
                action,
                Some(SchedulerAction::CommandFailed {
                    id,
                    error: Error::Timeout
                }) if id == cmd_id(1)
            ),
            "Duration limit should override high retry budget"
        );
    }

    #[test]
    fn test_retry_config_should_retry_method_parity() {
        // Verify that our implementation matches RetryConfig::should_retry semantics
        let config = RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_millis(500),
            exponential_backoff: true,
        };

        let start = Instant::now();

        // Test RetryConfig::should_retry directly
        assert!(config.should_retry(0, start), "Attempt 0 should retry");
        assert!(config.should_retry(1, start), "Attempt 1 should retry");
        assert!(config.should_retry(2, start), "Attempt 2 should retry");
        assert!(
            !config.should_retry(3, start),
            "Attempt 3 should NOT retry (max reached)"
        );

        // Verify SchedulerCore enforces the same semantics
        let timeout_config = TimeoutConfig::default();
        let mut core = SchedulerCore::with_retry_config(timeout_config, config);

        let cmd = make_duration_test_cmd(CommandCategory::Movement);
        core.register_pending_ack(
            cmd_id(1),
            cmd,
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            start,
        );

        let error = ViscaError::from_byte(0x41); // Retryable for Movement

        // Note: SchedulerCore uses category-based budgets which differ from raw max_retries
        // Movement category uses base max_retries (3), so behavior should align
        assert!(
            core.should_retry_command(cmd_id(1), &error, start),
            "SchedulerCore should align with RetryConfig at start"
        );
    }

    #[test]
    fn test_inquiry_spacing_blocks_too_fast_inquiries() {
        // Test that inquiries respect min_inquiry_spacing
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        core.set_min_inquiry_spacing(Duration::from_millis(100));
        let now = Instant::now();

        let inquiry = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
            kind: CommandKind::Inquiry,
            category: CommandCategory::Quick,
            response_type: Some(InquiryKind::ZoomPosition),
        });

        // Queue 2 inquiries
        for id in 1..=2 {
            core.queue_command(PendingCommand {
                id: cmd_id(id),
                command: inquiry.clone(),
                priority: Priority::Normal,
                category: CommandCategory::Quick,
                camera_id: CameraId::CAMERA_1,
                submitted_at: now,
                kind: CommandKind::Inquiry,
            });
        }

        // First inquiry should be sendable
        let inq1 = core.next_item_to_send(now);
        assert!(inq1.is_some(), "First inquiry should be sendable");
        assert_eq!(inq1.unwrap().id, cmd_id(1));
        core.start_inquiry(
            cmd_id(1),
            inquiry.clone(),
            Priority::Normal,
            CommandCategory::Quick,
            CameraId::CAMERA_1,
            CommandKind::Inquiry,
            now,
        );

        // Second inquiry should NOT be sendable immediately (spacing not satisfied)
        assert!(
            !core.can_send_inquiry(now),
            "Second inquiry should be blocked by spacing"
        );
        let inq2 = core.next_item_to_send(now);
        assert!(
            inq2.is_none(),
            "No inquiry should be returned when spacing not satisfied"
        );

        // After 50ms (less than spacing), still should not be sendable
        let too_soon = now + Duration::from_millis(50);
        assert!(
            !core.can_send_inquiry(too_soon),
            "Inquiry should still be blocked before spacing expires"
        );

        // After 100ms+ (spacing satisfied), should be sendable
        let after_spacing = now + Duration::from_millis(100);
        assert!(
            core.can_send_inquiry(after_spacing),
            "Inquiry should be sendable after spacing expires"
        );
        let inq2_delayed = core.next_item_to_send(after_spacing);
        assert!(
            inq2_delayed.is_some(),
            "Second inquiry should be returned after spacing"
        );
        assert_eq!(inq2_delayed.unwrap().id, cmd_id(2));
    }

    #[test]
    fn test_inquiry_spacing_allows_commands_while_blocking() {
        // Test that commands can still be sent even when inquiry spacing blocks inquiries
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        core.set_min_inquiry_spacing(Duration::from_millis(150));
        let now = Instant::now();

        let inquiry = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
            kind: CommandKind::Inquiry,
            category: CommandCategory::Quick,
            response_type: Some(InquiryKind::ZoomPosition),
        });

        let command = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]),
            kind: CommandKind::Command,
            category: CommandCategory::Movement,
            response_type: None,
        });

        // Send first inquiry
        core.queue_command(PendingCommand {
            id: cmd_id(1),
            command: inquiry.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Quick,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Inquiry,
        });

        let _inq1 = core.next_item_to_send(now).unwrap();
        core.start_inquiry(
            cmd_id(1),
            inquiry.clone(),
            Priority::Normal,
            CommandCategory::Quick,
            CameraId::CAMERA_1,
            CommandKind::Inquiry,
            now,
        );

        // Queue another inquiry and a command
        core.queue_command(PendingCommand {
            id: cmd_id(2),
            command: inquiry.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Quick,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Inquiry,
        });

        core.queue_command(PendingCommand {
            id: cmd_id(3),
            command: command.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Movement,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
            kind: CommandKind::Command,
        });

        // Inquiry should be blocked, but command should be sendable
        let next = core.next_item_to_send(now);
        assert!(next.is_some(), "Command should be sendable");
        let cmd = next.unwrap();
        assert_eq!(
            cmd.id,
            cmd_id(3),
            "Command should be returned, not blocked inquiry"
        );
        assert_eq!(cmd.kind, CommandKind::Command);
    }

    #[test]
    fn test_inquiry_spacing_zero_means_no_delay() {
        // Test that spacing of zero (default) doesn't block inquiries
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        // Default spacing is Duration::ZERO - no artificial delay
        let now = Instant::now();

        let inquiry = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
            kind: CommandKind::Inquiry,
            category: CommandCategory::Quick,
            response_type: Some(InquiryKind::ZoomPosition),
        });

        // Queue 3 inquiries
        for id in 1..=3 {
            core.queue_command(PendingCommand {
                id: cmd_id(id),
                command: inquiry.clone(),
                priority: Priority::Normal,
                category: CommandCategory::Quick,
                camera_id: CameraId::CAMERA_1,
                submitted_at: now,
                kind: CommandKind::Inquiry,
            });
        }

        // With default max_inquiries_inflight (8), all should be sendable immediately
        for expected_id in 1..=3 {
            let inq = core.next_item_to_send(now);
            assert!(
                inq.is_some(),
                "Inquiry {expected_id} should be sendable with zero spacing"
            );
            let inq = inq.unwrap();
            assert_eq!(inq.id, cmd_id(expected_id));
            core.start_inquiry(
                cmd_id(expected_id),
                inquiry.clone(),
                Priority::Normal,
                CommandCategory::Quick,
                CameraId::CAMERA_1,
                CommandKind::Inquiry,
                now,
            );
        }
    }

    // ==================== Stale Retry Filtering Tests ====================
    // These tests verify that get_ready_retries correctly filters out stale
    // retries for commands that have already completed or failed.

    #[test]
    fn test_stale_retry_dropped_after_command_completion() {
        // Arrange: register a command, schedule a retry, then complete the command
        // before retry_at. Verify the retry is dropped.
        let mut core = SchedulerCore::with_retry_config(
            TimeoutConfig::default(),
            RetryConfig {
                max_retries: 3,
                base_retry_delay: Duration::from_millis(100),
                max_retry_duration: Duration::from_secs(5),
                exponential_backoff: false,
            },
        );

        let cmd_id = cmd_id(1);
        let camera_id = CameraId::CAMERA_1;
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );
        let now = Instant::now();

        // Register command
        core.register_pending_ack(
            cmd_id,
            command.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );

        // Queue a retry
        let action = core.queue_retry_for_command(cmd_id, now);
        assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));

        // Verify retry is queued
        assert_eq!(core.retry_queue_depth(), 1);

        // Complete the command (simulating successful response before retry_at)
        core.complete_command(cmd_id);

        // Advance time past retry_at and try to get retries
        let retries = core.get_ready_retries(now + Duration::from_millis(200));

        // Assert: no retries should be returned since command is no longer active
        assert!(
            retries.is_empty(),
            "Stale retry should be dropped for completed command"
        );

        // The retry queue should now be empty (stale entry was popped and discarded)
        assert_eq!(core.retry_queue_depth(), 0);
    }

    #[test]
    fn test_stale_retry_dropped_after_inquiry_completion() {
        // Same as above but for inquiries
        let mut core = SchedulerCore::with_retry_config(
            TimeoutConfig::default(),
            RetryConfig {
                max_retries: 3,
                base_retry_delay: Duration::from_millis(100),
                max_retry_duration: Duration::from_secs(5),
                exponential_backoff: false,
            },
        );

        let cmd_id = cmd_id(1);
        let camera_id = CameraId::CAMERA_1;
        let command = create_test_command(
            vec![0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR],
            Some(InquiryKind::ZoomPosition),
            CommandCategory::Quick,
            camera_id,
        );
        let now = Instant::now();

        // Start inquiry (this registers it in inquiries_inflight)
        core.start_inquiry(
            cmd_id,
            command.clone(),
            Priority::Normal,
            CommandCategory::Quick,
            CameraId::CAMERA_1,
            CommandKind::Inquiry,
            now,
        );

        // Queue a retry
        let action = core.queue_retry_for_command(cmd_id, now);
        assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));

        // Complete the inquiry
        core.complete_inquiry(cmd_id);

        // Advance time past retry_at and try to get retries
        let retries = core.get_ready_retries(now + Duration::from_millis(200));

        // Assert: no retries should be returned
        assert!(
            retries.is_empty(),
            "Stale retry should be dropped for completed inquiry"
        );
    }

    #[test]
    fn test_stale_retry_dropped_after_command_failure() {
        // Arrange: schedule retry, then simulate a failure that removes metadata
        let mut core = SchedulerCore::with_retry_config(
            TimeoutConfig::default(),
            RetryConfig {
                max_retries: 3,
                base_retry_delay: Duration::from_millis(100),
                max_retry_duration: Duration::from_secs(5),
                exponential_backoff: false,
            },
        );

        let cmd_id = cmd_id(1);
        let camera_id = CameraId::CAMERA_1;
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );
        let now = Instant::now();

        // Register command
        core.register_pending_ack(
            cmd_id,
            command.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );

        // Queue a retry
        let action = core.queue_retry_for_command(cmd_id, now);
        assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));

        // Cancel the command (simulating failure cleanup)
        core.cancel_command(cmd_id);

        // Note: cancel_command already removes the retry from the queue,
        // but if there were a race condition where a retry was added after
        // cancel started, it would be filtered by get_ready_retries.

        // Advance time and verify no retries
        let retries = core.get_ready_retries(now + Duration::from_millis(200));
        assert!(
            retries.is_empty(),
            "No retries should be returned after cancel"
        );
    }

    #[test]
    fn test_superseded_retry_entries_are_ignored() {
        // Arrange: force two retries for the same cmd_id with different attempt values
        // This simulates overlapping timeout/error paths that could schedule multiple retries
        let mut core = SchedulerCore::with_retry_config(
            TimeoutConfig::default(),
            RetryConfig {
                max_retries: 5,
                base_retry_delay: Duration::from_millis(100),
                max_retry_duration: Duration::from_secs(5),
                exponential_backoff: false,
            },
        );

        let cmd_id = cmd_id(1);
        let camera_id = CameraId::CAMERA_1;
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );
        let now = Instant::now();

        // Register command
        core.register_pending_ack(
            cmd_id,
            command.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );

        // Queue first retry (attempt 1)
        let action1 = core.queue_retry_for_command(cmd_id, now);
        assert!(matches!(
            action1,
            Some(SchedulerAction::RetryCommand { .. })
        ));

        // Simulate another overlapping path queueing a second retry
        // This increments retry_attempts to 2 and queues another retry
        let action2 = core.queue_retry_for_command(cmd_id, now);
        assert!(matches!(
            action2,
            Some(SchedulerAction::RetryCommand { .. })
        ));

        // Now retry_attempts[cmd_id] == 2, but we have two entries in queue:
        // - One with attempt=1 (stale)
        // - One with attempt=2 (current)

        // Advance time past both retry_at values
        let retries = core.get_ready_retries(now + Duration::from_millis(300));

        // Assert: only one retry should be returned (the one with attempt=2)
        assert_eq!(
            retries.len(),
            1,
            "Only the current attempt should be returned, stale entry should be dropped"
        );
        assert_eq!(
            retries[0].attempt, 2,
            "The returned retry should be attempt 2"
        );

        // Both entries should have been popped from the queue
        assert_eq!(core.retry_queue_depth(), 0);
    }

    #[test]
    fn test_valid_retry_still_works() {
        // Sanity test: ensure valid retries are still returned properly
        let mut core = SchedulerCore::with_retry_config(
            TimeoutConfig::default(),
            RetryConfig {
                max_retries: 3,
                base_retry_delay: Duration::from_millis(100),
                max_retry_duration: Duration::from_secs(5),
                exponential_backoff: false,
            },
        );

        let cmd_id = cmd_id(1);
        let camera_id = CameraId::CAMERA_1;
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );
        let now = Instant::now();

        // Register command
        core.register_pending_ack(
            cmd_id,
            command.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            CameraId::CAMERA_1,
            CommandKind::Command,
            now,
        );

        // Queue a retry
        let action = core.queue_retry_for_command(cmd_id, now);
        assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));

        // Command is still active (not completed/cancelled)
        // Advance time past retry_at
        let retries = core.get_ready_retries(now + Duration::from_millis(200));

        // Assert: retry should be returned
        assert_eq!(retries.len(), 1, "Valid retry should be returned");
        assert_eq!(retries[0].id, cmd_id);
        assert_eq!(retries[0].attempt, 1);
    }

    #[test]
    fn test_multiple_commands_with_valid_and_stale_retries() {
        // Test with multiple commands: some completed, some still active
        let mut core = SchedulerCore::with_retry_config(
            TimeoutConfig::default(),
            RetryConfig {
                max_retries: 3,
                base_retry_delay: Duration::from_millis(100),
                max_retry_duration: Duration::from_secs(5),
                exponential_backoff: false,
            },
        );

        let camera_id = CameraId::CAMERA_1;
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );
        let now = Instant::now();

        // Register 3 commands
        for i in 1..=3 {
            core.register_pending_ack(
                cmd_id(i),
                command.clone(),
                Priority::Normal,
                CommandCategory::Movement,
                CameraId::CAMERA_1,
                CommandKind::Command,
                now,
            );
            // Queue a retry for each
            let action = core.queue_retry_for_command(cmd_id(i), now);
            assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));
        }

        // Complete command 1 and 3, leave 2 active
        core.complete_command(cmd_id(1));
        core.complete_command(cmd_id(3));

        // Advance time and get retries
        let retries = core.get_ready_retries(now + Duration::from_millis(200));

        // Only command 2's retry should be returned
        assert_eq!(
            retries.len(),
            1,
            "Only active command's retry should return"
        );
        assert_eq!(
            retries[0].id,
            cmd_id(2),
            "Command 2's retry should be returned"
        );
    }

    // =============================================================================
    // Sequence Correlation Safety Tests (Issue #475)
    //
    // These tests verify that sequenced replies with unmatched sequences are
    // ignored to prevent stale/duplicate UDP packets from completing or failing
    // the wrong command.
    // =============================================================================

    #[test]
    fn test_late_completion_after_socket_reuse_sequenced() {
        // Test: Late completion for a completed command (sequenced) must NOT
        // complete a new command that has reused the same socket.
        //
        // Scenario:
        // 1. Register command A, assign socket 1, register sequence seqA
        // 2. Complete A and ensure socket 1 becomes free + finish_sequence runs
        // 3. Start command B on socket 1 (simulate ACK)
        // 4. Inject a Completion event for socket 1 with sequence = Some(seqA) but cmd_id unresolved
        // Expected: B is not completed/freed; no actions emitted; ignored counter increments

        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();
        let camera_id = CameraId::CAMERA_1;

        // Create two commands
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );

        // Step 1: Register command A
        core.register_pending_ack(
            cmd_id(1),
            command.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(cmd_id(1), 100); // seqA = 100

        // ACK for command A
        let ack_a = SchedulerEvent::Ack {
            socket: Some(ViscaSocket::S1),
            cmd_id: Some(cmd_id(1)),
            sequence: Some(100),
        };
        core.process_event(ack_a, now);

        // Verify A is on socket 1
        let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
        assert!(!free);
        assert_eq!(socket_cmd_id, Some(cmd_id(1)));

        // Step 2: Complete command A
        let complete_a = SchedulerEvent::Completion {
            socket: Some(ViscaSocket::S1),
            cmd_id: Some(cmd_id(1)),
            sequence: Some(100),
            response: Response::Completion {
                socket: Some(ViscaSocket::S1),
            },
        };
        let actions = core.process_event(complete_a, now);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandComplete { id, .. } => assert_eq!(*id, cmd_id(1)),
            _ => panic!("Expected CommandComplete for A"),
        }

        // Socket 1 is now free
        let (free, _, _) = core.socket_state(ViscaSocket::S1);
        assert!(free);

        // Step 3: Start command B on socket 1
        core.register_pending_ack(
            cmd_id(2),
            command,
            Priority::Normal,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(cmd_id(2), 101); // seqB = 101

        // ACK for command B (gets socket 1)
        let ack_b = SchedulerEvent::Ack {
            socket: Some(ViscaSocket::S1),
            cmd_id: Some(cmd_id(2)),
            sequence: Some(101),
        };
        core.process_event(ack_b, now);

        // Verify B is on socket 1
        let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
        assert!(!free);
        assert_eq!(socket_cmd_id, Some(cmd_id(2)));

        // Record the counter before the stale event
        let counter_before = core.ignored_unmatched_sequenced_replies();

        // Step 4: Inject a late completion with sequence=100 (A's sequence)
        // The sequence won't resolve because A is already completed and finish_sequence was called
        let late_complete = SchedulerEvent::Completion {
            socket: Some(ViscaSocket::S1), // Same socket as B
            cmd_id: None,                  // Sequence lookup failed (A is gone)
            sequence: Some(100),           // Sequenced reply for old command A
            response: Response::Completion {
                socket: Some(ViscaSocket::S1),
            },
        };
        let actions = core.process_event(late_complete, now);

        // Assert: NO actions should be emitted
        assert!(
            actions.is_empty(),
            "Late sequenced completion must not produce any actions"
        );

        // Assert: Counter should increment
        assert_eq!(
            core.ignored_unmatched_sequenced_replies(),
            counter_before + 1,
            "Ignored counter should increment for unmatched sequenced reply"
        );

        // Assert: B is still active on socket 1
        let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
        assert!(!free, "Socket 1 should still be occupied by B");
        assert_eq!(
            socket_cmd_id,
            Some(cmd_id(2)),
            "B should still own socket 1"
        );
        assert!(
            core.is_command_pending(cmd_id(2)),
            "B should still be pending"
        );
    }

    #[test]
    fn test_late_error_without_socket_sequenced() {
        // Test: Late error with unmatched sequence (and socket=None) must NOT
        // attribute to the most-recent pending-ACK command.
        //
        // Scenario:
        // 1. Create two pending ACK commands
        // 2. Inject an Error event with sequence=Some(unknown) and socket=None
        // Expected: The scheduler does NOT attribute it to pending_ack.max_by_key(sent_time)

        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();
        let camera_id = CameraId::CAMERA_1;

        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );

        // Register two pending-ACK commands
        core.register_pending_ack(
            cmd_id(1),
            command.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_pending_ack(
            cmd_id(2),
            command,
            Priority::Normal,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now + Duration::from_millis(1), // Slightly later
        );

        // Both are pending
        assert!(core.is_command_pending(cmd_id(1)));
        assert!(core.is_command_pending(cmd_id(2)));

        let counter_before = core.ignored_unmatched_sequenced_replies();

        // Inject error with unknown sequence (simulates stale packet)
        let stale_error = SchedulerEvent::Error {
            socket: None,        // No socket nibble
            cmd_id: None,        // Sequence lookup failed
            sequence: Some(999), // Unknown sequence (stale/duplicate)
            code: 0x41,          // Not Executable
        };
        let actions = core.process_event(stale_error, now);

        // Assert: NO actions (especially no CommandFailed)
        assert!(
            actions.is_empty(),
            "Stale sequenced error must not produce any actions"
        );

        // Assert: Counter incremented
        assert_eq!(
            core.ignored_unmatched_sequenced_replies(),
            counter_before + 1,
            "Ignored counter should increment"
        );

        // Assert: Both commands are still pending
        assert!(
            core.is_command_pending(cmd_id(1)),
            "Command 1 should still be pending"
        );
        assert!(
            core.is_command_pending(cmd_id(2)),
            "Command 2 should still be pending"
        );
    }

    #[test]
    fn test_late_inquiry_reply_sequenced() {
        // Test: Late inquiry reply with unmatched sequence must NOT
        // pop from the FIFO order queue.

        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();
        let camera_id = CameraId::CAMERA_1;

        let inquiry = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
            kind: CommandKind::Inquiry,
            category: CommandCategory::Quick,
            response_type: Some(InquiryKind::ZoomPosition),
        });

        // Start an inquiry (adds to FIFO)
        core.start_inquiry(
            cmd_id(1),
            inquiry,
            Priority::Normal,
            CommandCategory::Quick,
            camera_id,
            CommandKind::Inquiry,
            now,
        );

        // Verify inquiry is in queue
        assert_eq!(core.inquiries_order.len(), 1);
        assert!(core.inflight_inquiry_ids.contains(&cmd_id(1)));

        let counter_before = core.ignored_unmatched_sequenced_replies();

        // Inject a stale inquiry reply with unknown sequence
        let stale_reply = SchedulerEvent::InquiryReply {
            cmd_id: None,         // Sequence lookup failed
            sequence: Some(9999), // Unknown sequence
            response: Response::Inquiry(crate::command::InquiryData::ZoomPosition {
                position: 0x1234,
            }),
        };
        let actions = core.process_event(stale_reply, now);

        // Assert: NO actions
        assert!(
            actions.is_empty(),
            "Stale sequenced inquiry reply must not produce actions"
        );

        // Assert: Counter incremented
        assert_eq!(
            core.ignored_unmatched_sequenced_replies(),
            counter_before + 1,
            "Ignored counter should increment"
        );

        // Assert: The inquiry is still in the queue (FIFO not popped)
        assert_eq!(
            core.inquiries_order.len(),
            1,
            "FIFO queue should not be popped"
        );
        assert!(
            core.inflight_inquiry_ids.contains(&cmd_id(1)),
            "Inquiry should still be inflight"
        );
    }

    #[test]
    fn test_unsequenced_completion_still_uses_socket_fallback() {
        // Test: Raw VISCA (no sequence) completions can still use socket fallback.
        // This ensures the change doesn't break raw VISCA behavior.

        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();
        let camera_id = CameraId::CAMERA_1;

        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );

        // Register command
        core.register_pending_ack(
            cmd_id(1),
            command,
            Priority::Normal,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now,
        );

        // ACK assigns socket (raw VISCA - no sequence)
        let ack = SchedulerEvent::Ack {
            socket: Some(ViscaSocket::S1),
            cmd_id: Some(cmd_id(1)),
            sequence: None, // Raw VISCA
        };
        core.process_event(ack, now);

        // Completion with no cmd_id but matching socket (raw VISCA)
        let complete = SchedulerEvent::Completion {
            socket: Some(ViscaSocket::S1),
            cmd_id: None,   // No sequence mapping
            sequence: None, // Raw VISCA - heuristic fallback allowed
            response: Response::Completion {
                socket: Some(ViscaSocket::S1),
            },
        };
        let actions = core.process_event(complete, now);

        // Assert: Command should complete via socket fallback
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandComplete { id, .. } => {
                assert_eq!(
                    *id,
                    cmd_id(1),
                    "Raw VISCA should complete via socket fallback"
                );
            }
            _ => panic!("Expected CommandComplete for raw VISCA"),
        }
    }

    #[test]
    fn test_unsequenced_error_still_uses_temporal_fallback() {
        // Test: Raw VISCA (no sequence) errors can still use temporal fallback.

        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();
        let camera_id = CameraId::CAMERA_1;

        // Use Quick category so 0x41 is NOT retryable (only Movement/Preset are retried)
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Quick,
            camera_id,
        );

        // Register a pending command
        core.register_pending_ack(
            cmd_id(1),
            command,
            Priority::Normal,
            CommandCategory::Quick,
            camera_id,
            CommandKind::Command,
            now,
        );

        // Error with no socket nibble (raw VISCA)
        let error = SchedulerEvent::Error {
            socket: None,
            cmd_id: None,
            sequence: None, // Raw VISCA - temporal fallback allowed
            code: 0x41,     // Not Executable (not retryable for Quick category)
        };
        let actions = core.process_event(error, now);

        // Assert: Error should be attributed via temporal fallback
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SchedulerAction::CommandFailed { id, .. } => {
                assert_eq!(
                    *id,
                    cmd_id(1),
                    "Raw VISCA error should attribute via temporal fallback"
                );
            }
            _ => panic!("Expected CommandFailed for raw VISCA error"),
        }
    }

    #[test]
    fn test_late_ack_with_unmatched_sequence() {
        // Test: Late ACK with unmatched sequence should be ignored.

        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();
        let camera_id = CameraId::CAMERA_1;

        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );

        // Register and complete a command
        core.register_pending_ack(
            cmd_id(1),
            command.clone(),
            Priority::Normal,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now,
        );
        core.register_sequence(cmd_id(1), 100);

        // ACK and complete the command
        let ack = SchedulerEvent::Ack {
            socket: Some(ViscaSocket::S1),
            cmd_id: Some(cmd_id(1)),
            sequence: Some(100),
        };
        core.process_event(ack, now);

        let complete = SchedulerEvent::Completion {
            socket: Some(ViscaSocket::S1),
            cmd_id: Some(cmd_id(1)),
            sequence: Some(100),
            response: Response::Completion {
                socket: Some(ViscaSocket::S1),
            },
        };
        core.process_event(complete, now);

        // Start a new command
        core.register_pending_ack(
            cmd_id(2),
            command,
            Priority::Normal,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now,
        );

        let counter_before = core.ignored_unmatched_sequenced_replies();

        // Late ACK for old sequence
        let late_ack = SchedulerEvent::Ack {
            socket: Some(ViscaSocket::S1),
            cmd_id: None,        // Sequence lookup failed
            sequence: Some(100), // Old sequence
        };
        let actions = core.process_event(late_ack, now);

        // Assert: No actions, counter incremented
        assert!(actions.is_empty(), "Late sequenced ACK must be ignored");
        assert_eq!(
            core.ignored_unmatched_sequenced_replies(),
            counter_before + 1,
            "Counter should increment for ignored ACK"
        );

        // Command 2 should still be pending (not affected)
        assert!(
            core.is_command_pending(cmd_id(2)),
            "Command 2 should still be pending"
        );
    }

    #[test]
    fn test_fail_after_receive_error_frees_socket() {
        // Test: fail_after_receive_error should free any socket allocated to the command.
        //
        // This ensures that receive-side errors (decode failures, protocol errors)
        // immediately release socket resources so subsequent commands can proceed.
        //
        // Scenario:
        // 1. Start a command and assign it a socket via ACK
        // 2. Call fail_after_receive_error for that command
        // 3. Verify the socket is freed

        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();
        let camera_id = CameraId::CAMERA_1;

        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );

        // Register command and assign socket via ACK
        core.register_pending_ack(
            cmd_id(1),
            command,
            Priority::Normal,
            CommandCategory::Movement,
            camera_id,
            CommandKind::Command,
            now,
        );

        // Process ACK to assign socket
        let ack = SchedulerEvent::Ack {
            socket: Some(ViscaSocket::S1),
            cmd_id: Some(cmd_id(1)),
            sequence: None,
        };
        core.process_event(ack, now);

        // Verify socket is occupied
        let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
        assert!(
            !free,
            "S1 should be occupied before fail_after_receive_error"
        );
        assert_eq!(
            socket_cmd_id,
            Some(cmd_id(1)),
            "S1 should be assigned to command 1"
        );

        // Fail the command with a receive error (simulates decode failure)
        let error = Error::invalid_response_length(1, &[0x01, 0x02, 0x03]);
        let action = core.fail_after_receive_error(cmd_id(1), error);

        // Verify action is CommandFailed
        assert!(action.is_some(), "Should produce CommandFailed action");
        match action.unwrap() {
            SchedulerAction::CommandFailed { id, error } => {
                assert_eq!(id, cmd_id(1));
                assert!(
                    matches!(error, Error::InvalidResponseLength { .. }),
                    "Error should be InvalidResponseLength"
                );
            }
            other => panic!("Expected CommandFailed, got {:?}", other),
        }

        // Verify socket is now free
        let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
        assert!(free, "S1 should be free after fail_after_receive_error");
        assert_eq!(
            socket_cmd_id, None,
            "S1 should not be assigned to any command"
        );

        // Verify command is no longer pending
        assert!(
            !core.is_command_pending(cmd_id(1)),
            "Command should be removed from pending"
        );
    }
}
