//! Async adapter for the runtime-agnostic scheduler core.
//!
//! This module provides an async wrapper around SchedulerCore, delegating all
//! state management to the core while handling async I/O and futures.

use flume::Sender;
use tracing::{debug, warn};

use std::{collections::HashMap, sync::Arc};

use crate::{
    camera_id::CameraId,
    command::response::{lift_inquiry, ViscaResponse, ViscaResponseType},
    error::{Error, Result},
    executor::Executor,
    protocol::response::{decode_basic, BasicKind},
    runtime::{
        core::{
            PendingCommand, Priority, RetryCommand, SchedulerAction, SchedulerCore, SchedulerEvent,
        },
        inquiry_matcher::{resolve_raw_inquiry_id, ResolveResult},
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

/// Async adapter wrapping the scheduler core.
pub(crate) struct AsyncAdapter<E: Executor> {
    /// The scheduler core for state management.
    core: SchedulerCore,
    /// Executor for time and async operations.
    executor: Arc<E>,
    /// Response channels for commands.
    response_channels: HashMap<u32, Sender<Result<ViscaResponse>>>,
    /// Response types for inquiries (for parsing DataReply).
    inquiry_response_types: HashMap<u32, ViscaResponseType>,
    /// Queue of in-flight inquiries (for raw VISCA DataReply without socket info).
    /// We process DataReplies in FIFO order since raw VISCA doesn't identify which
    /// inquiry a response belongs to.
    active_inquiry_ids: std::collections::VecDeque<u32>,
    /// Metrics tracking.
    metrics: Metrics,
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

impl<E: Executor> AsyncAdapter<E> {
    /// Create a new async adapter.
    pub fn new(timeout_config: TimeoutConfig, retry_config: RetryConfig, executor: Arc<E>) -> Self {
        Self {
            core: SchedulerCore::with_retry_config(timeout_config, retry_config),
            executor,
            response_channels: HashMap::new(),
            inquiry_response_types: HashMap::new(),
            active_inquiry_ids: std::collections::VecDeque::new(),
            metrics: Metrics::default(),
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

                // Store response type if present
                if let Some(rt) = response_type {
                    self.inquiry_response_types.insert(id, rt);
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
            // Also remove from inquiry tracking if applicable
            self.inquiry_response_types.remove(&failed_id);
        }
    }

    /// Mark an inquiry as in-flight after successful send.
    ///
    /// This adds the inquiry ID to the active queue for DataReply handling.
    /// Should only be called after a successful send.
    pub fn mark_inquiry_inflight(&mut self, id: u32) {
        debug_assert!(
            !self.active_inquiry_ids.contains(&id),
            "mark_inquiry_inflight called for already in-flight inquiry {}",
            id
        );
        debug_assert!(
            self.response_channels.contains_key(&id),
            "mark_inquiry_inflight called for inquiry {} without response channel",
            id
        );
        self.active_inquiry_ids.push_back(id);
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

                // Get the expected response type for this inquiry
                let response_type = cmd_id.and_then(|id| self.inquiry_response_types.get(&id));

                let response = lift_inquiry(&basic, response_type)?;
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
                let cmd_id = sequence.and_then(|seq| self.core.get_command_by_sequence(seq));
                SchedulerEvent::Error {
                    socket: basic.socket,
                    cmd_id,
                    code,
                }
            }
            BasicKind::DataReply => {
                // Data replies are completions for inquiries
                // Try to find command ID from sequence (Sony) first
                let cmd_id = sequence
                    .and_then(|seq| self.core.get_command_by_sequence(seq))
                    .or_else(|| {
                        // No sequence - use content-based matching for raw VISCA
                        match resolve_raw_inquiry_id(
                            basic.payload.as_slice(),
                            &self.inquiry_response_types,
                        ) {
                            ResolveResult::Unique(id) => Some(id),
                            ResolveResult::Ambiguous(_) => {
                                // Fall back to FIFO for ambiguous cases
                                debug!("Ambiguous inquiry match, falling back to FIFO");
                                self.active_inquiry_ids.front().copied()
                            }
                            ResolveResult::None => {
                                // No match - try FIFO as last resort
                                debug!("No inquiry match, falling back to FIFO");
                                self.active_inquiry_ids.front().copied()
                            }
                        }
                    });

                // Get the expected response type for this inquiry
                let response_type = cmd_id.and_then(|id| self.inquiry_response_types.get(&id));

                let response = lift_inquiry(&basic, response_type)?;

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
            SchedulerAction::CommandComplete { id, response } => {
                self.metrics.commands_completed += 1;

                // Clean up inquiry tracking
                self.inquiry_response_types.remove(&id);
                // Remove from active inquiry queue
                self.active_inquiry_ids.retain(|&x| x != id);

                if let Some(tx) = self.response_channels.remove(&id) {
                    let _ = tx.send_async(Ok(response)).await;
                }
            }
            SchedulerAction::CommandFailed { id, error } => {
                self.metrics.commands_failed += 1;

                // Clean up inquiry tracking
                self.inquiry_response_types.remove(&id);
                // Remove from active inquiry queue
                self.active_inquiry_ids.retain(|&x| x != id);

                if let Some(tx) = self.response_channels.remove(&id) {
                    let _ = tx.send_async(Err(error)).await;
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

    /// Handle a network error by failing all pending commands.
    pub async fn handle_network_error(&mut self) -> Result<()> {
        self.metrics.network_errors += 1;

        let now = self.executor.now();
        let actions = self.core.process_event(SchedulerEvent::NetworkError, now);
        for action in actions {
            self.handle_action(action).await?;
        }
        Ok(())
    }

    /// Check if we can send another command.
    pub fn can_send_command(&self) -> bool {
        self.core.can_send_command()
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
}
