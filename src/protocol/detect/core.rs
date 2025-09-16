//! Runtime-agnostic protocol detection core using a finite state machine.
//!
//! This module provides the DetectorCore FSM that makes detection decisions
//! without performing I/O directly. The FSM emits Actions that runners
//! (async or blocking) interpret with their respective I/O primitives.

use core::time::Duration;
use std::time::Instant;

use crate::{
    capabilities::ProtocolStyle,
    command::CommandKind,
    protocol::framer::ProtocolFramer,
    protocol::response::decode_basic,
    transport::{
        buffer::{BufferConfig, BufferManager},
        envelope::TransportEnvelope,
        RetryConfig,
    },
};

use super::DetectionResult;

/// Actions that the detector core requests from the runner.
///
/// These are intents that the runner interprets with appropriate I/O operations.
pub(crate) enum Action {
    /// Send a pre-framed (enveloped) VISCA inquiry.
    SendInquiry,
    /// Receive until this absolute deadline (Instant).
    /// Runner should call `on_recv()` if bytes arrive.
    RecvUntil(Instant),
    /// Pause/backoff before next attempt.
    Pause(Duration),
    /// Finished with success.
    Done(DetectionResult),
    /// Finished with no response.
    NoResponse,
}

/// Runtime-agnostic protocol detection state machine.
///
/// This struct encapsulates all the detection logic without performing I/O.
/// It maintains state between calls and emits Actions for the runner to execute.
pub(crate) struct DetectorCore {
    schedule: TrySchedule,
    framer: ProtocolFramer,
    envelope: TransportEnvelope,
    buffer_manager: BufferManager,
    state: State,
    protocol_style: ProtocolStyle,
}

/// Internal state of the detection FSM.
enum State {
    /// Ready to start a new try
    StartTry,
    /// Waiting for response with deadline
    WaitingResponse { try_deadline: Instant },
    /// Transitioning to next try
    NextTry,
    /// Detection complete
    Finish(DetectionResult),
}

/// Manages retry scheduling and timeouts.
struct TrySchedule {
    try_num: u32,
    max_tries: u32,
    per_try_timeout: Duration,
    retry_config: RetryConfig,
}

impl TrySchedule {
    fn new(retry_config: RetryConfig, per_try_timeout: Duration) -> Self {
        Self {
            try_num: 0,
            max_tries: retry_config.max_retries + 1, // +1 for initial attempt
            per_try_timeout,
            retry_config,
        }
    }

    fn begin_try(&mut self, now: Instant) -> Instant {
        self.try_num += 1;
        now + self.per_try_timeout
    }

    fn backoff(&self) -> Duration {
        if self.try_num >= self.max_tries {
            Duration::ZERO
        } else {
            // Use the retry config's calculate_delay method
            self.retry_config
                .calculate_delay(self.try_num.saturating_sub(1), None)
        }
    }

    fn exhausted(&self) -> bool {
        self.try_num >= self.max_tries
    }
}

impl DetectorCore {
    /// Create a new detector core for testing a specific protocol style.
    pub(crate) fn new(
        protocol_style: ProtocolStyle,
        buffer_config: BufferConfig,
        retry_config: RetryConfig,
        per_try_timeout: Duration,
    ) -> Self {
        Self {
            schedule: TrySchedule::new(retry_config, per_try_timeout),
            framer: ProtocolFramer::new_with_config(buffer_config),
            envelope: TransportEnvelope::new(protocol_style),
            buffer_manager: BufferManager::new(buffer_config),
            state: State::StartTry,
            protocol_style,
        }
    }

    /// Build the inquiry frame for this protocol style.
    ///
    /// Returns the framed command ready to send.
    pub(crate) fn build_inquiry_frame(&self) -> Vec<u8> {
        use crate::command::bytes::VISCA_TERMINATOR;

        // Test command: Version Inquiry - should be supported by all VISCA cameras
        let test_command = &[0x81, 0x09, 0x00, 0x02, VISCA_TERMINATOR];

        self.envelope
            .frame_bytes_with_kind(test_command, CommandKind::Inquiry, &self.buffer_manager)
            .to_vec()
    }

    /// Get the next action for the runner to perform.
    ///
    /// The runner should execute the action and call the appropriate
    /// callback method (on_recv, on_deadline, resume_after_pause).
    pub(crate) fn next_action(&mut self, now: Instant) -> Action {
        match self.state {
            State::StartTry => {
                if self.schedule.exhausted() {
                    self.state = State::Finish(DetectionResult::NoResponse);
                    return Action::NoResponse;
                }

                let try_deadline = self.schedule.begin_try(now);
                self.state = State::WaitingResponse { try_deadline };
                Action::SendInquiry
            }
            State::WaitingResponse { try_deadline } => Action::RecvUntil(try_deadline),
            State::NextTry => {
                let pause = self.schedule.backoff();
                if pause.is_zero() {
                    self.state = State::StartTry;
                    // Immediately transition to StartTry
                    self.next_action(now)
                } else {
                    Action::Pause(pause)
                }
            }
            State::Finish(result) => Action::Done(result),
        }
    }

    /// Process received bytes during detection.
    ///
    /// Returns Some(action) if detection completes or state changes,
    /// None to continue receiving.
    pub(crate) fn on_recv(&mut self, bytes: &[u8], now: Instant) -> Option<Action> {
        use tracing::debug;

        debug!(
            "Received {} bytes for {:?} detection",
            bytes.len(),
            self.protocol_style
        );

        // Push chunk to framer
        if let Err(e) = self.framer.push_slice(bytes) {
            debug!("Framer buffer exceeded limits: {}", e);
            self.state = State::NextTry;
            return Some(self.next_action(now));
        }

        // Try to extract a complete frame - we only process the first one for detection
        let frame_result = self.framer.drain_frames().next();
        if let Some(frame_result) = frame_result {
            let frame = match frame_result {
                Ok(frame) => frame,
                Err(e) => {
                    debug!("Failed to extract frame: {}", e);
                    self.state = State::NextTry;
                    return Some(self.next_action(now));
                }
            };

            // Try to extract VISCA payload
            match self.envelope.extract_response(&frame) {
                Ok(visca_payload) => {
                    // Validate this looks like a VISCA response
                    if decode_basic(&visca_payload).is_some() {
                        debug!(
                            "Valid VISCA response detected for {:?}",
                            self.protocol_style
                        );
                        let result = match self.protocol_style {
                            ProtocolStyle::SonyEncapsulated => DetectionResult::SonyEncapsulated,
                            ProtocolStyle::RawVisca => DetectionResult::RawVisca,
                        };
                        self.state = State::Finish(result);
                        return Some(Action::Done(result));
                    } else {
                        debug!("Received data but not a valid VISCA response");
                        self.state = State::NextTry;
                        return Some(self.next_action(now));
                    }
                }
                Err(e) => {
                    debug!("Failed to extract VISCA payload: {}", e);
                    self.state = State::NextTry;
                    return Some(self.next_action(now));
                }
            }
        }

        // No complete frame yet, continue receiving
        None
    }

    /// Handle receive timeout.
    ///
    /// Called when RecvUntil deadline expires without receiving data.
    pub(crate) fn on_deadline(&mut self, now: Instant) -> Action {
        use tracing::debug;
        debug!("Detection timeout for {:?}", self.protocol_style);

        self.state = State::NextTry;
        self.next_action(now)
    }

    /// Resume after pause/backoff.
    ///
    /// Called after the runner completes a Pause action.
    pub(crate) fn resume_after_pause(&mut self) {
        self.state = State::StartTry;
    }
}
