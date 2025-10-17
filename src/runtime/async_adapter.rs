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
    command::response::{lift_inquiry_for, InquiryKind, Response},
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
#[derive(Clone)]
pub(crate) enum TxItem {
    /// A command that requires a socket and expects ACK/Completion.
    Command {
        /// Unique identifier for this command.
        id: u32,
        /// The pre-encoded command to send.
        command: Arc<crate::command::encode::EncodedCommand>,
        /// Priority level for scheduling.
        priority: Priority,
        /// Category for timeout calculation.
        category: CommandCategory,
        /// Camera ID used to encode the command.
        camera_id: CameraId,
        /// Channel to send response back.
        response_tx: Sender<Result<Response>>,
    },
    /// An inquiry that doesn't require a socket, expects DataReply.
    Inquiry {
        /// Unique identifier for this inquiry.
        id: u32,
        /// The pre-encoded command to send.
        command: Arc<crate::command::encode::EncodedCommand>,
        /// Category for timeout calculation.
        category: CommandCategory,
        /// Camera ID used to encode the inquiry.
        camera_id: CameraId,
        /// Expected response type for parsing DataReply.
        response_type: Option<InquiryKind>,
        /// Channel to send response back.
        response_tx: Sender<Result<Response>>,
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

impl std::fmt::Debug for TxItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxItem::Command {
                id,
                priority,
                category,
                camera_id,
                ..
            } => f
                .debug_struct("TxItem::Command")
                .field("id", id)
                .field("priority", priority)
                .field("category", category)
                .field("camera_id", camera_id)
                .finish(),
            TxItem::Inquiry {
                id,
                category,
                camera_id,
                response_type,
                ..
            } => f
                .debug_struct("TxItem::Inquiry")
                .field("id", id)
                .field("category", category)
                .field("camera_id", camera_id)
                .field("response_type", response_type)
                .finish(),
            TxItem::Cancel { socket } => f
                .debug_struct("TxItem::Cancel")
                .field("socket", socket)
                .finish(),
            TxItem::CancelById { id } => f
                .debug_struct("TxItem::CancelById")
                .field("id", id)
                .finish(),
        }
    }
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
    response_channels: HashMap<u32, Sender<Result<Response>>>,
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
                command,
                priority,
                category,
                camera_id,
                response_tx,
            } => {
                self.response_channels.insert(id, response_tx);

                // Queue command in core
                let now = self.executor.now();
                let pending_cmd = PendingCommand {
                    id,
                    command,
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
                command,
                category,
                camera_id,
                response_type,
                response_tx,
            } => {
                self.response_channels.insert(id, response_tx);

                // Store response type in core if present
                if let Some(rt) = response_type {
                    self.core.register_inquiry_type(id, rt);
                }

                // Queue inquiry in core (same as commands but with Quick priority)
                let now = self.executor.now();
                let pending_cmd = PendingCommand {
                    id,
                    command,
                    priority: Priority::Normal,
                    category,
                    camera_id,
                    submitted_at: now,
                    kind: crate::command::CommandKind::Inquiry,
                };
                self.core.queue_command(pending_cmd);

                self.metrics.commands_sent += 1;

                id
            }
            TxItem::Cancel { .. } | TxItem::CancelById { .. } => 0,
        }
    }

    /// Get the next item to send (command or inquiry).
    pub fn next_item_to_send(&mut self) -> Option<PendingCommand> {
        self.core.next_item_to_send()
    }

    /// Register that a command was sent and is pending ACK.
    pub fn register_pending_ack(&mut self, cmd: &PendingCommand) {
        let now = self.executor.now();
        self.core.register_pending_ack(
            cmd.id,
            cmd.command.clone(),
            cmd.priority,
            cmd.category,
            cmd.camera_id,
            cmd.kind,
            now,
        );
    }

    /// Start tracking an inquiry (no socket allocation).
    pub fn start_inquiry(&mut self, cmd: &PendingCommand) {
        let now = self.executor.now();
        self.core.start_inquiry(
            cmd.id,
            cmd.command.clone(),
            cmd.priority,
            cmd.category,
            cmd.camera_id,
            cmd.kind,
            now,
        );
    }

    /// Register a Sony sequence number for a command.
    ///
    /// Should only be called after a successful send.
    pub fn register_sequence(&mut self, cmd_id: u32, sequence: u32) {
        debug_assert!(
            self.core.is_command_pending(cmd_id),
            "register_sequence called for non-pending command {cmd_id}"
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

    /// Process a received VISCA response.
    pub async fn process_response(&mut self, payload: &[u8], sequence: Option<u32>) -> Result<()> {
        // Parse VISCA response type using decode_basic
        let basic = decode_basic(payload).ok_or_else(|| Error::InvalidResponse {
            expected: std::borrow::Cow::Borrowed("Valid VISCA response"),
            actual: payload.to_vec(),
        })?;

        // Log response classification at trace level
        use tracing::trace;
        trace!(
            kind = ?basic.kind,
            socket = ?basic.socket,
            "Decoded VISCA response"
        );

        let now = self.executor.now();

        let event = match basic.kind {
            BasicKind::Ack => {
                let cmd_id = sequence.and_then(|seq| self.core.get_command_by_sequence(seq));
                SchedulerEvent::Ack {
                    socket: basic.socket,
                    cmd_id,
                }
            }
            BasicKind::Completion => {
                let cmd_id = sequence.and_then(|seq| self.core.get_command_by_sequence(seq));
                let response_type = cmd_id.and_then(|id| self.core.get_inquiry_type(id));
                let response = lift_inquiry_for::<P>(&basic, response_type)?;
                SchedulerEvent::Completion {
                    socket: basic.socket,
                    cmd_id,
                    response,
                }
            }
            BasicKind::Error(code) => {
                match code {
                    0x03 | 0x04 => self.metrics.busy_errors += 1,
                    _ => self.metrics.protocol_errors += 1,
                }

                let mut cmd_id = sequence.and_then(|seq| self.core.get_command_by_sequence(seq));

                // If no cmd_id and no socket, try to resolve inquiry via core (FIFO fallback)
                if cmd_id.is_none() && basic.socket.is_none() {
                    use crate::command::response::payload::Payload;
                    cmd_id = self.core.resolve_inquiry_id(Payload::new(&[]), sequence);
                }

                // Log errors appropriately based on severity
                // Syntax errors (0x02) and "Not Executable" (0x41) are notable issues
                // that likely indicate configuration problems or unsupported commands
                if code == 0x02 || code == 0x41 {
                    let inquiry_type = cmd_id
                        .and_then(|id| self.core.get_inquiry_type(id))
                        .map(|ty| format!("{:?}", ty));

                    let message = if code == 0x02 {
                        "Syntax Error - command likely unsupported by camera"
                    } else {
                        "Command Not Executable in current state"
                    };

                    tracing::error!(
                        cmd_id,
                        inquiry_type,
                        code = format!("0x{:02x}", code),
                        message
                    );
                }

                SchedulerEvent::Error {
                    socket: basic.socket,
                    cmd_id,
                    code,
                }
            }
            BasicKind::DataReply => {
                let cmd_id = self.core.resolve_inquiry_id(basic.payload, sequence);
                let response_type = cmd_id.and_then(|id| self.core.get_inquiry_type(id));
                let response = lift_inquiry_for::<P>(&basic, response_type)?;
                SchedulerEvent::InquiryReply { cmd_id, response }
            }
            BasicKind::NetworkChange | BasicKind::Unknown => return Ok(()),
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
                warn!("Unexpected SendCommand action from process_event");
            }
            SchedulerAction::CommandComplete {
                id,
                category,
                camera_id,
                response,
            } => {
                self.metrics.commands_completed += 1;

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

                if let Some(tx) = self.response_channels.remove(&id) {
                    let _ = tx.send_async(Err(error)).await;
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
        }
        Ok(())
    }

    /// Notify a waiting future that its command has timed out.
    #[inline]
    fn notify_timeout(&mut self, id: u32) {
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
            pending_queue_depth: 0,
            retry_queue_depth: self.core.retry_queue_depth(),
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

#[cfg(all(test, feature = "test-utils"))]
#[allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{
        camera::profiles::PtzOpticsG2,
        command::{bytes::VISCA_TERMINATOR, encode::EncodedCommand, CommandKind},
        testing::testkit::deterministic_executor::DeterministicExecutor,
    };
    use smallvec::SmallVec;
    use std::sync::Arc;

    /// Test that async adapter delegates unattributed errors to SchedulerCore for FIFO attribution.
    ///
    /// This test verifies the fix for issue #428: when an error frame arrives with no socket
    /// and no sequence number (common for raw VISCA inquiry errors), the async adapter should
    /// use SchedulerCore's resolve_inquiry_id method to attribute the error via FIFO fallback,
    /// rather than dropping the error.
    #[test]
    fn test_async_error_without_socket_attributes_to_inflight_inquiry() {
        // Create deterministic executor for testing
        let (executor, _clock) = DeterministicExecutor::new();
        let timeout_config = TimeoutConfig::default();
        let retry_config = RetryConfig::default();

        let mut adapter =
            AsyncAdapter::<PtzOpticsG2, _>::new(timeout_config, retry_config, executor.clone());

        let camera_id = CameraId::CAMERA_1;

        let inq = Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x81, 0x09, 0x04, 0x35, VISCA_TERMINATOR]), // WB Mode Inquiry
            kind: CommandKind::Inquiry,
            category: CommandCategory::Quick,
            response_type: Some(InquiryKind::Power),
        });

        adapter.core.start_inquiry(
            42,
            inq.clone(),
            Priority::Normal,
            CommandCategory::Quick,
            camera_id,
            CommandKind::Inquiry,
            executor.now(),
        );

        let (response_tx, response_rx) = flume::unbounded();
        adapter.response_channels.insert(42, response_tx);

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
}
