//! Independent architecture-table oracle for the closed request lifecycle.
//!
//! Every fixture reaches its state through real engine inputs. The expected
//! outcomes below are literal transition-table rows, not predictions obtained
//! from the lifecycle implementation or its deadline helpers.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Ready,
    Sending,
    AwaitingAck,
    AwaitingCompletion,
    Executing,
    AwaitingReply,
    Backoff,
    AwaitingCancellationResolution,
}

impl State {
    const ALL: [Self; 8] = [
        Self::Ready,
        Self::Sending,
        Self::AwaitingAck,
        Self::AwaitingCompletion,
        Self::Executing,
        Self::AwaitingReply,
        Self::Backoff,
        Self::AwaitingCancellationResolution,
    ];

    fn of(phase: Phase) -> Self {
        match phase {
            Phase::Ready => Self::Ready,
            Phase::Sending { .. } => Self::Sending,
            Phase::AwaitingAck { .. } => Self::AwaitingAck,
            Phase::AwaitingCompletion { .. } => Self::AwaitingCompletion,
            Phase::Executing { .. } | Phase::ExecutingDisplaced { .. } => Self::Executing,
            Phase::AwaitingReply { .. } => Self::AwaitingReply,
            Phase::Backoff { .. } => Self::Backoff,
            Phase::AwaitingCancellationResolution { .. } => Self::AwaitingCancellationResolution,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Dispatch,
    WriteOk,
    WriteFailed,
    Ack,
    Completion,
    CameraError,
    Cancel,
    Due,
    ReceiveFault,
}

impl Action {
    const ALL: [Self; 9] = [
        Self::Dispatch,
        Self::WriteOk,
        Self::WriteFailed,
        Self::Ack,
        Self::Completion,
        Self::CameraError,
        Self::Cancel,
        Self::Due,
        Self::ReceiveFault,
    ];
}

#[derive(Debug, Clone, Copy)]
enum Expected {
    Live(State),
    Applied,
    Cancelled,
    WriteFailure,
    SyntaxFailure,
    AfterAckFailure,
    Unconfirmed,
    UnwrittenBudgetTimeout,
    InquiryTimeout,
}

use Expected::{
    AfterAckFailure, Applied, Cancelled, InquiryTimeout, Live, SyntaxFailure, Unconfirmed,
    UnwrittenBudgetTimeout, WriteFailure,
};
use State::{
    AwaitingAck, AwaitingCancellationResolution, AwaitingCompletion, AwaitingReply, Backoff,
    Executing, Ready, Sending,
};

/// Columns follow `Action::ALL`; rows follow `State::ALL`.
///
/// The matrix uses Raw datagrams with one isolated request, no pacing, and
/// no retry allowance except the conclusive rejection that constructs Backoff.
/// This pins phase semantics without silently relying on another facade's
/// scheduler or sequence resolver. Existing engine models cover other shapes.
const EXPECTED: [[Expected; 9]; 8] = [
    [
        Live(Sending),
        Live(Ready),
        Live(Ready),
        Live(Ready),
        Live(Ready),
        Live(Ready),
        Cancelled,
        UnwrittenBudgetTimeout,
        Live(Ready),
    ],
    [
        Live(Sending),
        Live(AwaitingAck),
        WriteFailure,
        Live(Sending),
        Live(Sending),
        Live(Sending),
        Live(Sending),
        Unconfirmed,
        Live(Sending),
    ],
    [
        Live(AwaitingAck),
        Live(AwaitingAck),
        Live(AwaitingAck),
        Live(Executing),
        Live(AwaitingAck),
        SyntaxFailure,
        Live(AwaitingAck),
        Unconfirmed,
        Live(AwaitingAck),
    ],
    [
        Live(AwaitingCompletion),
        Live(AwaitingCompletion),
        Live(AwaitingCompletion),
        Live(AwaitingCompletion),
        Applied,
        SyntaxFailure,
        Live(AwaitingCompletion),
        Unconfirmed,
        Live(AwaitingCompletion),
    ],
    [
        Live(Executing),
        Live(Executing),
        Live(Executing),
        Live(Executing),
        Applied,
        AfterAckFailure,
        Live(Executing),
        Unconfirmed,
        Live(Executing),
    ],
    [
        Live(AwaitingReply),
        Live(AwaitingReply),
        Live(AwaitingReply),
        Live(AwaitingReply),
        Live(AwaitingReply),
        SyntaxFailure,
        Live(AwaitingReply),
        InquiryTimeout,
        Live(AwaitingReply),
    ],
    [
        Live(Backoff),
        Live(Backoff),
        Live(Backoff),
        Live(Backoff),
        Live(Backoff),
        Live(Backoff),
        Cancelled,
        Live(Sending),
        Live(Backoff),
    ],
    [
        Live(AwaitingCancellationResolution),
        Live(AwaitingCancellationResolution),
        Live(AwaitingCancellationResolution),
        Live(AwaitingCancellationResolution),
        Applied,
        AfterAckFailure,
        Live(AwaitingCancellationResolution),
        Unconfirmed,
        Live(AwaitingCancellationResolution),
    ],
];

/// The architecture's admissible flattened pairs, independently of
/// which combinations the stored lifecycle type can represent.
fn pair_is_admissible(phase: Phase, cancellation: CancelState) -> bool {
    match phase {
        Phase::Ready
        | Phase::Sending { .. }
        | Phase::Backoff { .. }
        | Phase::AwaitingReply { .. }
        | Phase::AwaitingCompletion { .. }
        | Phase::ExecutingDisplaced { .. } => matches!(cancellation, CancelState::None),
        Phase::AwaitingAck { .. } => {
            matches!(
                cancellation,
                CancelState::None | CancelState::Requested { .. }
            )
        }
        Phase::Executing { .. } => matches!(
            cancellation,
            CancelState::None
                | CancelState::Requested { .. }
                | CancelState::Sending { .. }
                | CancelState::ObservationFailed { .. }
        ),
        Phase::AwaitingCancellationResolution { .. } => matches!(
            cancellation,
            CancelState::Requested { .. }
                | CancelState::Sending { .. }
                | CancelState::AwaitingTerminal { .. }
                | CancelState::ObservationFailed { .. }
        ),
    }
}

/// Shared with the generated-input property: audit the independent table
/// after every generated step, including every retained retry/cancel state.
pub(super) fn all_entries_have_admissible_pairs(engine: &ProtocolEngine) -> bool {
    engine
        .entries
        .values()
        .all(|entry| pair_is_admissible(entry.phase(), entry.cancellation()))
}

fn checked_input(engine: &mut ProtocolEngine, input: Input, now: Instant) -> Vec<Effect> {
    let effects = engine.handle_turn(input, now, EngineTurn::InputOnly);
    assert!(all_entries_have_admissible_pairs(engine));
    engine.assert_invariants().unwrap();
    effects
}

struct Fixture {
    engine: ProtocolEngine,
    id: RequestId,
    transmission: TransmissionId,
    now: Instant,
}

impl Fixture {
    fn new(state: State) -> Self {
        let now = Instant::now();
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let mut request = match state {
            AwaitingReply => inquiry(1),
            AwaitingCompletion => command_with_reply_shape(
                1,
                CancellationPolicy::Supported,
                ReplyShape::CompletionOnly,
            ),
            _ => command(1, CancellationPolicy::Supported),
        };
        if state != Backoff {
            request.context_mut().retry.max_retries = 0;
        }
        let admitted_effects = checked_input(
            &mut engine,
            Input::Admit {
                ticket: AdmissionTicket(1),
                request,
                slot: AdmissionSlot::Ordinary,
            },
            now,
        );
        let id = admitted(&admitted_effects);
        let mut transmission = TransmissionId::from_nonzero(NonZeroU64::new(999_999).unwrap());
        if state != Ready {
            let dispatch = engine.advance(now);
            transmission = request_transmit(&dispatch).0;
            assert!(all_entries_have_admissible_pairs(&engine));
            if state != Sending {
                checked_input(
                    &mut engine,
                    Input::TransmissionFinished {
                        request: id,
                        transmission,
                        result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
                    },
                    now,
                );
            }
        }
        if matches!(state, Executing | AwaitingCancellationResolution) {
            checked_input(
                &mut engine,
                frame(
                    1,
                    None,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            );
        }
        if state == Backoff {
            checked_input(
                &mut engine,
                frame(
                    1,
                    None,
                    DecodedResponse::Error {
                        socket: None,
                        code: 0x03,
                    },
                ),
                now,
            );
        }
        if state == AwaitingCancellationResolution {
            let cancel = checked_input(&mut engine, Input::Cancel { id }, now);
            transmission = cancel_transmit(&cancel).0;
            checked_input(
                &mut engine,
                Input::TransmissionFinished {
                    request: id,
                    transmission,
                    result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
                },
                now,
            );
        }
        assert_eq!(State::of(engine.entry(id).unwrap().phase()), state);
        Self {
            engine,
            id,
            transmission,
            now,
        }
    }

    fn apply(&mut self, action: Action) -> Vec<Effect> {
        let input = match action {
            Action::Dispatch => return self.engine.advance(self.now),
            Action::WriteOk => Input::TransmissionFinished {
                request: self.id,
                transmission: self.transmission,
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
            },
            Action::WriteFailed => Input::TransmissionFinished {
                request: self.id,
                transmission: self.transmission,
                result: WriteOutcome::Failed(Error::transport_error(std::io::Error::other(
                    "matrix write failed",
                ))),
            },
            Action::Ack => frame(
                1,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            Action::Completion => frame(1, None, DecodedResponse::Completion { socket: None }),
            Action::CameraError => {
                let socket = matches!(
                    self.engine.entry(self.id).unwrap().phase(),
                    Phase::Executing { .. } | Phase::AwaitingCancellationResolution { .. }
                )
                .then_some(ViscaSocket::S1);
                frame(1, None, DecodedResponse::Error { socket, code: 0x02 })
            }
            Action::Cancel => Input::Cancel { id: self.id },
            Action::Due => {
                let entry = self.engine.entry(self.id).unwrap();
                let due = match entry.phase() {
                    Phase::Ready | Phase::Sending { .. } => {
                        entry.submitted_at
                            + entry.request.context().retry.total_budget.unwrap().get()
                    }
                    Phase::Backoff { ready_at, .. } => ready_at,
                    Phase::AwaitingAck { deadline, .. }
                    | Phase::AwaitingCompletion { deadline, .. }
                    | Phase::ExecutingDisplaced { deadline, .. }
                    | Phase::Executing { deadline, .. }
                    | Phase::AwaitingReply { deadline, .. }
                    | Phase::AwaitingCancellationResolution { deadline, .. } => deadline,
                };
                return self.engine.advance(due);
            }
            Action::ReceiveFault => Input::ReceiveFault {
                error: Error::transport_error(std::io::Error::other(
                    "matrix transient receive fault",
                )),
            },
        };
        checked_input(&mut self.engine, input, self.now)
    }
}

fn assert_expected(fixture: &Fixture, effects: &[Effect], expected: Expected) {
    assert_eq!(fixture.engine.state(), SessionState::Running);
    if let Live(state) = expected {
        assert_eq!(
            fixture
                .engine
                .entry(fixture.id)
                .map(|entry| State::of(entry.phase())),
            Some(state)
        );
        assert!(terminal_outcome(effects, fixture.id).is_none());
        return;
    }
    assert!(fixture.engine.entry(fixture.id).is_none());
    let outcome = terminal_outcome(effects, fixture.id).expect("one terminal outcome");
    let protocol_outcome = match &outcome {
        RuntimeOutcome::Inquiry(InquiryOutcome::Failed(Error::InquiryFailed { source })) => {
            RuntimeOutcome::Command(CommandOutcome::Failed(source.as_ref().clone()))
        }
        outcome => outcome.clone(),
    };
    let correct = match expected {
        Applied => matches!(
            &protocol_outcome,
            RuntimeOutcome::Command(CommandOutcome::Applied)
        ),
        Cancelled => matches!(
            &protocol_outcome,
            RuntimeOutcome::Command(CommandOutcome::Cancelled)
        ),
        WriteFailure => {
            matches!(&protocol_outcome, RuntimeOutcome::Command(CommandOutcome::Failed(Error::TransportError { source: message, .. })) if message.to_string().ends_with("matrix write failed"))
        }
        SyntaxFailure => matches!(
            &protocol_outcome,
            RuntimeOutcome::Command(CommandOutcome::Failed(Error::SyntaxError))
        ),
        AfterAckFailure => {
            matches!(&protocol_outcome, RuntimeOutcome::Command(CommandOutcome::Failed(Error::CommandFailedAfterAck { source, .. })) if matches!(source.as_ref(), Error::SyntaxError))
        }
        Unconfirmed => matches!(
            protocol_outcome,
            RuntimeOutcome::Command(CommandOutcome::Failed(Error::UnsequencedCommandUnconfirmed))
        ),
        UnwrittenBudgetTimeout | InquiryTimeout => {
            matches!(
                &protocol_outcome,
                RuntimeOutcome::Command(CommandOutcome::Failed(Error::Timeout { .. }))
            )
        }
        Live(_) => unreachable!("live outcome was checked above"),
    };
    assert!(correct, "expected {expected:?}, got {outcome:?}");
    if let UnwrittenBudgetTimeout | InquiryTimeout = expected {
        let error = outcome
            .failure()
            .expect("the terminal lane retains its failure");
        let certainty = if matches!(expected, UnwrittenBudgetTimeout) {
            Certainty::NotAccepted
        } else {
            Certainty::FailedConclusively
        };
        assert_eq!(
            error.failure_context(),
            Some(FailureContext::new(FailureStage::Terminal, certainty))
        );
    }
    assert_eq!(
        effects
            .iter()
            .filter(|effect| matches!(effect, Effect::Terminal { id, .. } if *id == fixture.id))
            .count(),
        1
    );
}

#[test]
fn every_lifecycle_state_matches_the_architecture_input_table() {
    let mut cells = 0;
    for (row, state) in State::ALL.into_iter().enumerate() {
        for (column, action) in Action::ALL.into_iter().enumerate() {
            if state == Sending && !matches!(action, Action::WriteOk | Action::WriteFailed) {
                continue; // A write result precedes every unrelated boundary input.
            }
            let mut fixture = Fixture::new(state);
            let effects = fixture.apply(action);
            assert_expected(&fixture, &effects, EXPECTED[row][column]);
            assert!(
                all_entries_have_admissible_pairs(&fixture.engine),
                "{state:?}/{action:?}"
            );
            fixture.engine.assert_invariants().unwrap();

            if action == Action::Cancel {
                match state {
                    Sending => unreachable!("Sending accepts only its write result"),
                    AwaitingAck => assert!(matches!(
                        fixture.engine.entry(fixture.id).unwrap().cancellation(),
                        CancelState::Requested { .. }
                    )),
                    Executing => {
                        assert!(matches!(
                            fixture.engine.entry(fixture.id).unwrap().cancellation(),
                            CancelState::Sending { .. }
                        ));
                        assert_eq!(
                            effects
                                .iter()
                                .filter(|effect| matches!(
                                    effect,
                                    Effect::Transmit {
                                        kind: Transmission::Cancel { .. },
                                        ..
                                    }
                                ))
                                .count(),
                            1
                        );
                    }
                    AwaitingCompletion | AwaitingReply => {
                        assert!(effects.iter().any(|effect| matches!(
                            effect,
                            Effect::CancellationObservation {
                                id,
                                observation: CancellationObservation::Failed(error),
                            } if *id == fixture.id && matches!(
                                (state, error),
                                (AwaitingReply, Error::InquiryNotCancellable)
                                    | (AwaitingCompletion, Error::CancellationNotSupported)
                            )
                        )));
                        assert_eq!(
                            fixture.engine.entry(fixture.id).unwrap().cancellation(),
                            CancelState::None
                        );
                        assert!(cancel_transmit_optional(&effects).is_none());
                    }
                    AwaitingCancellationResolution => {
                        assert!(effects.iter().any(|effect| matches!(
                            effect,
                            Effect::Ignored(IgnoreReason::DuplicateCancellation)
                        )));
                        assert!(cancel_transmit_optional(&effects).is_none());
                    }
                    Ready | Backoff => assert!(cancel_transmit_optional(&effects).is_none()),
                }
            }
            cells += 1;
        }
    }
    assert_eq!(cells, 65);
}

#[test]
fn flattened_pair_oracle_has_exactly_the_sixteen_documented_pairs() {
    let now = Instant::now();
    let transmission = TransmissionId::from_nonzero(NonZeroU64::new(999_999).unwrap());
    let cancellations = [
        CancelState::None,
        CancelState::Requested {
            ambiguity_deadline: now,
        },
        CancelState::Sending {
            transmission,
            socket: ViscaSocket::S1,
            ambiguity_deadline: now,
        },
        CancelState::AwaitingTerminal {
            ambiguity_deadline: now,
            observation_deadline: now,
        },
        CancelState::ObservationFailed {
            socket: ViscaSocket::S1,
            ambiguity_deadline: now,
        },
    ];
    let mut legal = 0;
    for state in State::ALL {
        let fixture = Fixture::new(state);
        let phase = fixture.engine.entry(fixture.id).unwrap().phase();
        for cancellation in cancellations {
            legal += usize::from(pair_is_admissible(phase, cancellation));
        }
    }
    let displaced = Phase::ExecutingDisplaced {
        started_at: now,
        deadline: now,
    };
    for cancellation in cancellations {
        legal += usize::from(pair_is_admissible(displaced, cancellation));
    }
    assert_eq!(legal, 16, "the documented pair set changed");
}

#[test]
fn sony_lost_ack_backoff_cancel_is_an_unconfirmed_terminal() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let mut request = command(1, CancellationPolicy::Supported);
    request.context_mut().retry = retrying();
    let (write, id) = admit(&mut engine, 1, request, now);
    send_ok(&mut engine, &write, Some(0x1001), now);
    engine.advance(now + Duration::from_millis(20));
    assert!(matches!(phase_of(&engine, id), Some(Phase::Backoff { .. })));
    let effects = engine.handle(Input::Cancel { id }, now + Duration::from_millis(20));
    assert!(matches!(
        terminal_failure(&effects, id),
        Some(Error::CancellationUnconfirmed)
    ));
    assert!(!effects
        .iter()
        .any(|effect| matches!(effect, Effect::Transmit { .. })));
    engine.assert_invariants().unwrap();
}
