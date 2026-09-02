//! Bounded, mode-independent owner observability.
//!
//! Diagnostics are deliberately a projection of the private owner trace.  The
//! projection contains no wire bytes, timestamps, socket/transmission
//! authority, sequence numbers, arbitrary strings, or publicly usable
//! lifecycle numbers. All queues are bounded by the owner policy: the
//! diagnostic ring retains at most 128 events and an owner retains at most
//! four diagnostic subscribers, each with capacity at most 128.

use std::fmt;

#[cfg(feature = "async")]
use crate::Error;
use crate::{state_cache::StateKey, CameraId, ErrorKind};

/// The externally observable state of one owner session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SessionStatus {
    /// The owner accepts work.
    #[default]
    Running,
    /// The protocol session closed normally.
    Closed,
    /// The owner was explicitly shut down.
    Shutdown,
    /// A stream or framing failure made the session unusable.
    Poisoned,
}

/// A stable copy of the bounded owner counters and current queue state.
///
/// Counters saturate at `u64::MAX`; `active` and `pending` are bounded by the
/// immutable session admission policy.  A snapshot contains only scalar data
/// and therefore does not clone the diagnostic ring or any subscriber queue.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MetricsSnapshot {
    /// Requests admitted into authoritative engine state.
    pub admitted: u64,
    /// Requests rejected before admission.
    pub admission_rejected: u64,
    /// Transport writes attempted.
    pub writes: u64,
    /// Transport writes that returned an error.
    pub write_failures: u64,
    /// Requests reaching a terminal outcome.
    pub terminal: u64,
    /// Cancellation requests observed by the owner.
    pub cancellations: u64,
    /// Exact applied-state effects committed to the target cache.
    pub cache_updates: u64,
    /// Acknowledgement deadlines that expired on a sent command.
    pub ack_timeouts: u64,
    /// Completion deadlines that expired while an acknowledged command or
    /// completion-only raw command awaited completion.
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
    /// Diagnostic observations evicted before or from the bounded owner ring.
    pub dropped_diagnostics: u64,
    /// Diagnostic events dropped because a subscriber queue was full.
    pub dropped_diagnostic_events: u64,
    /// Completion-observer events dropped after their receiver disappeared.
    pub dropped_observer_events: u64,
    /// Applied-state events dropped because a subscriber queue was full.
    pub dropped_applied_events: u64,
    /// Boundary messages discarded while the owner terminated.
    pub dropped_boundary_work: u64,
    /// Requests currently admitted and not terminal.
    pub active: usize,
    /// Requests staged at the owner boundary but not yet admitted.
    pub pending: usize,
    /// Current session state.
    pub session: SessionStatus,
}

/// Opaque identity for one diagnostic request lifecycle.
///
/// The value is intentionally not constructible, inspectable, or convertible
/// to a number by callers.  It is useful only for equality and correlation of
/// events from one subscription.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiagnosticId(u64);

impl DiagnosticId {
    pub(crate) const fn from_owner(value: u64) -> Self {
        Self(value)
    }
}

impl fmt::Debug for DiagnosticId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DiagnosticId(..)")
    }
}

/// Request class carried by a diagnostic event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DiagnosticLane {
    /// A command or operation request.
    Command,
    /// An inquiry request.
    Inquiry,
}

/// Protocol phase without private timestamps, sockets, or transmission IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// Waiting for an inquiry reply.
    AwaitingReply,
    /// Waiting before a retry.
    Backoff,
    /// Resolving a cancellation.
    AwaitingCancellationResolution,
    /// Resolving an acknowledgement after an ambiguous cancellation.
    AwaitingLateAck,
}

/// Sanitized response category from one received frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// A network-change response was received.
    NetworkChange,
    /// A response could not be classified.
    Unknown,
}

/// Which of a request's own protocol deadlines expired.
///
/// Cancellation deadlines are not reported here; they arrive as a
/// [`DiagnosticEvent::CancellationObserved`] instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[non_exhaustive]
pub enum DiagnosticOutcome {
    /// A raw plain fire-and-forget write reached the local transport. This is
    /// not evidence that the camera accepted or applied the command.
    Written,
    /// A command's applied-state effect was committed.
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
#[non_exhaustive]
pub enum DiagnosticIgnoreReason {
    /// The request identity was not present.
    UnknownRequest,
    /// A queue ticket was stale.
    StaleQueueTicket,
    /// A transmission identity was stale.
    StaleTransmission,
    /// A write result did not match the active transmission.
    IncompatibleTransmissionResult,
    /// The frame was malformed.
    MalformedFrame,
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
#[non_exhaustive]
pub enum DiagnosticEvent {
    /// A request entered authoritative owner state.
    Admitted {
        /// Opaque request lifecycle identity.
        id: DiagnosticId,
        /// Target camera.
        target: CameraId,
        /// Request class.
        lane: DiagnosticLane,
    },
    /// Admission was rejected before an identity was allocated.
    AdmissionRejected {
        /// Target camera.
        target: CameraId,
        /// Request class.
        lane: DiagnosticLane,
        /// Bounded rejection category.
        error: ErrorKind,
    },
    /// A response frame reached the owner.
    FrameReceived {
        /// Target camera.
        target: CameraId,
        /// Sanitized response category.
        response: DiagnosticResponse,
    },
    /// A request changed private protocol phase.
    Transition {
        /// Opaque request lifecycle identity.
        id: DiagnosticId,
        /// Target camera.
        target: CameraId,
        /// Previous phase.
        from: DiagnosticPhase,
        /// New phase.
        to: DiagnosticPhase,
    },
    /// A transport write completed.
    WriteFinished {
        /// Opaque request lifecycle identity.
        id: DiagnosticId,
        /// Target camera.
        target: CameraId,
        /// Whether this was a cancellation write.
        cancellation: bool,
        /// Whether the write succeeded.
        success: bool,
    },
    /// A retry was scheduled.
    RetryScheduled {
        /// Opaque request lifecycle identity.
        id: DiagnosticId,
        /// Target camera.
        target: CameraId,
        /// Retry attempt, bounded by the request policy.
        attempt: u32,
    },
    /// A request's own protocol deadline expired.
    ///
    /// `will_retry` is the scheduler's decision for this exact expiry, so a
    /// subscriber never has to infer it from a [`DiagnosticEvent::Transition`]
    /// and the absence of a following [`DiagnosticEvent::RetryScheduled`].
    DeadlineExpired {
        /// Opaque request lifecycle identity.
        id: DiagnosticId,
        /// Target camera.
        target: CameraId,
        /// Which deadline expired.
        deadline: DiagnosticDeadline,
        /// Whether this expiry scheduled another attempt.
        will_retry: bool,
    },
    /// Cancellation entered the owner.
    CancellationRecorded {
        /// Opaque request lifecycle identity.
        id: DiagnosticId,
        /// Target camera.
        target: CameraId,
    },
    /// Cancellation changed observation state.
    CancellationObserved {
        /// Opaque request lifecycle identity.
        id: DiagnosticId,
        /// Target camera.
        target: CameraId,
        /// Sanitized observation.
        observation: DiagnosticCancellation,
    },
    /// An exact applied-state effect was committed.
    AppliedState {
        /// Opaque request lifecycle identity.
        id: DiagnosticId,
        /// Target camera.
        target: CameraId,
        /// Applied-state key.
        key: StateKey,
    },
    /// A request reached a terminal outcome.
    Terminal {
        /// Opaque request lifecycle identity.
        id: DiagnosticId,
        /// Target camera.
        target: CameraId,
        /// Sanitized outcome.
        outcome: DiagnosticOutcome,
    },
    /// Session state changed.
    SessionChanged {
        /// Previous session state.
        from: SessionStatus,
        /// New session state.
        to: SessionStatus,
        /// Bounded reason category.
        reason: ErrorKind,
    },
    /// The engine deliberately ignored an input.
    Ignored {
        /// Bounded reason.
        reason: DiagnosticIgnoreReason,
    },
}

/// A bounded diagnostic subscription owned by the owner actor.
///
/// Dropping a subscription removes its receiver; the owner never awaits or
/// invokes user callbacks.  A full subscriber queue causes only that
/// subscriber's event to be dropped and increments the owner's dropped-event
/// counter.
#[derive(Debug)]
pub struct DiagnosticSubscription {
    pub(crate) inner: crate::runtime::owner::DiagnosticSubscription,
}

impl DiagnosticSubscription {
    #[cfg(feature = "async")]
    pub(crate) fn from_owner(inner: crate::runtime::owner::DiagnosticSubscription) -> Self {
        Self { inner }
    }

    /// Receives one event without waiting.  Empty and disconnected queues both
    /// return `None`.
    pub fn try_recv(&self) -> Option<DiagnosticEvent> {
        self.inner.try_recv().map(DiagnosticEvent::from_owner)
    }

    /// Receives one event asynchronously under the selected executor.
    #[cfg(feature = "async")]
    pub async fn recv(&self) -> Result<DiagnosticEvent, Error> {
        self.inner
            .recv_async()
            .await
            .map(DiagnosticEvent::from_owner)
    }
}

impl DiagnosticEvent {
    pub(crate) fn from_owner(event: crate::runtime::owner::DiagnosticEvent) -> Self {
        use crate::runtime::engine::{DeadlineKind, IgnoreReason, Phase, SessionState};
        use crate::runtime::owner::{
            CancellationDiagnostic, DiagnosticEvent as OwnerEvent, OutcomeDiagnostic, RequestLane,
            ResponseDiagnostic,
        };

        let phase = |value: Phase| match value {
            Phase::Ready { .. } => DiagnosticPhase::Ready,
            Phase::Sending { .. } => DiagnosticPhase::Sending,
            Phase::AwaitingAck { .. } => DiagnosticPhase::AwaitingAck,
            Phase::AwaitingCompletion { .. } => DiagnosticPhase::AwaitingCompletion,
            Phase::Executing { .. } => DiagnosticPhase::Executing,
            Phase::AwaitingReply { .. } => DiagnosticPhase::AwaitingReply,
            Phase::Backoff { .. } => DiagnosticPhase::Backoff,
            Phase::AwaitingCancellationResolution { .. } => {
                DiagnosticPhase::AwaitingCancellationResolution
            }
            Phase::AwaitingLateAck { .. } => DiagnosticPhase::AwaitingLateAck,
        };
        let session = |value: SessionState| match value {
            SessionState::Running => SessionStatus::Running,
            SessionState::Closed => SessionStatus::Closed,
            SessionState::Shutdown => SessionStatus::Shutdown,
            SessionState::Poisoned => SessionStatus::Poisoned,
        };
        let lane = |value: RequestLane| match value {
            RequestLane::Command => DiagnosticLane::Command,
            RequestLane::Inquiry => DiagnosticLane::Inquiry,
        };
        let response = |value: ResponseDiagnostic| match value {
            ResponseDiagnostic::Ack(_) => DiagnosticResponse::Ack,
            ResponseDiagnostic::Completion(_) => DiagnosticResponse::Completion,
            ResponseDiagnostic::InquiryReply => DiagnosticResponse::InquiryReply,
            ResponseDiagnostic::Error { .. } => DiagnosticResponse::Error,
            ResponseDiagnostic::NetworkChange => DiagnosticResponse::NetworkChange,
            ResponseDiagnostic::Unknown => DiagnosticResponse::Unknown,
        };
        let deadline = |value: DeadlineKind| match value {
            DeadlineKind::Ack => DiagnosticDeadline::Ack,
            DeadlineKind::Completion => DiagnosticDeadline::Completion,
            DeadlineKind::InquiryReply => DiagnosticDeadline::InquiryReply,
        };
        let cancellation = |value: CancellationDiagnostic| match value {
            CancellationDiagnostic::Recorded => DiagnosticCancellation::Recorded,
            CancellationDiagnostic::Cancelled => DiagnosticCancellation::Cancelled,
            CancellationDiagnostic::Completed => DiagnosticCancellation::Completed,
            CancellationDiagnostic::Failed(error) => DiagnosticCancellation::Failed(error),
        };
        let outcome = |value: OutcomeDiagnostic| match value {
            OutcomeDiagnostic::Written => DiagnosticOutcome::Written,
            OutcomeDiagnostic::Applied => DiagnosticOutcome::Applied,
            OutcomeDiagnostic::Reply => DiagnosticOutcome::Reply,
            OutcomeDiagnostic::Cancelled => DiagnosticOutcome::Cancelled,
            OutcomeDiagnostic::Failed(error) => DiagnosticOutcome::Failed(error),
        };
        let ignored = |value: IgnoreReason| match value {
            IgnoreReason::UnknownRequest => DiagnosticIgnoreReason::UnknownRequest,
            IgnoreReason::StaleQueueTicket => DiagnosticIgnoreReason::StaleQueueTicket,
            IgnoreReason::StaleTransmission => DiagnosticIgnoreReason::StaleTransmission,
            IgnoreReason::IncompatibleTransmissionResult => {
                DiagnosticIgnoreReason::IncompatibleTransmissionResult
            }
            IgnoreReason::MalformedFrame => DiagnosticIgnoreReason::MalformedFrame,
            IgnoreReason::UnmatchedFrame => DiagnosticIgnoreReason::UnmatchedFrame,
            IgnoreReason::UnmatchedSequencedFrame => {
                DiagnosticIgnoreReason::UnmatchedSequencedFrame
            }
            IgnoreReason::TargetIncompatibleSequence => {
                DiagnosticIgnoreReason::TargetIncompatibleSequence
            }
            IgnoreReason::AmbiguousLower16Sequence => {
                DiagnosticIgnoreReason::AmbiguousLower16Sequence
            }
            IgnoreReason::SocketConflict => DiagnosticIgnoreReason::SocketConflict,
            IgnoreReason::DuplicateCancellation => DiagnosticIgnoreReason::DuplicateCancellation,
            IgnoreReason::SessionNotRunning => DiagnosticIgnoreReason::SessionNotRunning,
        };

        match event {
            OwnerEvent::Admitted {
                id,
                target,
                lane: value,
                ..
            } => Self::Admitted {
                id: DiagnosticId::from_owner(id.get()),
                target,
                lane: lane(value),
            },
            OwnerEvent::AdmissionRejected {
                target,
                lane: value,
                error,
            } => Self::AdmissionRejected {
                target,
                lane: lane(value),
                error,
            },
            OwnerEvent::FrameReceived {
                target,
                response: value,
                ..
            } => Self::FrameReceived {
                target,
                response: response(value),
            },
            OwnerEvent::Transition {
                id,
                target,
                from,
                to,
                ..
            } => Self::Transition {
                id: DiagnosticId::from_owner(id.get()),
                target,
                from: phase(from),
                to: phase(to),
            },
            OwnerEvent::WriteFinished {
                id,
                target,
                cancellation,
                success,
                ..
            } => Self::WriteFinished {
                id: DiagnosticId::from_owner(id.get()),
                target,
                cancellation,
                success,
            },
            OwnerEvent::RetryScheduled {
                id,
                target,
                attempt,
                ..
            } => Self::RetryScheduled {
                id: DiagnosticId::from_owner(id.get()),
                target,
                attempt,
            },
            OwnerEvent::DeadlineExpired {
                id,
                target,
                deadline: value,
                will_retry,
            } => Self::DeadlineExpired {
                id: DiagnosticId::from_owner(id.get()),
                target,
                deadline: deadline(value),
                will_retry,
            },
            OwnerEvent::CancellationRecorded { id, target } => Self::CancellationRecorded {
                id: DiagnosticId::from_owner(id.get()),
                target,
            },
            OwnerEvent::CancellationObserved {
                id,
                target,
                observation,
            } => Self::CancellationObserved {
                id: DiagnosticId::from_owner(id.get()),
                target,
                observation: cancellation(observation),
            },
            OwnerEvent::AppliedState { id, target, key } => Self::AppliedState {
                id: DiagnosticId::from_owner(id.get()),
                target,
                key,
            },
            OwnerEvent::Terminal {
                id,
                target,
                outcome: value,
            } => Self::Terminal {
                id: DiagnosticId::from_owner(id.get()),
                target,
                outcome: outcome(value),
            },
            OwnerEvent::SessionChanged { from, to, reason } => Self::SessionChanged {
                from: session(from),
                to: session(to),
                reason,
            },
            OwnerEvent::Ignored(value) => Self::Ignored {
                reason: ignored(value),
            },
        }
    }
}

pub(crate) fn metrics_snapshot(
    metrics: crate::runtime::owner::OwnerMetrics,
    active: usize,
    pending: usize,
    session: crate::runtime::engine::SessionState,
) -> MetricsSnapshot {
    let session = match session {
        crate::runtime::engine::SessionState::Running => SessionStatus::Running,
        crate::runtime::engine::SessionState::Closed => SessionStatus::Closed,
        crate::runtime::engine::SessionState::Shutdown => SessionStatus::Shutdown,
        crate::runtime::engine::SessionState::Poisoned => SessionStatus::Poisoned,
    };
    MetricsSnapshot {
        admitted: metrics.admitted,
        admission_rejected: metrics.admission_rejected,
        writes: metrics.writes,
        write_failures: metrics.write_failures,
        terminal: metrics.terminal,
        cancellations: metrics.cancellations,
        cache_updates: metrics.cache_updates,
        ack_timeouts: metrics.ack_timeouts,
        completion_timeouts: metrics.completion_timeouts,
        inquiry_timeouts: metrics.inquiry_timeouts,
        busy_errors: metrics.busy_errors,
        protocol_errors: metrics.protocol_errors,
        retries_scheduled: metrics.retries_scheduled,
        received_frames: metrics.received_frames,
        ignored_unmatched_sequenced_replies: metrics.ignored_unmatched_sequenced_replies,
        ignored_malformed_frames: metrics.ignored_malformed_frames,
        dropped_diagnostics: metrics.dropped_diagnostics,
        dropped_diagnostic_events: metrics.dropped_diagnostic_events,
        dropped_observer_events: metrics.dropped_observer_events,
        dropped_applied_events: metrics.dropped_applied_events,
        dropped_boundary_work: metrics.dropped_boundary_work,
        active,
        pending,
        session,
    }
}
