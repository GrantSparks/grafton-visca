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
    transport::RetryAttempt,
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
/// - Cancel tracking: `cancel_requested`
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
    /// Whether a cancel has been requested for this command.
    ///
    /// When set to true before a socket is assigned (ACK received), the cancel
    /// will be sent immediately after the socket is assigned. This eliminates
    /// the need for an out-of-band cancel tracking map and ensures cancels are
    /// bounded to command lifetime.
    pub cancel_requested: bool,

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
    /// Send a cancel command for a specific socket.
    ///
    /// This action is emitted when a cancel was requested for a command before
    /// a socket was assigned, and an ACK has now assigned the socket. The cancel
    /// should be sent immediately to the transport.
    SendCancel {
        /// Camera ID for addressing the cancel message.
        camera_id: crate::camera_id::CameraId,
        /// Socket to cancel.
        socket: ViscaSocket,
    },
}

/// Kind of timeout that can occur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutKind {
    /// Timeout waiting for ACK.
    Ack,
    // Response, // Can be added later for post-ACK response timeouts
}

/// Categorizes the source of a timeout for unified handling.
///
/// This enum enables consolidation of the three nearly identical timeout-handling
/// patterns in `check_timeouts()` into a single `handle_timeout()` method.
#[derive(Debug, Clone, Copy)]
pub(crate) enum TimeoutSource {
    /// Command timed out waiting for ACK (no socket assigned yet).
    Ack,
    /// Command timed out while holding a socket.
    Socket(ViscaSocket),
    /// Inquiry timed out waiting for data reply.
    Inquiry,
}

/// How a reply is correlated to its originating command.
///
/// This enum consolidates the reply identification triplet that was previously
/// spread across `(socket: Option<ViscaSocket>, cmd_id: Option<CommandId>, sequence: Option<u32>)`
/// into semantically meaningful variants that encode the protocol's correlation rules.
///
/// # Variants
///
/// - **Resolved**: The reply has already been matched to a specific command ID
///   (via sequence lookup or other means). This is the authoritative case.
///
/// - **Sequenced**: The reply carries a Sony sequence number. If the corresponding
///   command is still active, the caller should have resolved it to `Resolved`.
///   An unresolved `Sequenced` variant indicates a stale/duplicate reply that
///   must be ignored (no heuristic fallback allowed).
///
/// - **BySocket**: Raw VISCA mode - no sequence number, but a socket nibble is present.
///   Correlation is done by socket assignment. Heuristic fallback is allowed.
///
/// - **Unknown**: No sequence, no socket nibble (e.g., inquiry errors with y=0).
///   Requires FIFO or temporal heuristics for attribution.
#[derive(Debug, Clone, Copy)]
pub enum ReplySource {
    /// Already resolved to a specific command.
    ///
    /// This variant is used when sequence-based or other correlation has
    /// definitively identified the originating command.
    Resolved {
        /// The command this reply belongs to.
        cmd_id: CommandId,
        /// Socket nibble from frame (if present, for socket cleanup).
        socket: Option<ViscaSocket>,
    },
    /// Sony-sequenced reply that did not resolve to an active command.
    ///
    /// When a sequenced reply's sequence number has no corresponding active
    /// command, it's a stale or duplicate reply. These must be ignored without
    /// falling back to socket or temporal heuristics.
    Sequenced {
        /// The 32-bit Sony sequence number from the reply.
        sequence: u32,
        /// Socket nibble from frame (for logging/debugging).
        socket: Option<ViscaSocket>,
    },
    /// Raw VISCA mode - correlation by socket assignment.
    ///
    /// Used when no sequence number is present but a socket nibble (y in 90 4y)
    /// identifies which socket slot the reply belongs to. Heuristic fallback
    /// to pending commands is allowed if no command is currently assigned
    /// to the socket.
    BySocket {
        /// Socket from the reply frame.
        socket: ViscaSocket,
    },
    /// No sequence, no socket nibble - requires heuristic attribution.
    ///
    /// Used for inquiry errors (y=0) and other replies without explicit
    /// correlation info. Resolution uses FIFO order queue for inquiries
    /// or temporal correlation (most recent pending) for commands.
    Unknown,
}

impl ReplySource {
    /// Construct a `ReplySource` from raw protocol fields.
    ///
    /// This constructor encapsulates the logic of choosing the appropriate
    /// variant based on what correlation information is available.
    ///
    /// # Arguments
    /// - `cmd_id`: Pre-resolved command ID (from sequence lookup)
    /// - `sequence`: Sony sequence number from frame metadata
    /// - `socket`: Socket nibble from reply frame
    #[inline]
    pub fn from_fields(
        cmd_id: Option<CommandId>,
        sequence: Option<u32>,
        socket: Option<ViscaSocket>,
    ) -> Self {
        if let Some(id) = cmd_id {
            // Sequence lookup succeeded - we have authoritative attribution
            ReplySource::Resolved { cmd_id: id, socket }
        } else if let Some(seq) = sequence {
            // Sequence present but no match - stale/duplicate
            ReplySource::Sequenced {
                sequence: seq,
                socket,
            }
        } else if let Some(sock) = socket {
            // Raw VISCA mode - use socket
            ReplySource::BySocket { socket: sock }
        } else {
            // No correlation info
            ReplySource::Unknown
        }
    }

    /// Returns the pre-resolved command ID, if any.
    #[inline]
    pub fn cmd_id(&self) -> Option<CommandId> {
        match self {
            ReplySource::Resolved { cmd_id, .. } => Some(*cmd_id),
            _ => None,
        }
    }

    /// Returns the socket from the reply, if known.
    #[inline]
    pub fn socket(&self) -> Option<ViscaSocket> {
        match self {
            ReplySource::Resolved { socket, .. } => *socket,
            ReplySource::Sequenced { socket, .. } => *socket,
            ReplySource::BySocket { socket } => Some(*socket),
            ReplySource::Unknown => None,
        }
    }

    /// Returns true if this is an unmatched sequenced reply.
    ///
    /// Unmatched sequenced replies must be ignored without fallback heuristics.
    #[inline]
    pub fn is_unmatched_sequenced(&self) -> bool {
        matches!(self, ReplySource::Sequenced { .. })
    }

    /// Returns true if heuristic fallback resolution is allowed.
    ///
    /// Only `BySocket` and `Unknown` variants allow fallback to pending-ACK
    /// or FIFO-based resolution. Sequenced replies are authoritative.
    #[inline]
    pub fn allows_heuristic_fallback(&self) -> bool {
        matches!(self, ReplySource::BySocket { .. } | ReplySource::Unknown)
    }
}

/// Events that can be fed to the scheduler core.
#[derive(Debug)]
pub enum SchedulerEvent {
    /// ACK received for a socket.
    Ack {
        /// How this reply is correlated to its command.
        source: ReplySource,
    },
    /// Command completed.
    Completion {
        /// How this reply is correlated to its command.
        source: ReplySource,
        /// Response from the camera.
        response: Response,
    },
    /// Inquiry data reply (no socket allocation).
    InquiryReply {
        /// How this reply is correlated to its command.
        source: ReplySource,
        /// Response from the camera.
        response: Response,
    },
    /// Error response.
    Error {
        /// How this reply is correlated to its command.
        source: ReplySource,
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
                cancel_requested: false,
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

    /// Request cancellation of a command by ID.
    ///
    /// This method implements lifecycle-aware cancel-by-id semantics:
    ///
    /// - **Command not active**: Returns `None` (no-op, no state retained).
    /// - **Socket already assigned**: Returns `Some((camera_id, socket))` so the caller
    ///   can send the cancel command immediately.
    /// - **Awaiting ACK (no socket yet)**: Sets `cancel_requested = true` on the command
    ///   state and returns `None`. The cancel will be emitted as a `SchedulerAction::SendCancel`
    ///   when the ACK arrives and assigns a socket.
    ///
    /// This design eliminates the need for an out-of-band `pending_cancel_ids` map,
    /// ensuring cancels are bounded to command lifetime and cleaned up automatically.
    pub fn request_cancel_by_id(
        &mut self,
        cmd_id: CommandId,
    ) -> Option<(crate::camera_id::CameraId, ViscaSocket)> {
        // Check if the command is active
        let state = self.commands.get_mut(&cmd_id)?;

        // Get camera_id from the command state (type-safe, no external CameraId needed)
        let camera_id = state.camera_id;

        // Check if a socket is already assigned
        if let Some(socket) = self.find_socket_for_command(cmd_id) {
            // Socket is assigned - caller should send cancel immediately
            debug!(
                %cmd_id,
                ?socket,
                ?camera_id,
                "Cancel requested for command with socket - returning immediately"
            );
            Some((camera_id, socket))
        } else {
            // Command is active but no socket yet - mark for cancel on ACK
            let state = self.commands.get_mut(&cmd_id)?;
            state.cancel_requested = true;
            debug!(
                %cmd_id,
                ?camera_id,
                "Cancel requested for command awaiting ACK - flagged for cancel on socket assignment"
            );
            None
        }
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
            SchedulerEvent::Ack { source } => {
                // Handle unmatched sequenced replies
                if source.is_unmatched_sequenced() {
                    if let ReplySource::Sequenced { sequence, socket } = source {
                        debug!(
                            "Ignoring unmatched sequenced ACK (sequence={}, socket={:?})",
                            sequence, socket
                        );
                    }
                    self.ignored_unmatched_sequenced_replies += 1;
                    return actions;
                }

                let socket = source.socket();
                let cmd_id = source.cmd_id();
                let (assigned_cmd_id, cancel_action) = self.handle_ack_with_id(socket, cmd_id, now);
                if let Some(cmd_id) = assigned_cmd_id {
                    trace!("Command {cmd_id} assigned to socket {socket:?}");
                }
                // If a cancel was pending for this command, emit the SendCancel action
                if let Some(action) = cancel_action {
                    actions.push(action);
                }
            }
            SchedulerEvent::Completion { source, response } => {
                // Resolve command ID using ReplySource
                let resolved_cmd_id = self.resolve_command_for_completion(&source);

                if let Some(cmd_id) = resolved_cmd_id {
                    if let Some(socket) = source
                        .socket()
                        .or_else(|| self.find_socket_for_command(cmd_id))
                    {
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
            SchedulerEvent::InquiryReply { source, response } => {
                // Resolve inquiry ID using ReplySource
                let resolved_cmd_id = self.resolve_command_for_inquiry(&source);

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
            SchedulerEvent::Error { source, code } => {
                let error = ViscaError::from_byte(code);

                // Resolve which command this error belongs to using ReplySource
                let resolved_cmd_id = self.resolve_command_for_error(&source, code);

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
                        if let Some(socket) = source
                            .socket()
                            .or_else(|| self.find_socket_for_command(cmd_id))
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
                } else if source.allows_heuristic_fallback() {
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

    /// Resolve command ID for Completion events.
    ///
    /// Returns `Some(cmd_id)` if the command can be identified, `None` if the reply
    /// should be ignored (e.g., unmatched sequenced reply) or cannot be attributed.
    fn resolve_command_for_completion(&mut self, source: &ReplySource) -> Option<CommandId> {
        match source {
            ReplySource::Resolved { cmd_id, .. } => Some(*cmd_id),
            ReplySource::Sequenced { sequence, socket } => {
                // Sequenced reply with no cmd_id match: this is a stale/duplicate reply.
                // Do NOT fall back to socket-based heuristics - that would misattribute.
                debug!(
                    "Ignoring unmatched sequenced Completion (sequence={}, socket={:?})",
                    sequence, socket
                );
                self.ignored_unmatched_sequenced_replies += 1;
                None
            }
            ReplySource::BySocket { socket } => {
                // Raw VISCA (no sequence): fall back to socket-based correlation.
                self.find_command_on_socket(*socket)
            }
            ReplySource::Unknown => {
                // No sequence, no socket: cannot attribute.
                None
            }
        }
    }

    /// Resolve command ID for InquiryReply events.
    ///
    /// Returns `Some(cmd_id)` if the inquiry can be identified, `None` if the reply
    /// should be ignored (e.g., unmatched sequenced reply) or cannot be attributed.
    fn resolve_command_for_inquiry(&mut self, source: &ReplySource) -> Option<CommandId> {
        match source {
            ReplySource::Resolved { cmd_id, .. } => {
                // Remove from order queue if present (for sequence-based reply)
                self.inquiries_order.retain(|&x| x != *cmd_id);
                Some(*cmd_id)
            }
            ReplySource::Sequenced { sequence, .. } => {
                // Sequenced reply with no cmd_id match: stale/duplicate inquiry reply.
                // Do NOT fall back to FIFO order queue - that would misattribute.
                debug!(
                    "Ignoring unmatched sequenced InquiryReply (sequence={})",
                    sequence
                );
                self.ignored_unmatched_sequenced_replies += 1;
                None
            }
            ReplySource::BySocket { .. } | ReplySource::Unknown => {
                // Raw VISCA (no sequence): pop from order queue.
                self.inquiries_order.pop_front()
            }
        }
    }

    /// Resolve command ID for Error events.
    ///
    /// For errors, resolution is more complex due to the variety of fallback
    /// heuristics needed for raw VISCA mode.
    fn resolve_command_for_error(&mut self, source: &ReplySource, code: u8) -> Option<CommandId> {
        match source {
            ReplySource::Resolved { cmd_id, .. } => Some(*cmd_id),
            ReplySource::Sequenced { sequence, socket } => {
                // Sequenced reply with no cmd_id match: stale/duplicate error.
                // Do NOT fall back to socket or temporal heuristics - that would misattribute.
                debug!(
                    "Ignoring unmatched sequenced Error (sequence={}, socket={:?}, code=0x{:02X})",
                    sequence, socket, code
                );
                self.ignored_unmatched_sequenced_replies += 1;
                None
            }
            ReplySource::BySocket { socket } => {
                // Raw VISCA: socket nibble present. Try socket mapping, then temporal fallback.
                // Some cameras emit 90 6y EE without a prior ACK; the socket nibble may not be reliable.
                self.find_command_on_socket(*socket).or_else(|| {
                    // Fall back to most-recent pending ACK when no command is actually allocated to that socket.
                    self.find_most_recent_pending_command()
                })
            }
            ReplySource::Unknown => {
                // Raw VISCA: y == 0 case (inquiry errors and some syntax errors).
                // If an inquiry is in flight, attribute to the oldest. Otherwise, use temporal correlation.
                self.inquiries_order
                    .front()
                    .copied()
                    .or_else(|| self.find_most_recent_pending_command())
            }
        }
    }

    /// Find the most recently sent pending command (temporal correlation fallback).
    fn find_most_recent_pending_command(&self) -> Option<CommandId> {
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
    }

    /// Check for timeouts and return commands that need action.
    pub fn check_timeouts(&mut self, now: Instant) -> Vec<SchedulerAction> {
        let mut actions = Vec::new();

        // Collect timed-out socket commands
        let mut timed_out_sockets = Vec::new();
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
                    timed_out_sockets.push((socket, *command_id));
                }
            }
        }

        // Collect timed-out inquiries
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

        // Collect timed-out ACK commands
        let ack_timeout = self.timeout_config.ack_timeout;
        let mut timed_out_acks = Vec::new();
        for &cmd_id in &self.pending_ack_ids {
            if let Some(state) = self.commands.get(&cmd_id) {
                if let Some(sent_at) = state.sent_at {
                    let elapsed = now.duration_since(sent_at);
                    if elapsed > ack_timeout {
                        warn!(
                            "Command {} timed out waiting for ACK after {:?}",
                            cmd_id, ack_timeout
                        );
                        timed_out_acks.push(cmd_id);
                    }
                }
            }
        }

        // Handle all timeouts using the unified handler
        for (socket, cmd_id) in timed_out_sockets {
            self.handle_timeout(TimeoutSource::Socket(socket), cmd_id, now, &mut actions);
        }

        for cmd_id in timed_out_inquiries {
            self.handle_timeout(TimeoutSource::Inquiry, cmd_id, now, &mut actions);
        }

        for cmd_id in timed_out_acks {
            // Only process if command still exists (wasn't already handled by another timeout)
            if self.commands.contains_key(&cmd_id) {
                self.handle_timeout(TimeoutSource::Ack, cmd_id, now, &mut actions);
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
                    Self::update_earliest(&mut earliest, deadline);
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
                Self::update_earliest(&mut earliest, deadline);
            }
        }

        // Check inquiry timeouts
        for &cmd_id in &self.inflight_inquiry_ids {
            if let Some(state) = self.commands.get(&cmd_id) {
                if let Some(sent_at) = state.sent_at {
                    let timeout = self.timeout_config.get_timeout(state.category);
                    let deadline = sent_at + timeout;
                    Self::update_earliest(&mut earliest, deadline);
                }
            }
        }

        // Check retry queue (peek at the earliest retry)
        if let Some(retry_key) = self.retry_queue.peek() {
            Self::update_earliest(&mut earliest, retry_key.command.retry_at);
        }

        // Check inquiry spacing deadline (when queued inquiries can be sent)
        if !self.inquiry_queue.is_empty() && !self.min_inquiry_spacing.is_zero() {
            if let Some(last_sent) = self.last_inquiry_sent {
                let next_inquiry_eligible = last_sent + self.min_inquiry_spacing;
                Self::update_earliest(&mut earliest, next_inquiry_eligible);
            }
        }

        earliest
    }

    // Private helper methods

    /// Returns `true` if the command should be retried based on budget and duration.
    fn should_retry_timeout(&self, cmd_id: CommandId, now: Instant) -> bool {
        self.commands.get(&cmd_id).is_some_and(|state| {
            let max_retries = self.retry_budget.for_category(state.category);
            let within_duration =
                now.duration_since(state.submitted_at) < self.retry_config.max_retry_duration;
            state.attempt < max_retries && within_duration
        })
    }

    /// Build the appropriate error for a terminal timeout failure.
    fn timeout_terminal_error(&self, cmd_id: CommandId) -> Error {
        let transport_error = self
            .commands
            .get(&cmd_id)
            .is_some_and(|s| s.transport_error);
        if transport_error {
            Error::TransportError("Network error after max retries".into())
        } else {
            Error::Timeout
        }
    }

    /// Remove a command from all tracking structures.
    ///
    /// Note: Currently unused because each timeout source has slightly different
    /// cleanup patterns (e.g., ACK timeouts don't need inquiry cleanup). This helper
    /// is kept for potential future consolidation when all cleanup paths can be unified.
    #[allow(dead_code)]
    fn cleanup_command_state(&mut self, cmd_id: CommandId) {
        self.finish_sequence(cmd_id);
        self.pending_ack_ids.remove(&cmd_id);
        self.inflight_inquiry_ids.remove(&cmd_id);
        self.inquiries_order.retain(|&id| id != cmd_id);
        self.commands.remove(&cmd_id);
    }

    /// Update earliest deadline if candidate is earlier.
    #[inline]
    fn update_earliest(earliest: &mut Option<Instant>, candidate: Instant) {
        *earliest = match *earliest {
            None => Some(candidate),
            Some(e) if candidate < e => Some(candidate),
            _ => *earliest,
        };
    }

    /// Handle a timeout from any source with unified retry/fail logic.
    ///
    /// This method consolidates the three previously-duplicated timeout handling
    /// patterns (socket, inquiry, ACK) into a single dispatch point.
    fn handle_timeout(
        &mut self,
        source: TimeoutSource,
        cmd_id: CommandId,
        now: Instant,
        actions: &mut Vec<SchedulerAction>,
    ) {
        // Source-specific pre-cleanup
        match source {
            TimeoutSource::Socket(socket) => {
                self.free_socket(socket);
            }
            TimeoutSource::Inquiry => {
                self.inflight_inquiry_ids.remove(&cmd_id);
                self.inquiries_order.retain(|&id| id != cmd_id);
            }
            TimeoutSource::Ack => {
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

                // For ACK timeouts, emit a Timeout action to notify the adapter
                let attempts = self.commands.get(&cmd_id).map_or(0, |s| s.attempt);
                let will_retry = self.should_retry_timeout(cmd_id, now);
                actions.push(SchedulerAction::Timeout {
                    id: cmd_id,
                    kind: TimeoutKind::Ack,
                    attempt: attempts + 1, // 1-based attempt number
                    will_retry,
                });
            }
        }

        if self.should_retry_timeout(cmd_id, now) {
            // For ACK timeouts, we need manual retry queueing with capped exponent
            // For other timeouts, use queue_retry_for_command
            match source {
                TimeoutSource::Ack => {
                    let state_copy = self.commands.get(&cmd_id).cloned();
                    if let Some(state) = state_copy {
                        let attempts = state.attempt;
                        let max_retries = self.retry_budget.for_category(state.category);

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
                        // For ACK timeouts, we cap the exponent at 5 (2^5 = 32) to prevent
                        // excessively long delays on repeated timeouts.
                        let capped_attempt_num = (attempts + 1).min(6);
                        let retry_attempt =
                            RetryAttempt::new(capped_attempt_num).unwrap_or(RetryAttempt::FIRST);
                        let retry_delay = self.retry_config.calculate_delay(retry_attempt, None);

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
                    }
                }
                TimeoutSource::Socket(_) | TimeoutSource::Inquiry => {
                    if let Some(retry_action) = self.queue_retry_for_command(cmd_id, now) {
                        actions.push(retry_action);
                    }
                }
            }
        } else {
            // Terminal failure
            match source {
                TimeoutSource::Ack => {
                    let state_copy = self.commands.get(&cmd_id).cloned();
                    if let Some(state) = state_copy {
                        let max_retries = self.retry_budget.for_category(state.category);
                        let error = self.timeout_terminal_error(cmd_id);

                        debug!(
                            "Command {} exceeded max ACK retries, failing with {:?}",
                            cmd_id, error
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

                        actions.push(SchedulerAction::CommandFailed { id: cmd_id, error });
                    }
                }
                TimeoutSource::Socket(_) | TimeoutSource::Inquiry => {
                    let error = self.timeout_terminal_error(cmd_id);
                    self.finish_sequence(cmd_id);
                    self.commands.remove(&cmd_id);
                    actions.push(SchedulerAction::CommandFailed { id: cmd_id, error });
                }
            }
        }
    }

    /// Handle an ACK and assign a socket to the command.
    ///
    /// Returns `(assigned_cmd_id, optional_cancel_action)`:
    /// - `assigned_cmd_id`: The command ID that was assigned to a socket, if any.
    /// - `optional_cancel_action`: A `SendCancel` action if the command had
    ///   `cancel_requested` set before the socket was assigned.
    fn handle_ack_with_id(
        &mut self,
        socket: Option<ViscaSocket>,
        cmd_id: Option<CommandId>,
        now: Instant,
    ) -> (Option<CommandId>, Option<SchedulerAction>) {
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
        };

        let target_id = match target_id {
            Some(id) => id,
            None => return (None, None),
        };

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
                        return (None, None);
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
                    return (None, None);
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

            // Check if cancel was requested before socket was assigned
            let cancel_action = if cmd_state.cancel_requested {
                // Clear the flag and emit a cancel action
                if let Some(state) = self.commands.get_mut(&target_id) {
                    state.cancel_requested = false;
                }
                debug!(
                    %target_id,
                    ?assigned_socket,
                    camera_id = ?cmd_state.camera_id,
                    "Emitting SendCancel for command that had cancel_requested set"
                );
                Some(SchedulerAction::SendCancel {
                    camera_id: cmd_state.camera_id,
                    socket: assigned_socket,
                })
            } else {
                None
            };

            (Some(target_id), cancel_action)
        } else {
            warn!("Failed to get command state for {target_id}");
            (None, None)
        }
    }

    /// Start tracking an inquiry (no socket allocation).
    ///
    /// This method uses upsert semantics to preserve retry state across resends:
    /// - If the inquiry already exists in `commands`, update send-related fields
    ///   (`command`, `priority`, `category`, `camera_id`, `kind`, `sent_at`) while
    ///   preserving lifecycle tracking fields (`submitted_at`, `attempt`,
    ///   `transport_error`, `cancel_requested`).
    /// - If this is a new inquiry, create fresh state with `attempt = 0` and
    ///   `submitted_at = now`.
    ///
    /// This mirrors the `register_pending_ack` pattern for commands and ensures
    /// that retry budgets (`max_retries`, `max_retry_duration`) are honored.
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
        debug_assert!(
            kind == CommandKind::Inquiry,
            "start_inquiry called with non-inquiry kind: {:?}",
            kind
        );

        // Get response_type from pending_inquiry_types if not already in commands.
        // Only remove from pending_inquiry_types for new inquiries.
        let pending_response_type = self.pending_inquiry_types.remove(&id);

        // Update existing state if it exists (preserves retry tracking),
        // otherwise create a new state
        if let Some(existing) = self.commands.get_mut(&id) {
            // Update send-related fields, preserve retry tracking
            existing.command = command;
            existing.priority = priority;
            existing.category = category;
            existing.camera_id = camera_id;
            existing.kind = kind;
            existing.sent_at = Some(now);
            // Note: preserve attempt, transport_error, cancel_requested, and submitted_at
            // If response_type was None but we have a pending type, fill it in
            if existing.response_type.is_none() {
                existing.response_type = pending_response_type;
            }

            trace!(
                %id,
                inquiry_type = ?existing.response_type,
                attempt = existing.attempt,
                "Updated existing inquiry state for resend"
            );
        } else {
            // New inquiry - create fresh state
            let response_type = pending_response_type;
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
                cancel_requested: false,
                response_type,
            };
            self.commands.insert(id, state);

            trace!(
                %id,
                inquiry_type = ?response_type,
                "Created new inquiry state"
            );
        }

        self.inflight_inquiry_ids.insert(id);

        // Remove any existing occurrence to prevent duplicates, then add to back
        // This maintains FIFO ordering while ensuring at most one entry per id
        self.inquiries_order.retain(|&x| x != id);
        self.inquiries_order.push_back(id);

        // Update last inquiry sent time for spacing enforcement
        self.last_inquiry_sent = Some(now);

        // Debug assertion: inquiries_order should not contain duplicates
        debug_assert!(
            {
                let mut seen = HashSet::new();
                self.inquiries_order.iter().all(|&x| seen.insert(x))
            },
            "inquiries_order contains duplicates after start_inquiry"
        );

        let inflight_count = self.inflight_inquiry_ids.len();
        trace!(
            %id,
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
            // new_attempt is 1-based (state.attempt starts at 0, we added 1 above)
            let retry_attempt = RetryAttempt::new(new_attempt).unwrap_or(RetryAttempt::FIRST);
            let delay = self.retry_config.calculate_delay(retry_attempt, None);

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
mod tests;
