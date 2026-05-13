//! Blocking runner for VISCA commands using the scheduler core.
//!
//! This module provides a blocking implementation that uses the runtime-agnostic
//! scheduler core to manage Sony sequence tracking, ACK/completion routing, and
//! retry logic without any async dependencies.

use bytes::BytesMut;
use tracing::{debug, trace, warn};

use core::marker::PhantomData;
use std::{
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, Instant},
};

use crate::{
    camera::inflight::CommandId,
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
        core::{
            PendingCommand, Priority, ReplySource, SchedulerAction, SchedulerCore, SchedulerEvent,
        },
        driver::{scheduler::BlockingScheduler, send_one, SendResult},
    },
    timeout::{Deadline, TimeoutConfig},
    transport::{
        buffer::{BufferConfig, BufferManager},
        builder::AddressingMode,
        envelope::Envelope,
        BlockingTransport, HasTransportConfig, RetryConfig, SendSemantics,
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
    _profile: PhantomData<P>,
}

/// Builder for constructing a [`BlockingRunner`] with custom configuration.
///
/// # Example
///
/// ```ignore
/// use grafton_visca::runtime::BlockingRunner;
/// use grafton_visca::camera::profiles::PtzOpticsG2;
///
/// let runner = BlockingRunner::<PtzOpticsG2>::builder(TimeoutConfig::default())
///     .retry_config(RetryConfig::default().max_retries(5))
///     .buffer_config(BufferConfig::default())
///     .addressing(AddressingMode::Serial)
///     .build();
/// ```
#[derive(Debug)]
pub struct BlockingRunnerBuilder<P: Profile> {
    timeout_config: TimeoutConfig,
    retry_config: RetryConfig,
    buffer_config: BufferConfig,
    addressing: AddressingMode,
    _profile: PhantomData<P>,
}

impl<P: Profile> BlockingRunnerBuilder<P> {
    /// Create a new builder with the given timeout configuration.
    fn new(timeout_config: TimeoutConfig) -> Self {
        Self {
            timeout_config,
            retry_config: RetryConfig::default(),
            buffer_config: BufferConfig::default(),
            addressing: AddressingMode::Ip,
            _profile: PhantomData,
        }
    }

    /// Set the retry configuration.
    #[must_use]
    pub fn retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    /// Set the buffer configuration.
    #[must_use]
    pub fn buffer_config(mut self, config: BufferConfig) -> Self {
        self.buffer_config = config;
        self
    }

    /// Set the addressing mode.
    #[must_use]
    pub fn addressing(mut self, mode: AddressingMode) -> Self {
        self.addressing = mode;
        self
    }

    /// Build the [`BlockingRunner`] with the configured options.
    pub fn build(self) -> BlockingRunner<P> {
        let mut core = SchedulerCore::with_retry_config(self.timeout_config, self.retry_config);
        // Apply profile-specific spacing
        core.set_min_inquiry_spacing(P::MIN_INQUIRY_SPACING);
        core.set_min_command_spacing(P::MIN_COMMAND_SPACING);
        BlockingRunner {
            core,
            envelope: P::Envelope::new(self.addressing),
            buffer_manager: BufferManager::new(self.buffer_config),
            framer: ProtocolFramer::new_with_config(self.buffer_config),
            next_id: AtomicU32::new(1),
            _profile: PhantomData,
        }
    }
}

impl<P: Profile> BlockingRunner<P> {
    /// Create a builder for configuring a new blocking runner.
    ///
    /// This is the preferred way to construct a `BlockingRunner` with custom
    /// retry, buffer, or addressing configuration.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let runner = BlockingRunner::<PtzOpticsG2>::builder(TimeoutConfig::default())
    ///     .retry_config(RetryConfig::default().max_retries(5))
    ///     .build();
    /// ```
    pub fn builder(timeout_config: TimeoutConfig) -> BlockingRunnerBuilder<P> {
        BlockingRunnerBuilder::new(timeout_config)
    }

    /// Create a new blocking runner with default configuration.
    ///
    /// This is a convenience method equivalent to:
    /// ```ignore
    /// BlockingRunner::builder(timeout_config).build()
    /// ```
    #[cfg(any(test, feature = "test-utils"))]
    pub fn new(timeout_config: TimeoutConfig) -> Self {
        Self::builder(timeout_config).build()
    }

    /// Send a command and wait for the response.
    ///
    /// The timeout category is derived from the command's `TIMEOUT_CATEGORY` constant,
    /// ensuring consistency between the command type and its timeout handling.
    pub fn send_command<T: BlockingTransport + HasTransportConfig>(
        &mut self,
        transport: &mut T,
        command: &impl ViscaCommand,
        camera_id: CameraId,
    ) -> Result<Response> {
        self.send_command_with_deadline(transport, command, camera_id, None)
    }

    /// Send a command with an optional deadline for the entire operation.
    ///
    /// When a deadline is provided, the operation will return `Error::Timeout`
    /// if the deadline is exceeded, even if the command's category timeout
    /// hasn't been reached. This is useful for movement detection where
    /// individual inquiries must not exceed the overall operation budget.
    ///
    /// The timeout category is derived from the command's `TIMEOUT_CATEGORY` constant,
    /// ensuring consistency between the command type and its timeout handling.
    ///
    /// # Arguments
    ///
    /// * `transport` - The transport to send the command on
    /// * `command` - The VISCA command to send
    /// * `camera_id` - Target camera ID
    /// * `deadline` - Optional deadline; if `Some`, operation fails if deadline is exceeded
    pub fn send_command_with_deadline<T: BlockingTransport + HasTransportConfig>(
        &mut self,
        transport: &mut T,
        command: &impl ViscaCommand,
        camera_id: CameraId,
        deadline: Option<Deadline>,
    ) -> Result<Response> {
        // Check deadline before even starting
        if let Some(ref d) = deadline {
            if d.is_expired() {
                return Err(Error::Timeout);
            }
        }

        // Allocate a unique command ID, skipping zero on wraparound
        let cmd_id = loop {
            let id = self.next_id.fetch_add(1, Ordering::SeqCst);
            // Skip zero on wraparound (when u32::MAX wraps to 0)
            if let Some(cmd_id) = CommandId::from_raw(id) {
                break cmd_id;
            }
            // id was 0, loop again to get the next value (1)
        };

        let prepared_cmd = std::sync::Arc::new(
            crate::command::encode::EncodedCommand::new(command, camera_id).map_err(|e| {
                tracing::error!("Failed to prepare command: {e:?}");
                e
            })?,
        );

        let now = Instant::now();
        let pending_cmd = PendingCommand {
            id: cmd_id,
            command: prepared_cmd.clone(),
            priority: Priority::Normal,
            camera_id,
            submitted_at: now,
        };

        self.core.queue_command(pending_cmd);

        self.run_until_complete(transport, cmd_id, deadline)
    }

    /// Update the timeout configuration.
    pub fn update_timeout_config(&mut self, timeout_config: TimeoutConfig) {
        self.core.set_timeout_config(timeout_config);
    }

    /// Compute the effective receive timeout based on scheduler deadlines,
    /// call-level deadline, and transport configuration.
    ///
    /// This method returns the minimum non-zero duration among:
    /// - Time until the scheduler's next deadline (ACK timeout, socket timeout, retry, etc.)
    /// - Time until the call-level deadline (if present)
    /// - Transport's configured read timeout
    ///
    /// If the scheduler has no pending deadlines but a command is still inflight,
    /// this falls back to the transport's read timeout.
    fn compute_receive_timeout<T: HasTransportConfig>(
        &self,
        transport: &T,
        now: Instant,
        call_deadline: Option<&Deadline>,
    ) -> Duration {
        let transport_timeout = transport.transport_config().read_timeout;

        // Get scheduler's next deadline
        let core_wait = self
            .core
            .next_deadline(now)
            .map(|deadline| deadline.saturating_duration_since(now));

        // Get call-level deadline remaining time
        let call_wait = call_deadline.map(|d| d.remaining_at(now));

        // Select the minimum non-zero timeout
        let recv_timeout = match (core_wait, call_wait) {
            (Some(core), Some(call)) => {
                // Both present: take the minimum
                let min = core.min(call);
                if min.is_zero() {
                    // If we have a zero timeout, iterate immediately
                    Duration::ZERO
                } else {
                    min.min(transport_timeout)
                }
            }
            (Some(core), None) => {
                if core.is_zero() {
                    Duration::ZERO
                } else {
                    core.min(transport_timeout)
                }
            }
            (None, Some(call)) => {
                if call.is_zero() {
                    Duration::ZERO
                } else {
                    call.min(transport_timeout)
                }
            }
            (None, None) => {
                // No scheduler deadline and no call deadline.
                // This is an edge case (invariant: if a command is pending, there should be
                // a scheduler deadline). Fall back to transport timeout.
                if std::env::var("RUNTIME_TRACE").as_deref() == Ok("1") {
                    eprintln!(
                        "[BlockingRunner] No scheduler or call deadline while command pending; \
                        falling back to transport_timeout={:?}",
                        transport_timeout
                    );
                }
                transport_timeout
            }
        };

        recv_timeout
    }

    /// Run the scheduler until a specific command completes.
    ///
    /// # Arguments
    ///
    /// * `transport` - The transport to communicate with
    /// * `target_cmd_id` - The command ID we're waiting for
    /// * `deadline` - Optional deadline; returns `Error::Timeout` if exceeded
    fn run_until_complete<T: BlockingTransport + HasTransportConfig>(
        &mut self,
        transport: &mut T,
        target_cmd_id: CommandId,
        deadline: Option<Deadline>,
    ) -> Result<Response> {
        let mut read_buf = vec![0u8; self.buffer_manager.config().recv_buffer_size];

        let mut send_buf = BytesMut::with_capacity(self.buffer_manager.config().send_buffer_size);

        loop {
            // Check external deadline first - this ensures movement detection
            // respects the overall timeout budget
            if let Some(ref d) = deadline {
                if d.is_expired() {
                    // Clean up the pending command before returning
                    self.core.cancel_command(target_cmd_id);
                    trace!("Command {target_cmd_id} cancelled due to deadline expiration");
                    return Err(Error::Timeout);
                }
            }

            let now = Instant::now();

            if let Some(cmd) = self.core.next_item_to_send(now) {
                let mut scheduler = BlockingScheduler {
                    core: &mut self.core,
                    now,
                };

                let write_timeout = transport.transport_config().write_timeout;

                let pending_cmd = PendingCommand {
                    id: cmd.id,
                    command: cmd.command.clone(),
                    priority: cmd.priority,
                    camera_id: cmd.camera_id,
                    submitted_at: now,
                };

                match send_one(
                    transport,
                    &mut scheduler,
                    pending_cmd,
                    &self.envelope,
                    &mut send_buf,
                    write_timeout,
                ) {
                    SendResult::Ok => {}
                    SendResult::Err { error, action } => {
                        debug!("Send operation failed: {error:?}");
                        // For stream transports, a send failure poisons the transport
                        if transport.send_semantics() == SendSemantics::Stream {
                            let reason = format!("Send failed: {error}");
                            tracing::error!(
                                send_semantics = ?transport.send_semantics(),
                                reason = %reason,
                                "Stream transport poisoned - failing command and exiting"
                            );
                            self.core.clear_all();
                            return Err(Error::StreamPoisoned {
                                reason: reason.into(),
                            });
                        }
                        // For datagram transports, check if this failure is for our target command
                        if let Some(SchedulerAction::CommandFailed { id, error }) = action {
                            if id == target_cmd_id {
                                return Err(error);
                            }
                        }
                        continue;
                    }
                }
            }

            let ready_retries = self.core.get_ready_retries(now);
            for retry in ready_retries {
                let kind = retry.kind();

                let mut scheduler = BlockingScheduler {
                    core: &mut self.core,
                    now,
                };

                let write_timeout = transport.transport_config().write_timeout;

                let pending_cmd = PendingCommand {
                    id: retry.id,
                    command: retry.command.clone(),
                    priority: retry.priority,
                    camera_id: retry.camera_id,
                    submitted_at: now,
                };

                match send_one(
                    transport,
                    &mut scheduler,
                    pending_cmd,
                    &self.envelope,
                    &mut send_buf,
                    write_timeout,
                ) {
                    SendResult::Ok => {
                        debug!(
                            "Sent retry for {} {retry_id} (attempt {attempt})",
                            if kind == CommandKind::Inquiry {
                                "inquiry"
                            } else {
                                "command"
                            },
                            retry_id = retry.id,
                            attempt = retry.attempt
                        );
                    }
                    SendResult::Err { error, action } => {
                        debug!("Send retry operation failed: {error:?}");
                        // For stream transports, a send failure poisons the transport
                        if transport.send_semantics() == SendSemantics::Stream {
                            let reason = format!("Send failed during retry: {error}");
                            tracing::error!(
                                send_semantics = ?transport.send_semantics(),
                                reason = %reason,
                                "Stream transport poisoned - failing command and exiting"
                            );
                            self.core.clear_all();
                            return Err(Error::StreamPoisoned {
                                reason: reason.into(),
                            });
                        }
                        // For datagram transports, check if this failure is for our target command
                        if let Some(SchedulerAction::CommandFailed { id, error }) = action {
                            if id == target_cmd_id {
                                return Err(error);
                            }
                        }
                        continue;
                    }
                }
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

            // Refresh timestamp and compute deadline-driven receive timeout
            let now = Instant::now();
            let recv_timeout = self.compute_receive_timeout(transport, now, deadline.as_ref());

            match transport.recv_into_with_timeout(&mut read_buf, recv_timeout) {
                Ok(0) => {
                    warn!("Connection closed by peer");
                    return Err(Error::ConnectionClosed {
                        reason: Some("peer closed connection".into()),
                    });
                }
                Ok(n) => {
                    trace!("Received {n} bytes from transport");

                    // Use push_slice_with_resync to handle buffer overflow gracefully.
                    // This clears the buffer and retries if overflow occurs, preventing
                    // permanent runtime stalls from un-framable data accumulation.
                    match self.framer.push_slice_with_resync(&read_buf[..n]) {
                        Ok(normal_push) => {
                            if !normal_push {
                                debug!("Framer resynced after buffer overflow");
                            }
                        }
                        Err(e) => {
                            // Resync failed - chunk alone exceeds max_buffer_size.
                            // This indicates a configuration issue but we continue
                            // to allow processing and avoid permanent stall.
                            warn!("Framer resync failed (chunk exceeds max_buffer_size): {e}");
                        }
                    }

                    // Always drain frames after push attempt (even after resync)
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
                                warn!("Failed to decode VISCA frame: {payload:02X?}");
                                continue;
                            }
                        };

                        let event = match basic.kind {
                            BasicKind::Ack => {
                                let socket = basic.socket;
                                let cmd_id = meta
                                    .sequence
                                    .and_then(|seq| self.core.get_command_by_sequence(seq));
                                debug!("Received ACK for socket {socket:?}, cmd_id {cmd_id:?}");
                                let source =
                                    ReplySource::from_fields(cmd_id, meta.sequence, socket);
                                SchedulerEvent::Ack { source }
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
                                        debug!("Command {cmd_id} completed successfully");
                                        // Clean up command state before returning
                                        // (Socket is embedded in phase, freed with command)
                                        self.core.complete_command(cmd_id);
                                        let response_type = self.core.get_inquiry_type(cmd_id);
                                        let response =
                                            lift_inquiry_for::<P>(&basic, response_type.as_ref())?;
                                        return Ok(response);
                                    }
                                }

                                debug!("Received completion for socket {socket:?}");
                                let response_type =
                                    cmd_id.and_then(|id| self.core.get_inquiry_type(id));
                                let response =
                                    lift_inquiry_for::<P>(&basic, response_type.as_ref())?;
                                let source =
                                    ReplySource::from_fields(cmd_id, meta.sequence, socket);
                                SchedulerEvent::Completion { source, response }
                            }
                            BasicKind::Error(code) => {
                                let socket = basic.socket;
                                let mut cmd_id = meta
                                    .sequence
                                    .and_then(|seq| self.core.get_command_by_sequence(seq));

                                // Only use FIFO fallback for raw VISCA (no sequence).
                                // For sequenced transports, process_event will gate the heuristic.
                                if cmd_id.is_none() && socket.is_none() && meta.sequence.is_none() {
                                    use crate::command::response::payload::Payload;
                                    cmd_id = self
                                        .core
                                        .resolve_inquiry_id(Payload::new(&[]), meta.sequence);
                                }

                                debug!(
                                    "Received error 0x{code:02X} for socket {socket:?}, cmd_id {cmd_id:?}"
                                );
                                let source =
                                    ReplySource::from_fields(cmd_id, meta.sequence, socket);
                                SchedulerEvent::Error { source, code }
                            }
                            BasicKind::DataReply => {
                                let cmd_id =
                                    self.core.resolve_inquiry_id(basic.payload, meta.sequence);

                                let response_type =
                                    cmd_id.and_then(|id| self.core.get_inquiry_type(id));

                                if let Some(cmd_id) = cmd_id {
                                    if cmd_id == target_cmd_id {
                                        debug!("Inquiry {cmd_id} completed successfully");
                                        // Clean up inquiry state before returning
                                        self.core.complete_inquiry(cmd_id);
                                        let response =
                                            lift_inquiry_for::<P>(&basic, response_type.as_ref())?;
                                        return Ok(response);
                                    }
                                }

                                debug!("Received data reply (inquiry response)");
                                let response =
                                    lift_inquiry_for::<P>(&basic, response_type.as_ref())?;
                                // InquiryReply has no socket, so pass None
                                let source = ReplySource::from_fields(cmd_id, meta.sequence, None);
                                SchedulerEvent::InquiryReply { source, response }
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
                    warn!("Transport receive error: {e}");
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::camera::profiles::PtzOpticsG2;
    use crate::command::encode::ViscaCommand;
    use crate::timeout::CommandCategory;
    use crate::transport::builder::TransportConfig;

    /// Helper function to create CommandId from u32 in tests.
    /// Panics if value is 0 (invalid for CommandId).
    fn cmd_id(value: u32) -> CommandId {
        CommandId::from_raw(value).expect("test command ID must be non-zero")
    }

    #[test]
    fn test_scheduler_core_creation() {
        let timeout_config = TimeoutConfig::default();
        let runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);
        let now = Instant::now();

        assert!(runner.core.can_send_command(now));
    }

    #[test]
    fn test_scheduler_core_with_raw_visca() {
        let timeout_config = TimeoutConfig::default();
        let runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);
        let now = Instant::now();

        assert!(runner.core.can_send_command(now));
    }

    #[test]
    fn test_pending_command_queue() {
        use crate::command::bytes::VISCA_TERMINATOR;

        let timeout_config = TimeoutConfig::default();
        let mut runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);

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

        let camera_id = CameraId::CAMERA_1;
        let test_cmd = TestCmd {
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        };
        let prepared_cmd = std::sync::Arc::new(
            crate::command::encode::EncodedCommand::new(&test_cmd, camera_id).unwrap(),
        );

        let cmd = PendingCommand {
            id: cmd_id(1),
            command: prepared_cmd.clone(),
            priority: Priority::Normal,
            camera_id,
            submitted_at: Instant::now(),
        };

        runner.core.queue_command(cmd);

        let now = Instant::now();
        let next = runner.core.next_item_to_send(now);
        assert!(next.is_some(), "should have command");
        if let Some(cmd) = next {
            assert_eq!(cmd.id, cmd_id(1));
        }
    }

    /// Mock transport that fails on send for testing send failure propagation.
    struct FailingSendTransport {
        config: TransportConfig,
    }

    impl FailingSendTransport {
        fn new() -> Self {
            Self {
                config: TransportConfig::default(),
            }
        }
    }

    impl BlockingTransport for FailingSendTransport {
        fn send_with_kind(&mut self, _bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
            Err(Error::TransportError("Simulated send failure".into()))
        }

        fn recv_into(&mut self, _dst: &mut [u8]) -> Result<usize, Error> {
            Ok(0)
        }

        fn recv_into_with_timeout(
            &mut self,
            _dst: &mut [u8],
            _timeout: Duration,
        ) -> Result<usize, Error> {
            Err(Error::Timeout)
        }

        // Use Datagram semantics to test send failure error propagation
        // without triggering stream poisoning behavior
        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    impl HasTransportConfig for FailingSendTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    /// Test that blocking send failures preserve the original error with context.
    ///
    /// This test verifies issue #469: when a transport send fails in blocking mode,
    /// the error should be propagated immediately with:
    /// 1. "Send failed" context wrapping the original error
    /// 2. The original error message preserved (e.g., "Simulated send failure")
    /// 3. The original error kind preserved (for timeout classification)
    #[test]
    fn test_blocking_send_failure_preserves_error_with_context() {
        use crate::command::bytes::VISCA_TERMINATOR;

        let timeout_config = TimeoutConfig::default();
        let mut runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);

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

        let mut transport = FailingSendTransport::new();
        let camera_id = CameraId::CAMERA_1;
        let test_cmd = TestCmd {
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        };

        // Track start time to verify we return immediately (not after timeout)
        let start = Instant::now();

        let result = runner.send_command(&mut transport, &test_cmd, camera_id);

        let elapsed = start.elapsed();

        // Should return immediately with error, not wait for timeout
        assert!(
            result.is_err(),
            "Expected error from failing transport, got Ok"
        );

        let error = result.unwrap_err();

        // Verify the error preserves the original cause with "Send failed" context
        let error_msg = error.to_string();
        assert!(
            error_msg.contains("Send failed"),
            "Error should contain 'Send failed' context, got: {error_msg}"
        );
        assert!(
            error_msg.contains("Simulated send failure"),
            "Error should contain original error message 'Simulated send failure', got: {error_msg}"
        );

        // Verify the error kind is preserved (TransportError -> IoClosed)
        assert_eq!(
            error.kind(),
            crate::ErrorKind::IoClosed,
            "Error kind should be preserved through context wrapping"
        );

        // Verify the error was returned promptly (not after timeout)
        // Default timeout is ~500ms, so if we're under 100ms we know it was immediate
        assert!(
            elapsed.as_millis() < 100,
            "Error should be returned immediately, took {elapsed:?}"
        );
    }

    /// Test that timeout errors preserve ErrorKind::Timeout through send failure context.
    ///
    /// This test verifies issue #469's key requirement: write timeouts should remain
    /// as ErrorKind::Timeout even when wrapped with "Send failed" context.
    #[test]
    fn test_blocking_send_timeout_preserves_timeout_kind() {
        use crate::command::bytes::VISCA_TERMINATOR;

        let timeout_config = TimeoutConfig::default();
        let mut runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);

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

        /// Mock transport that returns Timeout on send
        struct TimeoutSendTransport {
            config: TransportConfig,
        }

        impl TimeoutSendTransport {
            fn new() -> Self {
                Self {
                    config: TransportConfig::default(),
                }
            }
        }

        impl BlockingTransport for TimeoutSendTransport {
            fn send_with_kind(&mut self, _bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
                Err(Error::Timeout)
            }

            fn recv_into(&mut self, _dst: &mut [u8]) -> Result<usize, Error> {
                Ok(0)
            }

            fn recv_into_with_timeout(
                &mut self,
                _dst: &mut [u8],
                _timeout: Duration,
            ) -> Result<usize, Error> {
                Err(Error::Timeout)
            }

            // Use Datagram semantics to test timeout-kind preservation
            // (Stream transports get StreamPoisoned instead - see separate test)
            fn send_semantics(&self) -> SendSemantics {
                SendSemantics::Datagram
            }
        }

        impl HasTransportConfig for TimeoutSendTransport {
            fn transport_config(&self) -> &TransportConfig {
                &self.config
            }
        }

        let mut transport = TimeoutSendTransport::new();
        let camera_id = CameraId::CAMERA_1;
        let test_cmd = TestCmd {
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        };

        let result = runner.send_command(&mut transport, &test_cmd, camera_id);

        assert!(result.is_err(), "Expected timeout error, got Ok");

        let error = result.unwrap_err();

        // The critical assertion: timeout classification MUST be preserved for datagram transports
        assert_eq!(
            error.kind(),
            crate::ErrorKind::Timeout,
            "Timeout ErrorKind must be preserved through 'Send failed' context wrapping. Got: {:?}",
            error
        );

        // Verify the error message still contains "Send failed" context
        let error_msg = error.to_string();
        assert!(
            error_msg.contains("Send failed"),
            "Error should contain 'Send failed' context, got: {error_msg}"
        );
    }

    /// Mock transport for testing compute_receive_timeout.
    struct MockTransportConfig {
        config: TransportConfig,
    }

    impl MockTransportConfig {
        fn new(read_timeout: Duration) -> Self {
            Self {
                config: TransportConfig {
                    read_timeout,
                    ..TransportConfig::default()
                },
            }
        }
    }

    impl HasTransportConfig for MockTransportConfig {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    /// Test that compute_receive_timeout selects the minimum of available waits.
    #[test]
    fn test_compute_receive_timeout_selects_minimum() {
        let timeout_config = TimeoutConfig::default();
        let runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);

        let now = Instant::now();
        let transport = MockTransportConfig::new(Duration::from_secs(5));

        // With no deadline and no scheduler state, should use transport timeout
        let timeout = runner.compute_receive_timeout(&transport, now, None);
        assert_eq!(timeout, Duration::from_secs(5));
    }

    /// Test that call-level deadline is respected.
    #[test]
    fn test_compute_receive_timeout_respects_call_deadline() {
        let timeout_config = TimeoutConfig::default();
        let runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);

        let now = Instant::now();
        let transport = MockTransportConfig::new(Duration::from_secs(5));

        // Call deadline of 100ms should be used since it's smaller
        let call_deadline = Deadline::from_timeout_at(now, Duration::from_millis(100));
        let timeout = runner.compute_receive_timeout(&transport, now, Some(&call_deadline));
        assert_eq!(timeout, Duration::from_millis(100));
    }

    /// Test that expired deadline returns zero.
    #[test]
    fn test_compute_receive_timeout_expired_deadline_returns_zero() {
        let timeout_config = TimeoutConfig::default();
        let runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);

        let now = Instant::now();
        let transport = MockTransportConfig::new(Duration::from_secs(5));

        // Create a deadline that's already expired
        let past = now - Duration::from_millis(100);
        let expired_deadline = Deadline::from_instant(past, Duration::from_millis(50));
        let timeout = runner.compute_receive_timeout(&transport, now, Some(&expired_deadline));
        assert_eq!(timeout, Duration::ZERO);
    }

    /// Test that transport timeout caps the result.
    #[test]
    fn test_compute_receive_timeout_capped_by_transport() {
        let timeout_config = TimeoutConfig::default();
        let runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);

        let now = Instant::now();
        // Very short transport timeout
        let transport = MockTransportConfig::new(Duration::from_millis(50));

        // Call deadline is much longer, but transport caps it
        let call_deadline = Deadline::from_timeout_at(now, Duration::from_secs(10));
        let timeout = runner.compute_receive_timeout(&transport, now, Some(&call_deadline));
        assert_eq!(timeout, Duration::from_millis(50));
    }

    /// Test timeout computation with scheduler state.
    #[test]
    fn test_compute_receive_timeout_with_scheduler_deadline() {
        use crate::command::bytes::VISCA_TERMINATOR;

        // Set a short ACK timeout for testing
        let timeout_config = TimeoutConfig {
            ack_timeout: Duration::from_millis(200),
            ..TimeoutConfig::default()
        };

        let mut runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);

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

        // Queue a command to create scheduler state
        let camera_id = CameraId::CAMERA_1;
        let test_cmd = TestCmd {
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        };
        let prepared_cmd = std::sync::Arc::new(
            crate::command::encode::EncodedCommand::new(&test_cmd, camera_id).unwrap(),
        );

        let now = Instant::now();
        let cmd = PendingCommand {
            id: cmd_id(1),
            command: prepared_cmd.clone(),
            priority: Priority::Normal,
            camera_id,
            submitted_at: now,
        };

        runner.core.queue_command(cmd);

        // Simulate the command being sent (moves to pending_ack)
        let sent_cmd = runner.core.next_item_to_send(now).unwrap();
        runner.core.register_pending_ack(
            sent_cmd.id,
            sent_cmd.command,
            sent_cmd.priority,
            sent_cmd.camera_id,
            now,
        );

        let transport = MockTransportConfig::new(Duration::from_secs(5));

        // Now the scheduler should have an ACK timeout deadline
        let timeout = runner.compute_receive_timeout(&transport, now, None);

        // The timeout should be approximately the ACK timeout (200ms), not the transport timeout (5s)
        assert!(
            timeout <= Duration::from_millis(250),
            "Expected timeout around 200ms, got {:?}",
            timeout
        );
        assert!(
            timeout >= Duration::from_millis(150),
            "Timeout too short: {:?}",
            timeout
        );
    }

    /// Mock stream transport that fails on send for testing stream poisoning behavior.
    struct FailingStreamTransport {
        config: TransportConfig,
    }

    impl FailingStreamTransport {
        fn new() -> Self {
            Self {
                config: TransportConfig::default(),
            }
        }
    }

    impl BlockingTransport for FailingStreamTransport {
        fn send_with_kind(&mut self, _bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
            Err(Error::TransportError(
                "Simulated stream send failure".into(),
            ))
        }

        fn recv_into(&mut self, _dst: &mut [u8]) -> Result<usize, Error> {
            Ok(0)
        }

        fn recv_into_with_timeout(
            &mut self,
            _dst: &mut [u8],
            _timeout: Duration,
        ) -> Result<usize, Error> {
            Err(Error::Timeout)
        }

        // Stream semantics - send failures should poison the transport
        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Stream
        }
    }

    impl HasTransportConfig for FailingStreamTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    /// Test that stream transport send failures result in StreamPoisoned error.
    ///
    /// This test verifies issue #476: when a stream transport (TCP, Serial) experiences
    /// a send failure, it should return StreamPoisoned instead of the underlying error
    /// because the byte stream may be in an unknown state.
    #[test]
    fn test_stream_transport_send_failure_returns_stream_poisoned() {
        use crate::command::bytes::VISCA_TERMINATOR;

        let timeout_config = TimeoutConfig::default();
        let mut runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);

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

        let mut transport = FailingStreamTransport::new();
        let camera_id = CameraId::CAMERA_1;
        let test_cmd = TestCmd {
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        };

        // Track start time to verify we return immediately (not after timeout)
        let start = Instant::now();

        let result = runner.send_command(&mut transport, &test_cmd, camera_id);

        let elapsed = start.elapsed();

        // Should return immediately with error
        assert!(
            result.is_err(),
            "Expected error from failing stream transport, got Ok"
        );

        let error = result.unwrap_err();

        // Verify the error is StreamPoisoned (not the underlying TransportError)
        let error_msg = error.to_string();
        assert!(
            error_msg.contains("Stream transport poisoned"),
            "Error should be StreamPoisoned, got: {error_msg}"
        );

        // Verify the error contains the original cause
        assert!(
            error_msg.contains("Simulated stream send failure"),
            "Error should contain original error message, got: {error_msg}"
        );

        // Verify the error kind is IoClosed (StreamPoisoned is a transport failure)
        assert_eq!(
            error.kind(),
            crate::ErrorKind::IoClosed,
            "StreamPoisoned error should have ErrorKind::IoClosed"
        );

        // Verify the error was returned promptly (not after timeout)
        assert!(
            elapsed.as_millis() < 100,
            "Error should be returned immediately, took {elapsed:?}"
        );
    }

    /// Test that stream transport timeout results in StreamPoisoned error.
    ///
    /// This test verifies issue #476: when a stream transport (TCP, Serial) experiences
    /// a send timeout, it should return StreamPoisoned because a partial write may have
    /// occurred, leaving the byte stream in an unknown state.
    #[test]
    fn test_stream_transport_timeout_returns_stream_poisoned() {
        use crate::command::bytes::VISCA_TERMINATOR;

        let timeout_config = TimeoutConfig::default();
        let mut runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);

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

        /// Mock stream transport that returns Timeout on send
        struct TimeoutStreamTransport {
            config: TransportConfig,
        }

        impl TimeoutStreamTransport {
            fn new() -> Self {
                Self {
                    config: TransportConfig::default(),
                }
            }
        }

        impl BlockingTransport for TimeoutStreamTransport {
            fn send_with_kind(&mut self, _bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
                Err(Error::Timeout)
            }

            fn recv_into(&mut self, _dst: &mut [u8]) -> Result<usize, Error> {
                Ok(0)
            }

            fn recv_into_with_timeout(
                &mut self,
                _dst: &mut [u8],
                _timeout: Duration,
            ) -> Result<usize, Error> {
                Err(Error::Timeout)
            }

            // Stream semantics - timeouts should poison the transport
            fn send_semantics(&self) -> SendSemantics {
                SendSemantics::Stream
            }
        }

        impl HasTransportConfig for TimeoutStreamTransport {
            fn transport_config(&self) -> &TransportConfig {
                &self.config
            }
        }

        let mut transport = TimeoutStreamTransport::new();
        let camera_id = CameraId::CAMERA_1;
        let test_cmd = TestCmd {
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        };

        let result = runner.send_command(&mut transport, &test_cmd, camera_id);

        assert!(result.is_err(), "Expected error, got Ok");

        let error = result.unwrap_err();

        // The critical assertion: stream transport timeout should return StreamPoisoned
        let error_msg = error.to_string();
        assert!(
            error_msg.contains("Stream transport poisoned"),
            "Stream transport timeout should return StreamPoisoned, got: {error_msg}"
        );
    }

    /// Test that datagram transport send failures preserve the original error.
    ///
    /// This test verifies issue #476: when a datagram transport (UDP) experiences
    /// a send failure, it should return the original error (not StreamPoisoned)
    /// because datagram sends are atomic and don't affect subsequent sends.
    #[test]
    fn test_datagram_transport_send_failure_preserves_error() {
        use crate::command::bytes::VISCA_TERMINATOR;

        let timeout_config = TimeoutConfig::default();
        let mut runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);

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

        /// Mock datagram transport that fails on send
        struct FailingDatagramTransport {
            config: TransportConfig,
        }

        impl FailingDatagramTransport {
            fn new() -> Self {
                Self {
                    config: TransportConfig::default(),
                }
            }
        }

        impl BlockingTransport for FailingDatagramTransport {
            fn send_with_kind(&mut self, _bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
                Err(Error::TransportError(
                    "Simulated datagram send failure".into(),
                ))
            }

            fn recv_into(&mut self, _dst: &mut [u8]) -> Result<usize, Error> {
                Ok(0)
            }

            fn recv_into_with_timeout(
                &mut self,
                _dst: &mut [u8],
                _timeout: Duration,
            ) -> Result<usize, Error> {
                Err(Error::Timeout)
            }

            // Datagram semantics - send failures should NOT poison the transport
            fn send_semantics(&self) -> SendSemantics {
                SendSemantics::Datagram
            }
        }

        impl HasTransportConfig for FailingDatagramTransport {
            fn transport_config(&self) -> &TransportConfig {
                &self.config
            }
        }

        let mut transport = FailingDatagramTransport::new();
        let camera_id = CameraId::CAMERA_1;
        let test_cmd = TestCmd {
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        };

        let result = runner.send_command(&mut transport, &test_cmd, camera_id);

        assert!(result.is_err(), "Expected error, got Ok");

        let error = result.unwrap_err();

        // The critical assertion: datagram transport should NOT return StreamPoisoned
        let error_msg = error.to_string();
        assert!(
            !error_msg.contains("Stream transport poisoned"),
            "Datagram transport should NOT return StreamPoisoned, got: {error_msg}"
        );

        // Should preserve the original error with "Send failed" context
        assert!(
            error_msg.contains("Send failed"),
            "Error should contain 'Send failed' context, got: {error_msg}"
        );
        assert!(
            error_msg.contains("Simulated datagram send failure"),
            "Error should contain original error message, got: {error_msg}"
        );
    }
}
