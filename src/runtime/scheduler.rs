//! VISCA runtime scheduler using flume channels.
//!
//! This module implements the core VISCA runtime with proper socket management,
//! command scheduling, and protocol-compliant timing.

#[cfg(feature = "async")]
use flume::{Receiver, Sender};
#[cfg(feature = "async")]
use tracing::{debug, trace, warn};

#[cfg(feature = "async")]
use std::{
    cmp::Ordering as CmpOrdering,
    collections::{BinaryHeap, HashMap},
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
    time::{Duration, Instant},
};

#[cfg(feature = "async")]
use crate::{
    command::response::ViscaResponse,
    error::Result,
    timeout::{CommandCategory, TimeoutConfig},
    visca_socket::ViscaSocket,
};

/// Represents an item to be transmitted (command, inquiry, or cancel).
#[cfg(feature = "async")]
#[derive(Debug, Clone)]
pub(crate) enum TxItem {
    /// A command that requires a socket and expects ACK/Completion.
    Command {
        /// Unique identifier for this command.
        id: u32,
        /// Raw VISCA bytes to send.
        bytes: bytes::Bytes,
        /// Priority level for scheduling.
        priority: Priority,
        /// Category for timeout calculation.
        category: CommandCategory,
        /// Camera ID used to encode the command.
        camera_id: crate::camera_id::CameraId,
        /// Channel to send response back.
        response_tx: Sender<Result<ViscaResponse>>,
    },
    /// An inquiry that doesn't require a socket, expects DataReply.
    Inquiry {
        /// Unique identifier for this inquiry.
        id: u32,
        /// Raw VISCA bytes to send.
        bytes: bytes::Bytes,
        /// Camera ID used to encode the inquiry.
        /// Note: Currently unused but kept for consistency with Command variant.
        #[allow(dead_code)]
        camera_id: crate::camera_id::CameraId,
        /// Expected response type.
        response_type: Option<crate::command::response::ViscaResponseType>,
        /// Channel to send response back.
        response_tx: Sender<Result<ViscaResponse>>,
    },
    /// Cancel a command on a specific socket.
    Cancel {
        /// Socket to cancel (1 or 2).
        socket: ViscaSocket,
    },
    /// Cancel a command by its ID.
    CancelById {
        /// Command ID to cancel.
        id: u32,
    },
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
///
/// This is a thin wrapper around error bytes for internal use in the scheduler.
/// It delegates to the public Error type for actual error semantics.
#[cfg(feature = "async")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ViscaError(u8);

#[cfg(feature = "async")]
impl ViscaError {
    /// Create from VISCA error byte.
    pub fn from_byte(byte: u8) -> Self {
        ViscaError(byte)
    }

    /// Convert to VISCA error byte.
    pub fn as_byte(&self) -> u8 {
        self.0
    }

    /// Check if this error should trigger a retry.
    ///
    /// Delegates to the public Error type for consistency.
    pub fn is_retryable(&self, category: Option<CommandCategory>) -> bool {
        // Convert to public error type to check retryability
        let error = crate::Error::from_code(self.0);

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

/// Socket state tracking.
#[cfg(feature = "async")]
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

#[cfg(feature = "async")]
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
#[cfg(feature = "async")]
#[derive(Debug)]
pub(crate) struct Scheduler {
    /// Socket states.
    sockets: [SocketState; 2],
    /// Command ID generator.
    next_id: AtomicU32,
    /// Timeout configuration.
    timeout_config: TimeoutConfig,
    /// Minimum inter-command spacing.
    #[cfg(feature = "async")]
    command_spacing: Duration,
    /// Last command sent time.
    #[cfg(feature = "async")]
    last_command_time: Option<Instant>,
    /// Track response channels for commands by ID.
    command_channels: HashMap<u32, Sender<Result<ViscaResponse>>>,
    /// Track pending inquiries (ID, response channel, response type).
    pending_inquiries: Vec<(
        u32,
        Sender<Result<ViscaResponse>>,
        Option<crate::command::response::ViscaResponseType>,
    )>,
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
    command_queue: BinaryHeap<PriorityQueueItem>,
    /// Maximum retries per command category.
    max_retries_per_category: HashMap<CommandCategory, u32>,
    /// Runtime metrics.
    pub metrics: SchedulerMetrics,
}

/// Command waiting to be retried.
#[cfg(feature = "async")]
#[derive(Debug, Clone)]
pub(crate) struct RetryCommand {
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

/// Runtime metrics for monitoring scheduler performance.
#[cfg(feature = "async")]
#[derive(Debug, Default)]
pub(crate) struct SchedulerMetrics {
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

#[cfg(feature = "async")]
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
#[cfg(feature = "async")]
#[derive(Debug, Clone)]
struct PriorityQueueItem {
    /// The actual command/inquiry item.
    pub item: TxItem,
    /// Submission time for FIFO ordering within same priority.
    pub submitted_at: Instant,
}

#[cfg(feature = "async")]
impl PartialEq for PriorityQueueItem {
    fn eq(&self, other: &Self) -> bool {
        // Compare by priority and submission time
        self.priority() == other.priority() && self.submitted_at == other.submitted_at
    }
}

#[cfg(feature = "async")]
impl Eq for PriorityQueueItem {}

#[cfg(feature = "async")]
impl PartialOrd for PriorityQueueItem {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

#[cfg(feature = "async")]
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

#[cfg(feature = "async")]
impl PriorityQueueItem {
    /// Get the priority of this item.
    fn priority(&self) -> Priority {
        match &self.item {
            TxItem::Command { priority, .. } => *priority,
            TxItem::Inquiry { .. } => Priority::Normal, // Inquiries default to normal priority
            TxItem::Cancel { .. } => Priority::Critical, // Cancels have highest priority
            TxItem::CancelById { .. } => Priority::Critical, // Cancel by ID also has highest priority
        }
    }
}

#[cfg(feature = "async")]
impl Scheduler {
    /// Create a new scheduler with given channels.
    #[cfg(test)]
    pub fn new(_submit_rx: Receiver<TxItem>) -> Self {
        Self::with_timeout_config(_submit_rx, TimeoutConfig::default())
    }

    /// Create a new scheduler with custom timeout configuration.
    pub fn with_timeout_config(
        _submit_rx: Receiver<TxItem>,
        timeout_config: TimeoutConfig,
    ) -> Self {
        // Set default max retries per category
        let mut max_retries = HashMap::new();
        max_retries.insert(CommandCategory::Quick, 5); // Quick commands can retry more
        max_retries.insert(CommandCategory::Movement, 3); // Movement commands retry moderately
        max_retries.insert(CommandCategory::Preset, 3); // Preset commands retry moderately
        max_retries.insert(CommandCategory::Network, 2); // Network commands retry less
        max_retries.insert(CommandCategory::LongRunning, 1); // Long operations retry minimally
        max_retries.insert(CommandCategory::Custom, 3); // Custom commands use default

        Self {
            sockets: Default::default(),
            next_id: AtomicU32::new(1),
            timeout_config,
            #[cfg(feature = "async")]
            command_spacing: Duration::from_millis(50), // Default 50ms spacing
            #[cfg(feature = "async")]
            last_command_time: None,
            command_channels: HashMap::new(),
            pending_inquiries: Vec::new(),
            retry_queue: Vec::new(),
            command_metadata: HashMap::new(),
            retry_attempts: HashMap::new(),
            command_queue: BinaryHeap::new(),
            max_retries_per_category: max_retries,
            metrics: SchedulerMetrics::new(),
            pending_ack: HashMap::new(),
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
    #[cfg(test)]
    pub fn allocate_socket(
        &mut self,
        command_id: u32,
        category: CommandCategory,
        now: Instant,
    ) -> Option<ViscaSocket> {
        for (idx, socket) in self.sockets.iter_mut().enumerate() {
            if socket.free {
                socket.free = false;
                socket.command_id = Some(command_id);
                socket.started_at = Some(now);
                socket.category = Some(category);

                let socket_id = if idx == 0 {
                    ViscaSocket::S1
                } else {
                    ViscaSocket::S2
                };
                debug!("Allocated {socket_id:?} for command {command_id}");
                return Some(socket_id);
            }
        }
        None
    }

    /// Free a socket after command completion.
    pub fn free_socket(&mut self, socket: ViscaSocket) {
        let idx = socket.as_index();
        let state = &mut self.sockets[idx];

        if let Some(cmd_id) = state.command_id {
            debug!("Freeing {socket:?} from command {cmd_id}");
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
    pub fn socket_command(&self, socket: ViscaSocket) -> Option<u32> {
        self.sockets[socket.as_index()].command_id
    }

    /// Add a command to pending ACK list when sent.
    pub fn add_pending_ack(
        &mut self,
        id: u32,
        bytes: bytes::Bytes,
        priority: Priority,
        category: CommandCategory,
        now: Instant,
        camera_id: crate::camera_id::CameraId,
    ) {
        self.pending_ack
            .insert(id, (bytes, priority, category, now, camera_id));
        debug!("Added command {} to pending ACK list", id);
    }

    /// Handle ACK received - assign socket to command.
    pub fn handle_ack(&mut self, socket: ViscaSocket, now: Instant) -> Option<u32> {
        // Find oldest pending command (FIFO order for ACKs)
        let oldest_id = self
            .pending_ack
            .iter()
            .min_by_key(|(_, (_, _, _, sent_time, _))| *sent_time)
            .map(|(id, _)| *id)?;

        // Remove from pending and assign to socket
        if let Some((bytes, priority, category, _, camera_id)) = self.pending_ack.remove(&oldest_id)
        {
            // Now allocate the specific socket the camera assigned
            let idx = socket.as_index();
            let state = &mut self.sockets[idx];

            if !state.free {
                warn!(
                    "Camera assigned {:?} but it's already occupied by command {:?}",
                    socket, state.command_id
                );
                // This shouldn't happen with proper VISCA implementation
                return None;
            }

            state.free = false;
            state.command_id = Some(oldest_id);
            state.started_at = Some(now);
            state.category = Some(category);

            // Store metadata for potential retry
            self.store_command_metadata(oldest_id, bytes, priority, category, camera_id);

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

    /// Check if we can send another command (have room for pending ACK).
    /// VISCA cameras support max 2 concurrent commands.
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

    /// Get count of pending ACK commands.
    pub fn pending_ack_count(&self) -> usize {
        self.pending_ack.len()
    }

    /// Handle error for pending ACK commands, returning bytes for retry.
    /// When error arrives without socket (0x03 BufferFull), it applies to pending command.
    pub fn handle_pending_ack_error_with_bytes(
        &mut self,
        error: ViscaError,
    ) -> Option<(
        u32,
        Priority,
        CommandCategory,
        bytes::Bytes,
        crate::camera_id::CameraId,
    )> {
        // Find oldest pending command that would get this error
        let oldest = self
            .pending_ack
            .iter()
            .min_by_key(|(_, (_, _, _, sent_time, _))| *sent_time)
            .map(|(id, (bytes, priority, category, _, camera_id))| {
                (*id, *priority, *category, bytes.clone(), *camera_id)
            })?;

        // Remove from pending since it got an error
        self.pending_ack.remove(&oldest.0);

        debug!(
            "Removed command {} from pending ACK due to error {:?}",
            oldest.0, error
        );
        Some(oldest)
    }

    /// Enforce minimum command spacing.
    #[cfg(feature = "async")]
    pub async fn enforce_spacing_with<E: crate::executor::Executor>(
        &mut self,
        executor: &E,
        now: Instant,
    ) {
        if let Some(last_time) = self.last_command_time {
            let target = last_time + self.command_spacing;
            if now < target {
                let wait = target - now;
                trace!("Waiting {:?} for command spacing", wait);
                executor.sleep(wait).await;
            }
            self.last_command_time = Some(target);
        } else {
            self.last_command_time = Some(now);
        }
    }

    /// Check for timed out commands.
    pub fn check_timeouts(&mut self, now: Instant) -> Vec<(ViscaSocket, u32)> {
        let mut timed_out = Vec::new();

        for (idx, socket) in self.sockets.iter().enumerate() {
            if !socket.free {
                if let (Some(started), Some(category), Some(cmd_id)) =
                    (socket.started_at, socket.category, socket.command_id)
                {
                    let timeout = self.timeout_config.get_timeout(category);
                    if now.duration_since(started) > timeout {
                        let socket_id = if idx == 0 {
                            ViscaSocket::S1
                        } else {
                            ViscaSocket::S2
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

    /// Check for timed out pending ACK commands.
    /// Returns list of command IDs that have timed out while waiting for ACK.
    pub fn check_pending_ack_timeouts(&mut self, now: Instant) -> Vec<u32> {
        let mut timed_out = Vec::new();

        // Check each pending ACK command
        let mut to_remove = Vec::new();
        for (id, (_, _, _category, sent_time, _)) in self.pending_ack.iter() {
            // Use ACK timeout from configuration
            // According to VISCA spec, ACK should arrive within ~33ms
            // But we're generous to account for network delays
            let ack_timeout = self.timeout_config.ack_timeout;

            if now.duration_since(*sent_time) > ack_timeout {
                warn!(
                    "Command {} timed out waiting for ACK after {:?}",
                    id, ack_timeout
                );
                timed_out.push(*id);
                to_remove.push(*id);
            }
        }

        // Remove timed out commands from pending_ack
        for id in to_remove {
            self.pending_ack.remove(&id);
        }

        timed_out
    }

    /// Set maximum retries for a specific command category.
    #[cfg(test)]
    pub fn set_max_retries(&mut self, category: CommandCategory, max_retries: u32) {
        self.max_retries_per_category.insert(category, max_retries);
        debug!("Max retries for {category:?} set to {max_retries}");
    }

    /// Get the response channel for a command ID.
    ///
    /// Returns the channel if the command is still pending.
    pub fn get_response_channel(&mut self, cmd_id: u32) -> Option<Sender<Result<ViscaResponse>>> {
        self.command_channels.remove(&cmd_id)
    }

    /// Peek at the response channel for a command without removing it.
    ///
    /// Returns the channel if the command is still pending.
    pub fn peek_response_channel(&self, cmd_id: u32) -> Option<&Sender<Result<ViscaResponse>>> {
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
    pub fn store_command_channel(&mut self, cmd_id: u32, channel: Sender<Result<ViscaResponse>>) {
        self.command_channels.insert(cmd_id, channel);
    }

    /// Store command metadata for potential retry.
    ///
    /// This should be called when a command is sent.
    pub fn store_command_metadata(
        &mut self,
        cmd_id: u32,
        bytes: bytes::Bytes,
        priority: Priority,
        category: CommandCategory,
        camera_id: crate::camera_id::CameraId,
    ) {
        self.command_metadata
            .insert(cmd_id, (bytes, priority, category, camera_id));
    }

    /// Check if a command is in pending ACK state.
    ///
    /// Returns true if the command is waiting for ACK from the camera.
    pub fn is_pending_ack(&self, cmd_id: u32) -> bool {
        self.pending_ack.contains_key(&cmd_id)
    }

    /// Remove a command from pending ACK and get its data.
    ///
    /// Returns the command data if it was pending ACK.
    pub fn remove_pending_ack(
        &mut self,
        cmd_id: u32,
    ) -> Option<(
        bytes::Bytes,
        Priority,
        CommandCategory,
        Instant,
        crate::camera_id::CameraId,
    )> {
        self.pending_ack.remove(&cmd_id)
    }

    /// Get command metadata including camera ID.
    ///
    /// Returns the command metadata if it exists.
    pub fn get_command_metadata(
        &self,
        cmd_id: u32,
    ) -> Option<&(
        bytes::Bytes,
        Priority,
        CommandCategory,
        crate::camera_id::CameraId,
    )> {
        self.command_metadata.get(&cmd_id)
    }

    /// Remove command metadata.
    pub fn remove_command_metadata(&mut self, cmd_id: u32) {
        self.command_metadata.remove(&cmd_id);
    }

    /// Check if there is a pending inquiry without consuming it.
    ///
    /// Returns true if there's at least one pending inquiry.
    pub fn has_pending_inquiry(&self) -> bool {
        !self.pending_inquiries.is_empty()
    }

    /// Get the pending inquiry and its response channel.
    ///
    /// Returns the inquiry ID, response channel, and response type if there's a pending inquiry.
    /// This removes the inquiry from the pending list.
    pub fn get_pending_inquiry(
        &mut self,
    ) -> Option<(
        u32,
        Sender<Result<ViscaResponse>>,
        Option<crate::command::response::ViscaResponseType>,
    )> {
        self.pending_inquiries.pop()
    }

    /// Store a pending inquiry.
    ///
    /// This should be called when an inquiry is submitted.
    pub fn store_pending_inquiry(
        &mut self,
        id: u32,
        channel: Sender<Result<ViscaResponse>>,
        response_type: Option<crate::command::response::ViscaResponseType>,
    ) {
        self.pending_inquiries.push((id, channel, response_type));
    }

    /// Get the response type for a pending inquiry.
    ///
    /// Returns the response type if the inquiry is still pending.
    #[cfg(test)]
    pub fn get_inquiry_response_type(
        &self,
        id: u32,
    ) -> Option<crate::command::response::ViscaResponseType> {
        self.pending_inquiries
            .iter()
            .find(|(inquiry_id, _, _)| *inquiry_id == id)
            .and_then(|(_, _, response_type)| *response_type)
    }

    /// Add a command to the retry queue.
    ///
    /// This is called when a command receives a busy response.
    /// Returns true if the command was queued for retry, false if it has exhausted retries.
    pub fn queue_for_retry(
        &mut self,
        id: u32,
        bytes: bytes::Bytes,
        priority: Priority,
        category: CommandCategory,
        camera_id: crate::camera_id::CameraId,
        now: Instant,
    ) -> bool {
        // Track retry metrics
        self.metrics.retry_attempts.fetch_add(1, Ordering::Relaxed);
        let cat_idx = SchedulerMetrics::category_index(category);
        self.metrics.retry_by_category[cat_idx].fetch_add(1, Ordering::Relaxed);

        let max_retries = self
            .max_retries_per_category
            .get(&category)
            .copied()
            .unwrap_or(3);

        // Get or initialize the attempt count from our tracking map
        let attempt = self.retry_attempts.entry(id).or_insert(0);
        *attempt += 1;
        let current_attempt = *attempt;

        // Check if we've exhausted retries
        if current_attempt > max_retries {
            // Remove from retry queue if it exists
            if let Some(idx) = self.retry_queue.iter().position(|c| c.id == id) {
                self.retry_queue.swap_remove(idx);
            }

            self.metrics
                .retries_exhausted
                .fetch_add(1, Ordering::Relaxed);
            debug!(
                "Command {} exhausted retries after {} attempts (max: {})",
                id, current_attempt, max_retries
            );

            // Send error to the waiting command
            if let Some(response_tx) = self.get_response_channel(id) {
                let _ = response_tx.send(Err(crate::Error::MaxRetriesExceeded));
            }

            // Clean up command metadata and retry tracking
            self.command_metadata.remove(&id);
            self.retry_attempts.remove(&id);

            // Update retry queue depth metrics
            let new_depth = self.retry_queue.len() as u32;
            self.metrics.update_retry_queue_depth(new_depth);

            return false;
        }

        // Calculate exponential backoff: 100ms * 2^(attempt-1)
        let backoff_ms = 100 * (1 << (current_attempt - 1).min(5)); // Cap at 3.2 seconds
        let retry_at = now + Duration::from_millis(backoff_ms);

        // Check if this command is already in the retry queue
        if let Some(idx) = self.retry_queue.iter().position(|c| c.id == id) {
            // Update existing entry
            let cmd = &mut self.retry_queue[idx];
            cmd.attempt = current_attempt;
            cmd.retry_at = retry_at;
            debug!(
                "Command {} queued for retry attempt {} with {}ms backoff",
                id, current_attempt, backoff_ms
            );
        } else {
            // Add new retry command
            self.retry_queue.push(RetryCommand {
                id,
                bytes,
                priority,
                category,
                camera_id,
                attempt: current_attempt,
                max_retries,
                retry_at,
            });
            debug!(
                "Command {} queued for retry attempt {} with {}ms backoff (max retries: {})",
                id, current_attempt, backoff_ms, max_retries
            );
        }

        // Update retry queue depth metrics
        let new_depth = self.retry_queue.len() as u32;
        self.metrics.update_retry_queue_depth(new_depth);

        true
    }

    /// Get the next command to retry.
    ///
    /// Returns the highest priority command from the retry queue that is ready to be retried.
    /// Returns None if no commands are ready or if all ready commands have exhausted retries.
    pub fn get_next_retry(&mut self, now: Instant) -> Option<RetryCommand> {
        if self.retry_queue.is_empty() {
            return None;
        }

        // Find the highest priority command that is ready to retry and hasn't exhausted retries
        let mut best_idx = None;
        let mut best_priority = Priority::Low;
        let mut exhausted_commands = Vec::new();

        for (idx, cmd) in self.retry_queue.iter().enumerate() {
            // Skip commands that aren't ready yet
            if cmd.retry_at > now {
                continue;
            }

            // Check if this command has exhausted retries
            if cmd.attempt > cmd.max_retries {
                exhausted_commands.push(idx);
                continue;
            }

            // Select this command if it has higher priority
            if best_idx.is_none() || cmd.priority > best_priority {
                best_idx = Some(idx);
                best_priority = cmd.priority;
            }
        }

        // Remove exhausted commands from the retry queue (in reverse order to maintain indices)
        for idx in exhausted_commands.into_iter().rev() {
            let exhausted_cmd = self.retry_queue.swap_remove(idx);
            self.metrics
                .retries_exhausted
                .fetch_add(1, Ordering::Relaxed);
            debug!(
                "Command {} exhausted retries (attempt {} of {}), removing from retry queue",
                exhausted_cmd.id, exhausted_cmd.attempt, exhausted_cmd.max_retries
            );

            // Send error to the waiting command
            if let Some(response_tx) = self.get_response_channel(exhausted_cmd.id) {
                let _ = response_tx.send(Err(crate::Error::MaxRetriesExceeded));
            }

            // Clean up command metadata and retry tracking
            self.command_metadata.remove(&exhausted_cmd.id);
            self.retry_attempts.remove(&exhausted_cmd.id);
        }

        // Remove and return the selected command if found
        if let Some(idx) = best_idx {
            // Remove the command from the retry queue (single-shot retry)
            // It will be re-queued if it gets another BUSY response
            let cmd = self.retry_queue.swap_remove(idx);

            // Update retry queue depth metrics after removing the command
            let new_depth = self.retry_queue.len() as u32;
            self.metrics.update_retry_queue_depth(new_depth);

            Some(cmd)
        } else {
            // Update retry queue depth metrics after removing exhausted commands
            let new_depth = self.retry_queue.len() as u32;
            self.metrics.update_retry_queue_depth(new_depth);
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
    ) -> Option<(
        bytes::Bytes,
        Priority,
        CommandCategory,
        crate::camera_id::CameraId,
    )> {
        self.command_metadata.get(&cmd_id).cloned()
    }

    /// Enqueue a command/inquiry to the priority queue.
    ///
    /// Commands are sorted by priority and submission time.
    pub fn enqueue_command(&mut self, item: TxItem, now: Instant) {
        let queue_item = PriorityQueueItem {
            item,
            submitted_at: now,
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

    /// Peek at the priority of the next command in the queue without removing it.
    pub fn peek_queue_priority(&self) -> Option<Priority> {
        self.command_queue.peek().map(|item| item.priority())
    }

    /// Check if the scheduler is idle (no pending work).
    pub fn is_idle(&self) -> bool {
        self.is_queue_empty()
            && self.retry_queue.is_empty()
            && self.pending_inquiries.is_empty()
            && self.sockets.iter().all(|s| s.free)
    }

    /// Peek at the highest priority retry that is ready to be sent.
    pub fn peek_ready_retry_priority(&self, now: Instant) -> Option<Priority> {
        // Find the highest priority retry that is ready
        self.retry_queue
            .iter()
            .filter(|cmd| cmd.retry_at <= now)
            .map(|cmd| cmd.priority)
            .max()
    }

    /// Remove a command from the retry queue if it exists.
    ///
    /// This is called when a command completes successfully or when we receive
    /// a non-BUSY error for it.
    pub fn remove_from_retry_queue(&mut self, cmd_id: u32) {
        if let Some(idx) = self.retry_queue.iter().position(|c| c.id == cmd_id) {
            self.retry_queue.swap_remove(idx);
            debug!("Removed command {} from retry queue", cmd_id);

            // Update retry queue depth metrics
            let new_depth = self.retry_queue.len() as u32;
            self.metrics.update_retry_queue_depth(new_depth);
        }
        // Also clean up retry attempts tracking
        self.retry_attempts.remove(&cmd_id);
    }

    /// Get the earliest retry deadline from the retry queue.
    /// Returns None if the retry queue is empty.
    pub fn next_retry_deadline(&self) -> Option<Instant> {
        self.retry_queue.iter().map(|c| c.retry_at).min()
    }
}

#[cfg(all(test, feature = "async"))]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;

    #[test]
    fn test_can_send_command() {
        let (_tx, rx) = flume::unbounded();
        let mut scheduler = Scheduler::new(rx);
        let now = Instant::now();

        // Initially can send 2 commands
        assert!(scheduler.can_send_command());

        // Add first pending ACK
        scheduler.add_pending_ack(
            1,
            bytes::Bytes::from(vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]),
            Priority::Normal,
            CommandCategory::Quick,
            now,
            crate::camera_id::CameraId::CAMERA_1,
        );
        assert!(scheduler.can_send_command()); // Still room for 1 more

        // Add second pending ACK (with slightly later timestamp for deterministic ordering)
        scheduler.add_pending_ack(
            2,
            bytes::Bytes::from(vec![0x81, 0x01, 0x07, 0x00, 0x02, VISCA_TERMINATOR]),
            Priority::Normal,
            CommandCategory::Movement,
            now + Duration::from_nanos(1),
            crate::camera_id::CameraId::CAMERA_1,
        );
        assert!(!scheduler.can_send_command()); // Now at limit

        // Simulate ACK for first command - assigns socket 1
        let cmd_id = scheduler.handle_ack(ViscaSocket::S1, now).unwrap();
        assert_eq!(cmd_id, 1);
        assert!(!scheduler.can_send_command()); // Still at limit (1 pending + 1 allocated)

        // Simulate ACK for second command - assigns socket 2
        let cmd_id = scheduler.handle_ack(ViscaSocket::S2, now).unwrap();
        assert_eq!(cmd_id, 2);
        assert!(!scheduler.can_send_command()); // Still at limit (0 pending + 2 allocated)

        // Free socket 1
        scheduler.free_socket(ViscaSocket::S1);
        assert!(scheduler.can_send_command()); // Now have room for 1

        // Free socket 2
        scheduler.free_socket(ViscaSocket::S2);
        assert!(scheduler.can_send_command()); // Back to full capacity
    }

    #[test]
    fn test_socket_id_conversion() {
        assert_eq!(ViscaSocket::S1.as_index(), 0);
        assert_eq!(ViscaSocket::S2.as_index(), 1);
        assert_eq!(ViscaSocket::S1.as_protocol_byte(), 0x01);
        assert_eq!(ViscaSocket::S2.as_protocol_byte(), 0x02);

        assert_eq!(ViscaSocket::from_protocol_byte(0x01), Some(ViscaSocket::S1));
        assert_eq!(ViscaSocket::from_protocol_byte(0x02), Some(ViscaSocket::S2));
        assert_eq!(ViscaSocket::from_protocol_byte(0x41), Some(ViscaSocket::S1)); // With high nibble
        assert_eq!(ViscaSocket::from_protocol_byte(0x03), None);
    }

    #[test]
    fn test_visca_error_from_byte() {
        assert_eq!(ViscaError::from_byte(0x02).as_byte(), 0x02); // SyntaxError
        assert_eq!(ViscaError::from_byte(0x03).as_byte(), 0x03); // BufferFull
        assert_eq!(ViscaError::from_byte(0x04).as_byte(), 0x04); // CommandCancelled
        assert_eq!(ViscaError::from_byte(0x05).as_byte(), 0x05); // NoSocket
        assert_eq!(ViscaError::from_byte(0x41).as_byte(), 0x41); // NotExecutable
        assert_eq!(ViscaError::from_byte(0xFF).as_byte(), 0xFF); // Unknown
    }

    #[test]
    fn test_visca_error_mapping_table() {
        // Table-driven test for ViscaError byte mapping
        // Ensures consistency between from_byte and as_byte
        let cases = [
            (0x02, "SyntaxError"),
            (0x03, "BufferFull"),
            (0x04, "CommandCancelled"),
            (0x05, "NoSocket"),
            // VISCA 0x41 is "Command Not Executable" - command invalid in current state
            (0x41, "NotExecutable"),
        ];

        for (byte, name) in cases {
            let error = ViscaError::from_byte(byte);
            let back_to_byte = error.as_byte();
            assert_eq!(
                back_to_byte, byte,
                "Round-trip failed for {:#04x} ({}) -> {:#04x}",
                byte, name, back_to_byte
            );
        }
    }

    #[test]
    fn test_scheduler_socket_allocation() {
        let (_submit_tx, submit_rx) = flume::unbounded();
        let mut scheduler = Scheduler::new(submit_rx);

        let now = Instant::now();

        // Both sockets should be free initially
        assert!(scheduler.has_free_socket());

        // Allocate first socket
        let socket1 = scheduler.allocate_socket(1, CommandCategory::Movement, now);
        assert_eq!(socket1, Some(ViscaSocket::S1));
        assert!(scheduler.has_free_socket());

        // Allocate second socket
        let socket2 = scheduler.allocate_socket(2, CommandCategory::Quick, now);
        assert_eq!(socket2, Some(ViscaSocket::S2));
        assert!(!scheduler.has_free_socket());

        // Try to allocate when none free
        let socket3 = scheduler.allocate_socket(3, CommandCategory::Network, now);
        assert_eq!(socket3, None);

        // Free a socket
        scheduler.free_socket(ViscaSocket::S1);
        assert!(scheduler.has_free_socket());

        // Can allocate again
        let socket4 = scheduler.allocate_socket(4, CommandCategory::Movement, now);
        assert_eq!(socket4, Some(ViscaSocket::S1));
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
        let mut scheduler = Scheduler::new(submit_rx);

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
        let response_type = Some(crate::command::response::ViscaResponseType::Power);
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
        let mut scheduler = Scheduler::new(submit_rx);

        // Allocate a socket for a command
        let cmd_id = 123;
        let now = Instant::now();
        let socket = scheduler
            .allocate_socket(cmd_id, CommandCategory::Movement, now)
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

    #[test]
    fn test_retry_exhaustion() {
        let (_submit_tx, submit_rx) = flume::unbounded();
        let mut scheduler = Scheduler::new(submit_rx);

        // Set max retries to 2 for Quick commands
        scheduler.set_max_retries(CommandCategory::Quick, 2);

        let cmd_id = 456;
        let bytes = bytes::Bytes::from(vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]);
        let priority = Priority::Normal;
        let category = CommandCategory::Quick;

        // Store response channel
        let (response_tx, response_rx) = flume::bounded(1);
        scheduler.store_command_channel(cmd_id, response_tx);
        scheduler.store_command_metadata(
            cmd_id,
            bytes.clone(),
            priority,
            category,
            crate::camera_id::CameraId::CAMERA_1,
        );

        // First retry (attempt 1)
        let now = Instant::now();
        let queued = scheduler.queue_for_retry(
            cmd_id,
            bytes.clone(),
            priority,
            category,
            crate::camera_id::CameraId::CAMERA_1,
            now,
        );
        assert!(queued, "First retry should be queued");
        assert_eq!(scheduler.retry_queue.len(), 1);

        // Second retry (attempt 2)
        let queued = scheduler.queue_for_retry(
            cmd_id,
            bytes.clone(),
            priority,
            category,
            crate::camera_id::CameraId::CAMERA_1,
            now,
        );
        assert!(queued, "Second retry should be queued");
        assert_eq!(scheduler.retry_queue.len(), 1); // Still 1, same command

        // Third retry (attempt 3) - should exceed max retries of 2
        let queued = scheduler.queue_for_retry(
            cmd_id,
            bytes.clone(),
            priority,
            category,
            crate::camera_id::CameraId::CAMERA_1,
            now,
        );
        assert!(!queued, "Third retry should NOT be queued (exhausted)");
        assert_eq!(scheduler.retry_queue.len(), 0); // Should be removed

        // Check that error was sent
        let result = response_rx.try_recv();
        assert!(result.is_ok(), "Should have received response");
        assert!(matches!(
            result.unwrap(),
            Err(crate::Error::MaxRetriesExceeded)
        ));
    }
}
