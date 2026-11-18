//! The closed per-request lifecycle. Phase-local facts have one legal home.

use super::entry_table::{EntryChange, SocketedLifecycle, UnsocketedLifecycle};

#[cfg(test)]
use super::CancelState;
use super::{
    add_duration, CommandReply, CorrelationKind, NonZeroDuration, Phase, RuntimeRequest,
    TransmissionId,
};
use crate::ViscaSocket;
use std::time::Instant;

/// Facts belonging to one dispatched attempt. Retry waiting retains the last
/// written attempt separately so a late exact-sequence ACK can adopt it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AttemptFacts {
    pub(crate) socket_capacity: Option<crate::CommandSocketCount>,
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
    CancellationObservation {
        terminal_at: Instant,
    },
    RetryBudget {
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
        original_deadline: Instant,
        attempt: AttemptFacts,
        cancellation: ResolvingCancellation,
    },
}

/// One due raw cancellation release, with its correlation identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RawCancellationRelease {
    pub(crate) at: Instant,
    pub(crate) scope: ReleaseScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReleaseScope {
    ExactSocket(ViscaSocket),
    PreAckUnkeyed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RequestedCancellation {
    pub(crate) ambiguity_deadline: Instant,
    pub(crate) observer_open: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SendingCancellation {
    pub(crate) transmission: TransmissionId,
    pub(crate) ambiguity_deadline: Instant,
    pub(crate) observer_open: bool,
}

/// The response family selected before an immediate transport write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SendingReply {
    Stop,
    Inquiry,
    AckThenCompletion,
    CompletionOnly,
    NoReply,
}

impl SendingReply {
    const fn is_command(self) -> bool {
        match self {
            Self::Stop | Self::AckThenCompletion | Self::CompletionOnly | Self::NoReply => true,
            Self::Inquiry => false,
        }
    }

    pub(crate) const fn new(request: &RuntimeRequest) -> Self {
        match request {
            RuntimeRequest::Inquiry { .. } => Self::Inquiry,
            RuntimeRequest::Command { reply_shape, .. } => match reply_shape {
                CommandReply::Stop => Self::Stop,
                CommandReply::AckThenCompletion => Self::AckThenCompletion,
                CommandReply::CompletionOnly => Self::CompletionOnly,
                CommandReply::NoReply => Self::NoReply,
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
    /// Sony acceptance whose socket was subsequently reassigned by the camera.
    /// Only its sequence retains response authority; no cancellation may be sent.
    ExecutingDisplaced {
        started_at: Instant,
        deadline: Instant,
        attempt: AttemptFacts,
    },
    AwaitingReply {
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

    /// Acceptance survives loss of a camera-reassigned socket.
    pub(crate) const fn is_accepted(self) -> bool {
        matches!(
            self,
            Self::Executing { .. }
                | Self::ExecutingDisplaced { .. }
                | Self::AwaitingCancellationResolution { .. }
        )
    }

    /// A queued request owns no active wire attempt.
    pub(crate) const fn is_off_wire(self) -> bool {
        self.attempt().is_none()
    }
    pub(crate) const fn is_ready(self) -> bool {
        matches!(self, Self::Ready { .. })
    }
    pub(crate) const fn is_sending(self) -> bool {
        matches!(self, Self::Sending { .. })
    }
    pub(crate) const fn is_awaiting_ack(self) -> bool {
        matches!(self, Self::AwaitingAck { .. })
    }
    pub(crate) const fn is_awaiting_reply(self) -> bool {
        matches!(self, Self::AwaitingReply { .. })
    }
    pub(crate) const fn is_awaiting_completion(self) -> bool {
        matches!(self, Self::AwaitingCompletion { .. })
    }
    /// Inquiry occupancy includes the local write, before its reply deadline starts.
    pub(crate) const fn is_live_inquiry(self) -> bool {
        matches!(
            self,
            Self::Sending {
                reply: SendingReply::Inquiry,
                ..
            } | Self::AwaitingReply { .. }
        )
    }
    /// Physical command occupancy lasts through cancellation resolution.
    /// A Sony execution displaced by camera-proven socket reuse retains no
    /// physical capacity, although its sequence and acceptance remain live.
    pub(crate) const fn is_command_on_wire(self) -> bool {
        match self {
            Self::Sending { reply, .. } => reply.is_command(),
            _ => matches!(
                self,
                Self::AwaitingAck { .. }
                    | Self::AwaitingCompletion { .. }
                    | Self::Executing { .. }
                    | Self::AwaitingCancellationResolution { .. }
            ),
        }
    }
    pub(crate) const fn is_unacknowledged_command(self) -> bool {
        match self {
            Self::Sending { reply, .. } => reply.is_command(),
            _ => matches!(
                self,
                Self::AwaitingAck { .. } | Self::AwaitingCompletion { .. }
            ),
        }
    }
    pub(crate) const fn is_uncorrelated_command(self) -> bool {
        matches!(
            self,
            Self::Sending {
                reply: SendingReply::CompletionOnly | SendingReply::NoReply,
                ..
            } | Self::AwaitingCompletion { .. }
        )
    }

    /// An immediate request or cancellation write still awaits its receipt.
    pub(crate) const fn staged_write(self) -> bool {
        self.is_sending() || self.cancellation_write().is_some()
    }

    /// End of the ACK response window. Cancellation intent may extend the
    /// original deadline, but must never shorten it.
    pub(crate) fn awaiting_ack_end(self) -> Option<Instant> {
        match self {
            Self::AwaitingAck {
                deadline,
                cancellation,
                ..
            } => Some(
                cancellation.map_or(deadline, |request| deadline.max(request.ambiguity_deadline)),
            ),
            _ => None,
        }
    }

    /// End of the active response window, including any cancellation extension.
    pub(crate) fn response_window_end(self) -> Option<Instant> {
        match self {
            Self::AwaitingAck { .. } => self.awaiting_ack_end(),
            Self::AwaitingCompletion { deadline, .. }
            | Self::AwaitingReply { deadline, .. }
            | Self::ExecutingDisplaced { deadline, .. } => Some(deadline),
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
            Self::Ready { .. } | Self::Sending { .. } | Self::Backoff { .. } => None,
        }
    }

    /// Stable diagnostic/test projection, never an independently stored phase.
    pub(crate) fn phase(self) -> Phase {
        // Every response-bearing phase has a window. Off-wire phases never
        // consult it; their timestamps describe scheduling rather than replies.
        let window = self.response_window_end();
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
            } => Phase::AwaitingAck {
                sent_at,
                deadline: window.unwrap_or(deadline),
            },
            Self::AwaitingCompletion {
                sent_at, deadline, ..
            } => Phase::AwaitingCompletion {
                sent_at,
                deadline: window.unwrap_or(deadline),
            },
            Self::Executing {
                socket,
                started_at,
                deadline,
                ..
            } => Phase::Executing {
                socket,
                started_at,
                deadline: window.unwrap_or(deadline),
            },
            Self::ExecutingDisplaced {
                started_at,
                deadline,
                ..
            } => Phase::ExecutingDisplaced {
                started_at,
                deadline: window.unwrap_or(deadline),
            },
            Self::AwaitingReply {
                sent_at, deadline, ..
            } => Phase::AwaitingReply {
                sent_at,
                deadline: window.unwrap_or(deadline),
            },
            Self::Backoff { ready_at, .. } => Phase::Backoff { ready_at },
            Self::AwaitingCancellationResolution {
                socket, deadline, ..
            } => Phase::AwaitingCancellationResolution {
                socket,
                deadline: window.unwrap_or(deadline),
            },
        }
    }

    /// The immutable request class and active protocol phase must agree.
    pub(crate) fn accepts_reply(self, family: SendingReply) -> bool {
        match self {
            Self::Ready { .. } | Self::Backoff { .. } => true,
            Self::Sending { reply, .. } => reply == family,
            Self::AwaitingReply { .. } => matches!(family, SendingReply::Inquiry),
            Self::AwaitingCompletion { .. } => matches!(family, SendingReply::CompletionOnly),
            Self::AwaitingAck { .. }
            | Self::Executing { .. }
            | Self::ExecutingDisplaced { .. }
            | Self::AwaitingCancellationResolution { .. } => {
                matches!(family, SendingReply::Stop | SendingReply::AckThenCompletion)
            }
        }
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
                self.awaiting_ack_end()?,
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
                deadline,
                attempt,
                cancellation,
                ..
            } => {
                let action = match cancellation.into_resolution() {
                    Some(cancellation) => CompletionDeadline::Cancelling {
                        original_deadline: deadline,
                        attempt,
                        cancellation,
                    },
                    None => CompletionDeadline::Accepted,
                };
                Some((self.response_window_end()?, DueAction::Completion(action)))
            }
            Self::ExecutingDisplaced { deadline, .. } => Some((
                deadline,
                DueAction::Completion(CompletionDeadline::Accepted),
            )),
            Self::AwaitingReply {
                deadline, attempt, ..
            } => Some((
                deadline,
                DueAction::InquiryReply {
                    retry: RetryEligibility { attempt },
                },
            )),
            Self::AwaitingCancellationResolution { .. } => {
                let terminal_at = self.response_window_end()?;
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
                due = Some((budget, DueAction::RetryBudget { certainty }));
            }
        }
        due
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
            | Self::ExecutingDisplaced { .. }
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
            | Self::ExecutingDisplaced { attempt, .. }
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
                    SendingReply::Inquiry | SendingReply::Stop | SendingReply::AckThenCompletion
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
            | Self::ExecutingDisplaced { .. }
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
            | Self::ExecutingDisplaced { .. }
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
            | Self::ExecutingDisplaced { .. }
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
            | Self::ExecutingDisplaced { .. }
            | Self::Ready { .. }
            | Self::Backoff { .. }
            | Self::AwaitingCompletion { .. }
            | Self::AwaitingReply { .. } => None,
        }
    }

    /// Accepted cancellation suppresses retries and their budget. The phase's
    /// effective response deadline remains authoritative until resolution.
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
        if self.is_off_wire() {
            return None;
        }
        [self.response_window_end(), budget]
            .into_iter()
            .flatten()
            .min()
    }

    pub(crate) fn raw_cancellation_release(self) -> Option<RawCancellationRelease> {
        let ambiguity = self.cancellation_ambiguity()?;
        let scope = match self {
            Self::AwaitingAck { .. } => ReleaseScope::PreAckUnkeyed,
            Self::Executing { socket, .. }
            | Self::AwaitingCancellationResolution { socket, .. } => {
                ReleaseScope::ExactSocket(socket)
            }
            Self::Sending { .. }
            | Self::ExecutingDisplaced { .. }
            | Self::Ready { .. }
            | Self::AwaitingCompletion { .. }
            | Self::AwaitingReply { .. }
            | Self::Backoff { .. } => return None,
        };
        Some(RawCancellationRelease {
            // A requested but not yet emitted cancellation first crosses
            // into resolution at the response window; only its ambiguity
            // deadline can release the identity.
            at: self.response_window_end()?.max(ambiguity),
            scope,
        })
    }

    pub(crate) const fn pending_cancellation_socket(self) -> Option<ViscaSocket> {
        match self.pending_cancellation() {
            Some(pending) => Some(pending.socket()),
            None => None,
        }
    }
}

/// Cancellation's input decision follows directly from the stored legal state.
pub(super) enum CancellationDecision {
    Local,
    Refused(crate::Error),
    Duplicate,
    Record(EntryChange),
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

/// The request phases that can accept command cancellation.
pub(super) enum CommandLifecycle {
    OffWire,
    Sending(CommandReply),
    AwaitingAck {
        sent_at: Instant,
        deadline: Instant,
        attempt: AttemptFacts,
        cancellation: Option<RequestedCancellation>,
    },
    AwaitingCompletion,
    Executing {
        started_at: Instant,
        deadline: Instant,
        attempt: AttemptFacts,
        cancellation: ExecutingCancellation,
    },
    ExecutingDisplaced,
    AwaitingCancellationResolution,
}

impl Lifecycle {
    pub(super) const fn command_view(self) -> Option<CommandLifecycle> {
        Some(match self {
            Self::Ready { .. } | Self::Backoff { .. } => CommandLifecycle::OffWire,
            Self::Sending { reply, .. } => CommandLifecycle::Sending(match reply {
                SendingReply::AckThenCompletion => CommandReply::AckThenCompletion,
                SendingReply::Stop => CommandReply::Stop,
                SendingReply::CompletionOnly => CommandReply::CompletionOnly,
                SendingReply::NoReply => CommandReply::NoReply,
                SendingReply::Inquiry => return None,
            }),
            Self::AwaitingReply { .. } => return None,
            Self::AwaitingAck {
                sent_at,
                deadline,
                attempt,
                cancellation,
            } => CommandLifecycle::AwaitingAck {
                sent_at,
                deadline,
                attempt,
                cancellation,
            },
            Self::AwaitingCompletion { .. } => CommandLifecycle::AwaitingCompletion,
            Self::Executing {
                started_at,
                deadline,
                attempt,
                cancellation,
                ..
            } => CommandLifecycle::Executing {
                started_at,
                deadline,
                attempt,
                cancellation,
            },
            Self::ExecutingDisplaced { .. } => CommandLifecycle::ExecutingDisplaced,
            Self::AwaitingCancellationResolution { .. } => {
                CommandLifecycle::AwaitingCancellationResolution
            }
        })
    }
}

impl CommandLifecycle {
    pub(super) fn cancellation_decision(
        self,
        supported: bool,
        ambiguity_deadline: Instant,
    ) -> CancellationDecision {
        let requested = RequestedCancellation {
            ambiguity_deadline,
            observer_open: true,
        };
        let refused = || CancellationDecision::Refused(crate::Error::CancellationNotSupported);
        match self {
            Self::OffWire => CancellationDecision::Local,
            Self::AwaitingCompletion => refused(),
            Self::ExecutingDisplaced => {
                CancellationDecision::Refused(crate::Error::CancellationUnconfirmed)
            }
            Self::AwaitingCancellationResolution => CancellationDecision::Duplicate,
            Self::Sending(
                CommandReply::AckThenCompletion
                | CommandReply::Stop
                | CommandReply::CompletionOnly
                | CommandReply::NoReply,
            ) => refused(),
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
                CancellationDecision::Record(
                    UnsocketedLifecycle::AwaitingAck {
                        sent_at,
                        deadline,
                        attempt,
                        cancellation: Some(requested),
                    }
                    .into(),
                )
            }
            Self::Executing {
                started_at,
                deadline,
                attempt,
                cancellation: ExecutingCancellation::None,
            } => {
                if !supported {
                    return refused();
                }
                CancellationDecision::Record(
                    SocketedLifecycle::Executing {
                        started_at,
                        deadline,
                        attempt,
                        cancellation: ExecutingCancellation::Requested(requested),
                    }
                    .into(),
                )
            }
        }
    }
}

impl Lifecycle {
    /// The closed write proof retains the phase facts a receipt will consume.
    pub(crate) fn active_write(self, transmission: TransmissionId) -> Option<ActiveWrite> {
        if let Self::Sending {
            transmission: active,
            attempt,
            reply,
            ..
        } = self
        {
            return (active == transmission).then_some(ActiveWrite::Request { attempt, reply });
        }
        let cancellation = self.cancellation_write()?;
        (cancellation.send.transmission == transmission)
            .then_some(ActiveWrite::Cancellation(cancellation))
    }

    const fn cancellation_write(self) -> Option<CancellationWrite> {
        match self {
            Self::Executing {
                socket: _,
                started_at,
                deadline,
                attempt,
                cancellation: ExecutingCancellation::Sending(send),
            } => Some(CancellationWrite {
                send,
                phase: SocketedPhase::Executing {
                    started_at,
                    deadline,
                    attempt,
                },
            }),
            Self::AwaitingCancellationResolution {
                socket: _,
                deadline,
                attempt,
                cancellation: ResolvingCancellation::Sending(send),
            } => Some(CancellationWrite {
                send,
                phase: SocketedPhase::Resolving { deadline, attempt },
            }),
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
            | Self::ExecutingDisplaced { .. }
            | Self::Ready { .. }
            | Self::Sending { .. }
            | Self::AwaitingAck { .. }
            | Self::AwaitingCompletion { .. }
            | Self::AwaitingReply { .. }
            | Self::Backoff { .. } => None,
        }
    }

    #[cfg(test)]
    pub(crate) const fn sending_cancellation(self) -> Option<SendingCancellation> {
        match self.cancellation_write() {
            Some(write) => Some(write.send),
            None => None,
        }
    }

    pub(crate) const fn unanswered_cancellation(self) -> Option<(ViscaSocket, Instant)> {
        if !self.awaiting_cancellation_answer() {
            return None;
        }
        match (self.socket(), self.cancellation_ambiguity()) {
            (Some(socket), Some(deadline)) => Some((socket, deadline)),
            _ => None,
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
            | Self::ExecutingDisplaced { .. }
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
            | Self::ExecutingDisplaced { .. }
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
enum SocketedPhase {
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

/// Proof that a receipt belongs to one currently active write.
/// Only `Lifecycle::active_write` constructs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActiveWrite {
    Request {
        attempt: AttemptFacts,
        reply: SendingReply,
    },
    Cancellation(CancellationWrite),
}

impl ActiveWrite {
    pub(super) const fn kind(self) -> CorrelationKind {
        match self {
            Self::Request { .. } => CorrelationKind::Request,
            Self::Cancellation(_) => CorrelationKind::Cancellation,
        }
    }
}

/// Socket and phase facts proved when a cancellation write is identified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CancellationWrite {
    phase: SocketedPhase,
    send: SendingCancellation,
}

impl CancellationWrite {
    pub(crate) const fn written(self, observation_deadline: Instant) -> SocketedLifecycle {
        let (deadline, attempt) = match self.phase {
            SocketedPhase::Executing {
                deadline, attempt, ..
            }
            | SocketedPhase::Resolving { deadline, attempt } => (deadline, attempt),
        };
        SocketedLifecycle::AwaitingCancellationResolution {
            deadline,
            attempt,
            cancellation: ResolvingCancellation::AwaitingTerminal {
                ambiguity_deadline: self.send.ambiguity_deadline,
                observation_deadline,
                observer_open: self.send.observer_open,
            },
        }
    }

    pub(crate) const fn refused(self) -> SocketedLifecycle {
        match self.phase {
            SocketedPhase::Executing {
                started_at,
                deadline,
                attempt,
            } => SocketedLifecycle::Executing {
                started_at,
                deadline,
                attempt,
                cancellation: ExecutingCancellation::ObservationFailed {
                    ambiguity_deadline: self.send.ambiguity_deadline,
                },
            },
            SocketedPhase::Resolving { deadline, attempt } => {
                SocketedLifecycle::AwaitingCancellationResolution {
                    deadline,
                    attempt,
                    cancellation: ResolvingCancellation::ObservationFailed {
                        ambiguity_deadline: self.send.ambiguity_deadline,
                    },
                }
            }
        }
    }
}

/// Proof that one socketed lifecycle currently holds a requested cancellation.
/// Only the exhaustive lifecycle projection constructs this emission input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PendingCancellation {
    socket: ViscaSocket,
    request: RequestedCancellation,
    phase: SocketedPhase,
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
    ) -> SocketedLifecycle {
        let send = SendingCancellation {
            transmission,
            ambiguity_deadline,
            observer_open: self.request.observer_open,
        };
        match self.phase {
            SocketedPhase::Executing {
                started_at,
                deadline,
                attempt,
            } => SocketedLifecycle::Executing {
                started_at,
                deadline,
                attempt,
                cancellation: ExecutingCancellation::Sending(send),
            },
            SocketedPhase::Resolving { deadline, attempt } => {
                SocketedLifecycle::AwaitingCancellationResolution {
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
                phase: SocketedPhase::Executing {
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
                phase: SocketedPhase::Resolving { deadline, attempt },
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
            | Self::ExecutingDisplaced { .. }
            | Self::Ready { .. }
            | Self::Sending { .. }
            | Self::AwaitingAck { .. }
            | Self::AwaitingCompletion { .. }
            | Self::AwaitingReply { .. }
            | Self::Backoff { .. } => None,
        }
    }
}
