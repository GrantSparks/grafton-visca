//! Blocking runner for VISCA commands using the scheduler core.
//!
//! This module provides a blocking implementation that uses the runtime-agnostic
//! scheduler core to manage Sony sequence tracking, ACK/completion routing, and
//! retry logic without any async dependencies.

use std::{
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, Instant},
};

use bytes::{Bytes, BytesMut};
use tracing::{debug, trace, warn};

use crate::{
    camera_id::CameraId,
    capabilities::ProtocolStyle,
    command::{
        response::{lift_inquiry, ViscaResponse},
        CommandKind, ViscaEncode,
    },
    error::{Error, Result},
    protocol::response::{decode_basic, BasicKind},
    runtime::core::{PendingCommand, Priority, SchedulerAction, SchedulerCore, SchedulerEvent},
    timeout::{CommandCategory, TimeoutConfig},
    transport::{
        buffer::{BufferConfig, BufferManager},
        envelope::TransportEnvelope,
        SyncTransport,
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
        let buffer_config = if matches!(style, ProtocolStyle::SonyEncapsulated { .. }) {
            BufferConfig::for_sony_ip()
        } else {
            BufferConfig::default()
        };

        Self {
            core: SchedulerCore::new(timeout_config),
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

        // Encode command
        let mut buf = [0u8; 64];
        let len = command.encode_into(camera_id, &mut buf)?;
        let visca_bytes = Bytes::copy_from_slice(&buf[..len]);

        let kind = command.command_kind();
        let is_inquiry = matches!(kind, CommandKind::Inquiry);

        // For inquiries, we can send directly without the scheduler
        if is_inquiry {
            return self.send_inquiry(transport, visca_bytes, kind);
        }

        // Queue the command
        let now = Instant::now();
        let pending_cmd = PendingCommand {
            id: cmd_id,
            bytes: visca_bytes.clone(),
            priority: Priority::Normal,
            category,
            camera_id,
            submitted_at: now,
        };

        self.core.queue_command(pending_cmd);

        // Process until command completes
        self.run_until_complete(transport, cmd_id)
    }

    /// Send an inquiry directly without using the scheduler.
    fn send_inquiry<T: SyncTransport>(
        &mut self,
        transport: &mut T,
        visca_bytes: Bytes,
        kind: CommandKind,
    ) -> Result<ViscaResponse> {
        // Frame and send
        let (framed, _meta) =
            self.envelope
                .frame_bytes_with_kind_owned(visca_bytes, kind, &self.buffer_manager);

        transport.send_with_kind(&framed, kind)?;

        // Receive response
        let response_bytes = transport.recv()?;
        let (payload, _meta) = self.envelope.extract_with_meta_owned(response_bytes)?;

        // Parse response using decode_basic and lift_inquiry
        let basic = decode_basic(&payload).ok_or_else(|| Error::InvalidResponse {
            expected: std::borrow::Cow::Borrowed("Valid VISCA response"),
            actual: payload.to_vec(),
        })?;

        lift_inquiry(&basic, None)
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
                // Frame the command
                let kind = if cmd.bytes[1] == 0x09 {
                    CommandKind::Inquiry
                } else {
                    CommandKind::Command
                };

                let (framed, meta) = self.envelope.frame_bytes_with_kind_owned(
                    cmd.bytes.clone(),
                    kind,
                    &self.buffer_manager,
                );

                // Register as pending ACK
                self.core.register_pending_ack(
                    cmd.id,
                    cmd.bytes,
                    cmd.priority,
                    cmd.category,
                    cmd.camera_id,
                    now,
                );

                // Register Sony sequence if applicable
                if let Some(sequence) = meta.sequence {
                    self.core.register_sequence(cmd.id, sequence);
                }

                // Send the command
                transport.send_with_kind(&framed, kind)?;
                trace!("Sent command {} with sequence {:?}", cmd.id, meta.sequence);
            }

            // Check for retries
            let ready_retries = self.core.get_ready_retries(now);
            for retry in ready_retries {
                // Frame and send the retry
                let kind = if retry.bytes[1] == 0x09 {
                    CommandKind::Inquiry
                } else {
                    CommandKind::Command
                };

                let (framed, meta) = self.envelope.frame_bytes_with_kind_owned(
                    retry.bytes.clone(),
                    kind,
                    &self.buffer_manager,
                );

                // Register as pending ACK
                self.core.register_pending_ack(
                    retry.id,
                    retry.bytes,
                    retry.priority,
                    retry.category,
                    retry.camera_id,
                    now,
                );

                // Register Sony sequence if applicable
                if let Some(sequence) = meta.sequence {
                    self.core.register_sequence(retry.id, sequence);
                }

                transport.send_with_kind(&framed, kind)?;
                debug!(
                    "Sent retry for command {} (attempt {})",
                    retry.id, retry.attempt
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
                            debug!("Received ACK for socket {:?}", socket);
                            SchedulerEvent::Ack {
                                socket: socket.unwrap_or(ViscaSocket::S1),
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
                                    // Convert to ViscaResponse for return
                                    let response = lift_inquiry(&basic, None)?;
                                    return Ok(response);
                                }
                            }

                            debug!("Received completion for socket {:?}", socket);
                            let response = lift_inquiry(&basic, None)?;
                            SchedulerEvent::Completion { socket, response }
                        }
                        BasicKind::Error(code) => {
                            let socket = basic.socket;
                            debug!("Received error 0x{:02X} for socket {:?}", code, socket);
                            SchedulerEvent::Error { socket, code }
                        }
                        BasicKind::DataReply | BasicKind::NetworkChange | BasicKind::Unknown => {
                            // Other response types (inquiries, etc) are handled separately
                            continue;
                        }
                    };

                    // Process the event
                    let actions = self.core.process_event(event, now);
                    for action in actions {
                        match action {
                            SchedulerAction::CommandComplete { id, response }
                                if id == target_cmd_id =>
                            {
                                return Ok(response);
                            }
                            SchedulerAction::CommandFailed { id, error } if id == target_cmd_id => {
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
                    let actions = self.core.process_event(SchedulerEvent::NetworkError, now);
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

    #[test]
    fn test_scheduler_core_creation() {
        let timeout_config = TimeoutConfig::default();
        let runner = BlockingRunner::new(
            ProtocolStyle::SonyEncapsulated { use_sequence: true },
            timeout_config,
        );

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
        let mut runner = BlockingRunner::new(
            ProtocolStyle::SonyEncapsulated { use_sequence: true },
            timeout_config,
        );

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
