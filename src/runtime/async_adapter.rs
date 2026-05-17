//! Async adapter for the runtime-agnostic scheduler core.
//!
//! This module provides an async wrapper around SchedulerCore, delegating all
//! state management to the core while handling async I/O and futures.

use flume::{Sender, TrySendError};
use tracing::{debug, trace};

use std::{collections::HashMap, collections::VecDeque, sync::Arc, time::Instant};

use crate::{
    camera::CommandId,
    camera_id::CameraId,
    capabilities::Profile,
    command::response::Response,
    error::{Error, Result},
    executor::Executor,
    runtime::core::{
        CancelOutcome, PendingCommand, Priority, RetryCommand, SchedulerAction, SchedulerCore,
        SchedulerEvent, TimeoutKind,
    },
    runtime::driver::{receive_one, IgnoreReason, ReceiveDisposition},
    timeout::{CommandCategory, TimeoutConfig},
    transport::RetryConfig,
    visca_socket::ViscaSocket,
};

fn log_ignored_receive(reason: IgnoreReason, sequence: Option<u32>) {
    trace!(?reason, ?sequence, "VISCA response ignored");
}

/// Data-plane request submitted across the handle-to-runtime boundary.
///
/// # Metadata Consolidation
///
/// The `command` field contains an `Arc<EncodedCommand>` which stores all
/// metadata needed for scheduling and timeout handling:
/// - `category`: Timeout category (via `command.category`)
/// - inquiry response routing metadata (via `command.behavior`)
/// - command vs inquiry behavior (via `command.behavior`)
///
/// This eliminates redundant storage that previously existed in both submit messages
/// and `EncodedCommand`, making `EncodedCommand` the single source of truth.
#[derive(Clone)]
pub(crate) enum SubmitRequest {
    /// A command that requires a socket and expects ACK/Completion.
    Command {
        /// Unique identifier for this command.
        id: CommandId,
        /// The pre-encoded command to send (contains category and kind).
        command: Arc<crate::command::encode::EncodedCommand>,
        /// Priority level for scheduling.
        priority: Priority,
        /// Camera ID used to encode the command.
        camera_id: CameraId,
        /// Admission reply sent once the runtime loop has accepted or rejected the command.
        admission_tx: Sender<Result<()>>,
        /// Channel to send response back.
        response_tx: Sender<Result<Response>>,
    },
    /// An inquiry that doesn't require a socket, expects DataReply.
    Inquiry {
        /// Unique identifier for this inquiry.
        id: CommandId,
        /// The pre-encoded command to send (contains category and behavior).
        command: Arc<crate::command::encode::EncodedCommand>,
        /// Camera ID used to encode the inquiry.
        camera_id: CameraId,
        /// Admission reply sent once the runtime loop has accepted or rejected the inquiry.
        admission_tx: Sender<Result<()>>,
        /// Channel to send response back.
        response_tx: Sender<Result<Response>>,
    },
}

impl SubmitRequest {
    /// Reply to a still-unprocessed request with the supplied failure.
    pub(crate) fn fail(self, error: Error) {
        match self {
            SubmitRequest::Command {
                admission_tx,
                response_tx,
                ..
            }
            | SubmitRequest::Inquiry {
                admission_tx,
                response_tx,
                ..
            } => {
                let _ = admission_tx.send(Err(error.clone()));
                let _ = response_tx.send(Err(error));
            }
        }
    }
}

impl std::fmt::Debug for SubmitRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubmitRequest::Command {
                id,
                command,
                priority,
                camera_id,
                ..
            } => f
                .debug_struct("SubmitRequest::Command")
                .field("id", id)
                .field("priority", priority)
                .field("category", &command.category)
                .field("camera_id", camera_id)
                .finish(),
            SubmitRequest::Inquiry {
                id,
                command,
                camera_id,
                ..
            } => f
                .debug_struct("SubmitRequest::Inquiry")
                .field("id", id)
                .field("category", &command.category)
                .field("camera_id", camera_id)
                .field("behavior", &command.behavior)
                .finish(),
        }
    }
}

/// High-priority control-plane requests.
///
/// These requests are independent of data-plane backpressure and are selected
/// before normal observability/subscription traffic in the runtime loop.
pub(crate) enum UrgentControlRequest {
    /// Cancel a command by its runtime-assigned command ID.
    CancelById {
        /// Camera ID supplied by the caller. The scheduler-owned command state
        /// remains authoritative once the command is known.
        camera_id: CameraId,
        /// Command ID to cancel.
        id: CommandId,
        /// Reply sent after the runtime deliberately processes the request.
        reply_tx: Sender<Result<()>>,
    },
    /// Cancel a specific VISCA socket directly.
    CancelSocket {
        /// Camera ID for addressing the cancel message.
        camera_id: CameraId,
        /// Socket to cancel.
        socket: ViscaSocket,
        /// Reply sent after the runtime deliberately handles the request.
        reply_tx: Sender<Result<()>>,
    },
    /// Begin explicit runtime shutdown.
    Shutdown {
        /// Reply sent after shutdown cleanup has completed.
        reply_tx: Sender<Result<()>>,
    },
}

impl UrgentControlRequest {
    /// Fail a queued urgent request during shutdown cleanup.
    pub(crate) fn fail(self, error: Error) {
        match self {
            UrgentControlRequest::CancelById { reply_tx, .. }
            | UrgentControlRequest::CancelSocket { reply_tx, .. }
            | UrgentControlRequest::Shutdown { reply_tx } => {
                let _ = reply_tx.send(Err(error));
            }
        }
    }
}

/// Normal-priority control-plane requests for observability and subscriptions.
pub(crate) enum ControlRequest {
    /// Request a runtime metrics snapshot.
    #[cfg(feature = "test-utils")]
    Metrics {
        /// Reply channel for the metrics snapshot.
        reply_tx: Sender<Result<MetricsSummary>>,
    },
    /// Request a completion-event subscription.
    SubscribeCompletions {
        /// Reply channel for the new subscription receiver.
        reply_tx: Sender<Result<flume::Receiver<CompletionEvent>>>,
    },
}

impl ControlRequest {
    /// Fail a queued normal control request during shutdown cleanup.
    pub(crate) fn fail(self, error: Error) {
        match self {
            #[cfg(feature = "test-utils")]
            ControlRequest::Metrics { reply_tx } => {
                let _ = reply_tx.send(Err(error));
            }
            ControlRequest::SubscribeCompletions { reply_tx } => {
                let _ = reply_tx.send(Err(error));
            }
        }
    }
}

/// Metrics summary for async runtime.
#[cfg(feature = "test-utils")]
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
    /// Total number of ignored unmatched sequenced replies.
    ///
    /// Tracks replies with a Sony sequence number that did not resolve to an active
    /// command. These are stale/duplicate packets that were correctly ignored.
    pub ignored_unmatched_sequenced_replies: u64,
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
}

/// Buffer size for per-subscriber completion event channels.
///
/// This bounds the maximum number of events that can be buffered for each
/// subscriber. When full, new events are dropped for that subscriber rather
/// than causing unbounded memory growth or blocking the runtime loop.
///
/// A buffer of 256 events provides reasonable headroom for transient consumer
/// lag while preventing memory issues from slow/forgotten subscribers.
const COMPLETIONS_BUFFER: usize = 256;

/// Async adapter wrapping the scheduler core.
pub(crate) struct AsyncAdapter<P: Profile, E: Executor> {
    /// The scheduler core for state management.
    core: SchedulerCore,
    /// Executor for time and async operations.
    executor: Arc<E>,
    /// Response channels for commands, keyed by CommandId for type safety.
    response_channels: HashMap<CommandId, Sender<Result<Response>>>,
    /// Metrics tracking.
    metrics: Metrics,
    /// Completion event subscribers.
    completion_subscribers: Vec<Sender<CompletionEvent>>,
    /// Maximum pending queue depth for admission control.
    max_pending_queue_depth: usize,
    /// Cancel outbox: queued cancel commands to be sent by loop_task.
    ///
    /// Populated when `SchedulerAction::SendCancel` is emitted (cancel-on-ACK).
    /// Drained by `loop_task` to send cancel frames to the transport.
    cancel_outbox: VecDeque<(CameraId, ViscaSocket)>,
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
    pub fn new(
        timeout_config: TimeoutConfig,
        retry_config: RetryConfig,
        executor: Arc<E>,
        max_pending_queue_depth: usize,
    ) -> Self {
        Self {
            core: SchedulerCore::with_retry_config(timeout_config, retry_config),
            executor,
            response_channels: HashMap::new(),
            metrics: Metrics::default(),
            completion_subscribers: Vec::new(),
            max_pending_queue_depth,
            cancel_outbox: VecDeque::new(),
            _profile: std::marker::PhantomData,
        }
    }

    /// Set the maximum number of concurrent inquiries.
    ///
    /// For envelopes without sequence correlation (Raw VISCA), this should be 1
    /// to ensure responses can be reliably matched to requests. For envelopes
    /// with sequence correlation (Sony Encapsulated), higher values enable
    /// concurrent inquiry execution.
    pub fn set_max_inquiries_inflight(&mut self, max: usize) {
        self.core.set_max_inquiries_inflight(max);
    }

    /// Set the minimum spacing between consecutive inquiry sends.
    ///
    /// Some cameras (e.g., PTZOptics) cannot process inquiries faster than
    /// ~125-150ms apart. Setting this enforces a minimum delay between sends.
    pub fn set_min_inquiry_spacing(&mut self, spacing: std::time::Duration) {
        self.core.set_min_inquiry_spacing(spacing);
    }

    /// Set the minimum spacing between consecutive sends of any kind.
    ///
    /// Prevents firmware buffer overflow on cameras that cannot process
    /// commands at wire speed. Applies to all sends (commands and inquiries).
    pub fn set_min_command_spacing(&mut self, spacing: std::time::Duration) {
        self.core.set_min_command_spacing(spacing);
    }

    /// Admit a command or inquiry into runtime scheduler state.
    ///
    /// If the pending queue is at capacity, the request is rejected with
    /// [`Error::RuntimeQueueFull`] on the admission channel. Admission success
    /// means the command ID is now represented in scheduler state and can be
    /// cancelled by ID even before any VISCA bytes are sent.
    pub fn admit_submit(&mut self, request: SubmitRequest) {
        match request {
            SubmitRequest::Command {
                id,
                command,
                priority,
                camera_id,
                admission_tx,
                response_tx,
            } => {
                // Admission control: reject if at capacity
                if self.core.pending_queue_depth() >= self.max_pending_queue_depth {
                    debug!(
                        %id,
                        capacity = self.max_pending_queue_depth,
                        "Rejecting command: queue at capacity"
                    );
                    let error = Error::RuntimeQueueFull {
                        capacity: self.max_pending_queue_depth,
                    };
                    let _ = admission_tx.send(Err(error.clone()));
                    let _ = response_tx.send(Err(error));
                    self.metrics.commands_failed += 1;
                    return;
                }

                self.response_channels.insert(id, response_tx);

                // Queue command in core (category and kind are derived from EncodedCommand)
                let now = self.executor.now();
                let pending_cmd = PendingCommand {
                    id,
                    command,
                    priority,
                    camera_id,
                    submitted_at: now,
                };
                self.core.queue_command(pending_cmd);

                self.metrics.commands_sent += 1;
                let _ = admission_tx.send(Ok(()));
            }
            SubmitRequest::Inquiry {
                id,
                command,
                camera_id,
                admission_tx,
                response_tx,
            } => {
                // Admission control: reject if at capacity
                if self.core.pending_queue_depth() >= self.max_pending_queue_depth {
                    debug!(
                        %id,
                        capacity = self.max_pending_queue_depth,
                        "Rejecting inquiry: queue at capacity"
                    );
                    let error = Error::RuntimeQueueFull {
                        capacity: self.max_pending_queue_depth,
                    };
                    let _ = admission_tx.send(Err(error.clone()));
                    let _ = response_tx.send(Err(error));
                    self.metrics.commands_failed += 1;
                    return;
                }

                self.response_channels.insert(id, response_tx);

                // Queue inquiry in core with Low priority.
                // Behavior is stored in EncodedCommand and copied into
                // InquiryEntry when the inquiry is started.
                // Inquiries are typically used for polling/status checks, so they should
                // not block user-initiated commands. This prevents command starvation when
                // polling generates many inquiries that timeout/retry (GitHub issue #381).
                // Category and behavior are derived from EncodedCommand.
                let now = self.executor.now();
                let pending_cmd = PendingCommand {
                    id,
                    command,
                    priority: Priority::Low,
                    camera_id,
                    submitted_at: now,
                };
                self.core.queue_command(pending_cmd);

                self.metrics.commands_sent += 1;
                let _ = admission_tx.send(Ok(()));
            }
        }
    }

    /// Get the next item to send (command or inquiry).
    pub fn next_item_to_send(&mut self) -> Option<PendingCommand> {
        let now = self.executor.now();
        self.core.next_item_to_send(now)
    }

    /// Register that a command was sent and is pending ACK.
    pub fn register_pending_ack(&mut self, cmd: &PendingCommand) {
        let now = self.executor.now();
        // Category and kind are derived from EncodedCommand
        self.core.register_pending_ack(
            cmd.id,
            cmd.command.clone(),
            cmd.priority,
            cmd.camera_id,
            now,
        );
    }

    /// Start tracking an inquiry (no socket allocation).
    pub fn start_inquiry(&mut self, cmd: &PendingCommand) {
        let now = self.executor.now();
        // Category and kind are derived from EncodedCommand
        self.core.start_inquiry(
            cmd.id,
            cmd.command.clone(),
            cmd.priority,
            cmd.camera_id,
            now,
        );
    }

    /// Register a Sony sequence number for a command.
    ///
    /// Should only be called after a successful send.
    pub fn register_sequence(&mut self, cmd_id: CommandId, sequence: u32) {
        debug_assert!(
            self.core.is_command_pending(cmd_id),
            "register_sequence called for non-pending command {cmd_id}"
        );
        self.core.register_sequence(cmd_id, sequence);
    }

    /// Handle a send failure - fails immediately with the original error wrapped in context.
    ///
    /// This routes the failure through the unified action handling path,
    /// ensuring consistent metrics tracking and response notification.
    ///
    /// The `cause` parameter preserves the original error (including timeout semantics)
    /// while adding "Send failed" context.
    pub fn fail_after_send_error(&mut self, id: CommandId, cause: Error) {
        if let Some(action) = self.core.fail_after_send_error(id, cause) {
            // Route through unified action handler for consistent metrics/notification
            self.apply_action(action);
        }
    }

    fn fail_all_pending(&mut self, error: Error) -> usize {
        debug!(
            pending_channels = self.response_channels.len(),
            %error,
            "Failing all pending commands"
        );

        let failed_count = self.response_channels.len();
        for (_id, tx) in self.response_channels.drain() {
            let _ = tx.send(Err(error.clone()));
            self.metrics.commands_failed += 1;
        }

        self.core.clear_all();
        self.cancel_outbox.clear();

        failed_count
    }

    /// Poison the transport and fail all pending commands.
    ///
    /// This is called when a stream transport (TCP, Serial) experiences a send
    /// failure or timeout that leaves the byte stream in an unknown state. All
    /// pending and queued commands are failed with `StreamPoisoned` error.
    ///
    /// # Arguments
    ///
    /// * `reason` - Description of why the transport is being poisoned
    ///
    /// # Returns
    ///
    /// The number of commands that were failed.
    pub fn poison_transport(&mut self, reason: impl Into<std::borrow::Cow<'static, str>>) -> usize {
        let reason = reason.into();
        debug!(
            pending_channels = self.response_channels.len(),
            %reason,
            "Poisoning transport: failing all pending commands"
        );

        self.fail_all_pending(Error::StreamPoisoned { reason })
    }

    /// Fail all pending commands with a runtime termination error.
    ///
    /// This is used when the runtime exits for a non-shutdown cause, such as a
    /// transport close. Pending response futures should observe that cause
    /// directly rather than an incidental channel closure.
    pub fn fail_runtime_terminated(&mut self, error: Error) -> usize {
        self.fail_all_pending(error)
    }

    /// Fail all runtime-owned waiters and clear scheduler state during explicit shutdown.
    pub fn shutdown(&mut self, error: Error) -> usize {
        debug!(
            pending_channels = self.response_channels.len(),
            "Runtime shutdown: failing pending commands and closing subscribers"
        );

        let failed_count = self.response_channels.len();
        for (_id, tx) in self.response_channels.drain() {
            let _ = tx.send(Err(error.clone()));
            self.metrics.commands_failed += 1;
        }

        self.core.clear_all();
        self.cancel_outbox.clear();
        self.completion_subscribers.clear();

        failed_count
    }

    /// Process a received VISCA response.
    pub async fn process_response(&mut self, payload: &[u8], sequence: Option<u32>) -> Result<()> {
        let now = self.executor.now();

        let event = match receive_one::<P>(&mut self.core, payload, sequence) {
            ReceiveDisposition::Event(event) => {
                if let SchedulerEvent::Error { code, source } = &event {
                    let cmd_id = source.cmd_id();
                    match *code {
                        0x03 | 0x04 => self.metrics.busy_errors += 1,
                        _ => self.metrics.protocol_errors += 1,
                    }

                    // Log errors appropriately based on severity
                    // Syntax errors (0x02) and "Not Executable" (0x41) are notable issues
                    // that likely indicate configuration problems or unsupported commands
                    if *code == 0x02 || *code == 0x41 {
                        let response_spec = cmd_id
                            .and_then(|id| self.core.get_inquiry_response_spec(id))
                            .map(|ty| format!("{:?}", ty));

                        let message = if *code == 0x02 {
                            "Syntax Error - command likely unsupported by camera"
                        } else {
                            "Command Not Executable in current state"
                        };

                        tracing::error!(
                            ?cmd_id,
                            response_spec,
                            code = format!("0x{:02x}", code),
                            message
                        );
                    }
                }
                event
            }
            ReceiveDisposition::AttributedDecodeFailure { id, error } => {
                self.metrics.protocol_errors += 1;
                if let Some(action) = self.core.fail_after_receive_error(id, error) {
                    self.apply_action(action);
                }
                return Ok(());
            }
            ReceiveDisposition::Ignored { reason } => {
                log_ignored_receive(reason, sequence);
                return Ok(());
            }
            ReceiveDisposition::Malformed(error) => {
                tracing::warn!(?error, ?sequence, "Malformed VISCA response ignored");
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

    /// Apply a scheduler action synchronously.
    ///
    /// This is the unified path for all command completion/failure notifications.
    /// It uses synchronous `send()` instead of `send_async()` because:
    /// - Response channels are `flume::bounded(1)` one-shot channels
    /// - They are removed from `response_channels` before sending
    /// - Therefore the channel should never be full at the first send
    ///
    /// Using sync send allows this method to be called from non-async contexts
    /// (like send-failure rollback paths) while maintaining consistent behavior.
    fn apply_action(&mut self, action: SchedulerAction) {
        match action {
            SchedulerAction::CommandComplete {
                id,
                category,
                camera_id,
                response,
            } => {
                self.metrics.commands_completed += 1;

                // Send response to the waiting command
                if let Some(tx) = self.response_channels.remove(&id) {
                    let _ = tx.send(Ok(response));
                }

                // Broadcast completion event to all subscribers
                if !self.completion_subscribers.is_empty() {
                    let event = CompletionEvent {
                        camera_id,
                        category,
                    };

                    // Broadcast to all subscribers, handling overflow and disconnection separately:
                    // - Disconnected: Remove the subscriber (receiver dropped)
                    // - Full: Keep subscriber but drop this event (slow consumer, best-effort delivery)
                    self.completion_subscribers.retain(|tx| {
                        match tx.try_send(event) {
                            Ok(()) => true, // Successfully sent, keep subscriber
                            Err(TrySendError::Full(_)) => {
                                // Buffer full - drop event but keep subscriber (best-effort semantics)
                                trace!(
                                    category = ?event.category,
                                    camera_id = ?event.camera_id,
                                    "Dropping completion event for slow subscriber (buffer full)"
                                );
                                true
                            }
                            Err(TrySendError::Disconnected(_)) => {
                                // Receiver dropped - remove this subscriber
                                false
                            }
                        }
                    });
                }
            }
            SchedulerAction::CommandFailed { id, error } => {
                self.metrics.commands_failed += 1;

                if let Some(tx) = self.response_channels.remove(&id) {
                    let _ = tx.send(Err(error));
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
                }
            }
            SchedulerAction::RetryCommand { id, delay } => {
                self.metrics.commands_retried += 1;

                debug!("Scheduling retry for command {id} after {delay:?}");
            }
            SchedulerAction::SendCancel { camera_id, socket } => {
                // Queue the cancel for sending by loop_task
                debug!(?camera_id, ?socket, "Queueing cancel for socket");
                self.cancel_outbox.push_back((camera_id, socket));
            }
        }
    }

    /// Handle a scheduler action asynchronously.
    ///
    /// This is a thin async wrapper around `apply_action` for the async event
    /// processing loop.
    async fn handle_action(&mut self, action: SchedulerAction) -> Result<()> {
        self.apply_action(action);
        Ok(())
    }

    /// Notify a waiting future that its command has timed out.
    #[inline]
    fn notify_timeout(&mut self, id: CommandId) {
        if let Some(tx) = self.response_channels.remove(&id) {
            let _ = tx.try_send(Err(Error::Timeout));
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

    /// Get the next deadline for time-based operations.
    pub fn next_deadline(&self) -> Option<Instant> {
        let now = self.executor.now();
        self.core.next_deadline(now)
    }

    /// Check if we can send another command.
    pub fn can_send_command(&self) -> bool {
        let now = self.executor.now();
        self.core.can_send_command(now)
    }

    /// Get the count of commands waiting for ACK.
    pub fn pending_ack_count(&self) -> usize {
        self.core.pending_ack_count()
    }

    /// Get metrics summary.
    #[cfg(feature = "test-utils")]
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
            ignored_unmatched_sequenced_replies: self.core.ignored_unmatched_sequenced_replies(),
            pending_queue_depth: self.core.pending_queue_depth(),
            retry_queue_depth: self.core.retry_queue_depth(),
        }
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
    /// Returns a receiver for completion events. The channel is bounded to
    /// [`COMPLETIONS_BUFFER`] events per subscriber.
    ///
    /// # Best-Effort Semantics
    ///
    /// Completion events are delivered on a **best-effort** basis. If a subscriber
    /// cannot keep up with the event rate and its buffer becomes full, events will
    /// be dropped for that subscriber. This design ensures:
    ///
    /// - The runtime loop never blocks waiting for slow consumers
    /// - No unbounded memory growth from unread events
    /// - Subscribers that drain promptly receive all events
    ///
    /// Subscribers should drain the receiver regularly. If your consumer is
    /// processing-intensive, consider buffering events in your own queue with
    /// appropriate backpressure handling.
    pub fn subscribe_completions(&mut self) -> flume::Receiver<CompletionEvent> {
        let (tx, rx) = flume::bounded(COMPLETIONS_BUFFER);
        self.completion_subscribers.push(tx);
        rx
    }

    /// Request cancellation of a command by ID.
    ///
    /// This routes the cancel request through the scheduler, which implements
    /// lifecycle-aware cancel semantics:
    ///
    /// - **Command not active**: No-op (command already completed, timed out, or unknown ID).
    /// - **Socket already assigned**: Returns `Some((camera_id, socket))` immediately for sending.
    /// - **Awaiting ACK**: Flags the command for cancel-on-ACK; the cancel will be emitted
    ///   as a `SchedulerAction::SendCancel` when the ACK arrives.
    ///
    /// If this method returns `Some((camera_id, socket))`, the caller should send
    /// the cancel command to the transport. If it returns `None`, either the command
    /// is inactive or the cancel has been deferred until ACK.
    pub fn request_cancel_by_id(&mut self, cmd_id: CommandId) -> CancelOutcome {
        let outcome = self.core.request_cancel_by_id(cmd_id);
        if matches!(outcome, CancelOutcome::QueuedRemoved) {
            if let Some(tx) = self.response_channels.remove(&cmd_id) {
                let _ = tx.send(Err(Error::CommandCanceled));
                self.metrics.commands_failed += 1;
            }
        }
        outcome
    }

    /// Drain the cancel outbox, returning all queued cancel requests.
    ///
    /// This should be called by `loop_task` to retrieve cancels that were
    /// queued via `SchedulerAction::SendCancel` (cancel-on-ACK path).
    ///
    /// Returns a `Vec` instead of an iterator to avoid borrow checker issues
    /// when the caller needs to mutate `adapter` while iterating.
    pub fn drain_cancel_outbox(&mut self) -> Vec<(CameraId, ViscaSocket)> {
        self.cancel_outbox.drain(..).collect()
    }
}

#[cfg(all(test, feature = "test-utils"))]
#[allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{
        camera::profiles::PtzOpticsG2,
        command::{
            bytes::VISCA_TERMINATOR, encode::EncodedCommand, response::InquiryKind,
            CommandBehavior, InquiryResponseSpec, Response,
        },
        testing::testkit::deterministic_executor::DeterministicExecutor,
        transport::builder::DEFAULT_MAX_PENDING_QUEUE_DEPTH,
    };
    use smallvec::SmallVec;
    use std::sync::Arc;

    /// Helper function to create CommandId from u32 in tests.
    /// Panics if value is 0 (invalid for CommandId).
    fn cmd_id(value: u32) -> CommandId {
        CommandId::from_raw(value).expect("test command ID must be non-zero")
    }

    fn admission_tx() -> Sender<Result<()>> {
        let (tx, _rx) = flume::bounded(1);
        tx
    }

    /// Test that async adapter delegates unattributed errors to SchedulerCore for FIFO attribution.
    ///
    /// This test verifies the fix for issue #428: when an error frame arrives with no socket
    /// and no sequence number (common for raw VISCA inquiry errors), the async adapter should
    /// use the shared receive driver to attribute the error via FIFO fallback,
    /// rather than dropping the error.
    #[test]
    fn test_async_error_without_socket_attributes_to_inflight_inquiry() {
        // Create deterministic executor for testing
        let (executor, _clock) = DeterministicExecutor::new();
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();

        let mut adapter = AsyncAdapter::<PtzOpticsG2, _>::new(
            timeout_config,
            retry_config,
            executor.clone(),
            DEFAULT_MAX_PENDING_QUEUE_DEPTH,
        );

        let camera_id = CameraId::CAMERA_1;

        let inq = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x81, 0x09, 0x04, 0x35, VISCA_TERMINATOR]), // WB Mode Inquiry
            behavior: CommandBehavior::Inquiry(InquiryResponseSpec::Builtin(InquiryKind::Power)),
            category: CommandCategory::Quick,
        });

        adapter.core.start_inquiry(
            cmd_id(42),
            inq.clone(),
            Priority::Normal,
            camera_id,
            executor.now(),
        );

        let (response_tx, response_rx) = flume::unbounded();
        adapter.response_channels.insert(cmd_id(42), response_tx);

        // Simulate an inquiry-style error without socket or sequence: 90 60 EE FF
        // This is the packet shape that was previously dropped by async_adapter
        let error_payload = vec![0x90, 0x60, 0x41, VISCA_TERMINATOR];

        let result = executor.block_on(adapter.process_response(&error_payload, None));
        assert!(result.is_ok(), "process_response should succeed");

        let response_result = response_rx.try_recv();
        assert!(
            response_result.is_ok(),
            "Inquiry should receive error immediately, not timeout"
        );

        match response_result.unwrap() {
            Err(Error::CommandNotExecutable) => {}
            other => panic!("Expected CommandNotExecutable error, got: {other:?}"),
        }

        let metrics = adapter.metrics_summary();
        assert_eq!(metrics.commands_failed, 1, "Should have 1 failed command");
        assert_eq!(
            metrics.protocol_errors, 1,
            "Should have 1 protocol error tracked"
        );
    }

    /// Test that decode errors in DataReply responses fail immediately instead of timing out.
    ///
    /// This test verifies issue #479: when a DataReply arrives with an invalid payload
    /// (e.g., wrong length for the expected InquiryKind), the command should receive
    /// an error immediately rather than waiting for a timeout.
    ///
    /// Before this fix, decode errors in `lift_inquiry_for` would propagate out of
    /// `process_response`, leaving the command in-flight until timeout.
    #[test]
    fn test_decode_error_in_data_reply_fails_immediately() {
        let (executor, _clock) = DeterministicExecutor::new();
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();

        let mut adapter = AsyncAdapter::<PtzOpticsG2, _>::new(
            timeout_config,
            retry_config,
            executor.clone(),
            DEFAULT_MAX_PENDING_QUEUE_DEPTH,
        );

        let camera_id = CameraId::CAMERA_1;

        // Create an inquiry expecting a Power response (which requires exactly 1 byte)
        let inq = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR]),
            behavior: CommandBehavior::Inquiry(InquiryResponseSpec::Builtin(InquiryKind::Power)),
            category: CommandCategory::Quick,
        });

        // Start the inquiry in the scheduler
        // The inquiry response spec comes from EncodedCommand.behavior (set above)
        adapter.core.start_inquiry(
            cmd_id(99),
            inq.clone(),
            Priority::Normal,
            camera_id,
            executor.now(),
        );

        // Register the response channel
        let (response_tx, response_rx) = flume::unbounded();
        adapter.response_channels.insert(cmd_id(99), response_tx);

        // Simulate a DataReply with WRONG length (3 bytes instead of 1)
        // Power inquiry expects 1 byte, but we send 3 bytes: 90 50 01 02 03 FF
        // This should trigger InvalidResponseLength error in the Power decoder
        let malformed_data_reply = vec![0x90, 0x50, 0x01, 0x02, 0x03, VISCA_TERMINATOR];

        // Process the malformed response - should NOT return Err
        let result = executor.block_on(adapter.process_response(&malformed_data_reply, None));
        assert!(
            result.is_ok(),
            "process_response should return Ok even on decode error: {result:?}"
        );

        // The inquiry should receive an error immediately (not timeout)
        let response_result = response_rx.try_recv();
        assert!(
            response_result.is_ok(),
            "Inquiry should receive error immediately, not timeout"
        );

        // Verify the error has the right context
        match response_result.unwrap() {
            Err(Error::WithContext {
                ref context,
                ref source,
            }) => {
                assert!(
                    context.contains("Response decode failed"),
                    "Error should have decode context, got: {context}"
                );
                // The underlying error should be a decode error (InvalidParameter or InvalidResponseLength)
                // The first byte 0x01 is now properly validated by parse_bool as an invalid parameter
                assert!(
                    matches!(**source, Error::InvalidParameter { .. } | Error::InvalidResponseLength { .. }),
                    "Source error should be InvalidParameter or InvalidResponseLength, got: {source:?}"
                );
            }
            other => {
                panic!("Expected WithContext error with 'Response decode failed', got: {other:?}")
            }
        }

        // Verify metrics
        let metrics = adapter.metrics_summary();
        assert_eq!(metrics.commands_failed, 1, "Should have 1 failed command");
        assert_eq!(
            metrics.protocol_errors, 1,
            "Should have 1 protocol error tracked"
        );

        // Verify command is no longer in-flight
        assert!(
            !adapter.core.is_command_pending(cmd_id(99)),
            "Command should be removed from in-flight state"
        );
    }

    /// Test that decode errors for unattributed DataReply frames are handled gracefully.
    ///
    /// When a DataReply cannot be attributed to any command (no sequence match, no FIFO match),
    /// decode errors should be logged but not cause process_response to fail.
    #[test]
    fn test_decode_error_for_unattributed_reply_is_handled() {
        let (executor, _clock) = DeterministicExecutor::new();
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();

        let mut adapter = AsyncAdapter::<PtzOpticsG2, _>::new(
            timeout_config,
            retry_config,
            executor.clone(),
            DEFAULT_MAX_PENDING_QUEUE_DEPTH,
        );

        // NO inquiry in-flight - the reply will be unattributed

        // Simulate a DataReply for a completely unknown/unmatched inquiry
        // This could happen if a stale reply arrives after command timeout
        let orphan_data_reply = vec![0x90, 0x50, 0x01, VISCA_TERMINATOR];

        // Process the orphan reply - should NOT return Err
        let result = executor.block_on(adapter.process_response(&orphan_data_reply, None));
        assert!(
            result.is_ok(),
            "process_response should succeed for unattributed replies: {result:?}"
        );

        // Metrics should not change (no command to fail)
        let metrics = adapter.metrics_summary();
        assert_eq!(
            metrics.commands_failed, 0,
            "Should have 0 failed commands for unattributed reply"
        );
        assert_eq!(
            metrics.protocol_errors, 0,
            "Should have 0 protocol errors for unattributed reply"
        );
    }

    /// Test that send failures are routed through the unified action handling path.
    ///
    /// This test verifies the fix for issue #458: send failures should:
    /// 1. Increment the `commands_failed` metric (was previously missing)
    /// 2. Deliver the error to the waiting response channel
    ///
    /// Before this fix, `AsyncAdapter::fail_after_send_error` bypassed the
    /// normal action handling path and didn't update metrics.
    #[test]
    fn test_send_failure_increments_commands_failed_metric() {
        // Create deterministic executor for testing
        let (executor, _clock) = DeterministicExecutor::new();
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();

        let mut adapter = AsyncAdapter::<PtzOpticsG2, _>::new(
            timeout_config,
            retry_config,
            executor.clone(),
            DEFAULT_MAX_PENDING_QUEUE_DEPTH,
        );

        let camera_id = CameraId::CAMERA_1;

        // Create a command
        let cmd = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR]), // Zoom Stop
            behavior: CommandBehavior::Command,
            category: CommandCategory::Quick,
        });

        // Register the command as pending ACK
        adapter.core.register_pending_ack(
            cmd_id(100),
            cmd.clone(),
            Priority::Normal,
            camera_id,
            executor.now(),
        );

        // Set up response channel
        let (response_tx, response_rx) = flume::bounded(1);
        adapter.response_channels.insert(cmd_id(100), response_tx);

        // Verify initial metrics
        let metrics_before = adapter.metrics_summary();
        assert_eq!(
            metrics_before.commands_failed, 0,
            "Should start with 0 failed commands"
        );

        // Simulate a send failure with original transport error
        let original_error = Error::TransportError("Simulated send failure".into());
        adapter.fail_after_send_error(cmd_id(100), original_error);

        // Verify the response channel received the error with context
        let response_result = response_rx.try_recv();
        assert!(
            response_result.is_ok(),
            "Command should receive error notification"
        );
        match response_result.unwrap() {
            Err(Error::WithContext { ref context, .. }) if context.contains("Send failed") => {
                // Expected: error should be wrapped with "Send failed" context
            }
            other => panic!("Expected WithContext error containing 'Send failed', got: {other:?}"),
        }

        // Verify metrics were incremented
        let metrics_after = adapter.metrics_summary();
        assert_eq!(
            metrics_after.commands_failed, 1,
            "Should have 1 failed command after send failure"
        );
    }

    /// Test that multiple send failures all increment the failed counter correctly.
    #[test]
    fn test_multiple_send_failures_track_correctly() {
        let (executor, _clock) = DeterministicExecutor::new();
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();

        let mut adapter = AsyncAdapter::<PtzOpticsG2, _>::new(
            timeout_config,
            retry_config,
            executor.clone(),
            DEFAULT_MAX_PENDING_QUEUE_DEPTH,
        );

        let camera_id = CameraId::CAMERA_1;

        // Create and register 3 commands with response channels
        for raw_id in [101, 102, 103] {
            let id = cmd_id(raw_id);
            let cmd = Arc::new(EncodedCommand {
                payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR]),
                behavior: CommandBehavior::Command,
                category: CommandCategory::Quick,
            });

            adapter
                .core
                .register_pending_ack(id, cmd, Priority::Normal, camera_id, executor.now());

            let (response_tx, _response_rx) = flume::bounded(1);
            adapter.response_channels.insert(id, response_tx);
        }

        // Fail all 3 commands with original errors
        adapter.fail_after_send_error(
            cmd_id(101),
            Error::TransportError("Simulated failure 1".into()),
        );
        adapter.fail_after_send_error(
            cmd_id(102),
            Error::TransportError("Simulated failure 2".into()),
        );
        adapter.fail_after_send_error(
            cmd_id(103),
            Error::TransportError("Simulated failure 3".into()),
        );

        // Verify all failures are tracked
        let metrics = adapter.metrics_summary();
        assert_eq!(
            metrics.commands_failed, 3,
            "Should have 3 failed commands after 3 send failures"
        );
    }

    // =========================================================================
    // Bounded Completion Subscriber Tests (Issue #462)
    // =========================================================================

    /// Test that completion subscriber buffer is bounded.
    ///
    /// Verifies that the subscriber cannot hold more than COMPLETIONS_BUFFER events,
    /// preventing unbounded memory growth from slow/unread consumers.
    #[test]
    fn test_completion_subscriber_buffer_is_bounded() {
        let (executor, _clock) = DeterministicExecutor::new();
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();

        let mut adapter = AsyncAdapter::<PtzOpticsG2, _>::new(
            timeout_config,
            retry_config,
            executor.clone(),
            DEFAULT_MAX_PENDING_QUEUE_DEPTH,
        );

        // Subscribe but don't drain
        let rx = adapter.subscribe_completions();

        // Emit more events than the buffer can hold
        let events_to_emit = COMPLETIONS_BUFFER + 100;
        for i in 1..=events_to_emit {
            // Simulate a command completion by calling apply_action directly
            adapter.apply_action(SchedulerAction::CommandComplete {
                id: cmd_id(i as u32),
                category: CommandCategory::Quick,
                camera_id: CameraId::CAMERA_1,
                response: Response::Completion { socket: None },
            });
        }

        // Count how many events we can drain - should be at most COMPLETIONS_BUFFER
        let mut received = 0;
        while rx.try_recv().is_ok() {
            received += 1;
        }

        assert_eq!(
            received, COMPLETIONS_BUFFER,
            "Should receive at most COMPLETIONS_BUFFER events"
        );

        // Verify subscriber is still registered (wasn't removed due to overflow)
        assert_eq!(
            adapter.completion_subscribers.len(),
            1,
            "Subscriber should still be registered after overflow"
        );
    }

    /// Test that overflow does not unsubscribe the consumer.
    ///
    /// When a subscriber's buffer is full, events should be dropped for that
    /// subscriber, but the subscriber should remain active and receive new
    /// events once buffer space is available.
    #[test]
    fn test_overflow_does_not_unsubscribe() {
        let (executor, _clock) = DeterministicExecutor::new();
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();

        let mut adapter = AsyncAdapter::<PtzOpticsG2, _>::new(
            timeout_config,
            retry_config,
            executor.clone(),
            DEFAULT_MAX_PENDING_QUEUE_DEPTH,
        );

        // Subscribe but don't drain
        let rx = adapter.subscribe_completions();

        // Fill the buffer completely
        for i in 1..=COMPLETIONS_BUFFER {
            adapter.apply_action(SchedulerAction::CommandComplete {
                id: cmd_id(i as u32),
                category: CommandCategory::Quick,
                camera_id: CameraId::CAMERA_1,
                response: Response::Completion { socket: None },
            });
        }

        // Try to send more events - these should be dropped
        for i in 1..=50 {
            adapter.apply_action(SchedulerAction::CommandComplete {
                id: cmd_id((COMPLETIONS_BUFFER + i) as u32),
                category: CommandCategory::Movement,
                camera_id: CameraId::CAMERA_1,
                response: Response::Completion { socket: None },
            });
        }

        // Subscriber should still be registered
        assert_eq!(
            adapter.completion_subscribers.len(),
            1,
            "Subscriber should remain registered after overflow"
        );

        // Drain some events to make room
        for _ in 0..10 {
            let _ = rx.try_recv();
        }

        // Now send new events - they should be delivered
        adapter.apply_action(SchedulerAction::CommandComplete {
            id: cmd_id(9999),
            category: CommandCategory::Preset,
            camera_id: CameraId::CAMERA_1,
            response: Response::Completion { socket: None },
        });

        // Drain remaining events and check we receive the new one
        let mut found_new_event = false;
        while let Ok(event) = rx.try_recv() {
            if event.category == CommandCategory::Preset {
                found_new_event = true;
            }
        }

        assert!(
            found_new_event,
            "Should receive new events after draining buffer"
        );
    }

    /// Test that disconnected subscribers are removed.
    ///
    /// When a subscriber drops its receiver, subsequent completion events
    /// should cause that subscriber to be removed from the list.
    #[test]
    fn test_disconnected_subscriber_is_removed() {
        let (executor, _clock) = DeterministicExecutor::new();
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();

        let mut adapter = AsyncAdapter::<PtzOpticsG2, _>::new(
            timeout_config,
            retry_config,
            executor.clone(),
            DEFAULT_MAX_PENDING_QUEUE_DEPTH,
        );

        // Subscribe and then immediately drop the receiver
        let rx = adapter.subscribe_completions();
        drop(rx);

        assert_eq!(
            adapter.completion_subscribers.len(),
            1,
            "Subscriber should exist before any events"
        );

        // Emit a completion event - this should trigger cleanup
        adapter.apply_action(SchedulerAction::CommandComplete {
            id: cmd_id(1),
            category: CommandCategory::Quick,
            camera_id: CameraId::CAMERA_1,
            response: Response::Completion { socket: None },
        });

        // The disconnected subscriber should now be removed
        assert_eq!(
            adapter.completion_subscribers.len(),
            0,
            "Disconnected subscriber should be removed after event emission"
        );
    }

    /// Test that multiple subscribers work independently.
    ///
    /// Each subscriber has its own buffer and overflow behavior should
    /// be independent per-subscriber.
    #[test]
    fn test_multiple_subscribers_independent_overflow() {
        let (executor, _clock) = DeterministicExecutor::new();
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();

        let mut adapter = AsyncAdapter::<PtzOpticsG2, _>::new(
            timeout_config,
            retry_config,
            executor.clone(),
            DEFAULT_MAX_PENDING_QUEUE_DEPTH,
        );

        // Create two subscribers
        let rx1 = adapter.subscribe_completions();
        let rx2 = adapter.subscribe_completions();

        // Subscriber 1: drain regularly
        // Subscriber 2: don't drain (will overflow)

        // Send enough events to overflow subscriber 2's buffer
        for i in 1..=COMPLETIONS_BUFFER + 50 {
            adapter.apply_action(SchedulerAction::CommandComplete {
                id: cmd_id(i as u32),
                category: CommandCategory::Quick,
                camera_id: CameraId::CAMERA_1,
                response: Response::Completion { socket: None },
            });

            // Subscriber 1 drains after each event
            let _ = rx1.try_recv();
        }

        // Both subscribers should still be registered
        assert_eq!(
            adapter.completion_subscribers.len(),
            2,
            "Both subscribers should remain registered"
        );

        // Subscriber 2 should have at most COMPLETIONS_BUFFER events
        let mut rx2_count = 0;
        while rx2.try_recv().is_ok() {
            rx2_count += 1;
        }
        assert_eq!(
            rx2_count, COMPLETIONS_BUFFER,
            "Slow subscriber should receive at most COMPLETIONS_BUFFER events"
        );
    }

    // =========================================================================
    // Admission Control Tests (Issue #464)
    // =========================================================================

    /// Test that queue depth is bounded by max_pending_queue_depth.
    ///
    /// This test verifies the core admission control mechanism: when the
    /// pending queue reaches capacity, new submissions are rejected with
    /// a RuntimeQueueFull error.
    #[test]
    fn test_admission_control_bounds_queue_depth() {
        let (executor, _clock) = DeterministicExecutor::new();
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let max_depth = 5; // Small capacity for testing

        let mut adapter = AsyncAdapter::<PtzOpticsG2, _>::new(
            timeout_config,
            retry_config,
            executor.clone(),
            max_depth,
        );

        let camera_id = CameraId::CAMERA_1;

        // Submit commands up to capacity
        for i in 1..=max_depth {
            let cmd = Arc::new(EncodedCommand {
                payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR]),
                behavior: CommandBehavior::Command,
                category: CommandCategory::Quick,
            });

            let (response_tx, response_rx) = flume::bounded(1);
            adapter.admit_submit(SubmitRequest::Command {
                id: cmd_id(i as u32),
                command: cmd,
                priority: Priority::Normal,
                camera_id,
                admission_tx: admission_tx(),
                response_tx,
            });

            // Should not have received an error yet
            assert!(
                response_rx.try_recv().is_err(),
                "Commands up to capacity should be accepted"
            );
        }

        // Verify queue depth is at capacity
        assert_eq!(
            adapter.core.pending_queue_depth(),
            max_depth,
            "Queue should be at capacity"
        );

        // Try to submit one more - should be rejected
        let cmd = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR]),
            behavior: CommandBehavior::Command,
            category: CommandCategory::Quick,
        });

        let (response_tx, response_rx) = flume::bounded(1);
        adapter.admit_submit(SubmitRequest::Command {
            id: cmd_id((max_depth + 1) as u32),
            command: cmd,
            priority: Priority::Normal,
            camera_id,
            admission_tx: admission_tx(),
            response_tx,
        });

        // Should receive RuntimeQueueFull error immediately
        let result = response_rx.try_recv();
        assert!(result.is_ok(), "Should receive error immediately");
        match result.unwrap() {
            Err(Error::RuntimeQueueFull { capacity }) => {
                assert_eq!(capacity, max_depth, "Capacity should match configuration");
            }
            other => panic!("Expected RuntimeQueueFull error, got: {other:?}"),
        }

        // Queue depth should not have grown beyond capacity
        assert_eq!(
            adapter.core.pending_queue_depth(),
            max_depth,
            "Queue should still be at capacity (not grown)"
        );
    }

    /// Test that rejected submissions don't grow response_channels.
    #[test]
    fn test_rejected_submission_does_not_grow_response_channels() {
        let (executor, _clock) = DeterministicExecutor::new();
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let max_depth = 3;

        let mut adapter = AsyncAdapter::<PtzOpticsG2, _>::new(
            timeout_config,
            retry_config,
            executor.clone(),
            max_depth,
        );

        let camera_id = CameraId::CAMERA_1;

        // Fill the queue
        for i in 1..=max_depth {
            let cmd = Arc::new(EncodedCommand {
                payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR]),
                behavior: CommandBehavior::Command,
                category: CommandCategory::Quick,
            });

            let (response_tx, _response_rx) = flume::bounded(1);
            adapter.admit_submit(SubmitRequest::Command {
                id: cmd_id(i as u32),
                command: cmd,
                priority: Priority::Normal,
                camera_id,
                admission_tx: admission_tx(),
                response_tx,
            });
        }

        let initial_channels = adapter.response_channels.len();
        assert_eq!(
            initial_channels, max_depth,
            "Should have max_depth channels"
        );

        // Try to submit more - should be rejected
        for i in 1..=10 {
            let cmd = Arc::new(EncodedCommand {
                payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR]),
                behavior: CommandBehavior::Command,
                category: CommandCategory::Quick,
            });

            let (response_tx, _response_rx) = flume::bounded(1);
            adapter.admit_submit(SubmitRequest::Command {
                id: cmd_id((max_depth + i) as u32),
                command: cmd,
                priority: Priority::Normal,
                camera_id,
                admission_tx: admission_tx(),
                response_tx,
            });
        }

        // response_channels should NOT have grown
        assert_eq!(
            adapter.response_channels.len(),
            max_depth,
            "response_channels should not grow when submissions are rejected"
        );
    }

    /// Test that metrics_summary reports actual pending_queue_depth.
    #[test]
    fn test_metrics_reports_actual_pending_queue_depth() {
        let (executor, _clock) = DeterministicExecutor::new();
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();

        let mut adapter = AsyncAdapter::<PtzOpticsG2, _>::new(
            timeout_config,
            retry_config,
            executor.clone(),
            DEFAULT_MAX_PENDING_QUEUE_DEPTH,
        );

        let camera_id = CameraId::CAMERA_1;

        // Initially, queue depth should be 0
        let metrics = adapter.metrics_summary();
        assert_eq!(
            metrics.pending_queue_depth, 0,
            "Initial pending_queue_depth should be 0"
        );

        // Submit 5 commands
        for i in 1..=5 {
            let cmd = Arc::new(EncodedCommand {
                payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR]),
                behavior: CommandBehavior::Command,
                category: CommandCategory::Quick,
            });

            let (response_tx, _response_rx) = flume::bounded(1);
            adapter.admit_submit(SubmitRequest::Command {
                id: cmd_id(i),
                command: cmd,
                priority: Priority::Normal,
                camera_id,
                admission_tx: admission_tx(),
                response_tx,
            });
        }

        // Queue depth should be 5
        let metrics = adapter.metrics_summary();
        assert_eq!(
            metrics.pending_queue_depth, 5,
            "pending_queue_depth should reflect actual queue depth"
        );

        // Submit 3 inquiries
        for i in 6..=8 {
            let inq = Arc::new(EncodedCommand {
                payload: SmallVec::from_slice(&[0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
                behavior: CommandBehavior::Inquiry(InquiryResponseSpec::Builtin(
                    InquiryKind::Power,
                )),
                category: CommandCategory::Quick,
            });

            let (response_tx, _response_rx) = flume::bounded(1);
            adapter.admit_submit(SubmitRequest::Inquiry {
                id: cmd_id(i),
                command: inq,
                camera_id,
                admission_tx: admission_tx(),
                response_tx,
            });
        }

        // Queue depth should be 8 (5 commands + 3 inquiries)
        let metrics = adapter.metrics_summary();
        assert_eq!(
            metrics.pending_queue_depth, 8,
            "pending_queue_depth should include both commands and inquiries"
        );
    }

    /// Test that commands_failed is incremented when submissions are rejected.
    #[test]
    fn test_rejected_submissions_increment_commands_failed() {
        let (executor, _clock) = DeterministicExecutor::new();
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let max_depth = 2;

        let mut adapter = AsyncAdapter::<PtzOpticsG2, _>::new(
            timeout_config,
            retry_config,
            executor.clone(),
            max_depth,
        );

        let camera_id = CameraId::CAMERA_1;

        // Fill the queue
        for i in 1..=max_depth {
            let cmd = Arc::new(EncodedCommand {
                payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR]),
                behavior: CommandBehavior::Command,
                category: CommandCategory::Quick,
            });

            let (response_tx, _response_rx) = flume::bounded(1);
            adapter.admit_submit(SubmitRequest::Command {
                id: cmd_id(i as u32),
                command: cmd,
                priority: Priority::Normal,
                camera_id,
                admission_tx: admission_tx(),
                response_tx,
            });
        }

        let metrics_before = adapter.metrics_summary();
        assert_eq!(
            metrics_before.commands_failed, 0,
            "Should start with 0 failed commands"
        );

        // Try to submit 5 more - all should be rejected
        for i in 1..=5 {
            let cmd = Arc::new(EncodedCommand {
                payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR]),
                behavior: CommandBehavior::Command,
                category: CommandCategory::Quick,
            });

            let (response_tx, _response_rx) = flume::bounded(1);
            adapter.admit_submit(SubmitRequest::Command {
                id: cmd_id((max_depth + i) as u32),
                command: cmd,
                priority: Priority::Normal,
                camera_id,
                admission_tx: admission_tx(),
                response_tx,
            });
        }

        let metrics_after = adapter.metrics_summary();
        assert_eq!(
            metrics_after.commands_failed, 5,
            "Should have 5 failed commands after rejection"
        );
    }

    /// Test that inquiry rejections also work correctly.
    #[test]
    fn test_inquiry_rejection_at_capacity() {
        let (executor, _clock) = DeterministicExecutor::new();
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();
        let max_depth = 2;

        let mut adapter = AsyncAdapter::<PtzOpticsG2, _>::new(
            timeout_config,
            retry_config,
            executor.clone(),
            max_depth,
        );

        let camera_id = CameraId::CAMERA_1;

        // Fill with inquiries
        for i in 1..=max_depth {
            let inq = Arc::new(EncodedCommand {
                payload: SmallVec::from_slice(&[0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
                behavior: CommandBehavior::Inquiry(InquiryResponseSpec::Builtin(
                    InquiryKind::Power,
                )),
                category: CommandCategory::Quick,
            });

            let (response_tx, _response_rx) = flume::bounded(1);
            adapter.admit_submit(SubmitRequest::Inquiry {
                id: cmd_id(i as u32),
                command: inq,
                camera_id,
                admission_tx: admission_tx(),
                response_tx,
            });
        }

        // Try to submit one more inquiry
        let inq = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
            behavior: CommandBehavior::Inquiry(InquiryResponseSpec::Builtin(InquiryKind::Power)),
            category: CommandCategory::Quick,
        });

        let (response_tx, response_rx) = flume::bounded(1);
        adapter.admit_submit(SubmitRequest::Inquiry {
            id: cmd_id((max_depth + 1) as u32),
            command: inq,
            camera_id,
            admission_tx: admission_tx(),
            response_tx,
        });

        // Should receive RuntimeQueueFull error
        let result = response_rx.try_recv();
        assert!(result.is_ok(), "Should receive error immediately");
        match result.unwrap() {
            Err(Error::RuntimeQueueFull { capacity }) => {
                assert_eq!(capacity, max_depth, "Capacity should match configuration");
            }
            other => panic!("Expected RuntimeQueueFull error, got: {other:?}"),
        }
    }
}
