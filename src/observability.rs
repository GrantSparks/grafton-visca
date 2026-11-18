//! Bounded, mode-independent owner observability.
//!
//! Diagnostics are deliberately a projection of the private owner trace.  The
//! projection contains no wire bytes, timestamps, socket/transmission
//! authority, sequence numbers, or arbitrary strings. Request identities
//! correlate observations. The owner bounds each queue. The
//! pre-admission staging retains at most 128 events and an owner retains at
//! most four diagnostic subscribers. Each queue is bounded by
//! [`DiagnosticCapacity::MAX`](crate::observability::DiagnosticCapacity::MAX).

use crate::{state_cache::StateKey, ErrorKind, IndividualCameraId, OperationId};

/// The checked event capacity of one diagnostic subscription, from 1 to 128.
#[crate::value(min = 1, max = 128)]
pub struct DiagnosticCapacity(u8);

/// The externally observable state of one owner session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum SessionStatus {
    /// The owner accepts work.
    #[default]
    Running,
    /// The transport reported a terminal connection closure.
    Closed,
    /// The owner was explicitly shut down.
    Shutdown,
    /// A stream or framing failure made the session unusable.
    Poisoned,
}

/// The terminal status reported when an owner session ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum SessionEndStatus {
    /// The transport reported a terminal connection closure.
    Closed,
    /// The owner was explicitly shut down.
    Shutdown,
    /// A stream or framing failure made the session unusable.
    Poisoned,
}

impl From<SessionEndStatus> for SessionStatus {
    fn from(status: SessionEndStatus) -> Self {
        match status {
            SessionEndStatus::Closed => Self::Closed,
            SessionEndStatus::Shutdown => Self::Shutdown,
            SessionEndStatus::Poisoned => Self::Poisoned,
        }
    }
}

/// A stable copy of the bounded owner counters and current queue state.
///
/// Counters saturate at `u64::MAX`; `active` and `pending` are bounded by the
/// immutable session admission policy.  A snapshot contains only scalar data
/// and therefore does not clone any subscriber queue.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub struct MetricsSnapshot {
    /// Requests admitted into authoritative engine state.
    pub admitted: u64,
    /// Requests rejected before admission, including async submissions whose
    /// caller abandoned the admission future before the owner admitted them.
    pub admission_rejected: u64,
    /// Urgent typed STOPs admitted into their target's control reserve
    /// rather than an ordinary admission slot (D26, #778).
    pub control_reserve_admitted: u64,
    /// Urgent typed STOPs rejected because their target's control reserve
    /// and the ordinary admission budget were both full. Included in
    /// `admission_rejected`.
    pub control_reserve_rejected: u64,
    /// Transport writes attempted.
    pub writes: u64,
    /// Transport writes that returned an error.
    pub write_failures: u64,
    /// Requests reaching a terminal outcome.
    pub terminal: u64,
    /// Cancellation intents the engine recorded.
    ///
    /// Excludes repeated cancellation calls from one handle and refused cancellations.
    pub cancellations: u64,
    /// Exact applied-state effects committed to the target cache.
    pub cache_updates: u64,
    /// Applied-state keys made unknown by unconfirmed writes.
    pub cache_invalidations: u64,
    /// Acknowledgement deadlines that expired on a sent command. A disputed
    /// ACK deadline extended for more evidence counts if that extension expires.
    pub ack_timeouts: u64,
    /// Completion deadlines that expired while an acknowledged command or
    /// completion-only raw command awaited completion. For a cancelling command,
    /// the completion window counts only if it ends the request there.
    pub completion_timeouts: u64,
    /// Reply deadlines that expired on a sent inquiry.
    pub inquiry_timeouts: u64,
    /// Error frames reporting that the camera cannot accept the request now.
    ///
    /// These are command buffer full (`0x03`), no socket available (`0x05`),
    /// and not executable in the current state (`0x41`) — the codes the
    /// scheduler itself treats as transient camera-side backpressure.
    pub busy_errors: u64,
    /// Every other error frame, excluding the cancellation reply (`0x04`).
    ///
    /// Both error counters count frames as they are decoded, including frames
    /// that no longer correlate to an active request.
    pub protocol_errors: u64,
    /// Requests re-queued for another attempt.
    ///
    /// Counted wherever the scheduler emits a retry, whatever motivated it: a
    /// busy camera, an expired deadline, or a transient receive fault.
    pub retries_scheduled: u64,
    /// Valid decoded VISCA response frames received from the camera.
    ///
    /// This is a positive liveness signal: compare snapshots around an
    /// application heartbeat to tell whether any valid peer response arrived.
    /// A timeout does not increment it, and a value that does not advance is
    /// not by itself proof that the transport is closed.
    pub received_frames: u64,
    /// Sequenced replies discarded because their sequence matched no request.
    ///
    /// Expected for stale or duplicated datagrams; a rising count means the
    /// sequence-correlation safety net is doing its job.
    pub ignored_unmatched_sequenced_replies: u64,
    /// Delimited frames, and consumed oversized/malformed datagrams rejected before
    /// framing, discarded because they did not classify as a valid VISCA response
    /// (#672).
    ///
    /// A frame the framer delimited at an `FF` boundary but that the strict
    /// decoder rejected — a padded ACK, a vendor socket nibble, an RS-485 echo, a
    /// truncated frame, line noise — is discarded and counted here while the
    /// session keeps running, exactly as a datagram already discards one. Only a
    /// genuine loss of the framing position poisons a stream, which this never
    /// counts. A steady trickle is normal on noisy serial/RS-485 links; a rising
    /// count points at a mis-wired or misconfigured device.
    pub ignored_malformed_frames: u64,
    /// Inquiry replies discarded because no retained inquiry declaration
    /// admitted their decoded data length and nibble structure.
    pub ignored_reply_shape_mismatches: u64,
    /// Diagnostic events evicted from pre-admission staging before delivery.
    pub dropped_staged_diagnostics: u64,
    /// Diagnostic events dropped because a subscriber queue was full.
    pub dropped_diagnostic_events: u64,
    /// Operation-observer events — terminal outcomes and cancellation
    /// failures — dropped because their handle was already gone.
    pub dropped_observer_events: u64,
    /// Boundary work dropped before the terminal snapshot was published.
    /// Work that races publication is still answered during teardown without
    /// changing the published snapshot.
    pub dropped_boundary_work: u64,
    /// Requests currently admitted and not terminal.
    pub active: usize,
    /// Requests staged at the owner boundary but not yet admitted.
    pub pending: usize,
    /// Current session state.
    pub session: SessionStatus,
}

/// Request class carried by a diagnostic event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum DiagnosticLane {
    /// A command or operation request.
    Command,
    /// An inquiry request.
    Inquiry,
}

/// Protocol phase without private timestamps, sockets, or transmission IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum DiagnosticPhase {
    /// Waiting in the owner ready queue.
    Ready,
    /// A transport write is in progress.
    Sending,
    /// Waiting for an acknowledgement.
    AwaitingAck,
    /// Waiting for a completion-only command's completion, having received no
    /// acknowledgement and owning no socket.
    AwaitingCompletion,
    /// Waiting for command completion.
    Executing,
    /// The Sony camera reassigned this command's socket to a later command.
    ///
    /// The command still receives replies by its Sony sequence until its
    /// original completion deadline. Cancellation returns
    /// [`crate::Error::CancellationUnconfirmed`].
    Displaced,
    /// Waiting for an inquiry reply.
    AwaitingReply,
    /// Waiting before a retry.
    Backoff,
    /// Resolving an emitted cancellation on an owned command socket.
    AwaitingCancellationResolution,
}

/// Sanitized response category from one received frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum DiagnosticResponse {
    /// A VISCA acknowledgement was received.
    Ack,
    /// A VISCA completion was received.
    Completion,
    /// An inquiry reply was received.
    InquiryReply,
    /// A protocol error response was received.
    Error,
    /// A Sony transport-control reply was received.
    #[non_exhaustive]
    SonyControl {
        /// The one- or two-byte control reply code, stored in network order.
        code: u16,
    },
    /// A network-change response was received.
    NetworkChange,
    /// A response could not be classified.
    Unknown,
}

/// Which of a request's own protocol deadlines expired.
///
/// Cancellation deadlines are not reported here; they arrive as a
/// [`crate::observability::DiagnosticEvent::CancellationObserved`] instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum DiagnosticDeadline {
    /// The acknowledgement deadline for a sent command expired.
    Ack,
    /// The completion deadline for an acknowledged command expired.
    Completion,
    /// The reply deadline for a sent inquiry expired.
    InquiryReply,
}

/// Sanitized terminal outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum DiagnosticOutcome {
    /// A raw plain fire-and-forget write reached the local transport. This is
    /// not evidence that the camera accepted or applied the command.
    Written,
    /// The command reached applied completion, whether or not it has a state-cache effect.
    Applied,
    /// An inquiry reply was delivered.
    Reply,
    /// The request was cancelled.
    Cancelled,
    /// The request failed with a bounded error category.
    Failed(ErrorKind),
}

/// Sanitized cancellation observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum DiagnosticCancellation {
    /// Cancellation was recorded by the engine.
    Recorded,
    /// The request was cancelled.
    Cancelled,
    /// The request completed before cancellation took effect.
    Completed,
    /// Cancellation observation failed.
    Failed(ErrorKind),
}

/// Sanitized reason for an ignored engine input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum DiagnosticIgnoreReason {
    /// The request identity was not present.
    UnknownRequest,
    /// A transmission identity was stale.
    StaleTransmission,
    /// A write result did not match the active transmission.
    IncompatibleTransmissionResult,
    /// The frame was malformed.
    MalformedFrame,
    /// No retained inquiry declaration admitted the reply payload's shape.
    ReplyShapeMismatch,
    /// No active request matched a frame.
    UnmatchedFrame,
    /// No active request matched a sequenced frame.
    UnmatchedSequencedFrame,
    /// A sequence belonged to another target.
    TargetIncompatibleSequence,
    /// A lower-width sequence was ambiguous.
    AmbiguousLower16Sequence,
    /// A socket was already claimed.
    SocketConflict,
    /// A duplicate cancellation was ignored.
    DuplicateCancellation,
    /// The session was no longer running.
    SessionNotRunning,
}

/// A bounded, sanitized owner event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum DiagnosticEvent {
    /// A request entered authoritative owner state.
    #[non_exhaustive]
    Admitted {
        /// Opaque request lifecycle identity.
        id: OperationId,
        /// Target camera.
        target: IndividualCameraId,
        /// Request class.
        lane: DiagnosticLane,
    },
    /// Admission was rejected before an identity was allocated.
    #[non_exhaustive]
    AdmissionRejected {
        /// Target camera.
        target: IndividualCameraId,
        /// Request class.
        lane: DiagnosticLane,
        /// Bounded rejection category.
        error: ErrorKind,
    },
    /// A response frame reached the owner.
    #[non_exhaustive]
    FrameReceived {
        /// Target camera.
        target: IndividualCameraId,
        /// Sanitized response category.
        response: DiagnosticResponse,
    },
    /// A request changed private protocol phase.
    #[non_exhaustive]
    Transition {
        /// Opaque request lifecycle identity.
        id: OperationId,
        /// Target camera.
        target: IndividualCameraId,
        /// Previous phase.
        from: DiagnosticPhase,
        /// New phase.
        to: DiagnosticPhase,
    },
    /// A transport write completed.
    #[non_exhaustive]
    WriteFinished {
        /// Opaque request lifecycle identity.
        id: OperationId,
        /// Target camera.
        target: IndividualCameraId,
        /// Whether this was a cancellation write.
        cancellation: bool,
        /// Whether the write succeeded.
        success: bool,
    },
    /// A retry was scheduled.
    #[non_exhaustive]
    RetryScheduled {
        /// Opaque request lifecycle identity.
        id: OperationId,
        /// Target camera.
        target: IndividualCameraId,
        /// Retry attempt, bounded by the request policy.
        attempt: u32,
    },
    /// A request's own protocol deadline expired. Extending a disputed ACK
    /// deadline or entering cancellation resolution emits only a
    /// [`crate::observability::DiagnosticEvent::Transition`].
    ///
    /// `will_retry` is the scheduler's decision for this exact expiry, so a
    /// subscriber never has to infer it from a [`crate::observability::DiagnosticEvent::Transition`]
    /// and the absence of a following [`crate::observability::DiagnosticEvent::RetryScheduled`].
    #[non_exhaustive]
    DeadlineExpired {
        /// Opaque request lifecycle identity.
        id: OperationId,
        /// Target camera.
        target: IndividualCameraId,
        /// Which deadline expired.
        deadline: DiagnosticDeadline,
        /// Whether this expiry scheduled another attempt.
        will_retry: bool,
    },
    /// Cancellation entered the owner.
    #[non_exhaustive]
    CancellationRecorded {
        /// Opaque request lifecycle identity.
        id: OperationId,
        /// Target camera.
        target: IndividualCameraId,
    },
    /// Cancellation changed observation state.
    #[non_exhaustive]
    CancellationObserved {
        /// Opaque request lifecycle identity.
        id: OperationId,
        /// Target camera.
        target: IndividualCameraId,
        /// Sanitized observation.
        observation: DiagnosticCancellation,
    },
    /// An exact applied-state effect was committed.
    #[non_exhaustive]
    AppliedState {
        /// Opaque request lifecycle identity.
        id: OperationId,
        /// Target camera.
        target: IndividualCameraId,
        /// Applied-state key.
        key: StateKey,
    },
    /// An unconfirmed write made a cached key unknown.
    #[non_exhaustive]
    StateInvalidated {
        /// Opaque request lifecycle identity.
        id: OperationId,
        /// Target camera.
        target: IndividualCameraId,
        /// Invalidated state key.
        key: StateKey,
    },
    /// A request reached a terminal outcome.
    #[non_exhaustive]
    Terminal {
        /// Opaque request lifecycle identity.
        id: OperationId,
        /// Target camera.
        target: IndividualCameraId,
        /// Sanitized outcome.
        outcome: DiagnosticOutcome,
    },
    /// The session reached a terminal state.
    #[non_exhaustive]
    SessionEnded {
        /// Terminal session status.
        status: SessionEndStatus,
        /// Bounded reason category.
        reason: ErrorKind,
    },
    /// The engine deliberately ignored an input.
    #[non_exhaustive]
    Ignored {
        /// Bounded reason.
        reason: DiagnosticIgnoreReason,
    },
}

/// One reusable diagnostic deadline, bound to the session's selected executor.
#[cfg(feature = "async")]
pub(crate) struct DiagnosticWait {
    clock: crate::runtime::owner::BoundClock,
    // A mutex preserves Sync without holding a guard across an await. Timed
    // reads require &mut self and access the timer through get_mut().
    timer: std::sync::Mutex<std::pin::Pin<Box<dyn crate::executor::ResettableTimer>>>,
}

#[cfg(feature = "async")]
impl DiagnosticWait {
    pub(crate) fn new(clock: crate::runtime::owner::BoundClock) -> Self {
        let timer = clock.diagnostic_timer(clock.now());
        Self {
            clock,
            timer: std::sync::Mutex::new(timer),
        }
    }

    pub(crate) async fn recv_with_timeout(
        &mut self,
        subscription: &crate::runtime::owner::DiagnosticSubscription,
        timeout: std::time::Duration,
    ) -> crate::Result<Option<DiagnosticEvent>> {
        use std::{
            future::{poll_fn, Future},
            pin::pin,
            task::Poll,
        };
        let Some(deadline) = self.clock.now().checked_add(timeout) else {
            return subscription.recv_async().await.map(Some);
        };
        let timer = self
            .timer
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        timer.as_mut().reset(deadline);
        let armed = DiagnosticTimerArm {
            timer,
            clock: &self.clock,
        };
        let mut receive = pin!(subscription.recv_async());
        poll_fn(|context| {
            if let Poll::Ready(event) = receive.as_mut().poll(context) {
                return Poll::Ready(event.map(Some));
            }
            if armed.timer.as_mut().poll(context).is_ready() {
                // A queued event or terminal cause at the deadline takes
                // precedence over an otherwise empty observation window.
                return match receive.as_mut().poll(context) {
                    Poll::Ready(event) => Poll::Ready(event.map(Some)),
                    Poll::Pending => Poll::Ready(Ok(None)),
                };
            }
            Poll::Pending
        })
        .await
    }
}

#[cfg(feature = "async")]
struct DiagnosticTimerArm<'a> {
    timer: &'a mut std::pin::Pin<Box<dyn crate::executor::ResettableTimer>>,
    clock: &'a crate::runtime::owner::BoundClock,
}

#[cfg(feature = "async")]
impl Drop for DiagnosticTimerArm<'_> {
    fn drop(&mut self) {
        // A completed or abandoned receive must not retain an armed virtual
        // deadline or the caller's waker. Reuse the existing timer storage.
        self.timer.as_mut().reset(self.clock.now());
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        let _ = std::future::Future::poll(self.timer.as_mut(), &mut context);
    }
}

#[cfg(any(feature = "blocking", feature = "async"))]
impl SessionStatus {
    /// The public status of one engine session state.
    pub(crate) const fn from_engine(state: crate::runtime::engine::SessionState) -> Self {
        use crate::runtime::engine::SessionState;

        match state {
            SessionState::Running => Self::Running,
            SessionState::Closed => Self::Closed,
            SessionState::Shutdown => Self::Shutdown,
            SessionState::Poisoned => Self::Poisoned,
        }
    }
}

#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) fn metrics_snapshot(
    metrics: crate::runtime::owner::OwnerMetrics,
    active: usize,
    pending: usize,
    session: crate::runtime::engine::SessionState,
) -> MetricsSnapshot {
    let session = SessionStatus::from_engine(session);
    MetricsSnapshot {
        admitted: metrics.admitted,
        admission_rejected: metrics.admission_rejected,
        control_reserve_admitted: metrics.control_reserve_admitted,
        control_reserve_rejected: metrics.control_reserve_rejected,
        writes: metrics.writes,
        write_failures: metrics.write_failures,
        terminal: metrics.terminal,
        cancellations: metrics.cancellations,
        cache_updates: metrics.cache_updates,
        cache_invalidations: metrics.cache_invalidations,
        ack_timeouts: metrics.ack_timeouts,
        completion_timeouts: metrics.completion_timeouts,
        inquiry_timeouts: metrics.inquiry_timeouts,
        busy_errors: metrics.busy_errors,
        protocol_errors: metrics.protocol_errors,
        retries_scheduled: metrics.retries_scheduled,
        received_frames: metrics.received_frames,
        ignored_unmatched_sequenced_replies: metrics.ignored_unmatched_sequenced_replies,
        ignored_malformed_frames: metrics.ignored_malformed_frames,
        ignored_reply_shape_mismatches: metrics.ignored_reply_shape_mismatches,
        dropped_staged_diagnostics: metrics.dropped_staged_diagnostics,
        dropped_diagnostic_events: metrics.dropped_diagnostic_events,
        dropped_observer_events: metrics.dropped_observer_events,
        dropped_boundary_work: metrics.dropped_boundary_work,
        active,
        pending,
        session,
    }
}

#[cfg(test)]
#[test]
fn diagnostic_capacity_checks_its_fixed_domain() {
    assert_eq!(
        DiagnosticCapacity::new(1).ok(),
        Some(DiagnosticCapacity::MIN)
    );
    assert_eq!(
        DiagnosticCapacity::new(128).ok(),
        Some(DiagnosticCapacity::MAX)
    );
    assert!(DiagnosticCapacity::new(0).is_err());
    assert!(DiagnosticCapacity::new(129).is_err());
}

#[cfg(all(test, target_pointer_width = "64"))]
mod diagnostic_size_tests {
    #[test]
    fn sanitized_events_fit_in_sixteen_byte_subscriber_slots() {
        assert!(size_of::<super::DiagnosticEvent>() <= 16);
    }
}

#[cfg(all(test, feature = "serde"))]
mod portable_data_tests {
    use super::*;

    #[test]
    fn diagnostic_capacity_rejects_out_of_range_json() {
        for capacity in [0, 129] {
            assert!(
                serde_json::from_value::<DiagnosticCapacity>(serde_json::json!(capacity)).is_err()
            );
        }
    }

    #[test]
    fn diagnostic_events_round_trip_as_closed_sanitized_records() -> Result<(), serde_json::Error> {
        let event = DiagnosticEvent::DeadlineExpired {
            id: OperationId::from_raw(42),
            target: IndividualCameraId::CAMERA_3,
            deadline: DiagnosticDeadline::InquiryReply,
            will_retry: true,
        };
        let json = serde_json::json!({
            "deadline_expired": {
                "id": 42, "target": 3, "deadline": "inquiry_reply", "will_retry": true
            }
        });
        assert_eq!(serde_json::to_value(event)?, json);
        assert_eq!(serde_json::from_value::<DiagnosticEvent>(json)?, event);
        for invalid in [0, 8] {
            assert!(
                serde_json::from_value::<DiagnosticEvent>(serde_json::json!({
                    "deadline_expired": {
                        "id": 42, "target": invalid, "deadline": "inquiry_reply", "will_retry": true
                    }
                }))
                .is_err()
            );
        }
        assert!(
            serde_json::from_value::<DiagnosticEvent>(serde_json::json!({
                "ignored": {"reason": "unknown_request", "wire_bytes": [144, 81, 255]}
            }))
            .is_err()
        );
        let invalidation = DiagnosticEvent::StateInvalidated {
            id: OperationId::from_raw(42),
            target: IndividualCameraId::CAMERA_3,
            key: StateKey::ExposureWideDynamicRange,
        };
        let json = serde_json::json!({
            "state_invalidated": {
                "id": 42, "target": 3, "key": "exposure_wide_dynamic_range"
            }
        });
        assert_eq!(serde_json::to_value(invalidation)?, json);
        assert_eq!(
            serde_json::from_value::<DiagnosticEvent>(json)?,
            invalidation
        );
        let displacement = DiagnosticEvent::Transition {
            id: OperationId::from_raw(42),
            target: IndividualCameraId::CAMERA_3,
            from: DiagnosticPhase::Executing,
            to: DiagnosticPhase::Displaced,
        };
        let json = serde_json::json!({
            "transition": {"id": 42, "target": 3, "from": "executing", "to": "displaced"}
        });
        assert_eq!(serde_json::to_value(displacement)?, json);
        assert_eq!(
            serde_json::from_value::<DiagnosticEvent>(json)?,
            displacement
        );
        let ended = DiagnosticEvent::SessionEnded {
            status: SessionEndStatus::Closed,
            reason: ErrorKind::IoClosed,
        };
        let json =
            serde_json::json!({"session_ended": {"status": "closed", "reason": "io_closed"}});
        assert_eq!(serde_json::to_value(ended)?, json);
        assert_eq!(serde_json::from_value::<DiagnosticEvent>(json)?, ended);
        assert!(
            serde_json::from_value::<DiagnosticEvent>(serde_json::json!({
                "session_ended": {"status": "running", "reason": "io_closed"}
            }))
            .is_err()
        );
        let terminal = DiagnosticOutcome::Failed(ErrorKind::Unconfirmed);
        assert_eq!(
            serde_json::to_value(terminal)?,
            serde_json::json!({"failed":"unconfirmed"})
        );
        Ok(())
    }

    #[test]
    fn metric_snapshots_keep_full_counters_and_session_state() -> Result<(), serde_json::Error> {
        let snapshot = MetricsSnapshot {
            admitted: u64::MAX,
            cache_invalidations: 2,
            active: 2,
            pending: 3,
            session: SessionStatus::Poisoned,
            ..MetricsSnapshot::default()
        };
        let mut json = serde_json::to_value(snapshot)?;
        assert_eq!(json["admitted"], u64::MAX);
        assert_eq!(json["cache_invalidations"], 2);
        assert_eq!(json["session"], "poisoned");
        assert_eq!(
            serde_json::from_value::<MetricsSnapshot>(json.clone())?,
            snapshot
        );
        json["private_queue"] = serde_json::json!([]);
        assert!(serde_json::from_value::<MetricsSnapshot>(json.clone()).is_err());
        if let Some(object) = json.as_object_mut() {
            object.remove("private_queue");
            object.remove("cache_invalidations");
        }
        assert!(serde_json::from_value::<MetricsSnapshot>(json).is_err());
        Ok(())
    }

    #[test]
    #[cfg(all(feature = "schemars", feature = "ts-rs"))]
    fn diagnostic_schema_and_typescript_describe_sanitized_payloads() {
        let schema = schemars::schema_for!(MetricsSnapshot);
        assert_eq!(schema.as_value()["additionalProperties"], false);
        assert_eq!(
            schema.as_value()["properties"]["admitted"]["type"],
            "integer"
        );
        let config = ts_rs::Config::default();
        let metrics = <MetricsSnapshot as ts_rs::TS>::decl(&config);
        assert!(metrics.contains("cache_invalidations: bigint"));
        assert!(schema.as_value()["required"]
            .as_array()
            .is_some_and(|required| required.contains(&serde_json::json!("cache_invalidations"))));
        let event = <DiagnosticEvent as ts_rs::TS>::decl(&config)
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                !line.starts_with('*') && !line.starts_with("/**")
            })
            .collect::<Vec<_>>()
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(event.contains("\"deadline_expired\": { id: OperationId, target: IndividualCameraId, deadline: DiagnosticDeadline, will_retry: boolean,"));
        assert_eq!(
            schema.as_value()["properties"]["cache_invalidations"]["type"],
            "integer"
        );
        let event_schema = schemars::schema_for!(DiagnosticEvent);
        let targets = &event_schema.as_value()["$defs"]["IndividualCameraId"];
        assert_eq!(targets["minimum"], 1);
        assert_eq!(targets["maximum"], 7);
        assert!(event_schema.as_value()["oneOf"]
            .as_array()
            .is_some_and(|variants| variants
                .iter()
                .any(|variant| variant["properties"]["state_invalidated"].is_object())));
        assert!(event.contains(
            "\"state_invalidated\": { id: OperationId, target: IndividualCameraId, key: StateKey,"
        ));
        assert!(event.contains("\"session_ended\": { status: SessionEndStatus, reason: ErrorKind,"));
        let definitions = &event_schema.as_value()["$defs"];
        let statuses: Vec<_> = definitions["SessionEndStatus"]["oneOf"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|variant| variant["const"].as_str())
            .collect();
        assert_eq!(statuses, ["closed", "shutdown", "poisoned"]);
        assert!(definitions["DiagnosticPhase"]["oneOf"]
            .as_array()
            .is_some_and(|variants| variants
                .iter()
                .any(|variant| variant["const"] == "displaced")));
        let end_status = <SessionEndStatus as ts_rs::TS>::decl(&config);
        assert!(
            end_status.contains("\"closed\" | \"shutdown\" | \"poisoned\""),
            "{end_status}"
        );
        let phase = <DiagnosticPhase as ts_rs::TS>::decl(&config);
        assert!(phase.contains("\"displaced\""), "{phase}");
        assert!(!event.contains("socket"));
        assert!(!event.contains("timestamp"));
    }
}
