//! Literal occupancy expectations for the legal phase/cancel pairs.
use super::*;

const OFF: u16 = 1 << 0;
const READY: u16 = 1 << 1;
const SENDING: u16 = 1 << 2;
const ACK: u16 = 1 << 3;
const REPLY: u16 = 1 << 4;
const COMPLETION: u16 = 1 << 5;
const INQUIRY: u16 = 1 << 6;
const COMMAND: u16 = 1 << 7;
const UNACKNOWLEDGED: u16 = 1 << 8;
const UNCORRELATED_WRITE: u16 = 1 << 9;
const UNCORRELATED_COMMAND: u16 = 1 << 10;

fn mask(lifecycle: Lifecycle) -> u16 {
    [
        lifecycle.is_off_wire(),
        lifecycle.is_ready(),
        lifecycle.is_sending(),
        lifecycle.is_awaiting_ack(),
        lifecycle.is_awaiting_reply(),
        lifecycle.is_awaiting_completion(),
        lifecycle.is_live_inquiry(),
        lifecycle.is_command_on_wire(),
        lifecycle.is_unacknowledged_command(),
        lifecycle.is_uncorrelated_write(),
        lifecycle.is_uncorrelated_command(),
    ]
    .into_iter()
    .enumerate()
    .fold(0, |mask, (bit, set)| mask | (u16::from(set) << bit))
}

#[test]
fn predicates_cover_phase_cancellation_pairs_and_sending_families() {
    let now = Instant::now();
    let deadline = now + Duration::from_millis(5);
    let ambiguity = now + Duration::from_millis(13);
    let observation = now + Duration::from_millis(7);
    let transmission = TransmissionId::from_nonzero(NonZeroU64::new(1).unwrap());
    let attempt = AttemptFacts {
        socket_capacity: Some(CommandSocketCount::TWO),
        stream_order: Some(1),
    };
    let requested = RequestedCancellation {
        ambiguity_deadline: ambiguity,
        observer_open: true,
    };
    let sending = SendingCancellation {
        transmission,
        framing: RequestFraming::Raw,
        ambiguity_deadline: ambiguity,
        observer_open: true,
    };
    let command_sending = Lifecycle::Sending {
        transmission,
        framing: RequestFraming::Raw,
        started_at: now,
        attempt,
        reply: SendingReply::AckThenCompletion,
    };
    let executing = |cancellation| Lifecycle::Executing {
        socket: ViscaSocket::S1,
        started_at: now,
        deadline,
        attempt,
        cancellation,
    };
    let resolving = |cancellation| Lifecycle::AwaitingCancellationResolution {
        socket: ViscaSocket::S1,
        deadline,
        attempt,
        cancellation,
    };
    // Expected bit sets and windows are written from the architecture table,
    // rather than obtained through Phase or another lifecycle helper.
    let rows = [
        (
            "ready",
            Lifecycle::Ready {
                previous_attempt: None,
            },
            OFF | READY,
            None,
        ),
        (
            "sending command",
            command_sending,
            SENDING | COMMAND | UNACKNOWLEDGED,
            None,
        ),
        (
            "ACK",
            Lifecycle::AwaitingAck {
                sent_at: now,
                deadline,
                attempt,
                cancellation: None,
            },
            ACK | COMMAND | UNACKNOWLEDGED,
            Some(deadline),
        ),
        (
            "ACK/cancel requested",
            Lifecycle::AwaitingAck {
                sent_at: now,
                deadline,
                attempt,
                cancellation: Some(requested),
            },
            ACK | COMMAND | UNACKNOWLEDGED,
            Some(ambiguity),
        ),
        (
            "completion only",
            Lifecycle::AwaitingCompletion {
                sent_at: now,
                deadline,
                attempt,
            },
            COMPLETION | COMMAND | UNACKNOWLEDGED | UNCORRELATED_COMMAND,
            Some(deadline),
        ),
        (
            "executing",
            executing(ExecutingCancellation::None),
            COMMAND,
            Some(deadline),
        ),
        (
            "executing displaced",
            Lifecycle::ExecutingDisplaced {
                started_at: now,
                deadline,
                attempt,
            },
            0,
            Some(deadline),
        ),
        (
            "executing/cancel requested",
            executing(ExecutingCancellation::Requested(requested)),
            COMMAND,
            Some(deadline),
        ),
        (
            "executing/cancel sending",
            executing(ExecutingCancellation::Sending(sending)),
            COMMAND,
            Some(ambiguity),
        ),
        (
            "executing/observation failed",
            executing(ExecutingCancellation::ObservationFailed {
                ambiguity_deadline: ambiguity,
            }),
            COMMAND,
            Some(ambiguity),
        ),
        (
            "inquiry reply",
            Lifecycle::AwaitingReply {
                sent_at: now,
                deadline,
                attempt: AttemptFacts {
                    socket_capacity: None,
                    stream_order: Some(1),
                },
            },
            REPLY | INQUIRY,
            Some(deadline),
        ),
        (
            "retry backoff",
            Lifecycle::Backoff {
                ready_at: now,
                previous_attempt: attempt,
            },
            OFF,
            None,
        ),
        (
            "resolving/requested",
            resolving(ResolvingCancellation::Requested(requested)),
            COMMAND,
            Some(ambiguity),
        ),
        (
            "resolving/sending",
            resolving(ResolvingCancellation::Sending(sending)),
            COMMAND,
            Some(ambiguity),
        ),
        (
            "resolving/awaiting terminal",
            resolving(ResolvingCancellation::AwaitingTerminal {
                ambiguity_deadline: ambiguity,
                observation_deadline: observation,
                observer_open: true,
            }),
            COMMAND,
            Some(ambiguity),
        ),
        (
            "resolving/observation failed",
            resolving(ResolvingCancellation::ObservationFailed {
                ambiguity_deadline: ambiguity,
            }),
            COMMAND,
            Some(ambiguity),
        ),
    ];
    assert_eq!(rows.len(), 16);
    for (name, lifecycle, expected, window) in rows {
        assert_eq!(mask(lifecycle), expected, "{name}: {lifecycle:?}");
        assert_eq!(lifecycle.response_window_end(), window, "{name}");
        let projected_window = match lifecycle.phase() {
            Phase::Ready | Phase::Sending { .. } | Phase::Backoff { .. } => None,
            Phase::AwaitingAck { deadline, .. }
            | Phase::AwaitingCompletion { deadline, .. }
            | Phase::Executing { deadline, .. }
            | Phase::ExecutingDisplaced { deadline, .. }
            | Phase::AwaitingReply { deadline, .. }
            | Phase::AwaitingCancellationResolution { deadline, .. } => Some(deadline),
        };
        assert_eq!(projected_window, window, "{name}: diagnostic projection");
        let expected_release = match name {
            "ACK/cancel requested" => Some(RawCancellationRelease {
                at: ambiguity,
                scope: ReleaseScope::PreAckUnkeyed,
            }),
            "executing/cancel requested"
            | "executing/cancel sending"
            | "executing/observation failed"
            | "resolving/requested"
            | "resolving/sending"
            | "resolving/awaiting terminal"
            | "resolving/observation failed" => Some(RawCancellationRelease {
                at: ambiguity,
                scope: ReleaseScope::ExactSocket(ViscaSocket::S1),
            }),
            _ => None,
        };
        assert_eq!(
            lifecycle.raw_cancellation_release(),
            expected_release,
            "{name}: release projection"
        );
        for budget in [None, Some(now), Some(ambiguity + Duration::from_millis(1))] {
            let expected = if lifecycle.is_off_wire() {
                None
            } else {
                [window, budget].into_iter().flatten().min()
            };
            assert_eq!(
                lifecycle.correlated_response_deadline(budget),
                expected,
                "{name}: correlation window"
            );
        }
    }
    // Retried Ready is still off-wire, independent of its retained facts.
    assert_eq!(
        mask(Lifecycle::Ready {
            previous_attempt: Some(attempt)
        }),
        OFF | READY
    );
    let sooner = now + Duration::from_millis(1);
    let early_request = RequestedCancellation {
        ambiguity_deadline: sooner,
        observer_open: true,
    };
    let early_send = SendingCancellation {
        ambiguity_deadline: sooner,
        ..sending
    };
    for lifecycle in [
        Lifecycle::AwaitingAck {
            sent_at: now,
            deadline,
            attempt,
            cancellation: Some(early_request),
        },
        executing(ExecutingCancellation::Sending(early_send)),
        executing(ExecutingCancellation::ObservationFailed {
            ambiguity_deadline: sooner,
        }),
        resolving(ResolvingCancellation::Requested(early_request)),
        resolving(ResolvingCancellation::Sending(early_send)),
        resolving(ResolvingCancellation::AwaitingTerminal {
            ambiguity_deadline: sooner,
            observation_deadline: ambiguity,
            observer_open: true,
        }),
        resolving(ResolvingCancellation::ObservationFailed {
            ambiguity_deadline: sooner,
        }),
    ] {
        assert_eq!(lifecycle.response_window_end(), Some(deadline), "an earlier cancel window or later observer deadline cannot replace the response window");
    }
    for (reply, expected) in [
        (SendingReply::Inquiry, SENDING | INQUIRY),
        (
            SendingReply::CompletionOnly,
            SENDING | COMMAND | UNACKNOWLEDGED | UNCORRELATED_WRITE | UNCORRELATED_COMMAND,
        ),
        (
            SendingReply::NoReply,
            SENDING | COMMAND | UNACKNOWLEDGED | UNCORRELATED_WRITE | UNCORRELATED_COMMAND,
        ),
    ] {
        let lifecycle = Lifecycle::Sending {
            transmission,
            framing: RequestFraming::Raw,
            started_at: now,
            attempt,
            reply,
        };
        assert_eq!(mask(lifecycle), expected, "{reply:?}");
        assert_eq!(lifecycle.response_window_end(), None);
    }
}
