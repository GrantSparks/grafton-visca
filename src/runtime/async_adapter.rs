//! Async adapter for the runtime-agnostic scheduler core.
//!
//! This module provides an async wrapper around SchedulerCore, delegating all
//! state management to the core while handling async I/O and futures.

use flume::Sender;
use tracing::{debug, warn};

use std::{collections::HashMap, sync::Arc, time::Instant};

use crate::{
    camera_id::CameraId,
    capabilities::Profile,
    command::response::{lift_inquiry_for, ViscaResponse, ViscaResponseType},
    error::{Error, Result},
    executor::Executor,
    protocol::response::{decode_basic, BasicKind},
    runtime::core::{
        PendingCommand, Priority, RetryCommand, SchedulerAction, SchedulerCore, SchedulerEvent,
        TimeoutKind,
    },
    timeout::{CommandCategory, TimeoutConfig},
    transport::RetryConfig,
    visca_socket::ViscaSocket,
};

/// Represents an item to be transmitted (command, inquiry, or cancel).
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
        camera_id: CameraId,
        /// Channel to send response back.
        response_tx: Sender<Result<ViscaResponse>>,
    },
    /// An inquiry that doesn't require a socket, expects DataReply.
    Inquiry {
        /// Unique identifier for this inquiry.
        id: u32,
        /// Raw VISCA bytes to send.
        bytes: bytes::Bytes,
        /// Category for timeout calculation.
        category: CommandCategory,
        /// Camera ID used to encode the inquiry.
        camera_id: CameraId,
        /// Expected response type for parsing DataReply.
        response_type: Option<ViscaResponseType>,
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

/// Metrics summary for async runtime.
#[derive(Debug, Clone, Copy, Default)]
pub struct MetricsSummary {
    /// Total number of commands sent.
    pub commands_sent: u64,
    /// Total number of commands completed successfully.
    pub commands_completed: u64,
    /// Total number of commands that failed.
    pub commands_failed: u64,
    /// Total number of commands that were retried.
    pub commands_retried: u64,
    /// Total number of busy errors received.
    pub busy_errors: u64,
    /// Total number of network errors.
    pub network_errors: u64,
    /// Total number of protocol errors.
    pub protocol_errors: u64,
    /// Total number of timeouts.
    pub timeouts: u64,
    /// Current depth of the pending command queue.
    pub pending_queue_depth: usize,
    /// Current depth of the retry queue.
    pub retry_queue_depth: usize,
}

/// Event emitted when a command completes.
///
/// Used for event-driven movement detection to observe when commands
/// in specific categories (e.g., Movement, Preset) have completed.
#[derive(Debug, Clone, Copy)]
pub struct CompletionEvent {
    /// The camera that completed the command.
    pub camera_id: CameraId,
    /// The category of the completed command.
    pub category: CommandCategory,
    /// When the completion occurred.
    pub when: Instant,
}

/// Async adapter wrapping the scheduler core.
pub(crate) struct AsyncAdapter<P: Profile, E: Executor> {
    /// The scheduler core for state management.
    core: SchedulerCore,
    /// Executor for time and async operations.
    executor: Arc<E>,
    /// Response channels for commands.
    response_channels: HashMap<u32, Sender<Result<ViscaResponse>>>,
    /// Metrics tracking.
    metrics: Metrics,
    /// Completion event subscribers.
    completion_subscribers: Vec<Sender<CompletionEvent>>,
    /// Profile marker (zero-sized type).
    _profile: std::marker::PhantomData<P>,
}

#[derive(Debug, Default)]
struct Metrics {
    commands_sent: u64,
    commands_completed: u64,
    commands_failed: u64,
    commands_retried: u64,
    busy_errors: u64,
    network_errors: u64,
    protocol_errors: u64,
    timeouts: u64,
}

impl<P: Profile, E: Executor> AsyncAdapter<P, E> {
    /// Create a new async adapter.
    pub fn new(timeout_config: TimeoutConfig, retry_config: RetryConfig, executor: Arc<E>) -> Self {
        Self {
            core: SchedulerCore::with_retry_config(timeout_config, retry_config),
            executor,
            response_channels: HashMap::new(),
            metrics: Metrics::default(),
            completion_subscribers: Vec::new(),
            _profile: std::marker::PhantomData,
        }
    }

    /// Submit a command or inquiry to the scheduler.
    pub fn submit(&mut self, item: TxItem) -> u32 {
        match item {
            TxItem::Command {
                id,
                bytes,
                priority,
                category,
                camera_id,
                response_tx,
            } => {
                // Store response channel
                self.response_channels.insert(id, response_tx);

                if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                    eprintln!(
                        "[AsyncAdapter] Submitted command id={}, stored response channel",
                        id
                    );
                }

                // Queue command in core
                let now = self.executor.now();
                let pending_cmd = PendingCommand {
                    id,
                    bytes,
                    priority,
                    category,
                    camera_id,
                    submitted_at: now,
                    kind: crate::command::CommandKind::Command,
                };
                self.core.queue_command(pending_cmd);

                self.metrics.commands_sent += 1;

                id
            }
            TxItem::Inquiry {
                id,
                bytes,
                category,
                camera_id,
                response_type,
                response_tx,
            } => {
                // Store response channel
                self.response_channels.insert(id, response_tx);

                // Store response type in core if present
                if let Some(rt) = response_type {
                    self.core.register_inquiry_type(id, rt);
                }

                // Queue inquiry in core (same as commands but with Quick priority)
                let now = self.executor.now();
                let pending_cmd = PendingCommand {
                    id,
                    bytes,
                    priority: Priority::Normal, // Inquiries use normal priority
                    category,
                    camera_id,
                    submitted_at: now,
                    kind: crate::command::CommandKind::Inquiry,
                };
                self.core.queue_command(pending_cmd);

                self.metrics.commands_sent += 1;

                id
            }
            TxItem::Cancel { .. } | TxItem::CancelById { .. } => {
                // Cancel operations are handled separately
                0
            }
        }
    }

    /// Get the next command to send.
    pub fn next_command_to_send(&mut self) -> Option<PendingCommand> {
        self.core.next_command_to_send()
    }

    /// Register that a command was sent and is pending ACK.
    pub fn register_pending_ack(&mut self, cmd: &PendingCommand) {
        let now = self.executor.now();
        self.core.register_pending_ack(
            cmd.id,
            cmd.bytes.clone(),
            cmd.priority,
            cmd.category,
            cmd.camera_id,
            now,
        );
    }

    /// Start tracking an inquiry (no socket allocation).
    pub fn start_inquiry(&mut self, cmd: &PendingCommand) {
        let now = self.executor.now();
        self.core.start_inquiry(
            cmd.id,
            cmd.bytes.clone(),
            cmd.priority,
            cmd.category,
            cmd.camera_id,
            now,
        );
    }

    /// Register a Sony sequence number for a command.
    ///
    /// Should only be called after a successful send.
    pub fn register_sequence(&mut self, cmd_id: u32, sequence: u32) {
        debug_assert!(
            self.core.is_command_pending(cmd_id),
            "register_sequence called for non-pending command {}",
            cmd_id
        );
        self.core.register_sequence(cmd_id, sequence);
    }

    /// Unregister a pending ACK (used for rollback on send failure).
    pub fn unregister_pending_ack(&mut self, id: u32) -> bool {
        self.core.unregister_pending_ack(id)
    }

    /// Free a reserved socket (used for rollback on inquiry send failure).
    pub fn free_socket(&mut self, socket: ViscaSocket) {
        self.core.free_socket(socket);
    }

    /// Handle a send failure - fails immediately with transport error.
    pub fn fail_after_send_error(&mut self, id: u32) {
        if let Some(SchedulerAction::CommandFailed {
            id: failed_id,
            error,
        }) = self.core.fail_after_send_error(id)
        {
            // Send failure to waiting future
            if let Some(tx) = self.response_channels.remove(&failed_id) {
                let _ = tx.send(Err(error));
            }
        }
    }

    /// Schedule a retry after a send error instead of failing immediately.
    /// This routes send failures through the existing scheduler retry pipeline.
    pub fn schedule_retry_after_send_error(&mut self, id: u32) {
        let now = self.executor.now();

        // Mark this retry as triggered by transport error so the final error
        // (after budget exhaustion) is classified correctly
        self.core.mark_retry_as_transport_error(id);

        // Queue retry for the command using existing metadata
        if let Some(action) = self.core.queue_retry_for_command(id, now) {
            // Handle the retry action (which could be RetryCommand or CommandFailed)
            match action {
                SchedulerAction::RetryCommand { id, delay, .. } => {
                    self.metrics.commands_retried += 1;
                    debug!(
                        "Scheduled retry for command {} after send error, delay: {:?}",
                        id, delay
                    );
                    // The retry will be picked up by get_ready_retries()
                    // Keep the response channel open so the future remains pending
                }
                SchedulerAction::CommandFailed {
                    id: failed_id,
                    error,
                } => {
                    // Budget exhausted, fail the command
                    self.metrics.commands_failed += 1;
                    if let Some(tx) = self.response_channels.remove(&failed_id) {
                        let _ = tx.send(Err(error));
                    }
                }
                _ => {
                    // Unexpected action type
                    warn!(
                        "Unexpected action from queue_retry_for_command: {:?}",
                        action
                    );
                }
            }
        } else {
            // No metadata found for command, fall back to immediate failure
            warn!(
                "No metadata found for command {} during send retry, failing immediately",
                id
            );
            self.fail_after_send_error(id);
        }
    }

    /// Process a received VISCA response.
    pub async fn process_response(&mut self, payload: &[u8], sequence: Option<u32>) -> Result<()> {
        // Parse VISCA response type using decode_basic
        let basic = decode_basic(payload).ok_or_else(|| Error::InvalidResponse {
            expected: std::borrow::Cow::Borrowed("Valid VISCA response"),
            actual: payload.to_vec(),
        })?;

        let now = self.executor.now();

        // Map to scheduler event
        let event = match basic.kind {
            BasicKind::Ack => {
                // For Sony, try to use sequence to find command
                let cmd_id = sequence.and_then(|seq| self.core.get_command_by_sequence(seq));
                SchedulerEvent::Ack {
                    socket: basic.socket.unwrap_or(ViscaSocket::S1),
                    cmd_id,
                }
            }
            BasicKind::Completion => {
                // For Sony, try to use sequence to find command
                let cmd_id = sequence.and_then(|seq| self.core.get_command_by_sequence(seq));

                // Get the expected response type from core
                let response_type = cmd_id.and_then(|id| self.core.get_inquiry_type(id));

                let response = lift_inquiry_for::<P>(&basic, response_type)?;
                SchedulerEvent::Completion {
                    socket: basic.socket,
                    cmd_id,
                    response,
                }
            }
            BasicKind::Error(code) => {
                // Track error types
                match code {
                    0x03 | 0x04 => self.metrics.busy_errors += 1,
                    _ => self.metrics.protocol_errors += 1,
                }

                // For Sony, try to use sequence to find command
                let mut cmd_id = sequence.and_then(|seq| self.core.get_command_by_sequence(seq));

                // If no cmd_id and no socket, this could be an inquiry error
                // Use resolve_inquiry_id to try to match it
                if cmd_id.is_none() && basic.socket.is_none() {
                    // For error responses, we can't use content-based matching on the error code,
                    // but we can use FIFO from the inquiry queue
                    cmd_id = self.core.resolve_inquiry_id(&[], sequence);
                }

                SchedulerEvent::Error {
                    socket: basic.socket,
                    cmd_id,
                    code,
                }
            }
            BasicKind::DataReply => {
                // Data replies are completions for inquiries
                // Use the core's centralized resolution
                let cmd_id = self
                    .core
                    .resolve_inquiry_id(basic.payload.as_slice(), sequence);

                // Get the expected response type from core
                let response_type = cmd_id.and_then(|id| self.core.get_inquiry_type(id));

                let response = lift_inquiry_for::<P>(&basic, response_type)?;

                // Use InquiryReply event for data replies
                SchedulerEvent::InquiryReply { cmd_id, response }
            }
            BasicKind::NetworkChange | BasicKind::Unknown => {
                // Ignore these for now
                return Ok(());
            }
        };

        // Process event through core and handle actions
        let actions = self.core.process_event(event, now);
        for action in actions {
            self.handle_action(action).await?;
        }

        Ok(())
    }

    /// Handle a scheduler action.
    async fn handle_action(&mut self, action: SchedulerAction) -> Result<()> {
        match action {
            SchedulerAction::SendCommand { .. } => {
                // This shouldn't happen from process_event
                warn!("Unexpected SendCommand action from process_event");
            }
            SchedulerAction::CommandComplete {
                id,
                category,
                camera_id,
                response,
            } => {
                self.metrics.commands_completed += 1;

                // Core handles all inquiry cleanup now

                // Send response to the waiting command
                if let Some(tx) = self.response_channels.remove(&id) {
                    let _ = tx.send_async(Ok(response)).await;
                }

                // Broadcast completion event to all subscribers
                if !self.completion_subscribers.is_empty() {
                    let event = CompletionEvent {
                        camera_id,
                        category,
                        when: Instant::now(),
                    };

                    // Remove any disconnected subscribers while broadcasting
                    self.completion_subscribers
                        .retain(|tx| tx.try_send(event).is_ok());
                }
            }
            SchedulerAction::CommandFailed { id, error } => {
                self.metrics.commands_failed += 1;

                // Core handles all inquiry cleanup now

                if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                    eprintln!(
                        "[AsyncAdapter] CommandFailed action for id={}, error={:?}, has_channel={}",
                        id,
                        error,
                        self.response_channels.contains_key(&id)
                    );
                }

                if let Some(tx) = self.response_channels.remove(&id) {
                    if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                        eprintln!(
                            "[AsyncAdapter] Sending error to response channel for id={}",
                            id
                        );
                    }
                    let _ = tx.send_async(Err(error)).await;
                } else if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                    eprintln!(
                        "[AsyncAdapter] No response channel found for failed command id={}",
                        id
                    );
                }
            }
            SchedulerAction::Timeout {
                id,
                kind: TimeoutKind::Ack,
                will_retry,
                ..
            } => {
                // Only notify the waiting future if no retries are left
                if !will_retry {
                    self.notify_timeout(id);
                } else if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                    eprintln!(
                        "[AsyncAdapter] Timeout for id={} but will retry, keeping future pending",
                        id
                    );
                }
            }
            SchedulerAction::RetryCommand {
                id,
                bytes: _,
                delay,
            } => {
                self.metrics.commands_retried += 1;

                debug!("Scheduling retry for command {} after {:?}", id, delay);
                // The retry will be picked up by get_ready_retries()
            }
        }
        Ok(())
    }

    /// Notify a waiting future that its command has timed out.
    #[inline]
    fn notify_timeout(&mut self, id: u32) {
        // Remove first to avoid double-notify races
        if let Some(tx) = self.response_channels.remove(&id) {
            // Complete the user's future deterministically
            let _ = tx.try_send(Err(Error::Timeout));
            if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                eprintln!(
                    "[AsyncAdapter] Timeout: completed future with Error::Timeout (id={})",
                    id
                );
            }
        } else {
            // No waiter: either already completed/cleaned up or late event
            if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                eprintln!(
                    "[AsyncAdapter] Timeout for id={} but no waiter; ignoring",
                    id
                );
            }
        }
    }

    /// Check for timeouts and return commands that need action.
    pub async fn check_timeouts(&mut self) -> Result<()> {
        let now = self.executor.now();
        let actions = self.core.check_timeouts(now);

        for action in &actions {
            if matches!(action, SchedulerAction::CommandFailed { .. }) {
                self.metrics.timeouts += 1;
            }
        }

        for action in actions {
            self.handle_action(action).await?;
        }
        Ok(())
    }

    /// Get retries that are ready to send.
    pub fn get_ready_retries(&mut self) -> Vec<RetryCommand> {
        let now = self.executor.now();
        self.core.get_ready_retries(now)
    }

    /// Check if we can send another command.
    pub fn can_send_command(&self) -> bool {
        self.core.can_send_command()
    }

    /// Get the count of commands waiting for ACK.
    pub fn pending_ack_count(&self) -> usize {
        self.core.pending_ack_count()
    }

    /// Get metrics summary.
    pub fn metrics_summary(&self) -> MetricsSummary {
        MetricsSummary {
            commands_sent: self.metrics.commands_sent,
            commands_completed: self.metrics.commands_completed,
            commands_failed: self.metrics.commands_failed,
            commands_retried: self.metrics.commands_retried,
            busy_errors: self.metrics.busy_errors,
            network_errors: self.metrics.network_errors,
            protocol_errors: self.metrics.protocol_errors,
            timeouts: self.metrics.timeouts,
            pending_queue_depth: 0, // Can be obtained from core if needed
            retry_queue_depth: self.core.retry_queue.len(),
        }
    }

    /// Find the socket for a given command ID.
    pub fn socket_for_command(&self, id: u32) -> Option<ViscaSocket> {
        self.core.find_socket_for_command(id)
    }

    /// Get the camera ID for a command by its ID.
    pub fn camera_id_for_command(&self, id: u32) -> Option<CameraId> {
        self.core.camera_id_for_command(id)
    }

    /// Get the camera ID for the command currently on a socket.
    pub fn camera_id_for_socket(&self, socket: ViscaSocket) -> Option<CameraId> {
        self.core
            .find_command_on_socket(socket)
            .and_then(|id| self.core.camera_id_for_command(id))
    }

    /// Handle a network error event.
    ///
    /// This mirrors the blocking runner's network error handling,
    /// causing all inflight commands to be queued for retry.
    pub async fn on_network_error(&mut self, error: Error) -> Result<()> {
        let now = self.executor.now();
        self.metrics.network_errors += 1;

        let actions = self
            .core
            .process_event(SchedulerEvent::NetworkError(error), now);
        for action in actions {
            self.handle_action(action).await?;
        }
        Ok(())
    }

    /// Add a completion event subscriber.
    ///
    /// Returns the receiver end of the channel for receiving completion events.
    pub fn subscribe_completions(&mut self) -> flume::Receiver<CompletionEvent> {
        let (tx, rx) = flume::unbounded();
        self.completion_subscribers.push(tx);
        rx
    }
}
