//! VISCA runtime scheduler using flume channels.
//!
//! This module implements the core VISCA runtime with proper socket management,
//! command scheduling, and protocol-compliant timing.

use flume::{Receiver, Sender};
use log::{debug, trace, warn};
use std::{
    cmp::Ordering as CmpOrdering,
    collections::{BinaryHeap, HashMap},
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
    time::{Duration, Instant},
};
use tracing::{info_span, instrument};

use crate::{
    command::response::Response,
    error::Result,
    timeout::{CommandCategory, TimeoutConfig},
};

/// Represents an item to be transmitted (command, inquiry, or cancel).
#[derive(Debug, Clone)]
pub enum TxItem {
    /// A command that requires a socket and expects ACK/Completion.
    Command {
        /// Unique identifier for this command.
        id: u32,
        /// Raw VISCA bytes to send.
        bytes: Vec<u8>,
        /// Priority level for scheduling.
        priority: Priority,
        /// Deadline for command execution.
        deadline: Instant,
        /// Category for timeout calculation.
        category: CommandCategory,
        /// Channel to send response back.
        response_tx: Sender<Result<Response>>,
    },
    /// An inquiry that doesn't require a socket, expects DataReply.
    Inquiry {
        /// Unique identifier for this inquiry.
        id: u32,
        /// Raw VISCA bytes to send.
        bytes: Vec<u8>,
        /// Deadline for inquiry response.
        deadline: Instant,
        /// Expected response type.
        response_type: Option<crate::command::response::ResponseType>,
        /// Channel to send response back.
        response_tx: Sender<Result<Response>>,
    },
    /// Cancel a command on a specific socket.
    Cancel {
        /// Socket to cancel (1 or 2).
        socket: SocketId,
    },
}

/// Events received from the VISCA device.
#[derive(Debug, Clone)]
pub enum RxEvent {
    /// Acknowledgment that a command has been accepted.
    Ack {
        /// Socket that received the ACK.
        socket: SocketId,
        /// Command ID that was acknowledged.
        id: u32,
    },
    /// Command has completed execution.
    Completion {
        /// Socket that completed.
        socket: SocketId,
        /// Command ID that completed.
        id: u32,
    },
    /// Data reply from an inquiry.
    DataReply {
        /// Inquiry ID that received data.
        id: u32,
        /// Response data bytes.
        data: Vec<u8>,
    },
    /// Error response from device.
    Error {
        /// Error code from VISCA protocol.
        code: ViscaError,
        /// Socket if error is socket-specific.
        socket: Option<SocketId>,
        /// Command/inquiry ID if applicable.
        id: Option<u32>,
    },
    /// Link state events.
    Link(LinkEvent),
}

/// Socket identifier for VISCA commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SocketId {
    /// First command socket.
    Socket1,
    /// Second command socket.
    Socket2,
}

impl SocketId {
    /// Convert to zero-based index.
    pub fn as_index(&self) -> usize {
        match self {
            SocketId::Socket1 => 0,
            SocketId::Socket2 => 1,
        }
    }

    /// Convert to VISCA socket byte (0x01 or 0x02).
    pub fn as_byte(&self) -> u8 {
        match self {
            SocketId::Socket1 => 0x01,
            SocketId::Socket2 => 0x02,
        }
    }

    /// Create from VISCA socket byte.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte & 0x0F {
            1 => Some(SocketId::Socket1),
            2 => Some(SocketId::Socket2),
            _ => None,
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

/// VISCA protocol errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViscaError {
    /// Syntax error in command.
    SyntaxError,
    /// Command buffer full.
    BufferFull,
    /// Command cancelled.
    CommandCancelled,
    /// Command not executable in current state.
    NotExecutable,
    /// No socket available.
    NoSocket,
    /// Camera busy - retry later.
    Busy,
    /// Network error.
    NetworkError,
    /// Timeout waiting for response.
    Timeout,
    /// Unknown error code.
    Unknown(u8),
}

impl ViscaError {
    /// Create from VISCA error byte.
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0x01 => ViscaError::Busy,
            0x02 => ViscaError::SyntaxError,
            0x03 => ViscaError::BufferFull,
            0x04 => ViscaError::CommandCancelled,
            0x05 => ViscaError::NoSocket,
            0x41 => ViscaError::NotExecutable,
            other => ViscaError::Unknown(other),
        }
    }

    /// Convert to VISCA error byte.
    pub fn to_byte(&self) -> u8 {
        match self {
            ViscaError::Busy => 0x01,
            ViscaError::SyntaxError => 0x02,
            ViscaError::BufferFull => 0x03,
            ViscaError::CommandCancelled => 0x04,
            ViscaError::NoSocket => 0x05,
            ViscaError::NotExecutable => 0x41,
            ViscaError::NetworkError => 0x70, // Custom code for network errors
            ViscaError::Timeout => 0x71,      // Custom code for timeouts
            ViscaError::Unknown(byte) => *byte,
        }
    }
}

/// Link state events.
#[derive(Debug, Clone)]
pub enum LinkEvent {
    /// Connected to device.
    Connected,
    /// Disconnected from device.
    Disconnected,
    /// Retrying connection/command.
    Retry {
        /// Attempt number.
        attempt: u32,
        /// Reason for retry.
        reason: String,
    },
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

/// VISCA runtime scheduler.
///
/// Manages command scheduling, socket allocation, and protocol timing.
#[derive(Debug)]
pub struct Scheduler {
    /// Channel for receiving submitted commands/inquiries.
    #[allow(dead_code)]
    submit_rx: Receiver<TxItem>,
    /// Channel for sending events.
    #[allow(dead_code)]
    event_tx: Sender<RxEvent>,
    /// Socket states.
    sockets: [SocketState; 2],
    /// Command ID generator.
    next_id: AtomicU32,
    /// Timeout configuration.
    timeout_config: TimeoutConfig,
    /// Minimum inter-command spacing.
    command_spacing: Duration,
    /// Last command sent time.
    last_command_time: Option<Instant>,
    /// Track response channels for commands by ID.
    command_channels: HashMap<u32, Sender<Result<Response>>>,
    /// Track pending inquiries (ID, response channel, response type).
    pending_inquiries: Vec<(
        u32,
        Sender<Result<Response>>,
        Option<crate::command::response::ResponseType>,
    )>,
    /// Commands waiting to be retried (after busy response).
    pub retry_queue: Vec<RetryCommand>,
    /// Store command metadata for potential retry.
    command_metadata: HashMap<u32, (Vec<u8>, Priority, CommandCategory)>,
    /// Priority queue for pending commands.
    command_queue: BinaryHeap<PriorityQueueItem>,
    /// Maximum retries per command category.
    max_retries_per_category: HashMap<CommandCategory, u32>,
    /// Runtime metrics.
    pub metrics: SchedulerMetrics,
}

/// Command waiting to be retried.
#[derive(Debug, Clone)]
pub struct RetryCommand {
    /// Command ID.
    pub id: u32,
    /// Command bytes.
    pub bytes: Vec<u8>,
    /// Command priority.
    pub priority: Priority,
    /// Command category.
    pub category: CommandCategory,
    /// Retry attempt number.
    pub attempt: u32,
    /// Maximum retries allowed.
    pub max_retries: u32,
    /// When to retry this command (for exponential backoff).
    pub retry_at: Instant,
}

/// Runtime metrics for monitoring scheduler performance.
#[derive(Debug, Default)]
pub struct SchedulerMetrics {
    /// Total commands submitted.
    pub commands_submitted: AtomicU64,
    /// Total inquiries submitted.
    pub inquiries_submitted: AtomicU64,
    /// Total commands completed successfully.
    pub commands_completed: AtomicU64,
    /// Total commands failed.
    pub commands_failed: AtomicU64,
    /// Total retry attempts.
    pub retry_attempts: AtomicU64,
    /// Total commands that exhausted retries.
    pub retries_exhausted: AtomicU64,
    /// Current queue depth.
    pub current_queue_depth: AtomicU32,
    /// Peak queue depth.
    pub peak_queue_depth: AtomicU32,
    /// Current retry queue depth.
    pub current_retry_queue_depth: AtomicU32,
    /// Peak retry queue depth.
    pub peak_retry_queue_depth: AtomicU32,
    /// Commands by priority.
    pub priority_counts: [AtomicU64; 4], // Low, Normal, High, Critical
    /// Retry counts by category.
    pub retry_by_category: [AtomicU64; 6], // Quick, Movement, Preset, Network, LongRunning, Custom
}

impl SchedulerMetrics {
    /// Create new metrics instance.
    pub fn new() -> Self {
        Self {
            priority_counts: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            retry_by_category: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            ..Default::default()
        }
    }

    /// Get priority index for metrics tracking.
    pub fn priority_index(priority: Priority) -> usize {
        match priority {
            Priority::Low => 0,
            Priority::Normal => 1,
            Priority::High => 2,
            Priority::Critical => 3,
        }
    }

    /// Get category index for metrics tracking.
    pub fn category_index(category: CommandCategory) -> usize {
        match category {
            CommandCategory::Quick => 0,
            CommandCategory::Movement => 1,
            CommandCategory::Preset => 2,
            CommandCategory::Network => 3,
            CommandCategory::LongRunning => 4,
            CommandCategory::Custom => 5,
        }
    }

    /// Update queue depth metrics.
    fn update_queue_depth(&self, new_depth: u32) {
        self.current_queue_depth.store(new_depth, Ordering::Relaxed);

        // Update peak if necessary
        let mut peak = self.peak_queue_depth.load(Ordering::Relaxed);
        while new_depth > peak {
            match self.peak_queue_depth.compare_exchange_weak(
                peak,
                new_depth,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => peak = x,
            }
        }
    }

    /// Update retry queue depth metrics.
    fn update_retry_queue_depth(&self, new_depth: u32) {
        self.current_retry_queue_depth
            .store(new_depth, Ordering::Relaxed);

        // Update peak if necessary
        let mut peak = self.peak_retry_queue_depth.load(Ordering::Relaxed);
        while new_depth > peak {
            match self.peak_retry_queue_depth.compare_exchange_weak(
                peak,
                new_depth,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => peak = x,
            }
        }
    }

    /// Get a summary of current metrics.
    pub fn summary(&self) -> MetricsSummary {
        MetricsSummary {
            commands_submitted: self.commands_submitted.load(Ordering::Relaxed),
            inquiries_submitted: self.inquiries_submitted.load(Ordering::Relaxed),
            commands_completed: self.commands_completed.load(Ordering::Relaxed),
            commands_failed: self.commands_failed.load(Ordering::Relaxed),
            retry_attempts: self.retry_attempts.load(Ordering::Relaxed),
            retries_exhausted: self.retries_exhausted.load(Ordering::Relaxed),
            current_queue_depth: self.current_queue_depth.load(Ordering::Relaxed),
            peak_queue_depth: self.peak_queue_depth.load(Ordering::Relaxed),
            current_retry_queue_depth: self.current_retry_queue_depth.load(Ordering::Relaxed),
            peak_retry_queue_depth: self.peak_retry_queue_depth.load(Ordering::Relaxed),
            priority_counts: [
                self.priority_counts[0].load(Ordering::Relaxed),
                self.priority_counts[1].load(Ordering::Relaxed),
                self.priority_counts[2].load(Ordering::Relaxed),
                self.priority_counts[3].load(Ordering::Relaxed),
            ],
            retry_by_category: [
                self.retry_by_category[0].load(Ordering::Relaxed),
                self.retry_by_category[1].load(Ordering::Relaxed),
                self.retry_by_category[2].load(Ordering::Relaxed),
                self.retry_by_category[3].load(Ordering::Relaxed),
                self.retry_by_category[4].load(Ordering::Relaxed),
                self.retry_by_category[5].load(Ordering::Relaxed),
            ],
        }
    }
}

/// Snapshot of scheduler metrics.
#[derive(Debug, Clone, Copy)]
pub struct MetricsSummary {
    /// Total number of commands submitted to the runtime.
    pub commands_submitted: u64,
    /// Total number of inquiries submitted to the runtime.
    pub inquiries_submitted: u64,
    /// Total number of commands that completed successfully.
    pub commands_completed: u64,
    /// Total number of commands that failed with an error.
    pub commands_failed: u64,
    /// Total number of retry attempts made.
    pub retry_attempts: u64,
    /// Total number of commands that exhausted their retry limit.
    pub retries_exhausted: u64,
    /// Current number of commands in the priority queue.
    pub current_queue_depth: u32,
    /// Peak number of commands in the priority queue.
    pub peak_queue_depth: u32,
    /// Current number of commands in the retry queue.
    pub current_retry_queue_depth: u32,
    /// Peak number of commands in the retry queue.
    pub peak_retry_queue_depth: u32,
    /// Count of commands by priority level [Low, Normal, High, Critical].
    pub priority_counts: [u64; 4],
    /// Count of retries by command category [Quick, Movement, Preset, Network, LongRunning, Custom].
    pub retry_by_category: [u64; 6],
}

/// Priority queue item wrapper for commands.
///
/// This struct wraps a TxItem to make it orderable for the BinaryHeap.
/// Higher priority commands will be processed first.
#[derive(Debug, Clone)]
struct PriorityQueueItem {
    /// The actual command/inquiry item.
    pub item: TxItem,
    /// Submission time for FIFO ordering within same priority.
    pub submitted_at: Instant,
}

impl PartialEq for PriorityQueueItem {
    fn eq(&self, other: &Self) -> bool {
        // Compare by priority and submission time
        self.priority() == other.priority() && self.submitted_at == other.submitted_at
    }
}

impl Eq for PriorityQueueItem {}

impl PartialOrd for PriorityQueueItem {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityQueueItem {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        // First compare by priority (higher priority first)
        match self.priority().cmp(&other.priority()) {
            CmpOrdering::Equal => {
                // For same priority, use FIFO (earlier submission time first)
                // Note: reverse comparison for submission time
                other.submitted_at.cmp(&self.submitted_at)
            }
            other => other,
        }
    }
}

impl PriorityQueueItem {
    /// Get the priority of this item.
    fn priority(&self) -> Priority {
        match &self.item {
            TxItem::Command { priority, .. } => *priority,
            TxItem::Inquiry { .. } => Priority::Normal, // Inquiries default to normal priority
            TxItem::Cancel { .. } => Priority::Critical, // Cancels have highest priority
        }
    }
}

impl Scheduler {
    /// Create a new scheduler with given channels.
    pub fn new(submit_rx: Receiver<TxItem>, event_tx: Sender<RxEvent>) -> Self {
        // Set default max retries per category
        let mut max_retries = HashMap::new();
        max_retries.insert(CommandCategory::Quick, 5); // Quick commands can retry more
        max_retries.insert(CommandCategory::Movement, 3); // Movement commands retry moderately
        max_retries.insert(CommandCategory::Preset, 3); // Preset commands retry moderately
        max_retries.insert(CommandCategory::Network, 2); // Network commands retry less
        max_retries.insert(CommandCategory::LongRunning, 1); // Long operations retry minimally
        max_retries.insert(CommandCategory::Custom, 3); // Custom commands use default

        Self {
            submit_rx,
            event_tx,
            sockets: Default::default(),
            next_id: AtomicU32::new(1),
            timeout_config: TimeoutConfig::default(),
            command_spacing: Duration::from_millis(50), // Default 50ms spacing
            last_command_time: None,
            command_channels: HashMap::new(),
            pending_inquiries: Vec::new(),
            retry_queue: Vec::new(),
            command_metadata: HashMap::new(),
            command_queue: BinaryHeap::new(),
            max_retries_per_category: max_retries,
            metrics: SchedulerMetrics::new(),
        }
    }

    /// Generate next command ID.
    pub fn next_id(&self) -> u32 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Check if any socket is free.
    pub fn has_free_socket(&self) -> bool {
        self.sockets.iter().any(|s| s.free)
    }

    /// Allocate a free socket for a command.
    pub fn allocate_socket(
        &mut self,
        command_id: u32,
        category: CommandCategory,
    ) -> Option<SocketId> {
        for (idx, socket) in self.sockets.iter_mut().enumerate() {
            if socket.free {
                socket.free = false;
                socket.command_id = Some(command_id);
                socket.started_at = Some(Instant::now());
                socket.category = Some(category);

                let socket_id = if idx == 0 {
                    SocketId::Socket1
                } else {
                    SocketId::Socket2
                };
                debug!("Allocated {:?} for command {}", socket_id, command_id);
                return Some(socket_id);
            }
        }
        None
    }

    /// Free a socket after command completion.
    pub fn free_socket(&mut self, socket: SocketId) {
        let idx = socket.as_index();
        let state = &mut self.sockets[idx];

        if let Some(cmd_id) = state.command_id {
            debug!("Freeing {:?} from command {}", socket, cmd_id);
            // Also remove the command channel if it's still there
            // BUT NOT if the command is queued for retry
            let is_queued_for_retry = self.retry_queue.iter().any(|rc| rc.id == cmd_id);
            if !is_queued_for_retry {
                self.command_channels.remove(&cmd_id);
            }
        }

        state.free = true;
        state.command_id = None;
        state.started_at = None;
        state.category = None;
    }

    /// Get command ID for a socket.
    pub fn socket_command(&self, socket: SocketId) -> Option<u32> {
        self.sockets[socket.as_index()].command_id
    }

    /// Enforce minimum command spacing.
    #[instrument(skip(self))]
    pub async fn enforce_spacing(&mut self) {
        if let Some(last_time) = self.last_command_time {
            let elapsed = last_time.elapsed();
            if elapsed < self.command_spacing {
                let wait_time = self.command_spacing - elapsed;
                trace!("Waiting {:?} for command spacing", wait_time);
                #[cfg(feature = "rt-tokio")]
                tokio::time::sleep(wait_time).await;
                #[cfg(not(feature = "rt-tokio"))]
                std::thread::sleep(wait_time);
            }
        }
        self.last_command_time = Some(Instant::now());
    }

    /// Check for timed out commands.
    pub fn check_timeouts(&mut self) -> Vec<(SocketId, u32)> {
        let mut timed_out = Vec::new();
        let now = Instant::now();

        for (idx, socket) in self.sockets.iter().enumerate() {
            if !socket.free {
                if let (Some(started), Some(category), Some(cmd_id)) =
                    (socket.started_at, socket.category, socket.command_id)
                {
                    let timeout = self.timeout_config.get_timeout(category);
                    if now.duration_since(started) > timeout {
                        let socket_id = if idx == 0 {
                            SocketId::Socket1
                        } else {
                            SocketId::Socket2
                        };
                        warn!(
                            "Command {} on {:?} timed out after {:?}",
                            cmd_id, socket_id, timeout
                        );
                        timed_out.push((socket_id, cmd_id));
                    }
                }
            }
        }

        timed_out
    }

    /// Set command spacing duration.
    pub fn set_command_spacing(&mut self, spacing: Duration) {
        self.command_spacing = spacing;
        info_span!("config").in_scope(|| {
            debug!("Command spacing set to {:?}", spacing);
        });
    }

    /// Set timeout configuration.
    pub fn set_timeout_config(&mut self, config: TimeoutConfig) {
        self.timeout_config = config;
        info_span!("config").in_scope(|| {
            debug!("Timeout configuration updated");
        });
    }

    /// Set maximum retries for a specific command category.
    pub fn set_max_retries(&mut self, category: CommandCategory, max_retries: u32) {
        self.max_retries_per_category.insert(category, max_retries);
        debug!("Max retries for {:?} set to {}", category, max_retries);
    }

    /// Get maximum retries for a specific command category.
    pub fn get_max_retries(&self, category: CommandCategory) -> u32 {
        self.max_retries_per_category
            .get(&category)
            .copied()
            .unwrap_or(3)
    }

    /// Get the response channel for a command ID.
    ///
    /// Returns the channel if the command is still pending.
    pub fn get_response_channel(&mut self, cmd_id: u32) -> Option<Sender<Result<Response>>> {
        self.command_channels.remove(&cmd_id)
    }

    /// Peek at the response channel for a command without removing it.
    ///
    /// Returns the channel if the command is still pending.
    pub fn peek_response_channel(&self, cmd_id: u32) -> Option<&Sender<Result<Response>>> {
        self.command_channels.get(&cmd_id)
    }

    /// Get the most recently sent command ID.
    ///
    /// This is useful for routing broadcast errors to the most likely command.
    pub fn most_recent_command(&self) -> Option<u32> {
        // Check both sockets for the most recent command
        let socket1_cmd = self.sockets[0].command_id;
        let socket2_cmd = self.sockets[1].command_id;

        // Return the command with the highest ID (most recent)
        match (socket1_cmd, socket2_cmd) {
            (Some(id1), Some(id2)) => Some(id1.max(id2)),
            (Some(id), None) | (None, Some(id)) => Some(id),
            (None, None) => None,
        }
    }

    /// Free the socket associated with a command ID.
    ///
    /// This is used when an error is received for a specific command.
    pub fn free_command_socket(&mut self, cmd_id: u32) {
        // Find and free the socket holding this command
        for i in 0..2 {
            if self.sockets[i].command_id == Some(cmd_id) {
                self.sockets[i].free = true;
                self.sockets[i].command_id = None;
                self.sockets[i].started_at = None;
                self.sockets[i].category = None;

                // Also remove the command channel and metadata
                self.command_channels.remove(&cmd_id);
                self.command_metadata.remove(&cmd_id);
                break;
            }
        }
    }

    /// Store response channel for a command.
    ///
    /// This should be called when a command is submitted.
    pub fn store_command_channel(&mut self, cmd_id: u32, channel: Sender<Result<Response>>) {
        self.command_channels.insert(cmd_id, channel);
    }

    /// Store command metadata for potential retry.
    ///
    /// This should be called when a command is sent.
    pub fn store_command_metadata(
        &mut self,
        cmd_id: u32,
        bytes: Vec<u8>,
        priority: Priority,
        category: CommandCategory,
    ) {
        self.command_metadata
            .insert(cmd_id, (bytes, priority, category));
    }

    /// Get the pending inquiry and its response channel.
    ///
    /// Returns the inquiry ID, response channel, and response type if there's a pending inquiry.
    /// This removes the inquiry from the pending list.
    pub fn get_pending_inquiry(
        &mut self,
    ) -> Option<(
        u32,
        Sender<Result<Response>>,
        Option<crate::command::response::ResponseType>,
    )> {
        self.pending_inquiries.pop()
    }

    /// Store a pending inquiry.
    ///
    /// This should be called when an inquiry is submitted.
    pub fn store_pending_inquiry(
        &mut self,
        id: u32,
        channel: Sender<Result<Response>>,
        response_type: Option<crate::command::response::ResponseType>,
    ) {
        self.pending_inquiries.push((id, channel, response_type));
    }

    /// Get the response type for a pending inquiry.
    ///
    /// Returns the response type if the inquiry is still pending.
    pub fn get_inquiry_response_type(
        &self,
        id: u32,
    ) -> Option<crate::command::response::ResponseType> {
        self.pending_inquiries
            .iter()
            .find(|(inquiry_id, _, _)| *inquiry_id == id)
            .and_then(|(_, _, response_type)| *response_type)
    }

    /// Add a command to the retry queue.
    ///
    /// This is called when a command receives a busy response.
    pub fn queue_for_retry(
        &mut self,
        id: u32,
        bytes: Vec<u8>,
        priority: Priority,
        category: CommandCategory,
    ) {
        // Track retry metrics
        self.metrics.retry_attempts.fetch_add(1, Ordering::Relaxed);
        let cat_idx = SchedulerMetrics::category_index(category);
        self.metrics.retry_by_category[cat_idx].fetch_add(1, Ordering::Relaxed);

        // Check if this command is already in the retry queue
        if let Some(cmd) = self.retry_queue.iter_mut().find(|c| c.id == id) {
            // Increment retry attempt
            cmd.attempt += 1;
            // Calculate exponential backoff: 100ms * 2^(attempt-1)
            let backoff_ms = 100 * (1 << (cmd.attempt - 1).min(5)); // Cap at 3.2 seconds
            cmd.retry_at = Instant::now() + Duration::from_millis(backoff_ms);
            debug!(
                "Command {} queued for retry attempt {} with {}ms backoff",
                id, cmd.attempt, backoff_ms
            );
        } else {
            // Add new retry command with initial backoff
            let backoff_ms = 100; // Initial backoff is 100ms
            let max_retries = self
                .max_retries_per_category
                .get(&category)
                .copied()
                .unwrap_or(3);
            self.retry_queue.push(RetryCommand {
                id,
                bytes,
                priority,
                category,
                attempt: 1,
                max_retries,
                retry_at: Instant::now() + Duration::from_millis(backoff_ms),
            });
            debug!(
                "Command {} queued for first retry with {}ms backoff (max retries: {})",
                id, backoff_ms, max_retries
            );
        }

        // Update retry queue depth metrics
        let new_depth = self.retry_queue.len() as u32;
        self.metrics.update_retry_queue_depth(new_depth);
    }

    /// Get the next command to retry.
    ///
    /// Returns the highest priority command from the retry queue that is ready to be retried.
    pub fn get_next_retry(&mut self) -> Option<RetryCommand> {
        if self.retry_queue.is_empty() {
            return None;
        }

        let now = Instant::now();

        // Find the highest priority command that is ready to retry
        let mut best_idx = None;
        let mut best_priority = Priority::Low;

        for (idx, cmd) in self.retry_queue.iter().enumerate() {
            // Skip commands that aren't ready yet
            if cmd.retry_at > now {
                continue;
            }

            // Select this command if it has higher priority
            if best_idx.is_none() || cmd.priority > best_priority {
                best_idx = Some(idx);
                best_priority = cmd.priority;
            }
        }

        // Remove and return the selected command if found
        if let Some(idx) = best_idx {
            let cmd = self.retry_queue.swap_remove(idx);

            // Update retry queue depth metrics
            let new_depth = self.retry_queue.len() as u32;
            self.metrics.update_retry_queue_depth(new_depth);

            // Check if this command has exhausted retries
            if cmd.attempt >= cmd.max_retries {
                self.metrics
                    .retries_exhausted
                    .fetch_add(1, Ordering::Relaxed);
                debug!(
                    "Command {} exhausted retries (attempt {} of {})",
                    cmd.id, cmd.attempt, cmd.max_retries
                );
            }

            Some(cmd)
        } else {
            None
        }
    }

    /// Check if there are commands waiting to be retried.
    pub fn has_retries(&self) -> bool {
        !self.retry_queue.is_empty()
    }

    /// Get the command bytes and metadata for a command ID.
    ///
    /// This is used to queue a command for retry when it gets a busy response.
    pub fn get_command_for_retry(
        &self,
        cmd_id: u32,
    ) -> Option<(Vec<u8>, Priority, CommandCategory)> {
        self.command_metadata.get(&cmd_id).cloned()
    }

    /// Enqueue a command/inquiry to the priority queue.
    ///
    /// Commands are sorted by priority and submission time.
    pub fn enqueue_command(&mut self, item: TxItem) {
        let queue_item = PriorityQueueItem {
            item,
            submitted_at: Instant::now(),
        };
        debug!(
            "Enqueueing command with priority {:?} to queue (size: {})",
            queue_item.priority(),
            self.command_queue.len()
        );
        self.command_queue.push(queue_item);

        // Update queue depth metrics
        let new_depth = self.command_queue.len() as u32;
        self.metrics.update_queue_depth(new_depth);
    }

    /// Dequeue the next command from the priority queue.
    ///
    /// Returns the highest priority command, or None if queue is empty.
    pub fn dequeue_command(&mut self) -> Option<TxItem> {
        let queue_item = self.command_queue.pop()?;
        debug!(
            "Dequeuing command with priority {:?} from queue (remaining: {})",
            queue_item.priority(),
            self.command_queue.len()
        );

        // Update queue depth metrics
        let new_depth = self.command_queue.len() as u32;
        self.metrics.update_queue_depth(new_depth);

        Some(queue_item.item)
    }

    /// Check if the command queue is empty.
    pub fn is_queue_empty(&self) -> bool {
        self.command_queue.is_empty()
    }

    /// Get the size of the command queue.
    pub fn queue_size(&self) -> usize {
        self.command_queue.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_id_conversion() {
        assert_eq!(SocketId::Socket1.as_index(), 0);
        assert_eq!(SocketId::Socket2.as_index(), 1);
        assert_eq!(SocketId::Socket1.as_byte(), 0x01);
        assert_eq!(SocketId::Socket2.as_byte(), 0x02);

        assert_eq!(SocketId::from_byte(0x01), Some(SocketId::Socket1));
        assert_eq!(SocketId::from_byte(0x02), Some(SocketId::Socket2));
        assert_eq!(SocketId::from_byte(0x41), Some(SocketId::Socket1)); // With high nibble
        assert_eq!(SocketId::from_byte(0x03), None);
    }

    #[test]
    fn test_visca_error_from_byte() {
        assert_eq!(ViscaError::from_byte(0x02), ViscaError::SyntaxError);
        assert_eq!(ViscaError::from_byte(0x03), ViscaError::BufferFull);
        assert_eq!(ViscaError::from_byte(0x04), ViscaError::CommandCancelled);
        assert_eq!(ViscaError::from_byte(0x05), ViscaError::NoSocket);
        assert_eq!(ViscaError::from_byte(0x41), ViscaError::NotExecutable);
        assert_eq!(ViscaError::from_byte(0xFF), ViscaError::Unknown(0xFF));
    }

    #[test]
    fn test_scheduler_socket_allocation() {
        let (_submit_tx, submit_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let mut scheduler = Scheduler::new(submit_rx, event_tx);

        // Both sockets should be free initially
        assert!(scheduler.has_free_socket());

        // Allocate first socket
        let socket1 = scheduler.allocate_socket(1, CommandCategory::Movement);
        assert_eq!(socket1, Some(SocketId::Socket1));
        assert!(scheduler.has_free_socket());

        // Allocate second socket
        let socket2 = scheduler.allocate_socket(2, CommandCategory::Quick);
        assert_eq!(socket2, Some(SocketId::Socket2));
        assert!(!scheduler.has_free_socket());

        // Try to allocate when none free
        let socket3 = scheduler.allocate_socket(3, CommandCategory::Network);
        assert_eq!(socket3, None);

        // Free a socket
        scheduler.free_socket(SocketId::Socket1);
        assert!(scheduler.has_free_socket());

        // Can allocate again
        let socket4 = scheduler.allocate_socket(4, CommandCategory::Movement);
        assert_eq!(socket4, Some(SocketId::Socket1));
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Normal);
        assert!(Priority::Normal > Priority::Low);
    }

    #[test]
    fn test_response_channel_tracking() {
        let (_submit_tx, submit_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let mut scheduler = Scheduler::new(submit_rx, event_tx);

        // Test command channel tracking
        let (response_tx, _response_rx) = flume::bounded(1);
        let cmd_id = 42;
        scheduler.store_command_channel(cmd_id, response_tx.clone());

        // Should be able to retrieve the channel
        let retrieved = scheduler.get_response_channel(cmd_id);
        assert!(retrieved.is_some());

        // Channel should be removed after retrieval
        let retrieved_again = scheduler.get_response_channel(cmd_id);
        assert!(retrieved_again.is_none());

        // Test inquiry tracking
        let (inquiry_tx, _inquiry_rx) = flume::bounded(1);
        let inquiry_id = 99;
        let response_type = Some(crate::command::response::ResponseType::Power);
        scheduler.store_pending_inquiry(inquiry_id, inquiry_tx.clone(), response_type);

        // Should be able to get the response type
        let retrieved_type = scheduler.get_inquiry_response_type(inquiry_id);
        assert_eq!(retrieved_type, response_type);

        // Should be able to retrieve the inquiry
        let retrieved_inquiry = scheduler.get_pending_inquiry();
        assert!(retrieved_inquiry.is_some());
        let (retrieved_id, _channel, retrieved_response_type) = retrieved_inquiry.unwrap();
        assert_eq!(retrieved_id, inquiry_id);
        assert_eq!(retrieved_response_type, response_type);

        // Inquiry should be removed after retrieval
        let retrieved_again = scheduler.get_pending_inquiry();
        assert!(retrieved_again.is_none());
    }

    #[test]
    fn test_socket_cleanup_removes_channel() {
        let (_submit_tx, submit_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let mut scheduler = Scheduler::new(submit_rx, event_tx);

        // Allocate a socket for a command
        let cmd_id = 123;
        let socket = scheduler
            .allocate_socket(cmd_id, CommandCategory::Movement)
            .unwrap();

        // Store a response channel for the command
        let (response_tx, _response_rx) = flume::bounded(1);
        scheduler.store_command_channel(cmd_id, response_tx);

        // Free the socket
        scheduler.free_socket(socket);

        // Channel should be removed when socket is freed
        let retrieved = scheduler.get_response_channel(cmd_id);
        assert!(retrieved.is_none());
    }
}
