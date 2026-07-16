//! Runtime-agnostic scheduler core state machine.
//!
//! This module implements the protocol state machine for VISCA command scheduling,
//! socket allocation, ACK/completion routing, and retry logic without any dependency
//! on async runtimes or channels.

use smallvec::SmallVec;
use tracing::{debug, error, trace, warn};

use std::{
    cell::Cell,
    cmp::Ordering as CmpOrdering,
    collections::{BinaryHeap, HashMap, HashSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    camera::CommandId,
    command::{
        encode::{EncodedCommand, InquiryResponseSpec},
        response::{parse_inquiry_payload, Response},
        CommandKind,
    },
    timeout::{CommandCategory, TimeoutConfig},
    transport::RetryAttempt,
    visca_socket::ViscaSocket,
    Error,
};

/// Command lifecycle phase.
///
/// This enum encodes the lifecycle phase of a command in the scheduler:
/// - A command starts in `Queued` (waiting to be sent)
/// - After being sent, it transitions to `AwaitingAck` (sent, waiting for ACK)
/// - After ACK, commands with socket allocation transition to `Executing`
///
/// Each variant carries the timing information needed for timeout detection,
/// eliminating the need for separate `sent_at` and socket tracking structures.
#[derive(Debug, Clone, Copy)]
pub enum CommandPhase {
    /// Command queued but not yet sent.
    Queued,
    /// Sent to transport, awaiting ACK (commands) or immediate response (inquiries).
    AwaitingAck {
        /// When the command was sent.
        sent_at: Instant,
    },
    /// ACK received, socket allocated, awaiting completion.
    Executing {
        /// Socket assigned by the camera.
        socket: ViscaSocket,
        /// When the socket was assigned.
        started_at: Instant,
    },
}

impl CommandPhase {
    /// Returns `true` if this command is awaiting ACK.
    #[inline]
    pub fn is_awaiting_ack(&self) -> bool {
        matches!(self, CommandPhase::AwaitingAck { .. })
    }

    /// Returns `true` if this command is executing (has a socket assigned).
    #[inline]
    pub fn is_executing(&self) -> bool {
        matches!(self, CommandPhase::Executing { .. })
    }

    /// Returns the socket if the command is in `Executing` phase.
    #[inline]
    pub fn socket(&self) -> Option<ViscaSocket> {
        match self {
            CommandPhase::Executing { socket, .. } => Some(*socket),
            _ => None,
        }
    }
}

/// Inquiry lifecycle phase.
#[derive(Debug, Clone, Copy)]
pub enum InquiryPhase {
    /// Inquiry queued for retry but not currently sent.
    Queued,
    /// Inquiry sent, awaiting data reply.
    AwaitingReply {
        /// When the inquiry was sent.
        sent_at: Instant,
    },
}

impl InquiryPhase {
    /// Returns the time when the inquiry was sent, if applicable.
    #[cfg(test)]
    #[inline]
    pub fn sent_at(&self) -> Option<Instant> {
        match self {
            InquiryPhase::Queued => None,
            InquiryPhase::AwaitingReply { sent_at } => Some(*sent_at),
        }
    }

    /// Returns `true` if this inquiry is waiting for a reply.
    #[inline]
    pub fn is_awaiting_reply(&self) -> bool {
        matches!(self, InquiryPhase::AwaitingReply { .. })
    }
}

/// Complete lifecycle state for a command.
///
/// This entry deliberately contains only command-owned state: ACK/socket
/// lifecycle, retry tracking, and cancellation.
///
/// # Metadata Consolidation
///
/// The `command` field contains an `Arc<EncodedCommand>` which stores:
/// - `category`: Timeout category (via `command.category`)
/// - `kind`: Command vs Inquiry (via `command.behavior`)
///
/// These are accessed via helper methods, making `EncodedCommand` the single
/// source of truth and eliminating redundant storage.
///
/// # Fields
/// - Core identity: `command`, `priority`, `camera_id`
/// - Timing: `submitted_at`, phase (with embedded timing)
/// - Retry tracking: `attempt`, `transport_error`
/// - Cancel tracking: `cancel_requested`
#[derive(Debug, Clone)]
pub struct CommandEntry {
    /// Pre-encoded command bytes (contains category and kind).
    pub command: Arc<EncodedCommand>,
    /// Scheduling priority.
    pub priority: Priority,
    /// Target camera.
    pub camera_id: crate::camera_id::CameraId,
    /// When first submitted to the scheduler.
    pub submitted_at: Instant,

    // --- Lifecycle tracking ---
    /// Current lifecycle phase (includes timing for each phase).
    pub phase: CommandPhase,
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
}

impl CommandEntry {
    /// Get the timeout category for this command.
    #[inline]
    pub fn category(&self) -> CommandCategory {
        self.command.category
    }
}

/// Complete lifecycle state for an inquiry.
///
/// Inquiry entries carry only inquiry-owned state. They have no socket, ACK
/// phase, or cancellation flag.
#[derive(Debug, Clone)]
pub struct InquiryEntry {
    /// Pre-encoded inquiry bytes.
    pub command: Arc<EncodedCommand>,
    /// Scheduling priority.
    pub priority: Priority,
    /// Target camera.
    pub camera_id: crate::camera_id::CameraId,
    /// When first submitted to the scheduler.
    pub submitted_at: Instant,
    /// Current lifecycle phase.
    pub phase: InquiryPhase,
    /// Current retry attempt (0 = first try).
    pub attempt: u32,
    /// Whether last failure was a transport error.
    pub transport_error: bool,
    /// Required response routing metadata used for inquiry reply correlation.
    pub response_spec: InquiryResponseSpec,
}

impl InquiryEntry {
    /// Get the timeout category for this inquiry.
    #[inline]
    pub fn category(&self) -> CommandCategory {
        self.command.category
    }

    /// Get sent_at time from phase.
    #[cfg(test)]
    #[inline]
    pub fn sent_at(&self) -> Option<Instant> {
        self.phase.sent_at()
    }
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
    #[cfg(any(all(feature = "mode-async", feature = "test-utils"), test))]
    High = 2,
    /// Critical priority - emergency/safety operations.
    #[cfg(any(all(feature = "mode-async", feature = "test-utils"), test))]
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
///
/// # Metadata Consolidation
///
/// The `command` field contains an `Arc<EncodedCommand>` which stores
/// `category` and behavior. These are accessed via helper methods rather
/// than redundant fields.
#[derive(Clone)]
pub struct RetryCommand {
    /// Command ID (type-safe, non-zero).
    pub id: CommandId,
    /// The pre-encoded command to retry (contains category and behavior).
    pub command: Arc<EncodedCommand>,
    /// Command priority.
    pub priority: Priority,
    /// Camera ID used to encode the command.
    pub camera_id: crate::camera_id::CameraId,
    /// Retry attempt number.
    pub attempt: u32,
    /// Maximum retries allowed.
    pub max_retries: u32,
    /// When to retry this command (for exponential backoff).
    pub retry_at: Instant,
}

impl RetryCommand {
    /// Get the command kind (Command or Inquiry).
    #[cfg(test)]
    #[inline]
    pub fn kind(&self) -> CommandKind {
        self.command.kind()
    }
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
            .field("category", &self.command.category)
            .field("camera_id", &self.camera_id)
            .field("kind", &self.command.kind())
            .field("attempt", &self.attempt)
            .field("max_retries", &self.max_retries)
            .field("retry_at", &self.retry_at)
            .finish()
    }
}

struct RetryState {
    command: Arc<EncodedCommand>,
    priority: Priority,
    camera_id: crate::camera_id::CameraId,
    submitted_at: Instant,
    attempt: u32,
    transport_error: bool,
    category: CommandCategory,
}

/// Outcome of the scheduler's contextual retry classification for a VISCA
/// protocol error attributed to an active command or inquiry.
enum RetryDecision {
    /// Queue a bounded retry through the existing retry/backoff machinery.
    ///
    /// `inquiry_syntax` is `true` only for the narrow transient inquiry-side
    /// `0x02` case, which additionally arms the inquiry-class cooldown.
    Retry {
        /// Whether this is a transient inquiry-side syntax error.
        inquiry_syntax: bool,
    },
    /// Fail terminally with this error; no retry is queued.
    Fail(Error),
}

/// Priority queue item wrapper for commands.
///
/// # Metadata Consolidation
///
/// The `command` field contains an `Arc<EncodedCommand>` which stores all
/// command metadata including:
/// - `category`: Timeout category (via `command.category`)
/// - `kind`: Command vs Inquiry (via `command.behavior`)
/// - inquiry response routing metadata (via `command.behavior`)
///
/// This makes `EncodedCommand` the single source of truth for command metadata,
/// eliminating redundant storage and potential for divergence.
#[derive(Clone)]
pub struct PendingCommand {
    /// Unique identifier for this command (type-safe, non-zero).
    pub id: CommandId,
    /// The pre-encoded command to send (contains category and behavior).
    pub command: Arc<EncodedCommand>,
    /// Priority level for scheduling.
    pub priority: Priority,
    /// Camera ID used to encode the command.
    pub camera_id: crate::camera_id::CameraId,
    /// When the command was submitted.
    pub submitted_at: Instant,
}

impl PendingCommand {
    /// Get the command kind (Command or Inquiry).
    #[inline]
    pub fn kind(&self) -> CommandKind {
        self.command.kind()
    }
}

impl std::fmt::Debug for PendingCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingCommand")
            .field("id", &self.id)
            .field("priority", &self.priority)
            .field("category", &self.command.category)
            .field("camera_id", &self.camera_id)
            .field("submitted_at", &self.submitted_at)
            .field("kind", &self.command.kind())
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
    /// Command completed successfully.
    CommandComplete {
        /// Command ID (type-safe).
        id: CommandId,
        /// Command category.
        #[cfg(feature = "mode-async")]
        category: CommandCategory,
        /// Camera ID.
        #[cfg(feature = "mode-async")]
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
        #[cfg(any(feature = "mode-async", test))]
        id: CommandId,
        /// Delay before retrying.
        #[cfg(any(feature = "mode-async", test))]
        delay: Duration,
    },
    /// A command exceeded one of the scheduler's timeouts.
    Timeout {
        /// Command ID that timed out (type-safe).
        #[cfg(any(feature = "mode-async", test))]
        id: CommandId,
        /// Kind of timeout that occurred.
        #[cfg(any(feature = "mode-async", test))]
        kind: TimeoutKind,
        /// True if a retry was enqueued.
        #[cfg(any(feature = "mode-async", test))]
        will_retry: bool,
    },
    /// Send a cancel command for a specific socket.
    ///
    /// This action is emitted when a cancel was requested for a command before
    /// a socket was assigned, and an ACK has now assigned the socket. The cancel
    /// should be sent immediately to the transport.
    SendCancel {
        /// Camera ID for addressing the cancel message.
        #[cfg(any(feature = "mode-async", test))]
        camera_id: crate::camera_id::CameraId,
        /// Socket to cancel.
        #[cfg(any(feature = "mode-async", test))]
        socket: ViscaSocket,
    },
}

/// Result of a cancellation request after scheduler processing.
#[cfg(any(feature = "mode-async", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CancelOutcome {
    /// A queued command was removed before any VISCA bytes were sent.
    QueuedRemoved,
    /// A command awaiting ACK was marked for cancel-on-ACK.
    MarkedCancelOnAck,
    /// A socket cancel should be sent immediately.
    SendCancel {
        /// Camera ID from the scheduler-owned command state.
        camera_id: crate::camera_id::CameraId,
        /// Socket assigned to the executing command.
        socket: ViscaSocket,
    },
    /// The command was already completed, failed, or unknown.
    NoOp,
}

/// Kind of timeout that can occur.
#[cfg(any(feature = "mode-async", test))]
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
///
/// # Command Lifecycle State
///
/// Command lifecycle is tracked through `CommandEntry`:
/// - `Queued`: Command waiting to be sent
/// - `AwaitingAck { sent_at }`: Sent, waiting for ACK
/// - `Executing { socket, started_at }`: ACK received, socket allocated
///
/// Inquiry lifecycle is tracked separately through `InquiryEntry`, so inquiries
/// cannot carry command-only state like socket ownership or pending cancellation.
#[derive(Debug)]
pub struct SchedulerCore {
    /// Timeout configuration.
    timeout_config: TimeoutConfig,
    /// Retry configuration.
    retry_config: crate::transport::RetryConfig,
    /// Tracks whether we've logged the idle state (zero commands in flight).
    last_logged_idle: Cell<bool>,

    // === Command and inquiry state ===
    /// Active commands indexed by ID.
    commands: HashMap<CommandId, CommandEntry>,
    /// Active inquiries indexed by ID.
    inquiries: HashMap<CommandId, InquiryEntry>,

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
    /// Minimum time spacing between consecutive command sends (any kind).
    min_command_spacing: Duration,
    /// When the last command (of any kind) was sent (for spacing enforcement).
    last_command_sent: Option<Instant>,
    /// Deadline until which inquiry sends are held back after a transient
    /// inquiry-side syntax error, letting an overloaded camera recover before
    /// the failed inquiry is resent or another queued inquiry is dispatched.
    inquiry_cooldown_until: Option<Instant>,
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
    #[cfg(test)]
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
            timeout_config,
            retry_config,
            last_logged_idle: Cell::new(false),
            commands: HashMap::new(),
            inquiries: HashMap::new(),
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
            min_command_spacing: Duration::ZERO,
            last_command_sent: None,
            inquiry_cooldown_until: None,
            ignored_unmatched_sequenced_replies: 0,
        }
    }

    /// Set the timeout configuration.
    #[cfg(not(feature = "mode-async"))]
    pub fn set_timeout_config(&mut self, timeout_config: TimeoutConfig) {
        self.timeout_config = timeout_config;
    }

    /// Set the maximum number of inquiries that can be in flight simultaneously.
    #[cfg(any(feature = "mode-async", test))]
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

    /// Set the minimum spacing between consecutive command sends (any kind).
    ///
    /// When set, this enforces a minimum delay between any two sends on the
    /// transport. This prevents firmware buffer overflow on cameras that
    /// cannot process commands at wire speed.
    pub fn set_min_command_spacing(&mut self, spacing: Duration) {
        self.min_command_spacing = spacing;
    }

    /// Queue a command for execution.
    pub fn queue_command(&mut self, command: PendingCommand) {
        let id = command.id;
        let now = command.submitted_at;

        // Route based on command kind (derived from EncodedCommand)
        match command.command.behavior {
            crate::command::CommandBehavior::Inquiry(response_spec) => {
                self.commands.remove(&id);
                self.inquiries.insert(
                    id,
                    InquiryEntry {
                        command: command.command.clone(),
                        priority: command.priority,
                        camera_id: command.camera_id,
                        submitted_at: now,
                        phase: InquiryPhase::Queued,
                        attempt: 0,
                        transport_error: false,
                        response_spec,
                    },
                );
                self.inquiry_queue.push(command);
            }
            crate::command::CommandBehavior::Command => {
                self.inquiries.remove(&id);
                self.commands.insert(
                    id,
                    CommandEntry {
                        command: command.command.clone(),
                        priority: command.priority,
                        camera_id: command.camera_id,
                        submitted_at: now,
                        phase: CommandPhase::Queued,
                        attempt: 0,
                        transport_error: false,
                        cancel_requested: false,
                    },
                );
                self.command_queue.push(command);
            }
        }
    }

    /// Check if we can send another command (have room for pending ACK and spacing satisfied).
    ///
    /// Uses phase-based counting to determine capacity:
    /// - Commands in `AwaitingAck` phase count toward the 2-slot limit
    /// - Commands in `Executing` phase count toward the 2-slot limit
    /// - Total must be < 2 to allow sending another command
    /// - The minimum command spacing since the last send must be satisfied
    pub fn can_send_command(&self, now: Instant) -> bool {
        // Check command spacing requirement (applies to all send types)
        if !self.check_command_spacing(now) {
            return false;
        }

        // Count command entries by phase.
        let awaiting_ack = self.count_awaiting_ack();
        let executing = self.count_executing();
        let total_in_flight = awaiting_ack + executing;

        let can_send = total_in_flight < 2;

        if !can_send && std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
            eprintln!(
                "[SchedulerCore] can_send_command=false: {} awaiting ACK + {} executing = {}/2 capacity",
                awaiting_ack, executing, total_in_flight
            );
        }

        // Log state transitions: always log once when going to idle, otherwise only when busy
        if total_in_flight > 0 {
            trace!(
                "Commands in flight: {} awaiting ACK + {} executing = {}/2",
                awaiting_ack,
                executing,
                total_in_flight
            );
            self.last_logged_idle.set(false);
        } else if !self.last_logged_idle.get() {
            trace!("Commands in flight: 0 awaiting ACK + 0 executing = 0/2");
            self.last_logged_idle.set(true);
        }

        can_send
    }

    /// Check if we can send an inquiry (not at max capacity and spacing satisfied).
    ///
    /// Returns true if:
    /// 1. The minimum command spacing since the last send has been satisfied
    /// 2. The number of in-flight inquiries is below the maximum limit
    /// 3. The minimum inquiry spacing requirement since the last inquiry has been satisfied
    ///
    /// Uses phase-based counting to determine in-flight inquiries.
    pub fn can_send_inquiry(&self, now: Instant) -> bool {
        // Check command spacing requirement (applies to all send types)
        if !self.check_command_spacing(now) {
            return false;
        }

        // Honor the inquiry-class cooldown armed after a transient inquiry
        // syntax error. This backs off the whole inquiry class (the failed
        // inquiry and any other queued inquiries) without pausing commands.
        if let Some(until) = self.inquiry_cooldown_until {
            if now < until {
                return false;
            }
        }

        // Count inquiry entries by phase.
        let inflight_count = self.count_awaiting_inquiry_reply();

        // Check concurrency limit
        if inflight_count >= self.max_inquiries_inflight {
            return false;
        }

        // Check inquiry-specific spacing requirement
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
    /// When priorities are equal, inquiries are preferred because they are
    /// typically quick status reads and do not consume command sockets.
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
        let command_available = self.can_send_command(now) && !self.command_queue.is_empty();

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

                // If command has strictly higher priority, prefer it.
                // Otherwise, prefer the inquiry because it does not consume a command socket.
                if command_priority > inquiry_priority {
                    self.command_queue.pop()
                } else {
                    self.inquiry_queue.pop()
                }
            }
        }
    }

    /// Register a command as pending ACK.
    ///
    /// # Arguments
    /// - `id`: Command ID (type-safe, non-zero)
    /// - `command`: Pre-encoded command (contains category and behavior)
    /// - `priority`: Scheduling priority
    /// - `camera_id`: Target camera
    /// - `now`: Current timestamp
    ///
    /// Category and kind are derived from the `EncodedCommand`, ensuring
    /// consistency and eliminating the possibility of mismatched metadata.
    pub fn register_pending_ack(
        &mut self,
        id: CommandId,
        command: Arc<EncodedCommand>,
        priority: Priority,
        camera_id: crate::camera_id::CameraId,
        now: Instant,
    ) {
        if command.kind() != CommandKind::Command {
            warn!(
                %id,
                kind = ?command.kind(),
                "Ignoring non-command passed to register_pending_ack"
            );
            return;
        }

        self.inquiries.remove(&id);

        // Update existing command state if it exists (preserves retry state),
        // otherwise create a new state
        if let Some(existing) = self.commands.get_mut(&id) {
            // Update send-related fields, preserve retry tracking
            existing.command = command;
            existing.priority = priority;
            existing.camera_id = camera_id;
            existing.phase = CommandPhase::AwaitingAck { sent_at: now };
            // Note: preserve attempt, transport_error, cancel_requested, and submitted_at
            // Category and kind come from command, so no update needed
        } else {
            // New command - create fresh state
            let state = CommandEntry {
                command,
                priority,
                camera_id,
                submitted_at: now,
                phase: CommandPhase::AwaitingAck { sent_at: now },
                attempt: 0,
                transport_error: false,
                cancel_requested: false,
            };
            self.commands.insert(id, state);
        }
        // Phase is tracked on the command entry; no separate pending-ACK index is needed.

        // Update last command sent time for spacing enforcement
        self.last_command_sent = Some(now);

        trace!(%id, "Registered command as pending ACK (phase=AwaitingAck)");
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
            if self.is_command_active(cmd_id) {
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
                .filter(|&cmd_id| self.is_command_active(cmd_id))
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
    /// A command is considered active if it exists in command or inquiry state.
    /// This matches the staleness checks used in `get_command_by_sequence`.
    ///
    /// This is used to filter out stale retries in `get_ready_retries`.
    fn is_command_active(&self, cmd_id: CommandId) -> bool {
        self.commands.contains_key(&cmd_id) || self.inquiries.contains_key(&cmd_id)
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
    #[cfg(any(not(feature = "mode-async"), test))]
    pub fn cancel_command(&mut self, cmd_id: CommandId) {
        trace!("Cancelling command {cmd_id}");

        // Clean up sequence mappings
        self.finish_sequence(cmd_id);

        // Remove from inquiry FIFO order (for raw VISCA correlation)
        self.inquiries_order.retain(|&id| id != cmd_id);

        // Remove active state.
        self.commands.remove(&cmd_id);
        self.inquiries.remove(&cmd_id);

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

    /// Remove a command from pending command/inquiry queues.
    #[cfg(any(feature = "mode-async", test))]
    fn remove_queued_command(&mut self, cmd_id: CommandId) {
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
    }

    /// Request cancellation of a command by ID.
    ///
    /// This method implements lifecycle-aware cancel-by-id semantics:
    ///
    /// - **Command not active**: Returns `CancelOutcome::NoOp`.
    /// - **Queued**: Removes the queued command and returns `CancelOutcome::QueuedRemoved`.
    /// - **Socket already assigned (Executing phase)**: Returns `CancelOutcome::SendCancel`
    ///   so the caller can send the cancel command immediately.
    /// - **Awaiting ACK (no socket yet)**: Sets `cancel_requested = true` on the command
    ///   state and returns `CancelOutcome::MarkedCancelOnAck`. The cancel will be emitted as
    ///   a `SchedulerAction::SendCancel` when the ACK arrives and assigns a socket.
    ///
    /// This design eliminates the need for an out-of-band `pending_cancel_ids` map,
    /// ensuring cancels are bounded to command lifetime and cleaned up automatically.
    #[cfg(any(feature = "mode-async", test))]
    pub fn request_cancel_by_id(&mut self, cmd_id: CommandId) -> CancelOutcome {
        // Check if the command is active and get its state
        let Some(state) = self.commands.get(&cmd_id) else {
            return CancelOutcome::NoOp;
        };

        // Copy the state needed for the decision so queued removal can mutate
        // the scheduler without holding the entry borrow.
        let camera_id = state.camera_id;
        let phase = state.phase;

        match phase {
            CommandPhase::Queued => {
                debug!(
                    %cmd_id,
                    ?camera_id,
                    "Cancel requested for queued command - removing before send"
                );
                self.finish_sequence(cmd_id);
                self.commands.remove(&cmd_id);
                self.remove_queued_command(cmd_id);
                CancelOutcome::QueuedRemoved
            }
            CommandPhase::Executing { socket, .. } => {
                debug!(
                    %cmd_id,
                    ?socket,
                    ?camera_id,
                    "Cancel requested for command with socket - returning immediately"
                );
                CancelOutcome::SendCancel { camera_id, socket }
            }
            CommandPhase::AwaitingAck { .. } => {
                if let Some(state) = self.commands.get_mut(&cmd_id) {
                    state.cancel_requested = true;
                }
                debug!(
                    %cmd_id,
                    ?camera_id,
                    "Cancel requested for command awaiting ACK - flagged for cancel on socket assignment"
                );
                CancelOutcome::MarkedCancelOnAck
            }
        }
    }

    /// Get the response routing spec for an inquiry from the command state.
    ///
    /// This returns the response spec stored in the `InquiryEntry`.
    /// Returns `None` if the command doesn't exist or is not an active inquiry.
    pub fn get_inquiry_response_spec(&self, id: CommandId) -> Option<InquiryResponseSpec> {
        self.inquiries.get(&id).map(|state| state.response_spec)
    }

    /// Resolve a raw VISCA inquiry reply using content matching and FIFO fallback.
    ///
    /// This method must only be called for replies without Sony sequence metadata.
    pub(crate) fn resolve_raw_inquiry_id(
        &self,
        payload: crate::command::response::Payload<'_>,
    ) -> Option<CommandId> {
        use tracing::{debug, trace};

        let mut active_count = 0usize;
        let mut match_count = 0usize;
        let mut matched = None;

        for (&id, state) in self
            .inquiries
            .iter()
            .filter(|(_, state)| state.phase.is_awaiting_reply())
        {
            active_count += 1;
            match state.response_spec {
                InquiryResponseSpec::Builtin(kind) => {
                    match parse_inquiry_payload(payload.as_slice(), &kind) {
                        Ok(_) => {
                            trace!(cmd_id = %id, response_spec = ?state.response_spec, "Matched");
                            match_count += 1;
                            matched.get_or_insert(id);
                        }
                        Err(_e) => {
                            trace!(cmd_id = %id, response_spec = ?state.response_spec, "No match");
                        }
                    }
                }
                InquiryResponseSpec::Raw => {
                    trace!(
                        cmd_id = %id,
                        response_spec = ?state.response_spec,
                        "Skipping content match for raw inquiry"
                    );
                }
            }
        }

        match match_count {
            0 => {
                // No match - fall back to FIFO as last resort
                let fifo_front = self.inquiries_order.front().copied();

                // Only log at DEBUG when FIFO fallback actually happens (indicates potential issue)
                if fifo_front.is_some() {
                    debug!(
                        cmd_id = ?fifo_front,
                        active_count,
                        "Content matching failed, using FIFO fallback"
                    );
                } else if active_count > 0 {
                    // This is unexpected - we have active inquiries but none matched and FIFO is empty
                    debug!(
                        active_count,
                        "No inquiry match and FIFO empty - inquiry may be orphaned"
                    );
                }
                fifo_front
            }
            1 => {
                // Unique match found - this is the expected path, only log at TRACE
                trace!(cmd_id = ?matched, "Content match successful");
                matched
            }
            _ => {
                // Ambiguous - multiple inquiries match the same payload type
                // This is unusual and worth logging at DEBUG
                let fifo_front = self.inquiries_order.front().copied();
                debug!(
                    fifo_fallback = ?fifo_front,
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
                    // Use finalize_command for unified cleanup
                    if let Some((category, camera_id)) =
                        self.finalize_command(cmd_id, source.socket())
                    {
                        actions.push(SchedulerAction::CommandComplete {
                            id: cmd_id,
                            #[cfg(feature = "mode-async")]
                            category,
                            #[cfg(feature = "mode-async")]
                            camera_id,
                            response,
                        });
                        #[cfg(not(feature = "mode-async"))]
                        let _ = (category, camera_id);
                    }
                }
            }
            SchedulerEvent::InquiryReply { source, response } => {
                // Resolve inquiry ID using ReplySource
                let resolved_cmd_id = self.resolve_command_for_inquiry(&source);

                if let Some(cmd_id) = resolved_cmd_id {
                    // Use finalize_command for unified cleanup (inquiries have no socket)
                    if let Some((category, camera_id)) = self.finalize_command(cmd_id, None) {
                        actions.push(SchedulerAction::CommandComplete {
                            id: cmd_id,
                            #[cfg(feature = "mode-async")]
                            category,
                            #[cfg(feature = "mode-async")]
                            camera_id,
                            response,
                        });
                        #[cfg(not(feature = "mode-async"))]
                        let _ = (category, camera_id);
                        trace!("Inquiry {cmd_id} completed with response");
                    }
                }
            }
            SchedulerEvent::Error { source, code } => {
                // Resolve which command this error belongs to using ReplySource
                let resolved_cmd_id = self.resolve_command_for_error(&source, code);

                if let Some(cmd_id) = resolved_cmd_id {
                    // An attributed inquiry error leaves the FIFO attribution
                    // order; a queued retry re-adds it exactly once on resend.
                    if self.is_awaiting_inquiry_reply(cmd_id) {
                        self.inquiries_order.retain(|&id| id != cmd_id);
                    }

                    match self.classify_error_retry(cmd_id, code, now) {
                        RetryDecision::Retry { inquiry_syntax } => {
                            if let Some(retry_action) =
                                self.queue_retry_for_command(cmd_id, now, None)
                            {
                                if inquiry_syntax {
                                    // Bounded transient inquiry-side syntax error:
                                    // arm the inquiry-class cooldown for the backoff
                                    // window and log at warning level.
                                    let attempt = self.active_attempt(cmd_id).unwrap_or(0);
                                    let delay = self.retry_delay_for(attempt, None);
                                    self.arm_inquiry_cooldown(now + delay);
                                    warn!(
                                        %cmd_id,
                                        response_spec = ?self.get_inquiry_response_spec(cmd_id),
                                        attempt,
                                        ?delay,
                                        "Transient inquiry syntax error (0x02); scheduler will retry with backoff"
                                    );
                                }
                                actions.push(retry_action);
                            }
                        }
                        RetryDecision::Fail(error) => {
                            // Command-side and exhausted-inquiry syntax errors are
                            // terminal; log 0x02 at error level to distinguish an
                            // unsupported command from a transient inquiry response.
                            if code == 0x02 {
                                error!(%cmd_id, code = "0x02", "Terminal syntax error: {error}");
                            }
                            self.finish_sequence(cmd_id);
                            self.commands.remove(&cmd_id);
                            self.inquiries.remove(&cmd_id);
                            actions.push(SchedulerAction::CommandFailed { id: cmd_id, error });
                        }
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
                // Network error - retry all commands in AwaitingAck phase
                let pending_cmds: Vec<_> = self.commands_awaiting_ack().collect();
                // Check if this is a transport error
                let is_transport_error = matches!(error, Error::TransportError(_));
                for cmd_id in pending_cmds {
                    // Store whether this retry was triggered by a transport error
                    if let Some(state) = self.commands.get_mut(&cmd_id) {
                        state.transport_error = is_transport_error;
                    }
                    if let Some(retry_action) = self.queue_retry_for_command(cmd_id, now, None) {
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
    ///
    /// Searches commands in AwaitingAck phase and returns the one with the most recent sent_at.
    fn find_most_recent_pending_command(&self) -> Option<CommandId> {
        self.commands
            .iter()
            .filter_map(|(&cmd_id, state)| {
                if let CommandPhase::AwaitingAck { sent_at } = state.phase {
                    Some((cmd_id, sent_at))
                } else {
                    None
                }
            })
            .max_by_key(|(_, sent_time)| *sent_time)
            .map(|(id, _)| id)
    }

    /// Check for timeouts and return commands that need action.
    ///
    /// Uses single-pass iteration over active commands and inquiries:
    /// - `AwaitingAck`: Check against `ack_timeout`
    /// - `Executing`: Check against category-specific timeout
    /// - `AwaitingReply`: Check against category-specific timeout
    /// - `Queued`: Not sent yet, no timeout check needed
    pub fn check_timeouts(&mut self, now: Instant) -> Vec<SchedulerAction> {
        let mut actions = Vec::new();
        let ack_timeout = self.timeout_config.ack_timeout;

        let mut timed_out: Vec<(CommandId, TimeoutSource)> = self
            .commands
            .iter()
            .filter_map(|(&cmd_id, state)| {
                let (sent_at, timeout_source, timeout) = match state.phase {
                    CommandPhase::Queued => return None,
                    CommandPhase::AwaitingAck { sent_at } => {
                        (sent_at, TimeoutSource::Ack, ack_timeout)
                    }
                    CommandPhase::Executing { socket, started_at } => {
                        let timeout = self.timeout_config.get_timeout(state.category());
                        (started_at, TimeoutSource::Socket(socket), timeout)
                    }
                };

                let elapsed = now.duration_since(sent_at);
                if elapsed > timeout {
                    // Log the timeout based on source
                    match &timeout_source {
                        TimeoutSource::Ack => {
                            warn!(
                                "Command {} timed out waiting for ACK after {:?}",
                                cmd_id, ack_timeout
                            );
                        }
                        TimeoutSource::Socket(socket) => {
                            warn!(
                                "Command {} on socket {:?} timed out after {:?}",
                                cmd_id, socket, timeout
                            );
                            if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                                eprintln!(
                                    "[SchedulerCore] Socket timeout: cmd_id={}, socket={:?}, category={:?}, duration={:?}",
                                    cmd_id, socket, state.category(), elapsed
                                );
                            }
                        }
                        TimeoutSource::Inquiry => unreachable!("inquiries are checked separately"),
                    }
                    Some((cmd_id, timeout_source))
                } else {
                    None
                }
            })
            .collect();

        timed_out.extend(self.inquiries.iter().filter_map(|(&cmd_id, state)| {
            let sent_at = match state.phase {
                InquiryPhase::Queued => return None,
                InquiryPhase::AwaitingReply { sent_at } => sent_at,
            };
            let timeout = self.timeout_config.get_timeout(state.category());
            let elapsed = now.duration_since(sent_at);
            if elapsed > timeout {
                warn!(
                    %cmd_id,
                    response_spec = ?state.response_spec,
                    timeout = ?timeout,
                    elapsed = ?elapsed,
                    "Inquiry timed out"
                );
                Some((cmd_id, TimeoutSource::Inquiry))
            } else {
                None
            }
        }));

        // Handle all timeouts using the unified handler
        for (cmd_id, timeout_source) in timed_out {
            // Only process if command still exists (wasn't already handled)
            if self.is_command_active(cmd_id) {
                self.handle_timeout(timeout_source, cmd_id, now, &mut actions);
            }
        }

        actions
    }

    /// Get the number of commands waiting to be retried.
    #[cfg(any(all(feature = "mode-async", feature = "test-utils"), test))]
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
    #[cfg(feature = "mode-async")]
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
                    let current_attempt = self.active_attempt(cmd_id).unwrap_or(0);
                    if retry_attempt != current_attempt {
                        trace!(
                            %cmd_id,
                            queued_attempt = retry_attempt,
                            current_attempt,
                            "Dropping stale retry: attempt count mismatch"
                        );
                        continue;
                    }

                    // Phase guard: only dispatch retries for commands in Queued phase.
                    // This prevents duplicate sends if a late response (e.g., late ACK)
                    // transitioned the command to a different phase between timeout and
                    // retry dispatch.
                    if !self.active_is_queued(cmd_id) {
                        trace!(
                            %cmd_id,
                            phase = self.active_phase_debug(cmd_id),
                            "Dropping retry: command not in Queued phase"
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

    /// Move retries whose backoff has elapsed back onto the normal send queues
    /// so they are dispatched through [`next_item_to_send`](Self::next_item_to_send),
    /// subjecting them to the same profile pacing and capacity gates as fresh
    /// sends (command/inquiry spacing, socket and in-flight limits, and the
    /// inquiry cooldown). This prevents ready retries from bypassing pacing.
    ///
    /// Returns the number of retries promoted. Stale retries (command completed,
    /// superseded attempt, or no longer in `Queued` phase) are dropped by
    /// [`get_ready_retries`](Self::get_ready_retries) and never promoted. The
    /// promoted entry keeps its incremented `attempt` and original `submitted_at`;
    /// resend re-adds it to the inquiry FIFO order exactly once.
    pub fn promote_ready_retries(&mut self, now: Instant) -> usize {
        let ready = self.get_ready_retries(now);
        let promoted = ready.len();
        for retry in ready {
            let submitted_at = self.active_submitted_at(retry.id).unwrap_or(retry.retry_at);
            let pending = PendingCommand {
                id: retry.id,
                command: retry.command,
                priority: retry.priority,
                camera_id: retry.camera_id,
                submitted_at,
            };
            match pending.command.behavior {
                crate::command::CommandBehavior::Inquiry(_) => self.inquiry_queue.push(pending),
                crate::command::CommandBehavior::Command => self.command_queue.push(pending),
            }
        }
        promoted
    }

    /// Original submission time of an active command or inquiry, if tracked.
    fn active_submitted_at(&self, cmd_id: CommandId) -> Option<Instant> {
        self.commands
            .get(&cmd_id)
            .map(|state| state.submitted_at)
            .or_else(|| self.inquiries.get(&cmd_id).map(|state| state.submitted_at))
    }

    /// Get the next deadline for time-based operations.
    ///
    /// Uses active command and inquiry state to find the earliest deadline among:
    /// - ACK timeouts (AwaitingAck phase)
    /// - Socket command timeouts (Executing phase)
    /// - Inquiry timeouts (AwaitingReply phase)
    /// - Retry eligibility
    /// - Inquiry spacing
    pub fn next_deadline(&self, _now: Instant) -> Option<Instant> {
        let mut earliest: Option<Instant> = None;
        let ack_timeout = self.timeout_config.ack_timeout;

        for state in self.commands.values() {
            let deadline = match state.phase {
                CommandPhase::Queued => continue,
                CommandPhase::AwaitingAck { sent_at } => sent_at + ack_timeout,
                CommandPhase::Executing { started_at, .. } => {
                    let timeout = self.timeout_config.get_timeout(state.category());
                    started_at + timeout
                }
            };
            Self::update_earliest(&mut earliest, deadline);
        }

        for state in self.inquiries.values() {
            let deadline = match state.phase {
                InquiryPhase::Queued => continue,
                InquiryPhase::AwaitingReply { sent_at } => {
                    let timeout = self.timeout_config.get_timeout(state.category());
                    sent_at + timeout
                }
            };
            Self::update_earliest(&mut earliest, deadline);
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

        // Inquiry cooldown deadline (when the inquiry class is released after a
        // transient syntax error). Only relevant while inquiries are waiting.
        if !self.inquiry_queue.is_empty() {
            if let Some(until) = self.inquiry_cooldown_until {
                Self::update_earliest(&mut earliest, until);
            }
        }

        // Check command spacing deadline (when any queued item can be sent)
        let has_queued = !self.command_queue.is_empty() || !self.inquiry_queue.is_empty();
        if has_queued && !self.min_command_spacing.is_zero() {
            if let Some(last_sent) = self.last_command_sent {
                let next_eligible = last_sent + self.min_command_spacing;
                Self::update_earliest(&mut earliest, next_eligible);
            }
        }

        earliest
    }

    // Private helper methods

    /// Check if the command spacing requirement is satisfied.
    ///
    /// Returns `true` if enough time has elapsed since the last send,
    /// or if command spacing is disabled (zero interval).
    fn check_command_spacing(&self, now: Instant) -> bool {
        if self.min_command_spacing.is_zero() {
            return true;
        }
        match self.last_command_sent {
            Some(last_sent) => now.duration_since(last_sent) >= self.min_command_spacing,
            None => true,
        }
    }

    /// Returns `true` if the command should be retried based on budget and duration.
    fn should_retry_timeout(&self, cmd_id: CommandId, now: Instant) -> bool {
        if let Some(state) = self.commands.get(&cmd_id) {
            let max_retries = self.retry_budget.for_category(state.category());
            let within_duration =
                now.duration_since(state.submitted_at) < self.retry_config.max_retry_duration;
            state.attempt < max_retries && within_duration
        } else if let Some(state) = self.inquiries.get(&cmd_id) {
            let max_retries = self.retry_budget.for_category(state.category());
            let within_duration =
                now.duration_since(state.submitted_at) < self.retry_config.max_retry_duration;
            state.attempt < max_retries && within_duration
        } else {
            false
        }
    }

    /// Complete a command successfully, extracting metadata and cleaning up all state.
    ///
    /// This consolidates the cleanup pattern used in Completion and InquiryReply handlers.
    /// Returns `Some((category, camera_id))` if the command existed, `None` otherwise.
    ///
    /// Cleanup steps:
    /// 1. Extract metadata from the active entry
    /// 2. Clean up sequence mappings
    /// 3. Remove from inquiry FIFO order
    /// 4. Remove from the active command or inquiry map
    ///
    /// Note: Socket cleanup is not done here - socket state is embedded in CommandPhase::Executing
    /// and is automatically cleaned up when the command is removed from the HashMap.
    fn finalize_command(
        &mut self,
        cmd_id: CommandId,
        _source_socket: Option<ViscaSocket>,
    ) -> Option<(CommandCategory, crate::camera_id::CameraId)> {
        // Extract metadata before cleanup
        let (category, camera_id) = if let Some(state) = self.commands.get(&cmd_id) {
            (state.category(), state.camera_id)
        } else {
            let state = self.inquiries.get(&cmd_id)?;
            (state.category(), state.camera_id)
        };

        // Clean up sequence mappings
        self.finish_sequence(cmd_id);

        // Remove from inquiry FIFO order (for raw VISCA correlation)
        self.inquiries_order.retain(|&id| id != cmd_id);

        // Remove active state.
        // This implicitly frees the socket since socket ownership is in CommandPhase::Executing
        self.commands.remove(&cmd_id);
        self.inquiries.remove(&cmd_id);

        Some((category, camera_id))
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
            TimeoutSource::Socket(_socket) => {
                // Socket state is now in CommandPhase::Executing - no separate cleanup needed.
                // The command will be removed from commands HashMap which implicitly frees the socket.
            }
            TimeoutSource::Inquiry => {
                // Remove from inquiry FIFO order
                self.inquiries_order.retain(|&id| id != cmd_id);
            }
            TimeoutSource::Ack => {
                // Phase transitions are handled by command state updates - no separate index to update.

                if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                    eprintln!(
                        "[SchedulerCore] ACK timeout: cmd_id={}, awaiting_ack_count={}",
                        cmd_id,
                        self.count_awaiting_ack()
                    );
                }

                // Debug assertion: command in AwaitingAck should not also have a socket in its phase
                #[cfg(debug_assertions)]
                {
                    if let Some(state) = self.commands.get(&cmd_id) {
                        if state.phase.socket().is_some() {
                            eprintln!(
                                "ERROR: Invariant violation: ACK-timed-out command {} has socket in phase",
                                cmd_id
                            );
                            debug_assert!(false, "ACK timeout invariant violation");
                        }
                    }
                }

                // For ACK timeouts, emit a Timeout action to notify the adapter
                let will_retry = self.should_retry_timeout(cmd_id, now);
                actions.push(SchedulerAction::Timeout {
                    #[cfg(any(feature = "mode-async", test))]
                    id: cmd_id,
                    #[cfg(any(feature = "mode-async", test))]
                    kind: TimeoutKind::Ack,
                    #[cfg(any(feature = "mode-async", test))]
                    will_retry,
                });
                #[cfg(not(any(feature = "mode-async", test)))]
                let _ = will_retry;
            }
        }

        // Unified retry/terminal-failure via queue_retry_for_command.
        // ACK timeouts cap the backoff exponent at 5 (2^5 = 32x base delay) to prevent
        // excessively long delays on repeated ACK timeouts. Socket and inquiry timeouts
        // use uncapped exponential backoff.
        let delay_exponent_cap = match source {
            TimeoutSource::Ack => Some(5),
            TimeoutSource::Socket(_) | TimeoutSource::Inquiry => None,
        };
        if let Some(action) = self.queue_retry_for_command(cmd_id, now, delay_exponent_cap) {
            actions.push(action);
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
            // Verify it's actually in AwaitingAck phase
            if self.is_awaiting_ack(id) {
                Some(id)
            } else {
                debug!("ACK with sequence {} not found in pending commands", id);
                None
            }
        } else {
            // Fall back to oldest pending command in AwaitingAck phase (FIFO order for raw VISCA)
            self.commands
                .iter()
                .filter_map(|(&cmd_id, state)| {
                    if let CommandPhase::AwaitingAck { sent_at } = state.phase {
                        Some((cmd_id, sent_at))
                    } else {
                        None
                    }
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
            // Determine which socket to use with fallback logic
            let assigned_socket = if let Some(s) = socket {
                // Camera specified a socket - try to use it
                if self.is_socket_free(s) {
                    // Requested socket is free, use it
                    s
                } else {
                    // Requested socket is busy, try the other one
                    let other = if s == ViscaSocket::S1 {
                        ViscaSocket::S2
                    } else {
                        ViscaSocket::S1
                    };

                    if self.is_socket_free(other) {
                        debug!(
                            "Camera requested {:?} but it's occupied, using {:?} instead",
                            s, other
                        );
                        other
                    } else {
                        // Both sockets are busy - command stays in AwaitingAck phase
                        warn!("Camera assigned {:?} but both sockets are occupied", s);
                        return (None, None);
                    }
                }
            } else {
                // No socket specified - pick the first free one
                if self.is_socket_free(ViscaSocket::S1) {
                    ViscaSocket::S1
                } else if self.is_socket_free(ViscaSocket::S2) {
                    ViscaSocket::S2
                } else {
                    // Both sockets are busy - command stays in AwaitingAck phase
                    warn!("ACK received without socket nibble but both sockets are occupied");
                    return (None, None);
                }
            };

            // Transition command phase to Executing (socket is now embedded in phase)
            if let Some(state) = self.commands.get_mut(&target_id) {
                state.phase = CommandPhase::Executing {
                    socket: assigned_socket,
                    started_at: now,
                };
            }

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
                    #[cfg(any(feature = "mode-async", test))]
                    camera_id: cmd_state.camera_id,
                    #[cfg(any(feature = "mode-async", test))]
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
    /// Category, kind, and response routing are derived from the `EncodedCommand`,
    /// ensuring consistency and eliminating the possibility of mismatched metadata.
    ///
    /// This method uses upsert semantics to preserve retry state across resends:
    /// - If the inquiry already exists in `commands`, update send-related fields
    ///   while preserving lifecycle tracking fields.
    /// - If this is a new inquiry, create fresh state with `attempt = 0` and
    ///   `submitted_at = now`.
    pub fn start_inquiry(
        &mut self,
        id: CommandId,
        command: Arc<EncodedCommand>,
        priority: Priority,
        camera_id: crate::camera_id::CameraId,
        now: Instant,
    ) {
        let response_spec = match command.behavior.inquiry_response_spec() {
            Some(response_spec) => response_spec,
            None => {
                warn!(
                    %id,
                    behavior = ?command.behavior,
                    "Ignoring invalid inquiry entry"
                );
                return;
            }
        };

        self.commands.remove(&id);

        // Update existing state if it exists (preserves retry tracking),
        // otherwise create a new state
        if let Some(existing) = self.inquiries.get_mut(&id) {
            // Update send-related fields, preserve retry tracking
            existing.command = command;
            existing.priority = priority;
            existing.camera_id = camera_id;
            existing.phase = InquiryPhase::AwaitingReply { sent_at: now };
            existing.response_spec = response_spec;
            // Note: preserve attempt, transport_error, and submitted_at
            // Category and kind come from command, so no update needed

            trace!(
                %id,
                response_spec = ?existing.response_spec,
                attempt = existing.attempt,
                "Updated existing inquiry state for resend"
            );
        } else {
            // New inquiry - create fresh state
            let state = InquiryEntry {
                command,
                priority,
                camera_id,
                submitted_at: now,
                phase: InquiryPhase::AwaitingReply { sent_at: now },
                attempt: 0,
                transport_error: false,
                response_spec,
            };
            self.inquiries.insert(id, state);

            trace!(
                %id,
                response_spec = ?response_spec,
                "Created new inquiry state"
            );
        }

        // Phase is now set to AwaitingReply - no separate index needed

        // Remove any existing occurrence to prevent duplicates, then add to back
        // This maintains FIFO ordering while ensuring at most one entry per id
        self.inquiries_order.retain(|&x| x != id);
        self.inquiries_order.push_back(id);

        // Update last inquiry sent time for spacing enforcement
        self.last_inquiry_sent = Some(now);

        // Update last command sent time for spacing enforcement (applies to all sends)
        self.last_command_sent = Some(now);

        // Debug assertion: inquiries_order should not contain duplicates
        debug_assert!(
            {
                let mut seen = HashSet::new();
                self.inquiries_order.iter().all(|&x| seen.insert(x))
            },
            "inquiries_order contains duplicates after start_inquiry"
        );

        let inflight_count = self.count_awaiting_inquiry_reply();
        trace!(
            %id,
            inflight_count,
            "Started inquiry (phase=AwaitingReply)"
        );
    }

    /// Check if a command is pending (either awaiting ACK or executing with socket).
    #[cfg(any(feature = "mode-async", test))]
    pub fn is_command_pending(&self, cmd_id: CommandId) -> bool {
        self.commands
            .get(&cmd_id)
            .is_some_and(|s| s.phase.is_awaiting_ack() || s.phase.is_executing())
            || self
                .inquiries
                .get(&cmd_id)
                .is_some_and(|s| s.phase.is_awaiting_reply())
    }

    /// Get the count of commands waiting for ACK.
    #[cfg(feature = "mode-async")]
    pub fn pending_ack_count(&self) -> usize {
        self.count_awaiting_ack()
    }

    /// Get the count of ignored unmatched sequenced replies.
    ///
    /// This counter tracks the number of times a sequenced reply (ACK, Completion,
    /// Error, InquiryReply) was received with a sequence number that did not resolve
    /// to an active command. This is expected for stale/duplicate UDP packets and
    /// indicates the sequence correlation safety mechanism is working correctly.
    #[cfg(any(all(feature = "mode-async", feature = "test-utils"), test))]
    pub fn ignored_unmatched_sequenced_replies(&self) -> u64 {
        self.ignored_unmatched_sequenced_replies
    }

    /// Record a sequenced reply that did not match an active command.
    pub(crate) fn record_unmatched_sequenced_reply(&mut self) {
        self.ignored_unmatched_sequenced_replies += 1;
    }

    /// Get socket state for testing.
    ///
    /// Returns (is_free, command_id, category) for the given socket.
    /// Socket state is derived from CommandPhase::Executing variants.
    #[cfg(test)]
    pub fn socket_state(
        &self,
        socket: ViscaSocket,
    ) -> (bool, Option<CommandId>, Option<CommandCategory>) {
        // Find any command in Executing phase with this socket
        for (&cmd_id, state) in &self.commands {
            if let CommandPhase::Executing {
                socket: s,
                started_at: _,
            } = state.phase
            {
                if s == socket {
                    return (false, Some(cmd_id), Some(state.category()));
                }
            }
        }
        (true, None, None)
    }

    /// Returns `true` if another retry is permitted by both the category retry
    /// budget and `RetryConfig::max_retry_duration` (measured from original
    /// submission).
    fn within_retry_bounds(
        &self,
        attempt: u32,
        category: CommandCategory,
        submitted_at: Instant,
        now: Instant,
    ) -> bool {
        let max_retries = self.retry_budget.for_category(category);
        let within_duration =
            now.duration_since(submitted_at) < self.retry_config.max_retry_duration;
        attempt < max_retries && within_duration
    }

    /// Contextual, scheduler-owned retry classification for an attributed VISCA
    /// protocol error.
    ///
    /// This is the single decision point for whether a protocol error retries
    /// or fails terminally, and which error to surface when it fails. It keeps
    /// `Error::SyntaxError.is_retryable()` `false`: the narrow transient
    /// inquiry-side `0x02` case is recognised here from the raw code plus live
    /// entry context (entry kind, response spec, lifecycle phase), never by
    /// broadening public error retryability.
    ///
    /// Rules:
    /// - Command-side errors: only the standard retryable codes (buffer full,
    ///   and `0x41` for movement/preset) retry; everything else — including
    ///   `0x02` — is terminal and diagnostic.
    /// - Inquiry-side errors: standard retryable codes retry as usual; a `0x02`
    ///   on a `Builtin` inquiry still awaiting its reply is treated as a
    ///   transient overload signal and retried within budget, then fails as a
    ///   contextual `SyntaxError` once exhausted. `Raw` inquiries stay terminal
    ///   so genuine malformed-byte bugs are not hidden.
    fn classify_error_retry(&self, cmd_id: CommandId, code: u8, now: Instant) -> RetryDecision {
        let error = ViscaError::from_byte(code);

        if let Some(state) = self.commands.get(&cmd_id) {
            let category = state.category();
            if error.is_retryable(Some(category))
                && self.within_retry_bounds(state.attempt, category, state.submitted_at, now)
            {
                return RetryDecision::Retry {
                    inquiry_syntax: false,
                };
            }
            return RetryDecision::Fail(Error::from_code(code));
        }

        if let Some(state) = self.inquiries.get(&cmd_id) {
            let category = state.category();
            let within_bounds =
                self.within_retry_bounds(state.attempt, category, state.submitted_at, now);

            if error.is_retryable(Some(category)) {
                return if within_bounds {
                    RetryDecision::Retry {
                        inquiry_syntax: false,
                    }
                } else {
                    RetryDecision::Fail(Error::from_code(code))
                };
            }

            if code == 0x02
                && matches!(state.response_spec, InquiryResponseSpec::Builtin(_))
                && state.phase.is_awaiting_reply()
            {
                return if within_bounds {
                    RetryDecision::Retry {
                        inquiry_syntax: true,
                    }
                } else {
                    // Exhausted by count or duration: surface a syntax error
                    // with context so retry exhaustion is distinguishable from a
                    // first-attempt syntax error, while staying non-retryable.
                    RetryDecision::Fail(
                        Error::SyntaxError
                            .with_context("inquiry syntax-error retry budget exhausted"),
                    )
                };
            }

            return RetryDecision::Fail(Error::from_code(code));
        }

        // No active entry (already cleaned up): nothing to retry.
        RetryDecision::Fail(Error::from_code(code))
    }

    /// Extend the inquiry-class cooldown so it lasts at least until `until`.
    fn arm_inquiry_cooldown(&mut self, until: Instant) {
        self.inquiry_cooldown_until = Some(match self.inquiry_cooldown_until {
            Some(existing) => existing.max(until),
            None => until,
        });
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
        // Remove from inquiry FIFO order
        self.inquiries_order.retain(|&x| x != cmd_id);

        // For send failures on first attempt, fail immediately with the original error
        // wrapped with "Send failed" context. This preserves the error kind (e.g., Timeout)
        // while adding context about when the failure occurred.
        self.finish_sequence(cmd_id);
        self.commands.remove(&cmd_id);
        self.inquiries.remove(&cmd_id);
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
        // Remove from inquiry FIFO order
        self.inquiries_order.retain(|&x| x != cmd_id);

        // Clean up sequence mappings and command state
        // (Socket is embedded in phase, removed with command)
        self.finish_sequence(cmd_id);
        self.commands.remove(&cmd_id);
        self.inquiries.remove(&cmd_id);

        Some(SchedulerAction::CommandFailed { id: cmd_id, error })
    }

    /// Mark a retry as being triggered by a transport error.
    /// This affects the final error classification when retries are exhausted.
    #[cfg(test)]
    pub fn mark_retry_as_transport_error(&mut self, cmd_id: CommandId) {
        if let Some(state) = self.commands.get_mut(&cmd_id) {
            state.transport_error = true;
        } else if let Some(state) = self.inquiries.get_mut(&cmd_id) {
            state.transport_error = true;
        }
    }

    /// Compute the backoff delay for a given (1-based) retry attempt.
    ///
    /// When `delay_exponent_cap` is `Some(cap)`, the attempt number used for the
    /// delay calculation is capped at `cap + 1` so the backoff exponent does not
    /// exceed `cap`. Shared by the retry queue and the inquiry cooldown so both
    /// derive identical timings from a single source.
    fn retry_delay_for(&self, new_attempt: u32, delay_exponent_cap: Option<u32>) -> Duration {
        let delay_attempt = match delay_exponent_cap {
            Some(cap) => new_attempt.min(cap + 1),
            None => new_attempt,
        };
        let retry_attempt = RetryAttempt::new(delay_attempt).unwrap_or(RetryAttempt::FIRST);
        self.retry_config.calculate_delay(retry_attempt, None)
    }

    /// Queue a command for retry based on the retry configuration.
    ///
    /// If `delay_exponent_cap` is `Some(cap)`, the backoff exponent is capped at `cap`
    /// (i.e., the attempt number used for delay calculation is capped at `cap + 1`).
    /// This is used for ACK timeouts to prevent excessively long delays (cap of 5
    /// limits backoff to 2^5 = 32x base delay).
    pub fn queue_retry_for_command(
        &mut self,
        cmd_id: CommandId,
        now: Instant,
        delay_exponent_cap: Option<u32>,
    ) -> Option<SchedulerAction> {
        let state = self.retry_state(cmd_id)?;

        // Reset phase to Queued for retry. For commands this frees any socket
        // because socket ownership is embedded in CommandPhase::Executing.
        self.set_active_phase_queued(cmd_id);

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
            self.remove_active_entry(cmd_id);
            return Some(SchedulerAction::CommandFailed { id: cmd_id, error });
        }

        // Update retry count in active state
        self.set_active_attempt(cmd_id, new_attempt);

        // Calculate backoff delay using RetryConfig (new_attempt is 1-based).
        let delay = self.retry_delay_for(new_attempt, delay_exponent_cap);

        let retry_cmd = RetryCommand {
            id: cmd_id,
            command: state.command.clone(),
            priority: state.priority,
            camera_id: state.camera_id,
            attempt: new_attempt,
            max_retries,
            retry_at: now + delay,
        };

        debug!(
            "Queueing retry for command {} (attempt {} of {})",
            cmd_id, new_attempt, max_retries
        );
        self.retry_queue.push(RetryKey { command: retry_cmd });

        #[cfg(not(any(feature = "mode-async", test)))]
        let _ = delay;

        Some(SchedulerAction::RetryCommand {
            #[cfg(any(feature = "mode-async", test))]
            id: cmd_id,
            #[cfg(any(feature = "mode-async", test))]
            delay,
        })
    }

    fn retry_state(&self, cmd_id: CommandId) -> Option<RetryState> {
        if let Some(state) = self.commands.get(&cmd_id) {
            Some(RetryState {
                command: state.command.clone(),
                priority: state.priority,
                camera_id: state.camera_id,
                submitted_at: state.submitted_at,
                attempt: state.attempt,
                transport_error: state.transport_error,
                category: state.category(),
            })
        } else {
            self.inquiries.get(&cmd_id).map(|state| RetryState {
                command: state.command.clone(),
                priority: state.priority,
                camera_id: state.camera_id,
                submitted_at: state.submitted_at,
                attempt: state.attempt,
                transport_error: state.transport_error,
                category: state.category(),
            })
        }
    }

    fn set_active_phase_queued(&mut self, cmd_id: CommandId) {
        if let Some(state) = self.commands.get_mut(&cmd_id) {
            state.phase = CommandPhase::Queued;
        } else if let Some(state) = self.inquiries.get_mut(&cmd_id) {
            state.phase = InquiryPhase::Queued;
        }
    }

    fn set_active_attempt(&mut self, cmd_id: CommandId, attempt: u32) {
        if let Some(state) = self.commands.get_mut(&cmd_id) {
            state.attempt = attempt;
        } else if let Some(state) = self.inquiries.get_mut(&cmd_id) {
            state.attempt = attempt;
        }
    }

    fn remove_active_entry(&mut self, cmd_id: CommandId) {
        self.commands.remove(&cmd_id);
        self.inquiries.remove(&cmd_id);
    }

    fn active_attempt(&self, cmd_id: CommandId) -> Option<u32> {
        self.commands
            .get(&cmd_id)
            .map(|state| state.attempt)
            .or_else(|| self.inquiries.get(&cmd_id).map(|state| state.attempt))
    }

    fn active_is_queued(&self, cmd_id: CommandId) -> bool {
        self.commands
            .get(&cmd_id)
            .is_some_and(|state| matches!(state.phase, CommandPhase::Queued))
            || self
                .inquiries
                .get(&cmd_id)
                .is_some_and(|state| matches!(state.phase, InquiryPhase::Queued))
    }

    fn active_phase_debug(&self, cmd_id: CommandId) -> &'static str {
        if let Some(state) = self.commands.get(&cmd_id) {
            match state.phase {
                CommandPhase::Queued => "Command::Queued",
                CommandPhase::AwaitingAck { .. } => "Command::AwaitingAck",
                CommandPhase::Executing { .. } => "Command::Executing",
            }
        } else {
            match self.inquiries.get(&cmd_id).map(|state| state.phase) {
                Some(InquiryPhase::Queued) => "Inquiry::Queued",
                Some(InquiryPhase::AwaitingReply { .. }) => "Inquiry::AwaitingReply",
                None => "Unknown",
            }
        }
    }

    // --- Phase-based counting helpers ---

    /// Count commands in the AwaitingAck phase.
    #[inline]
    fn count_awaiting_ack(&self) -> usize {
        self.commands
            .values()
            .filter(|s| s.phase.is_awaiting_ack())
            .count()
    }

    /// Count commands in the Executing phase.
    #[inline]
    fn count_executing(&self) -> usize {
        self.commands
            .values()
            .filter(|s| s.phase.is_executing())
            .count()
    }

    /// Count inquiries awaiting replies.
    #[inline]
    fn count_awaiting_inquiry_reply(&self) -> usize {
        self.inquiries
            .values()
            .filter(|s| s.phase.is_awaiting_reply())
            .count()
    }

    /// Clear all scheduler state.
    ///
    /// This is used when the transport is poisoned and all commands must be failed.
    /// It clears all internal state but does not send errors to response channels -
    /// that is handled by the caller.
    pub fn clear_all(&mut self) {
        self.commands.clear();
        self.inquiries.clear();
        self.retry_queue.clear();
        self.command_queue.clear();
        self.inquiry_queue.clear();
        self.seq_to_cmd.clear();
        self.cmd_to_seqs.clear();
        self.seq16_to_cmds.clear();
        self.cmd_to_seq16s.clear();
        self.inquiries_order.clear();
        self.last_logged_idle.set(false);
        self.last_inquiry_sent = None;
        self.last_command_sent = None;
        self.inquiry_cooldown_until = None;
    }

    /// Check if a command is in the AwaitingAck phase.
    ///
    /// Returns true if the command exists and is awaiting ACK.
    #[inline]
    fn is_awaiting_ack(&self, cmd_id: CommandId) -> bool {
        self.commands
            .get(&cmd_id)
            .is_some_and(|s| s.phase.is_awaiting_ack())
    }

    /// Check if an inquiry is in the AwaitingReply phase.
    ///
    /// Returns true if the inquiry exists and is awaiting reply.
    #[inline]
    pub(crate) fn is_awaiting_inquiry_reply(&self, cmd_id: CommandId) -> bool {
        self.inquiries
            .get(&cmd_id)
            .is_some_and(|s| s.phase.is_awaiting_reply())
    }

    /// Find the command ID currently assigned to a socket, if any.
    pub fn find_command_on_socket(&self, socket: ViscaSocket) -> Option<CommandId> {
        self.commands.iter().find_map(|(&cmd_id, state)| {
            if state.phase.socket() == Some(socket) {
                Some(cmd_id)
            } else {
                None
            }
        })
    }

    /// Check if a socket is free (not assigned to any command).
    fn is_socket_free(&self, socket: ViscaSocket) -> bool {
        !self
            .commands
            .values()
            .any(|s| s.phase.socket() == Some(socket))
    }

    /// Get commands in AwaitingAck phase.
    fn commands_awaiting_ack(&self) -> impl Iterator<Item = CommandId> + '_ {
        self.commands.iter().filter_map(|(&cmd_id, state)| {
            if state.phase.is_awaiting_ack() {
                Some(cmd_id)
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
mod tests;
