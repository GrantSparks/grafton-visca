//! Async adapter for the runtime-agnostic scheduler core.
//!
//! This module provides an async wrapper around SchedulerCore, delegating all
//! state management to the core while handling async I/O and futures.

use flume::Sender;
use tracing::{debug, warn};

use std::{collections::HashMap, sync::Arc};

use crate::{
    camera_id::CameraId,
    command::response::{lift_inquiry, ViscaResponse},
    error::{Error, Result},
    executor::Executor,
    protocol::response::{decode_basic, BasicKind},
    runtime::core::{
        PendingCommand, Priority, RetryCommand, SchedulerAction, SchedulerCore, SchedulerEvent,
    },
    timeout::{CommandCategory, TimeoutConfig},
    transport::{buffer::BufferManager, envelope::TransportEnvelope, AsyncTransport, RetryConfig},
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
                };
                self.core.queue_command(pending_cmd);

                self.metrics.commands_sent += 1;

                id
            }
            TxItem::Inquiry {
                id, response_tx, ..
            } => {
                // Inquiries bypass the scheduler and are handled directly
                // Store channel for direct response
                self.response_channels.insert(id, response_tx);
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

    /// Register a Sony sequence number for a command.
    pub fn register_sequence(&mut self, cmd_id: u32, sequence: u32) {
        self.core.register_sequence(cmd_id, sequence);
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
            BasicKind::Ack => SchedulerEvent::Ack {
                socket: basic.socket.unwrap_or(ViscaSocket::S1),
            },
            BasicKind::Completion => {
                // For Sony, try to use sequence to find command
                let _cmd_id = sequence.and_then(|seq| self.core.get_command_by_sequence(seq));

                let response = lift_inquiry(&basic, None)?;
                SchedulerEvent::Completion {
                    socket: basic.socket,
                    response,
                }
            }
            BasicKind::Error(code) => {
                // Track error types
                match code {
                    0x03 | 0x04 => self.metrics.busy_errors += 1,
                    _ => self.metrics.protocol_errors += 1,
                }

                SchedulerEvent::Error {
                    socket: basic.socket,
                    code,
                }
            }
            BasicKind::DataReply => {
                // Data replies (inquiries) bypass the scheduler
                return Ok(());
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

                if let Some(tx) = self.response_channels.remove(&id) {
                    let _ = tx.send_async(Ok(response)).await;
                }
            }
            SchedulerAction::CommandFailed { id, error } => {
                self.metrics.commands_failed += 1;

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
}

/// Process an inquiry directly without the scheduler.
pub async fn process_inquiry<T: AsyncTransport>(
    transport: &mut T,
    inquiry: TxItem,
    envelope: &TransportEnvelope,
    buffer_manager: &BufferManager,
) -> Result<()> {
    if let TxItem::Inquiry {
        bytes,
        response_type,
        response_tx,
        ..
    } = inquiry
    {
        // Frame and send
        let kind = crate::command::CommandKind::Inquiry;
        let (framed, _meta) = envelope.frame_bytes_with_kind_owned(bytes, kind, buffer_manager);

        transport.send(&framed).await?;

        // Receive response
        let response_bytes = transport.recv().await?;
        let (payload, _meta) = envelope.extract_with_meta_owned(response_bytes)?;

        // Parse response
        let basic = decode_basic(&payload).ok_or_else(|| Error::InvalidResponse {
            expected: std::borrow::Cow::Borrowed("Valid VISCA response"),
            actual: payload.to_vec(),
        })?;

        let response = lift_inquiry(&basic, response_type.as_ref())?;

        // Send response through channel
        let _ = response_tx.send_async(Ok(response)).await;

        Ok(())
    } else {
        Err(Error::InvalidResponse {
            expected: std::borrow::Cow::Borrowed("Inquiry"),
            actual: vec![],
        })
    }
}
