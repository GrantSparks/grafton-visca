//! The closed per-request lifecycle. Phase-local facts have one legal home.

#[cfg(test)]
use super::CancelState;
use super::{
    add_duration, NonZeroDuration, Phase, ReplyShape, RequestFraming, RuntimeRequest,
    TransmissionId,
};
use crate::ViscaSocket;
use std::time::Instant;

/// Facts belonging to one dispatched attempt. Retry waiting retains the last
/// written attempt separately so a late exact-sequence ACK can adopt it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AttemptFacts {
    pub(crate) socket_capacity: Option<u8>,
    pub(crate) stream_order: Option<u64>,
}

/// Proof that the lifecycle has not recorded acceptance of this attempt.
///
/// Only the unacknowledged response states below can produce this value; a
/// retry therefore cannot be scheduled from an executing command.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RetryEligibility {
    attempt: AttemptFacts,
}

impl RetryEligibility {
    pub(crate) const fn attempt(self) -> AttemptFacts {
        self.attempt
    }
}

/// One expiry selected from the closed lifecycle. Its payload proves the
/// response family and retains the facts its handler needs without re-reading
/// a flattened phase.
#[derive(Debug, Clone, Copy)]
pub(crate) enum DueAction {
    Ack {
        sent_at: Instant,
        deadline: Instant,
        budget_at: Option<Instant>,
        retry: RetryEligibility,
        cancellation: Option<RequestedCancellation>,
    },
    Completion(CompletionDeadline),
    InquiryReply {
        retry: RetryEligibility,
    },
    CancellationResolution {
        observation_deadline: Option<Instant>,
    },
    CancellationAmbiguity,
    CancellationObservation {
        terminal_at: Instant,
    },
    RetryBudget {
        retirement: BudgetRetirement,
        certainty: crate::Certainty,
    },
    RetryReady {
        previous_attempt: AttemptFacts,
        budget_at: Option<Instant>,
        certainty: crate::Certainty,
    },
}

impl DueAction {
    pub(crate) const fn priority(self) -> u8 {
        match self {
            Self::CancellationAmbiguity => 0,
            Self::Ack { .. }
            | Self::Completion(_)
            | Self::InquiryReply { .. }
            | Self::CancellationResolution { .. }
            | Self::CancellationObservation { .. }
            | Self::RetryBudget { .. } => 1,
            Self::RetryReady { .. } => 2,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum CompletionDeadline {
    Unacknowledged {
        retry: RetryEligibility,
    },
    Accepted,
    Cancelling {
        socket: ViscaSocket,
        original_deadline: Instant,
        attempt: AttemptFacts,
        cancellation: ResolvingCancellation,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum BudgetRetirement {
    Queued,
    Command,
    Inquiry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RequestedCancellation {
    pub(crate) ambiguity_deadline: Instant,
    pub(crate) observer_open: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SendingCancellation {
    pub(crate) transmission: TransmissionId,
    pub(crate) framing: RequestFraming,
    pub(crate) ambiguity_deadline: Instant,
    pub(crate) observer_open: bool,
}

/// The response family selected before an immediate transport write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SendingReply {
    Inquiry,
    AckThenCompletion,
    CompletionOnly,
    NoReply,
}

impl SendingReply {
    pub(crate) const fn new(request: &RuntimeRequest) -> Self {
        match request {
            RuntimeRequest::Inquiry { .. } => Self::Inquiry,
            RuntimeRequest::Command { reply_shape, .. } => match reply_shape {
                ReplyShape::AckThenCompletion => Self::AckThenCompletion,
                ReplyShape::CompletionOnly => Self::CompletionOnly,
                ReplyShape::NoReply => Self::NoReply,
            },
        }
    }
}

/// Executing commands may have requested/sending/failed cancellation, but
/// AwaitingTerminal belongs only to the resolution phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutingCancellation {
    None,
    Requested(RequestedCancellation),
    Sending(SendingCancellation),
    ObservationFailed { ambiguity_deadline: Instant },
}

/// Every resolution phase has an active cancellation; None is unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolvingCancellation {
    Requested(RequestedCancellation),
    Sending(SendingCancellation),
    AwaitingTerminal {
        ambiguity_deadline: Instant,
        observation_deadline: Instant,
        observer_open: bool,
    },
    ObservationFailed {
        ambiguity_deadline: Instant,
    },
}

/// Stored lifecycle authority. Socket identity belongs to its enclosing phase,
/// never a cancellation substate; the write is completed before further input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lifecycle {
    Ready {
        previous_attempt: Option<AttemptFacts>,
    },
    Sending {
        transmission: TransmissionId,
        framing: RequestFraming,
        started_at: Instant,
        attempt: AttemptFacts,
        reply: SendingReply,
    },
    AwaitingAck {
        sent_at: Instant,
        deadline: Instant,
        attempt: AttemptFacts,
        cancellation: Option<RequestedCancellation>,
    },
    AwaitingCompletion {
        sent_at: Instant,
        deadline: Instant,
        attempt: AttemptFacts,
    },
    Executing {
        socket: ViscaSocket,
        started_at: Instant,
        deadline: Instant,
        attempt: AttemptFacts,
        cancellation: ExecutingCancellation,
    },
    AwaitingReply {
        send_order: TransmissionId,
        sent_at: Instant,
        deadline: Instant,
        attempt: AttemptFacts,
    },
    Backoff {
        ready_at: Instant,
        previous_attempt: AttemptFacts,
    },
    AwaitingCancellationResolution {
        socket: ViscaSocket,
        deadline: Instant,
        attempt: AttemptFacts,
        cancellation: ResolvingCancellation,
    },
}

impl Lifecycle {
    /// The socket owned by the logical phase, if it owns one.
    pub(crate) const fn socket(self) -> Option<ViscaSocket> {
        match self {
            Self::Executing { socket, .. }
            | Self::AwaitingCancellationResolution { socket, .. } => Some(socket),
            _ => None,
        }
    }

    /// Stable diagnostic/test projection, never an independently stored phase.
    pub(crate) fn phase(self) -> Phase {
        match self {
            Self::Ready { .. } => Phase::Ready,
            Self::Sending {
                transmission,
                started_at,
                ..
            } => Phase::Sending {
                transmission,
                started_at,
            },
            Self::AwaitingAck {
                sent_at, deadline, ..
            } => Phase::AwaitingAck { sent_at, deadline },
            Self::AwaitingCompletion {
                sent_at, deadline, ..
            } => Phase::AwaitingCompletion { sent_at, deadline },
            Self::Executing {
                socket,
                started_at,
                deadline,
                cancellation,
                ..
            } => Phase::Executing {
                socket,
                started_at,
                deadline: cancellation.response_deadline(deadline),
            },
            Self::AwaitingReply {
                sent_at, deadline, ..
            } => Phase::AwaitingReply { sent_at, deadline },
            Self::Backoff { ready_at, .. } => Phase::Backoff { ready_at },
            Self::AwaitingCancellationResolution {
                socket,
                deadline,
                cancellation,
                ..
            } => Phase::AwaitingCancellationResolution {
                socket,
                deadline: deadline.max(cancellation.ambiguity_deadline()),
            },
        }
    }

    /// Dispatched phases determine their complete response family.
    pub(crate) const fn reply_family(self) -> Option<SendingReply> {
        Some(match self {
            Self::Ready { .. } | Self::Backoff { .. } => return None,
            Self::Sending { reply, .. } => reply,
            Self::AwaitingReply { .. } => SendingReply::Inquiry,
            Self::AwaitingCompletion { .. } => SendingReply::CompletionOnly,
            Self::AwaitingAck { .. }
            | Self::Executing { .. }
            | Self::AwaitingCancellationResolution { .. } => SendingReply::AckThenCompletion,
        })
    }

    /// The earliest expiry selected from this lifecycle and the enclosing
    /// entry's retry budget and prior-attempt certainty.
    pub(crate) fn due_action(
        self,
        budget_at: Option<Instant>,
        certainty: crate::Certainty,
    ) -> Option<(Instant, DueAction)> {
        let mut due = match self {
            Self::Ready { .. } | Self::Sending { .. } => None,
            Self::Backoff {
                ready_at,
                previous_attempt,
                ..
            } => Some((
                ready_at,
                DueAction::RetryReady {
                    previous_attempt,
                    budget_at,
                    certainty,
                },
            )),
            Self::AwaitingAck {
                sent_at,
                deadline,
                attempt,
                cancellation,
            } => Some((
                deadline,
                DueAction::Ack {
                    sent_at,
                    deadline,
                    budget_at,
                    retry: RetryEligibility { attempt },
                    cancellation,
                },
            )),
            Self::AwaitingCompletion {
                deadline, attempt, ..
            } => Some((
                deadline,
                DueAction::Completion(CompletionDeadline::Unacknowledged {
                    retry: RetryEligibility { attempt },
                }),
            )),
            Self::Executing {
                socket,
                deadline,
                attempt,
                cancellation,
                ..
            } => {
                let action = match cancellation.into_resolution() {
                    Some(cancellation) => CompletionDeadline::Cancelling {
                        socket,
                        original_deadline: deadline,
                        attempt,
                        cancellation,
                    },
                    None => CompletionDeadline::Accepted,
                };
                Some((
                    cancellation.response_deadline(deadline),
                    DueAction::Completion(action),
                ))
            }
            Self::AwaitingReply {
                deadline, attempt, ..
            } => Some((
                deadline,
                DueAction::InquiryReply {
                    retry: RetryEligibility { attempt },
                },
            )),
            Self::AwaitingCancellationResolution {
                deadline,
                cancellation,
                ..
            } => {
                let terminal_at = deadline.max(cancellation.ambiguity_deadline());
                let observation_deadline = self.cancellation_observation_deadline();
                Some(match observation_deadline {
                    Some(observation) if observation <= terminal_at => (
                        observation,
                        DueAction::CancellationObservation { terminal_at },
                    ),
                    Some(_) | None => (
                        terminal_at,
                        DueAction::CancellationResolution {
                            observation_deadline,
                        },
                    ),
                })
            }
        };
        if let Some(budget) = budget_at {
            if due.is_none_or(|(at, _)| budget <= at) {
                due = Some((
                    budget,
                    DueAction::RetryBudget {
                        retirement: self.budget_retirement(),
                        certainty,
                    },
                ));
            }
        }
        if let Some(ambiguity) = self.active_cancellation_ambiguity() {
            if due.is_none_or(|(at, _)| ambiguity <= at) {
                due = Some((ambiguity, DueAction::CancellationAmbiguity));
            }
        }
        due
    }

    const fn budget_retirement(self) -> BudgetRetirement {
        match self {
            Self::Ready { .. } | Self::Backoff { .. } => BudgetRetirement::Queued,
            Self::Sending {
                reply: SendingReply::Inquiry,
                ..
            }
            | Self::AwaitingReply { .. } => BudgetRetirement::Inquiry,
            Self::Sending {
                reply:
                    SendingReply::AckThenCompletion
                    | SendingReply::CompletionOnly
                    | SendingReply::NoReply,
                ..
            }
            | Self::AwaitingAck { .. }
            | Self::AwaitingCompletion { .. }
            | Self::Executing { .. }
            | Self::AwaitingCancellationResolution { .. } => BudgetRetirement::Command,
        }
    }

    /// The old flattened view is useful to independent transition oracles.
    #[cfg(test)]
    pub(crate) const fn cancellation(self) -> CancelState {
        match self {
            Self::AwaitingAck { cancellation, .. } => match cancellation {
                Some(request) => CancelState::Requested {
                    ambiguity_deadline: request.ambiguity_deadline,
                },
                None => CancelState::None,
            },
            Self::Executing {
                socket,
                cancellation,
                ..
            } => match cancellation {
                ExecutingCancellation::None => CancelState::None,
                ExecutingCancellation::Requested(request) => CancelState::Requested {
                    ambiguity_deadline: request.ambiguity_deadline,
                },
                ExecutingCancellation::Sending(send) => CancelState::Sending {
                    socket,
                    transmission: send.transmission,
                    ambiguity_deadline: send.ambiguity_deadline,
                },
                ExecutingCancellation::ObservationFailed { ambiguity_deadline } => {
                    CancelState::ObservationFailed {
                        socket,
                        ambiguity_deadline,
                    }
                }
            },
            Self::AwaitingCancellationResolution {
                socket,
                cancellation,
                ..
            } => match cancellation {
                ResolvingCancellation::Requested(request) => CancelState::Requested {
                    ambiguity_deadline: request.ambiguity_deadline,
                },
                ResolvingCancellation::Sending(send) => CancelState::Sending {
                    socket,
                    transmission: send.transmission,
                    ambiguity_deadline: send.ambiguity_deadline,
                },
                ResolvingCancellation::AwaitingTerminal {
                    ambiguity_deadline,
                    observation_deadline,
                    ..
                } => CancelState::AwaitingTerminal {
                    ambiguity_deadline,
                    observation_deadline,
                },
                ResolvingCancellation::ObservationFailed { ambiguity_deadline } => {
                    CancelState::ObservationFailed {
                        socket,
                        ambiguity_deadline,
                    }
                }
            },
            Self::Sending { .. }
            | Self::Ready { .. }
            | Self::Backoff { .. }
            | Self::AwaitingCompletion { .. }
            | Self::AwaitingReply { .. } => CancelState::None,
        }
    }

    pub(crate) const fn attempt(self) -> Option<AttemptFacts> {
        match self {
            Self::Ready { .. } | Self::Backoff { .. } => None,
            Self::Sending { attempt, .. }
            | Self::AwaitingAck { attempt, .. }
            | Self::AwaitingCompletion { attempt, .. }
            | Self::Executing { attempt, .. }
            | Self::AwaitingReply { attempt, .. }
            | Self::AwaitingCancellationResolution { attempt, .. } => Some(attempt),
        }
    }

    /// The last written attempt remains identifiable while a retry waits.
    pub(crate) const fn previous_attempt(self) -> Option<AttemptFacts> {
        match self {
            Self::Ready {
                previous_attempt, ..
            } => previous_attempt,
            Self::Backoff {
                previous_attempt, ..
            } => Some(previous_attempt),
            _ => None,
        }
    }

    pub(crate) const fn retry_eligibility(
        self,
        replayable_completion: bool,
    ) -> Option<RetryEligibility> {
        let attempt = match self {
            Self::AwaitingAck { attempt, .. } | Self::AwaitingReply { attempt, .. } => attempt,
            Self::AwaitingCompletion { attempt, .. } if replayable_completion => attempt,
            Self::Sending { attempt, reply, .. }
                if matches!(
                    reply,
                    SendingReply::Inquiry | SendingReply::AckThenCompletion
                ) || (replayable_completion
                    && matches!(reply, SendingReply::CompletionOnly)) =>
            {
                attempt
            }
            _ => return None,
        };
        Some(RetryEligibility { attempt })
    }

    pub(crate) const fn observer_open(self) -> bool {
        match self {
            Self::AwaitingAck { cancellation, .. } => match cancellation {
                Some(request) => request.observer_open,
                None => false,
            },
            Self::Executing { cancellation, .. } => match cancellation {
                ExecutingCancellation::Requested(request) => request.observer_open,
                ExecutingCancellation::Sending(send) => send.observer_open,
                ExecutingCancellation::None | ExecutingCancellation::ObservationFailed { .. } => {
                    false
                }
            },
            Self::AwaitingCancellationResolution { cancellation, .. } => match cancellation {
                ResolvingCancellation::Requested(request) => request.observer_open,
                ResolvingCancellation::Sending(send) => send.observer_open,
                ResolvingCancellation::AwaitingTerminal { observer_open, .. } => observer_open,
                ResolvingCancellation::ObservationFailed { .. } => false,
            },
            Self::Sending { .. }
            | Self::Ready { .. }
            | Self::Backoff { .. }
            | Self::AwaitingCompletion { .. }
            | Self::AwaitingReply { .. } => false,
        }
    }

    pub(crate) fn close_observer(&mut self) {
        match self {
            Self::AwaitingAck { cancellation, .. } => {
                if let Some(request) = cancellation {
                    request.observer_open = false;
                }
            }
            Self::Executing { cancellation, .. } => match cancellation {
                ExecutingCancellation::Requested(request) => request.observer_open = false,
                ExecutingCancellation::Sending(send) => send.observer_open = false,
                ExecutingCancellation::None | ExecutingCancellation::ObservationFailed { .. } => {}
            },
            Self::AwaitingCancellationResolution { cancellation, .. } => match cancellation {
                ResolvingCancellation::Requested(request) => request.observer_open = false,
                ResolvingCancellation::Sending(send) => send.observer_open = false,
                ResolvingCancellation::AwaitingTerminal { observer_open, .. } => {
                    *observer_open = false
                }
                ResolvingCancellation::ObservationFailed { .. } => {}
            },
            Self::Sending { .. }
            | Self::Ready { .. }
            | Self::Backoff { .. }
            | Self::AwaitingCompletion { .. }
            | Self::AwaitingReply { .. } => {}
        }
    }

    pub(crate) const fn attempted_socket(self) -> Option<ViscaSocket> {
        match self {
            Self::Executing {
                socket,
                cancellation:
                    ExecutingCancellation::Sending(_) | ExecutingCancellation::ObservationFailed { .. },
                ..
            }
            | Self::AwaitingCancellationResolution {
                socket,
                cancellation:
                    ResolvingCancellation::Sending(_)
                    | ResolvingCancellation::AwaitingTerminal { .. }
                    | ResolvingCancellation::ObservationFailed { .. },
                ..
            } => Some(socket),
            Self::Executing {
                cancellation: ExecutingCancellation::None | ExecutingCancellation::Requested(_),
                ..
            }
            | Self::AwaitingCancellationResolution {
                cancellation: ResolvingCancellation::Requested(_),
                ..
            }
            | Self::Ready { .. }
            | Self::Sending { .. }
            | Self::AwaitingAck { .. }
            | Self::AwaitingCompletion { .. }
            | Self::AwaitingReply { .. }
            | Self::Backoff { .. } => None,
        }
    }

    pub(crate) const fn cancellation_ambiguity(self) -> Option<Instant> {
        match self {
            Self::AwaitingAck { cancellation, .. } => match cancellation {
                Some(request) => Some(request.ambiguity_deadline),
                None => None,
            },
            Self::Executing { cancellation, .. } => match cancellation {
                ExecutingCancellation::None => None,
                ExecutingCancellation::Requested(request) => Some(request.ambiguity_deadline),
                ExecutingCancellation::Sending(send) => Some(send.ambiguity_deadline),
                ExecutingCancellation::ObservationFailed { ambiguity_deadline } => {
                    Some(ambiguity_deadline)
                }
            },
            Self::AwaitingCancellationResolution { cancellation, .. } => Some(match cancellation {
                ResolvingCancellation::Requested(request) => request.ambiguity_deadline,
                ResolvingCancellation::Sending(send) => send.ambiguity_deadline,
                ResolvingCancellation::AwaitingTerminal {
                    ambiguity_deadline, ..
                }
                | ResolvingCancellation::ObservationFailed { ambiguity_deadline } => {
                    ambiguity_deadline
                }
            }),
            Self::Sending { .. }
            | Self::Ready { .. }
            | Self::Backoff { .. }
            | Self::AwaitingCompletion { .. }
            | Self::AwaitingReply { .. } => None,
        }
    }

    pub(crate) const fn active_cancellation_ambiguity(self) -> Option<Instant> {
        match self {
            Self::Executing { .. } | Self::AwaitingCancellationResolution { .. } => None,
            Self::Ready { .. }
            | Self::Sending { .. }
            | Self::AwaitingAck { .. }
            | Self::AwaitingCompletion { .. }
            | Self::AwaitingReply { .. }
            | Self::Backoff { .. } => self.cancellation_ambiguity(),
        }
    }

    pub(crate) fn retry_budget_deadline(
        self,
        submitted_at: Instant,
        budget: Option<NonZeroDuration>,
    ) -> Option<Instant> {
        budget
            .filter(|_| self.cancellation_ambiguity().is_none())
            .map(|budget| add_duration(submitted_at, budget.get()))
    }

    pub(crate) fn correlated_response_deadline(self, budget: Option<Instant>) -> Option<Instant> {
        let deadline = match self {
            Self::AwaitingAck { deadline, .. } => self.cancellation_ambiguity().or(Some(deadline)),
            Self::AwaitingCompletion { deadline, .. } | Self::AwaitingReply { deadline, .. } => {
                Some(deadline)
            }
            Self::Executing {
                deadline,
                cancellation,
                ..
            } => Some(cancellation.response_deadline(deadline)),
            Self::AwaitingCancellationResolution {
                deadline,
                cancellation,
                ..
            } => Some(deadline.max(cancellation.ambiguity_deadline())),
            Self::Sending { .. } => None,
            Self::Ready { .. } | Self::Backoff { .. } => return None,
        };
        [deadline, self.active_cancellation_ambiguity(), budget]
            .into_iter()
            .flatten()
            .min()
    }

    pub(crate) fn raw_cancellation_release_deadline(self) -> Option<Instant> {
        let ambiguity = self.cancellation_ambiguity()?;
        match self {
            Self::Sending { .. } | Self::AwaitingAck { .. } => Some(ambiguity),
            Self::Executing { deadline, .. } => Some(deadline.max(ambiguity)),
            Self::AwaitingCancellationResolution { deadline, .. } => Some(deadline.max(ambiguity)),
            Self::Ready { .. }
            | Self::AwaitingCompletion { .. }
            | Self::AwaitingReply { .. }
            | Self::Backoff { .. } => None,
        }
    }

    pub(crate) const fn pending_cancellation_socket(self) -> Option<ViscaSocket> {
        match self.pending_cancellation() {
            Some(pending) => Some(pending.socket()),
            None => None,
        }
    }
}

/// Cancellation's input decision follows directly from the stored legal state.
pub(crate) enum CancellationDecision {
    Local,
    Refused(crate::Error),
    Duplicate,
    Record(Lifecycle),
}

impl ExecutingCancellation {
    const fn into_resolution(self) -> Option<ResolvingCancellation> {
        match self {
            Self::None => None,
            Self::Requested(request) => Some(ResolvingCancellation::Requested(request)),
            Self::Sending(send) => Some(ResolvingCancellation::Sending(send)),
            Self::ObservationFailed { ambiguity_deadline } => {
                Some(ResolvingCancellation::ObservationFailed { ambiguity_deadline })
            }
        }
    }

    fn response_deadline(self, completion: Instant) -> Instant {
        match self {
            Self::None | Self::Requested(_) => completion,
            Self::Sending(send) => completion.max(send.ambiguity_deadline),
            Self::ObservationFailed { ambiguity_deadline } => completion.max(ambiguity_deadline),
        }
    }
}

impl ResolvingCancellation {
    pub(crate) const fn ambiguity_deadline(self) -> Instant {
        match self {
            Self::Requested(request) => request.ambiguity_deadline,
            Self::Sending(send) => send.ambiguity_deadline,
            Self::AwaitingTerminal {
                ambiguity_deadline, ..
            }
            | Self::ObservationFailed { ambiguity_deadline } => ambiguity_deadline,
        }
    }
}

impl Lifecycle {
    pub(crate) fn cancellation_decision(
        self,
        supported: bool,
        ambiguity_deadline: Instant,
    ) -> CancellationDecision {
        let requested = RequestedCancellation {
            ambiguity_deadline,
            observer_open: true,
        };
        let refused = || CancellationDecision::Refused(crate::Error::NotSupported);
        match self {
            Self::Ready { .. } | Self::Backoff { .. } => CancellationDecision::Local,
            Self::AwaitingCompletion { .. } => refused(),
            Self::AwaitingReply { .. } => {
                CancellationDecision::Refused(crate::Error::InquiryNotCancelable)
            }
            Self::AwaitingCancellationResolution { .. } => CancellationDecision::Duplicate,
            Self::Sending {
                reply: SendingReply::Inquiry,
                ..
            } => CancellationDecision::Refused(crate::Error::InquiryNotCancelable),
            Self::Sending { .. } => refused(),
            Self::AwaitingAck {
                cancellation: Some(_),
                ..
            }
            | Self::Executing {
                cancellation:
                    ExecutingCancellation::Requested(_)
                    | ExecutingCancellation::Sending(_)
                    | ExecutingCancellation::ObservationFailed { .. },
                ..
            } => CancellationDecision::Duplicate,
            Self::AwaitingAck {
                sent_at,
                deadline,
                attempt,
                cancellation: None,
            } => {
                if !supported {
                    return refused();
                }
                CancellationDecision::Record(Self::AwaitingAck {
                    sent_at,
                    deadline,
                    attempt,
                    cancellation: Some(requested),
                })
            }
            Self::Executing {
                socket,
                started_at,
                deadline,
                attempt,
                cancellation: ExecutingCancellation::None,
            } => {
                if !supported {
                    return refused();
                }
                CancellationDecision::Record(Self::Executing {
                    socket,
                    started_at,
                    deadline,
                    attempt,
                    cancellation: ExecutingCancellation::Requested(requested),
                })
            }
        }
    }
}

impl Lifecycle {
    pub(crate) const fn sending_cancellation(self) -> Option<SendingCancellation> {
        match self {
            Self::Executing {
                cancellation: ExecutingCancellation::Sending(send),
                ..
            }
            | Self::AwaitingCancellationResolution {
                cancellation: ResolvingCancellation::Sending(send),
                ..
            } => Some(send),
            Self::Executing {
                cancellation:
                    ExecutingCancellation::None
                    | ExecutingCancellation::Requested(_)
                    | ExecutingCancellation::ObservationFailed { .. },
                ..
            }
            | Self::AwaitingCancellationResolution {
                cancellation:
                    ResolvingCancellation::Requested(_)
                    | ResolvingCancellation::AwaitingTerminal { .. }
                    | ResolvingCancellation::ObservationFailed { .. },
                ..
            }
            | Self::Ready { .. }
            | Self::Sending { .. }
            | Self::AwaitingAck { .. }
            | Self::AwaitingCompletion { .. }
            | Self::AwaitingReply { .. }
            | Self::Backoff { .. } => None,
        }
    }

    pub(crate) const fn awaiting_cancellation_answer(self) -> bool {
        match self {
            Self::Executing {
                cancellation: ExecutingCancellation::Sending(_),
                ..
            }
            | Self::AwaitingCancellationResolution {
                cancellation:
                    ResolvingCancellation::Sending(_) | ResolvingCancellation::AwaitingTerminal { .. },
                ..
            } => true,
            Self::Executing {
                cancellation:
                    ExecutingCancellation::None
                    | ExecutingCancellation::Requested(_)
                    | ExecutingCancellation::ObservationFailed { .. },
                ..
            }
            | Self::AwaitingCancellationResolution {
                cancellation:
                    ResolvingCancellation::Requested(_)
                    | ResolvingCancellation::ObservationFailed { .. },
                ..
            }
            | Self::Ready { .. }
            | Self::Sending { .. }
            | Self::AwaitingAck { .. }
            | Self::AwaitingCompletion { .. }
            | Self::AwaitingReply { .. }
            | Self::Backoff { .. } => false,
        }
    }

    pub(crate) const fn cancellation_observation_deadline(self) -> Option<Instant> {
        match self {
            Self::AwaitingCancellationResolution {
                cancellation:
                    ResolvingCancellation::AwaitingTerminal {
                        observation_deadline,
                        observer_open: true,
                        ..
                    },
                ..
            } => Some(observation_deadline),
            Self::AwaitingCancellationResolution {
                cancellation:
                    ResolvingCancellation::Requested(_)
                    | ResolvingCancellation::Sending(_)
                    | ResolvingCancellation::ObservationFailed { .. }
                    | ResolvingCancellation::AwaitingTerminal {
                        observer_open: false,
                        ..
                    },
                ..
            }
            | Self::Executing { .. }
            | Self::Ready { .. }
            | Self::Sending { .. }
            | Self::AwaitingAck { .. }
            | Self::AwaitingCompletion { .. }
            | Self::AwaitingReply { .. }
            | Self::Backoff { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingCancellationPhase {
    Executing {
        started_at: Instant,
        deadline: Instant,
        attempt: AttemptFacts,
    },
    Resolving {
        deadline: Instant,
        attempt: AttemptFacts,
    },
}

/// Proof that one socketed lifecycle currently holds a requested cancellation.
/// Only the exhaustive lifecycle projection constructs this emission input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PendingCancellation {
    socket: ViscaSocket,
    request: RequestedCancellation,
    phase: PendingCancellationPhase,
}

impl PendingCancellation {
    pub(crate) const fn socket(self) -> ViscaSocket {
        self.socket
    }
    pub(crate) const fn ambiguity_deadline(self) -> Instant {
        self.request.ambiguity_deadline
    }

    pub(crate) const fn emitted(
        self,
        transmission: TransmissionId,
        ambiguity_deadline: Instant,
        framing: RequestFraming,
    ) -> Lifecycle {
        let send = SendingCancellation {
            transmission,
            framing,
            ambiguity_deadline,
            observer_open: self.request.observer_open,
        };
        match self.phase {
            PendingCancellationPhase::Executing {
                started_at,
                deadline,
                attempt,
            } => Lifecycle::Executing {
                socket: self.socket,
                started_at,
                deadline,
                attempt,
                cancellation: ExecutingCancellation::Sending(send),
            },
            PendingCancellationPhase::Resolving { deadline, attempt } => {
                Lifecycle::AwaitingCancellationResolution {
                    socket: self.socket,
                    deadline,
                    attempt,
                    cancellation: ResolvingCancellation::Sending(send),
                }
            }
        }
    }
}

impl Lifecycle {
    pub(crate) const fn pending_cancellation(self) -> Option<PendingCancellation> {
        match self {
            Self::Executing {
                socket,
                started_at,
                deadline,
                attempt,
                cancellation: ExecutingCancellation::Requested(request),
            } => Some(PendingCancellation {
                socket,
                request,
                phase: PendingCancellationPhase::Executing {
                    started_at,
                    deadline,
                    attempt,
                },
            }),
            Self::AwaitingCancellationResolution {
                socket,
                deadline,
                attempt,
                cancellation: ResolvingCancellation::Requested(request),
            } => Some(PendingCancellation {
                socket,
                request,
                phase: PendingCancellationPhase::Resolving { deadline, attempt },
            }),
            Self::Executing {
                cancellation:
                    ExecutingCancellation::None
                    | ExecutingCancellation::Sending(_)
                    | ExecutingCancellation::ObservationFailed { .. },
                ..
            }
            | Self::AwaitingCancellationResolution {
                cancellation:
                    ResolvingCancellation::Sending(_)
                    | ResolvingCancellation::AwaitingTerminal { .. }
                    | ResolvingCancellation::ObservationFailed { .. },
                ..
            }
            | Self::Ready { .. }
            | Self::Sending { .. }
            | Self::AwaitingAck { .. }
            | Self::AwaitingCompletion { .. }
            | Self::AwaitingReply { .. }
            | Self::Backoff { .. } => None,
        }
    }
}
