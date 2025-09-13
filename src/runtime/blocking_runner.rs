//! Blocking runner for VISCA commands using the scheduler core.
//!
//! This module provides a blocking implementation that uses the runtime-agnostic
//! scheduler core to manage Sony sequence tracking, ACK/completion routing, and
//! retry logic without any async dependencies.

use bytes::BytesMut;
use tracing::{debug, warn};

use std::{
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, Instant},
};

use crate::{
    camera_id::CameraId,
    capabilities::ProtocolStyle,
    command::{
        response::{lift_inquiry, ViscaResponse},
        CommandKind, ViscaEncode,
    },
    error::{Error, Result},
    protocol::response::{decode_basic, BasicKind},
    runtime::{
        core::{PendingCommand, Priority, SchedulerAction, SchedulerCore, SchedulerEvent},
        driver::{scheduler::BlockingScheduler, send_one},
    },
    timeout::{CommandCategory, TimeoutConfig},
    transport::{
        buffer::{BufferConfig, BufferManager},
        envelope::TransportEnvelope,
        RetryConfig, SyncTransport,
    },
    visca_socket::ViscaSocket,
};

/// Blocking runner for VISCA commands.
///
/// This runner uses the scheduler core to manage command execution in blocking mode,
/// providing the same protocol state machine as the async runtime but without channels
/// or async executors.
#[derive(Debug)]
pub struct BlockingRunner {
    /// The scheduler core for state management.
    core: SchedulerCore,
    /// Transport envelope for framing.
    envelope: TransportEnvelope,
    /// Buffer manager for efficient memory usage.
    buffer_manager: BufferManager,
    /// Command ID generator.
    next_id: AtomicU32,
}

impl BlockingRunner {
    /// Create a new blocking runner.
    pub fn new(style: ProtocolStyle, timeout_config: TimeoutConfig) -> Self {
        // Use default retry config for backward compatibility
        Self::new_with_retry(style, timeout_config, RetryConfig::default())
    }

    /// Create a new blocking runner with retry configuration.
    pub fn new_with_retry(
        style: ProtocolStyle,
        timeout_config: TimeoutConfig,
        retry_config: RetryConfig,
    ) -> Self {
        let buffer_config = if matches!(style, ProtocolStyle::SonyEncapsulated) {
            BufferConfig::for_sony_ip()
        } else {
            BufferConfig::default()
        };

        Self {
            core: SchedulerCore::with_retry_config(timeout_config, retry_config),
            envelope: TransportEnvelope::new(style),
            buffer_manager: BufferManager::new(buffer_config),
            next_id: AtomicU32::new(1),
        }
    }

    /// Send a command and wait for the response.
    pub fn send_command<T: SyncTransport>(
        &mut self,
        transport: &mut T,
        command: &impl ViscaEncode,
        camera_id: CameraId,
        category: CommandCategory,
    ) -> Result<ViscaResponse> {
        let cmd_id = self.next_id.fetch_add(1, Ordering::SeqCst);

        // Encode command using zero-copy path
        let visca_bytes = command.try_into_bytes(camera_id)?;

        // Store response type in core for inquiries
        if let Some(rt) = command.response_type() {
            self.core.register_inquiry_type(cmd_id, rt);
        }

        // Queue the command (both commands and inquiries use the unified path)
        let now = Instant::now();
        // Determine kind from bytes
        let kind = if visca_bytes.len() > 1 && visca_bytes[1] == 0x09 {
            CommandKind::Inquiry
        } else {
            CommandKind::Command
        };
        let pending_cmd = PendingCommand {
            id: cmd_id,
            bytes: visca_bytes.clone(),
            priority: Priority::Normal,
            category,
            camera_id,
            submitted_at: now,
            kind,
        };

        self.core.queue_command(pending_cmd);

        // Process until command completes
        self.run_until_complete(transport, cmd_id)
    }

    /// Run the scheduler until a specific command completes.
    fn run_until_complete<T: SyncTransport>(
        &mut self,
        transport: &mut T,
        target_cmd_id: u32,
    ) -> Result<ViscaResponse> {
        let _recv_buffer = BytesMut::with_capacity(256);

        loop {
            let now = Instant::now();

            // Check for commands to send
            if let Some(cmd) = self.core.next_command_to_send() {
                // Use command kind from PendingCommand
                let kind = cmd.kind;

                // Use the shared driver for sending
                let mut scheduler = BlockingScheduler {
                    core: &mut self.core,
                    now,
                };

                // Use configurable write timeout from transport config
                let write_timeout = Duration::from_millis(500); // Default, should come from config

                // Convert PendingCommand
                let pending_cmd = PendingCommand {
                    id: cmd.id,
                    bytes: cmd.bytes.clone(),
                    priority: cmd.priority,
                    category: cmd.category,
                    camera_id: cmd.camera_id,
                    submitted_at: now,
                    kind,
                };

                if let Err(e) = send_one(
                    transport,
                    &mut scheduler,
                    pending_cmd,
                    &self.envelope,
                    &self.buffer_manager,
                    write_timeout,
                ) {
                    debug!("Send operation failed: {:?}", e);
                    // send_one already handled retry scheduling via schedule_retry_after_send_error
                    // The error returned here means send failed but retry was scheduled
                    // Continue processing other commands
                    continue;
                }
            }

            // Check for retries
            let ready_retries = self.core.get_ready_retries(now);
            for retry in ready_retries {
                // Determine command kind
                let kind = if retry.bytes.len() > 1 && retry.bytes[1] == 0x09 {
                    CommandKind::Inquiry
                } else {
                    CommandKind::Command
                };

                // Use the shared driver for sending retries
                let mut scheduler = BlockingScheduler {
                    core: &mut self.core,
                    now,
                };

                // Use configurable write timeout from transport config
                let write_timeout = Duration::from_millis(500); // Default, should come from config

                // Convert to PendingCommand
                let pending_cmd = PendingCommand {
                    id: retry.id,
                    bytes: retry.bytes.clone(),
                    priority: retry.priority,
                    category: retry.category,
                    camera_id: retry.camera_id,
                    submitted_at: now,
                    kind,
                };

                if let Err(e) = send_one(
                    transport,
                    &mut scheduler,
                    pending_cmd,
                    &self.envelope,
                    &self.buffer_manager,
                    write_timeout,
                ) {
                    debug!("Send retry operation failed: {:?}", e);
                    // send_one already handled retry scheduling
                    // For retries, check if budget is exhausted for our target command
                    continue;
                }

                // Core handles inquiry tracking now
                debug!(
                    "Sent retry for {} {} (attempt {})",
                    if kind == CommandKind::Inquiry {
                        "inquiry"
                    } else {
                        "command"
                    },
                    retry.id,
                    retry.attempt
                );
            }

            // Check for timeouts
            let timeout_actions = self.core.check_timeouts(now);
            for action in timeout_actions {
                match action {
                    SchedulerAction::CommandFailed { id, error } if id == target_cmd_id => {
                        return Err(error);
                    }
                    SchedulerAction::RetryCommand { .. } => {
                        // Retry will be handled in next iteration
                    }
                    _ => {}
                }
            }

            // Try to receive a response with short timeout
            match transport.recv_with_timeout(Duration::from_millis(10)) {
                Ok(response_bytes) => {
                    // Extract payload and metadata
                    let (payload, meta) = self.envelope.extract_with_meta_owned(response_bytes)?;

                    // Parse VISCA response type using decode_basic
                    let basic = match decode_basic(&payload) {
                        Some(b) => b,
                        None => {
                            warn!("Failed to decode VISCA frame: {:02X?}", payload);
                            continue;
                        }
                    };

                    let event = match basic.kind {
                        BasicKind::Ack => {
                            let socket = basic.socket;
                            // For Sony, try to use sequence to find command
                            let cmd_id = meta
                                .sequence
                                .and_then(|seq| self.core.get_command_by_sequence(seq));
                            debug!("Received ACK for socket {:?}, cmd_id {:?}", socket, cmd_id);
                            SchedulerEvent::Ack {
                                socket: socket.unwrap_or(ViscaSocket::S1),
                                cmd_id,
                            }
                        }
                        BasicKind::Completion => {
                            let socket = basic.socket;

                            // For Sony, try to use sequence to find command
                            let cmd_id = if let Some(sequence) = meta.sequence {
                                self.core.get_command_by_sequence(sequence)
                            } else {
                                None
                            };

                            if let Some(cmd_id) = cmd_id {
                                if cmd_id == target_cmd_id {
                                    debug!("Command {} completed successfully", cmd_id);
                                    // Get the expected response type from core
                                    let response_type = self.core.get_inquiry_type(cmd_id);
                                    // Convert to ViscaResponse for return
                                    let response = lift_inquiry(&basic, response_type)?;
                                    return Ok(response);
                                }
                            }

                            debug!("Received completion for socket {:?}", socket);
                            // Get the expected response type from core
                            let response_type =
                                cmd_id.and_then(|id| self.core.get_inquiry_type(id));
                            let response = lift_inquiry(&basic, response_type)?;
                            SchedulerEvent::Completion {
                                socket,
                                cmd_id,
                                response,
                            }
                        }
                        BasicKind::Error(code) => {
                            let socket = basic.socket;
                            // For Sony, try to use sequence to find command
                            let mut cmd_id = meta
                                .sequence
                                .and_then(|seq| self.core.get_command_by_sequence(seq));

                            // If no cmd_id and no socket, this could be an inquiry error
                            // Use resolve_inquiry_id to try to match it
                            if cmd_id.is_none() && socket.is_none() {
                                // For error responses, we can't use content-based matching on the error code,
                                // but we can use FIFO from the inquiry queue
                                cmd_id = self.core.resolve_inquiry_id(&[], meta.sequence);
                            }

                            debug!(
                                "Received error 0x{:02X} for socket {:?}, cmd_id {:?}",
                                code, socket, cmd_id
                            );
                            SchedulerEvent::Error {
                                socket,
                                cmd_id,
                                code,
                            }
                        }
                        BasicKind::DataReply => {
                            // Data replies are completions for inquiries
                            // Use the core's centralized resolution
                            let cmd_id = self.core.resolve_inquiry_id(&payload, meta.sequence);

                            // Get the expected response type from core
                            let response_type =
                                cmd_id.and_then(|id| self.core.get_inquiry_type(id).cloned());

                            if let Some(cmd_id) = cmd_id {
                                if cmd_id == target_cmd_id {
                                    debug!("Inquiry {} completed successfully", cmd_id);
                                    // Convert to ViscaResponse for return
                                    let response = lift_inquiry(&basic, response_type.as_ref())?;
                                    return Ok(response);
                                }
                            }

                            debug!("Received data reply (inquiry response)");
                            let response = lift_inquiry(&basic, response_type.as_ref())?;
                            // Use InquiryReply event for data replies
                            SchedulerEvent::InquiryReply { cmd_id, response }
                        }
                        BasicKind::NetworkChange | BasicKind::Unknown => {
                            // Other response types are ignored for now
                            continue;
                        }
                    };

                    // Process the event
                    let actions = self.core.process_event(event, now);
                    for action in actions {
                        match action {
                            SchedulerAction::CommandComplete { id, response, .. }
                                if id == target_cmd_id =>
                            {
                                // Core handles all inquiry cleanup now
                                return Ok(response);
                            }
                            SchedulerAction::CommandFailed { id, error } if id == target_cmd_id => {
                                // Core handles all inquiry cleanup now
                                return Err(error);
                            }
                            _ => {}
                        }
                    }
                }
                Err(Error::Timeout) => {
                    // No data available, continue
                }
                Err(e) => {
                    warn!("Transport receive error: {}", e);
                    // Generate network error event
                    let actions = self
                        .core
                        .process_event(SchedulerEvent::NetworkError(e), now);
                    for action in actions {
                        match action {
                            SchedulerAction::CommandFailed { id, error } if id == target_cmd_id => {
                                return Err(error);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn test_scheduler_core_creation() {
        let timeout_config = TimeoutConfig::default();
        let runner = BlockingRunner::new(ProtocolStyle::SonyEncapsulated, timeout_config);

        // Verify the runner was created successfully
        assert!(runner.core.can_send_command());
    }

    #[test]
    fn test_scheduler_core_with_raw_visca() {
        let timeout_config = TimeoutConfig::default();
        let runner = BlockingRunner::new(ProtocolStyle::RawVisca, timeout_config);

        // Verify the runner was created for raw VISCA
        assert!(runner.core.can_send_command());
    }

    #[test]
    fn test_pending_command_queue() {
        use crate::command::bytes::VISCA_TERMINATOR;

        let timeout_config = TimeoutConfig::default();
        let mut runner = BlockingRunner::new(ProtocolStyle::SonyEncapsulated, timeout_config);

        // Create a pending command
        // Use a valid camera ID - 1 is always valid
        let camera_id = CameraId::CAMERA_1;
        let cmd = PendingCommand {
            id: 1,
            bytes: Bytes::from_static(&[0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]),
            priority: Priority::Normal,
            category: CommandCategory::Quick,
            camera_id,
            submitted_at: Instant::now(),
            kind: CommandKind::Command,
        };

        // Queue the command
        runner.core.queue_command(cmd);

        // Verify it can be retrieved
        let next = runner.core.next_command_to_send();
        assert!(next.is_some());
        assert!(next.is_some(), "should have command");
        if let Some(cmd) = next {
            assert_eq!(cmd.id, 1);
        }
    }
}
