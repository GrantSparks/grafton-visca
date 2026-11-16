//! One producer-side projection from engine facts to public diagnostics.
use super::*;
use crate::observability::{
    DiagnosticCancellation, DiagnosticDeadline, DiagnosticId, DiagnosticIgnoreReason,
    DiagnosticLane, DiagnosticOutcome, DiagnosticPhase, DiagnosticResponse,
};
use crate::runtime::engine::Phase;

pub(super) fn id(id: RequestId) -> DiagnosticId {
    DiagnosticId::from_owner(id.get())
}

pub(super) fn phase(value: Lifecycle) -> DiagnosticPhase {
    match value.phase() {
        Phase::Ready => DiagnosticPhase::Ready,
        Phase::Sending { .. } => DiagnosticPhase::Sending,
        Phase::AwaitingAck { .. } => DiagnosticPhase::AwaitingAck,
        Phase::AwaitingCompletion { .. } => DiagnosticPhase::AwaitingCompletion,
        Phase::Executing { .. } | Phase::ExecutingDisplaced { .. } => DiagnosticPhase::Executing,
        Phase::AwaitingReply { .. } => DiagnosticPhase::AwaitingReply,
        Phase::Backoff { .. } => DiagnosticPhase::Backoff,
        Phase::AwaitingCancellationResolution { .. } => {
            DiagnosticPhase::AwaitingCancellationResolution
        }
    }
}

pub(super) fn lane(value: Lane) -> DiagnosticLane {
    match value {
        Lane::Command => DiagnosticLane::Command,
        Lane::Inquiry => DiagnosticLane::Inquiry,
    }
}

pub(super) fn deadline(value: DeadlineKind) -> DiagnosticDeadline {
    match value {
        DeadlineKind::Ack => DiagnosticDeadline::Ack,
        DeadlineKind::Completion => DiagnosticDeadline::Completion,
        DeadlineKind::InquiryReply => DiagnosticDeadline::InquiryReply,
    }
}

pub(super) fn ignored(value: IgnoreReason) -> DiagnosticIgnoreReason {
    match value {
        IgnoreReason::UnknownRequest => DiagnosticIgnoreReason::UnknownRequest,
        IgnoreReason::StaleTransmission => DiagnosticIgnoreReason::StaleTransmission,
        IgnoreReason::IncompatibleTransmissionResult => {
            DiagnosticIgnoreReason::IncompatibleTransmissionResult
        }
        IgnoreReason::MalformedFrame => DiagnosticIgnoreReason::MalformedFrame,
        IgnoreReason::ReplyShapeMismatch => DiagnosticIgnoreReason::ReplyShapeMismatch,
        IgnoreReason::UnmatchedFrame => DiagnosticIgnoreReason::UnmatchedFrame,
        IgnoreReason::UnmatchedSequencedFrame => DiagnosticIgnoreReason::UnmatchedSequencedFrame,
        IgnoreReason::TargetIncompatibleSequence => {
            DiagnosticIgnoreReason::TargetIncompatibleSequence
        }
        IgnoreReason::AmbiguousLower16Sequence => DiagnosticIgnoreReason::AmbiguousLower16Sequence,
        IgnoreReason::SocketConflict => DiagnosticIgnoreReason::SocketConflict,
        IgnoreReason::DuplicateCancellation => DiagnosticIgnoreReason::DuplicateCancellation,
        IgnoreReason::SessionNotRunning => DiagnosticIgnoreReason::SessionNotRunning,
    }
}

pub(super) fn outcome(outcome: &RuntimeOutcome) -> DiagnosticOutcome {
    match outcome {
        RuntimeOutcome::Command(CommandOutcome::Written) => DiagnosticOutcome::Written,
        RuntimeOutcome::Command(CommandOutcome::Applied) => DiagnosticOutcome::Applied,
        RuntimeOutcome::Inquiry(InquiryOutcome::Reply { .. }) => DiagnosticOutcome::Reply,
        RuntimeOutcome::Command(CommandOutcome::Cancelled) => DiagnosticOutcome::Cancelled,
        RuntimeOutcome::Command(CommandOutcome::Failed(error))
        | RuntimeOutcome::Inquiry(InquiryOutcome::Failed(error)) => {
            DiagnosticOutcome::Failed(error.kind())
        }
    }
}

pub(super) fn cancellation(observation: &CancellationObservation) -> DiagnosticCancellation {
    match observation {
        CancellationObservation::Recorded => DiagnosticCancellation::Recorded,
        CancellationObservation::Cancelled => DiagnosticCancellation::Cancelled,
        CancellationObservation::Completed => DiagnosticCancellation::Completed,
        CancellationObservation::Failed(error) => DiagnosticCancellation::Failed(error.kind()),
    }
}

pub(super) fn response(response: &DecodedResponse) -> DiagnosticResponse {
    match response {
        DecodedResponse::Ack { .. } => DiagnosticResponse::Ack,
        DecodedResponse::Completion { .. } => DiagnosticResponse::Completion,
        DecodedResponse::InquiryReply { .. } => DiagnosticResponse::InquiryReply,
        DecodedResponse::Error { .. } => DiagnosticResponse::Error,
        DecodedResponse::SonyControl { code } => DiagnosticResponse::SonyControl { code: *code },
        DecodedResponse::NetworkChange => DiagnosticResponse::NetworkChange,
        DecodedResponse::Unknown => DiagnosticResponse::Unknown,
    }
}
