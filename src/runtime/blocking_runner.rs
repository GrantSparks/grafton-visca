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
        driver::{scheduler::BlockingScheduler, send_one, SendResult},
    },
    timeout::{CommandCategory, Deadline, TimeoutConfig},
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
    _profile: PhantomData<P>,
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
        let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);
        // Apply profile-specific inquiry spacing
        core.set_min_inquiry_spacing(P::MIN_INQUIRY_SPACING);
        Self {
            core,
            envelope: P::Envelope::new(addressing),
            buffer_manager: BufferManager::new(buffer_config),
            framer: ProtocolFramer::new_with_config(buffer_config),
            next_id: AtomicU32::new(1),
            _profile: PhantomData,
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
        self.send_command_with_deadline(transport, command, camera_id, category, None)
    }

    /// Send a command with an optional deadline for the entire operation.
    ///
    /// When a deadline is provided, the operation will return `Error::Timeout`
    /// if the deadline is exceeded, even if the command's category timeout
    /// hasn't been reached. This is useful for movement detection where
    /// individual inquiries must not exceed the overall operation budget.
    ///
    /// # Arguments
    ///
    /// * `transport` - The transport to send the command on
    /// * `command` - The VISCA command to send
    /// * `camera_id` - Target camera ID
    /// * `category` - Command category for timeout calculation
    /// * `deadline` - Optional deadline; if `Some`, operation fails if deadline is exceeded
    pub fn send_command_with_deadline<T: BlockingTransport + HasTransportConfig>(
        &mut self,
        transport: &mut T,
        command: &(impl ViscaCommand + std::fmt::Debug + Clone + 'static),
        camera_id: CameraId,
        category: CommandCategory,
        deadline: Option<Deadline>,
    ) -> Result<Response> {
        // Check deadline before even starting
        if let Some(ref d) = deadline {
            if d.is_expired() {
                return Err(Error::Timeout);
            }
        }

        let cmd_id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let prepared_cmd = std::sync::Arc::new(
            crate::command::encode::EncodedCommand::new(command.clone(), camera_id).map_err(
                |e| {
                    tracing::error!("Failed to prepare command: {e:?}");
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
        target_cmd_id: u32,
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
                        // Check if this failure is for our target command - if so, return immediately
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
                        // Check if this failure is for our target command - if so, return immediately
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
                                        debug!("Command {cmd_id} completed successfully");
                                        // Clean up command state before returning
                                        if let Some(socket) = socket {
                                            self.core.free_socket(socket);
                                        }
                                        self.core.complete_command(cmd_id);
                                        let response_type = self.core.get_inquiry_type(cmd_id);
                                        let response =
                                            lift_inquiry_for::<P>(&basic, response_type)?;
                                        return Ok(response);
                                    }
                                }

                                debug!("Received completion for socket {socket:?}");
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
                                    "Received error 0x{code:02X} for socket {socket:?}, cmd_id {cmd_id:?}"
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::camera::profiles::PtzOpticsG2;
    use crate::command::encode::ViscaCommand;
    use crate::transport::builder::TransportConfig;

    #[test]
    fn test_scheduler_core_creation() {
        let timeout_config = TimeoutConfig::default();
        let runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);

        assert!(runner.core.can_send_command());
    }

    #[test]
    fn test_scheduler_core_with_raw_visca() {
        let timeout_config = TimeoutConfig::default();
        let runner = BlockingRunner::<PtzOpticsG2>::new(timeout_config);

        assert!(runner.core.can_send_command());
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
            crate::command::encode::EncodedCommand::new(test_cmd, camera_id).unwrap(),
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

        runner.core.queue_command(cmd);

        let now = Instant::now();
        let next = runner.core.next_item_to_send(now);
        assert!(next.is_some(), "should have command");
        if let Some(cmd) = next {
            assert_eq!(cmd.id, 1);
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

        let result =
            runner.send_command(&mut transport, &test_cmd, camera_id, CommandCategory::Quick);

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

        // Verify the error kind is preserved (TransportError -> Other)
        assert_eq!(
            error.kind(),
            crate::ErrorKind::Other,
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

        let result =
            runner.send_command(&mut transport, &test_cmd, camera_id, CommandCategory::Quick);

        assert!(result.is_err(), "Expected timeout error, got Ok");

        let error = result.unwrap_err();

        // The critical assertion: timeout classification MUST be preserved
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
            crate::command::encode::EncodedCommand::new(test_cmd, camera_id).unwrap(),
        );

        let now = Instant::now();
        let cmd = PendingCommand {
            id: 1,
            command: prepared_cmd.clone(),
            priority: Priority::Normal,
            category: CommandCategory::Quick,
            camera_id,
            submitted_at: now,
            kind: prepared_cmd.kind,
        };

        runner.core.queue_command(cmd);

        // Simulate the command being sent (moves to pending_ack)
        let sent_cmd = runner.core.next_item_to_send(now).unwrap();
        runner.core.register_pending_ack(
            sent_cmd.id,
            sent_cmd.command,
            sent_cmd.priority,
            sent_cmd.category,
            sent_cmd.camera_id,
            sent_cmd.kind,
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
}
