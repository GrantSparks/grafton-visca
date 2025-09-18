//! Blocking runner for VISCA commands using the scheduler core.
//!
//! This module provides a blocking implementation that uses the runtime-agnostic
//! scheduler core to manage Sony sequence tracking, ACK/completion routing, and
//! retry logic without any async dependencies.

use tracing::{debug, trace, warn};

use std::{
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, Instant},
};

use crate::{
    camera_id::CameraId,
    capabilities::Profile,
    command::{
        response::{lift_inquiry_for, Response},
        CommandKind, ViscaCommand,
    },
    error::{Error, Result},
    protocol::{
        framer::ProtocolFramer,
        response::{decode_basic, BasicKind},
    },
    runtime::{
        core::{PendingCommand, Priority, SchedulerAction, SchedulerCore, SchedulerEvent},
        driver::{scheduler::BlockingScheduler, send_one},
    },
    timeout::{CommandCategory, TimeoutConfig},
    transport::{
        buffer::{BufferConfig, BufferManager},
        builder::AddressingMode,
        envelope::Envelope,
        BlockingTransport, HasTransportConfig, RetryConfig,
    },
};

/// Blocking runner for VISCA commands.
///
/// This runner uses the scheduler core to manage command execution in blocking mode,
/// providing the same protocol state machine as the async runtime but without channels
/// or async executors.
#[derive(Debug)]
pub struct BlockingRunner<P: Profile> {
    /// The scheduler core for state management.
    core: SchedulerCore,
    /// Transport envelope for framing.
    envelope: P::Envelope,
    /// Buffer manager for efficient memory usage.
    buffer_manager: BufferManager,
    /// Protocol framer for extracting frames from stream data.
    framer: ProtocolFramer,
    /// Command ID generator.
    next_id: AtomicU32,
    /// Profile type marker.
    _profile: core::marker::PhantomData<P>,
}

impl<P: Profile> BlockingRunner<P> {
    /// Create a new blocking runner.
    pub fn new(timeout_config: TimeoutConfig) -> Self {
        Self::new_with_retry(timeout_config, RetryConfig::default())
    }

    /// Create a new blocking runner with retry configuration.
    pub fn new_with_retry(timeout_config: TimeoutConfig, retry_config: RetryConfig) -> Self {
        let buffer_config = BufferConfig::default();
        Self::new_with_buffer(timeout_config, retry_config, buffer_config)
    }

    /// Create a new blocking runner with full configuration including buffer config.
    pub fn new_with_buffer(
        timeout_config: TimeoutConfig,
        retry_config: RetryConfig,
        buffer_config: BufferConfig,
    ) -> Self {
        Self::new_with_addressing(
            timeout_config,
            retry_config,
            buffer_config,
            AddressingMode::Ip,
        )
    }

    /// Create a new blocking runner with full configuration including addressing mode.
    pub fn new_with_addressing(
        timeout_config: TimeoutConfig,
        retry_config: RetryConfig,
        buffer_config: BufferConfig,
        addressing: AddressingMode,
    ) -> Self {
        Self {
            core: SchedulerCore::with_retry_config(timeout_config, retry_config),
            envelope: P::Envelope::new(addressing),
            buffer_manager: BufferManager::new(buffer_config),
            framer: ProtocolFramer::new_with_config(buffer_config),
            next_id: AtomicU32::new(1),
            _profile: core::marker::PhantomData,
        }
    }

    /// Send a command and wait for the response.
    pub fn send_command<T: BlockingTransport + HasTransportConfig>(
        &mut self,
        transport: &mut T,
        command: &(impl ViscaCommand + std::fmt::Debug + Clone + 'static),
        camera_id: CameraId,
        category: CommandCategory,
    ) -> Result<Response> {
        let cmd_id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let prepared_cmd = std::sync::Arc::new(
            crate::command::encode::PreparedCommand::new(command.clone(), camera_id).map_err(
                |e| {
                    tracing::error!("Failed to prepare command: {:?}", e);
                    e
                },
            )?,
        );

        if let Some(rt) = prepared_cmd.response_type {
            self.core.register_inquiry_type(cmd_id, rt);
        }

        let now = Instant::now();
        let kind = prepared_cmd.kind;
        let pending_cmd = PendingCommand {
            id: cmd_id,
            command: prepared_cmd,
            priority: Priority::Normal,
            category,
            camera_id,
            submitted_at: now,
            kind,
        };

        self.core.queue_command(pending_cmd);

        self.run_until_complete(transport, cmd_id)
    }

    /// Update the timeout configuration.
    pub fn update_timeout_config(&mut self, timeout_config: TimeoutConfig) {
        self.core.set_timeout_config(timeout_config);
    }

    /// Run the scheduler until a specific command completes.
    fn run_until_complete<T: BlockingTransport + HasTransportConfig>(
        &mut self,
        transport: &mut T,
        target_cmd_id: u32,
    ) -> Result<Response> {
        let mut read_buf = vec![0u8; self.buffer_manager.config().recv_buffer_size];

        loop {
            let now = Instant::now();

            if let Some(cmd) = self.core.next_item_to_send() {
                let kind = cmd.kind;

                let mut scheduler = BlockingScheduler {
                    core: &mut self.core,
                    now,
                };

                let write_timeout = transport.transport_config().write_timeout;

                let pending_cmd = PendingCommand {
                    id: cmd.id,
                    command: cmd.command.clone(),
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
                    continue;
                }
            }

            let ready_retries = self.core.get_ready_retries(now);
            for retry in ready_retries {
                let kind = retry.kind;

                let mut scheduler = BlockingScheduler {
                    core: &mut self.core,
                    now,
                };

                let write_timeout = transport.transport_config().write_timeout;

                let pending_cmd = PendingCommand {
                    id: retry.id,
                    command: retry.command.clone(),
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
                    continue;
                }
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

            let timeout_actions = self.core.check_timeouts(now);
            for action in timeout_actions {
                match action {
                    SchedulerAction::CommandFailed { id, error } if id == target_cmd_id => {
                        return Err(error);
                    }
                    SchedulerAction::RetryCommand { .. } => {}
                    _ => {}
                }
            }

            match transport.recv_into_with_timeout(&mut read_buf, Duration::from_millis(10)) {
                Ok(0) => {
                    warn!("Connection closed by peer");
                    return Err(Error::ConnectionClosed {
                        reason: Some("peer closed connection".into()),
                    });
                }
                Ok(n) => {
                    trace!("Received {} bytes from transport", n);
                    if let Err(e) = self.framer.push_slice(&read_buf[..n]) {
                        warn!("Framer buffer exceeded limits: {e}");
                        continue;
                    }

                    for frame_result in self.framer.drain_frames() {
                        let frame = match frame_result {
                            Ok(frame) => frame,
                            Err(e) => {
                                warn!("Failed to extract frame: {e}");
                                continue;
                            }
                        };

                        let (payload, meta) = match self.envelope.extract_with_meta(frame) {
                            Ok(result) => result,
                            Err(e) => {
                                warn!("Failed to extract response from frame: {e}");
                                continue;
                            }
                        };

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
                                let cmd_id = meta
                                    .sequence
                                    .and_then(|seq| self.core.get_command_by_sequence(seq));
                                debug!("Received ACK for socket {:?}, cmd_id {:?}", socket, cmd_id);
                                SchedulerEvent::Ack { socket, cmd_id }
                            }
                            BasicKind::Completion => {
                                let socket = basic.socket;

                                let cmd_id = if let Some(sequence) = meta.sequence {
                                    self.core.get_command_by_sequence(sequence)
                                } else {
                                    None
                                };

                                if let Some(cmd_id) = cmd_id {
                                    if cmd_id == target_cmd_id {
                                        debug!("Command {} completed successfully", cmd_id);
                                        let response_type = self.core.get_inquiry_type(cmd_id);
                                        let response =
                                            lift_inquiry_for::<P>(&basic, response_type)?;
                                        return Ok(response);
                                    }
                                }

                                debug!("Received completion for socket {:?}", socket);
                                let response_type =
                                    cmd_id.and_then(|id| self.core.get_inquiry_type(id));
                                let response = lift_inquiry_for::<P>(&basic, response_type)?;
                                SchedulerEvent::Completion {
                                    socket,
                                    cmd_id,
                                    response,
                                }
                            }
                            BasicKind::Error(code) => {
                                let socket = basic.socket;
                                let mut cmd_id = meta
                                    .sequence
                                    .and_then(|seq| self.core.get_command_by_sequence(seq));

                                if cmd_id.is_none() && socket.is_none() {
                                    use crate::command::response::payload::Payload;
                                    cmd_id = self
                                        .core
                                        .resolve_inquiry_id(Payload::new(&[]), meta.sequence);
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
                                let cmd_id =
                                    self.core.resolve_inquiry_id(basic.payload, meta.sequence);

                                let response_type =
                                    cmd_id.and_then(|id| self.core.get_inquiry_type(id).cloned());

                                if let Some(cmd_id) = cmd_id {
                                    if cmd_id == target_cmd_id {
                                        debug!("Inquiry {} completed successfully", cmd_id);
                                        let response =
                                            lift_inquiry_for::<P>(&basic, response_type.as_ref())?;
                                        return Ok(response);
                                    }
                                }

                                debug!("Received data reply (inquiry response)");
                                let response =
                                    lift_inquiry_for::<P>(&basic, response_type.as_ref())?;
                                SchedulerEvent::InquiryReply { cmd_id, response }
                            }
                            BasicKind::NetworkChange | BasicKind::Unknown => {
                                continue;
                            }
                        };

                        let actions = self.core.process_event(event, now);
                        for action in actions {
                            match action {
                                SchedulerAction::CommandComplete { id, response, .. }
                                    if id == target_cmd_id =>
                                {
                                    return Ok(response);
                                }
                                SchedulerAction::CommandFailed { id, error }
                                    if id == target_cmd_id =>
                                {
                                    return Err(error);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Err(Error::Timeout) => {}
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::camera::profiles::PtzOpticsG2;
    use crate::command::encode::ViscaCommand;

    #[test]
    fn test_scheduler_core_creation() {
        let timeout_config = TimeoutConfig::default();
        let runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);

        // Verify the runner was created successfully
        assert!(runner.core.can_send_command());
    }

    #[test]
    fn test_scheduler_core_with_raw_visca() {
        let timeout_config = TimeoutConfig::default();
        let runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);

        // Verify the runner was created (protocol is determined by Profile type)
        assert!(runner.core.can_send_command());
    }

    #[test]
    fn test_pending_command_queue() {
        use crate::command::bytes::VISCA_TERMINATOR;

        let timeout_config = TimeoutConfig::default();
        let mut runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);

        // Create a test command helper struct
        #[derive(Debug, Clone)]
        struct TestCmd {
            bytes: Vec<u8>,
        }

        impl ViscaCommand for TestCmd {
            type Response = ();
            const MAX_SIZE: usize = 6;
            const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

            fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                let len = self.bytes.len();
                buffer[..len].copy_from_slice(&self.bytes);
                Ok(len)
            }

            fn response_kind(&self) -> Option<crate::command::response::InquiryKind> {
                None
            }
        }

        // Create a pending command
        // Use a valid camera ID - 1 is always valid
        let camera_id = CameraId::CAMERA_1;
        let test_cmd = TestCmd {
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        };
        let prepared_cmd = std::sync::Arc::new(
            crate::command::encode::PreparedCommand::new(test_cmd, camera_id).unwrap(),
        );

        let cmd = PendingCommand {
            id: 1,
            command: prepared_cmd.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Quick,
            camera_id,
            submitted_at: Instant::now(),
            kind: prepared_cmd.kind,
        };

        // Queue the command
        runner.core.queue_command(cmd);

        // Verify it can be retrieved
        let next = runner.core.next_item_to_send();
        assert!(next.is_some(), "should have command");
        if let Some(cmd) = next {
            assert_eq!(cmd.id, 1);
        }
    }
}
