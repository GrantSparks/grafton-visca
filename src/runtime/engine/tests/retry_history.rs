//! One retry history and one certainty rule serve every terminal boundary.
use super::*;
use crate::{KnownState, StateKey};

#[derive(Clone, Copy, Debug)]
enum History {
    NeverWritten,
    ConclusivelyRejected,
    NotExecutable,
    Unconfirmed,
    UnconfirmedThenRejected,
}

fn fixture(history: History) -> (ProtocolEngine, RequestId, Instant) {
    fixture_for(history, EnvelopeKind::Sony)
}

fn fixture_for(history: History, envelope: EnvelopeKind) -> (ProtocolEngine, RequestId, Instant) {
    fixture_with_retry(history, envelope, retrying())
}

fn fixture_with_retry(
    history: History,
    envelope: EnvelopeKind,
    retry: RetryPolicy,
) -> (ProtocolEngine, RequestId, Instant) {
    let now = Instant::now();
    let mut engine = engine(envelope, TransportKind::Datagram);
    let mut request = declared_motion(1, crate::AffectedAxes::PAN_TILT, 1);
    request.context_mut().retry = retry;
    request.context_mut().retry.total_budget =
        Some(NonZeroDuration::new(Duration::from_millis(200)).unwrap());
    request.context_mut().dispatch_deadline = Some(now + Duration::from_millis(100));
    let RuntimeRequest::Command { applied_state, .. } = &mut request else {
        unreachable!();
    };
    *applied_state = Some(AppliedStateProjection::set(KnownState::TallyBrightness(
        crate::types::TallyBrightness::High,
    )));
    let admitted_effects = engine.handle_turn(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request,
            slot: AdmissionSlot::Ordinary,
        },
        now,
        EngineTurn::InputOnly,
    );
    let id = admitted(&admitted_effects);
    if !matches!(history, History::NeverWritten) {
        let write = engine.advance(now);
        send_ok(&mut engine, &write, Some(0x1001), now);
        match history {
            History::ConclusivelyRejected | History::NotExecutable => {
                engine.handle(
                    frame(
                        1,
                        Some(0x1001),
                        DecodedResponse::Error {
                            socket: None,
                            code: if matches!(history, History::NotExecutable) {
                                0x41.into()
                            } else {
                                0x03.into()
                            },
                        },
                    ),
                    now,
                );
            }
            History::Unconfirmed | History::UnconfirmedThenRejected => {
                engine.advance(now + Duration::from_millis(20));
            }
            History::NeverWritten => unreachable!(),
        }
    }
    if matches!(history, History::UnconfirmedThenRejected) {
        let Phase::Backoff { ready_at: at, .. } = phase_of(&engine, id).unwrap() else {
            unreachable!();
        };
        let retry = dispatch(&mut engine, id, now);
        send_ok(&mut engine, &retry, Some(0x1001), at);
        engine.handle(
            frame(
                1,
                Some(0x1001),
                DecodedResponse::Error {
                    socket: None,
                    code: 0x03.into(),
                },
            ),
            at,
        );
    }
    engine.assert_invariants().unwrap();
    (engine, id, now)
}

fn expected(history: History) -> Certainty {
    match history {
        History::NeverWritten | History::ConclusivelyRejected | History::NotExecutable => {
            Certainty::NotAccepted
        }
        History::Unconfirmed | History::UnconfirmedThenRejected => Certainty::Unconfirmed,
    }
}

fn invalidates(effects: &[Effect], id: RequestId) -> bool {
    effects.iter().any(|effect| matches!(effect,
        Effect::AppliedState { effect: AppliedStateEffect { request, projection: AppliedStateProjection::Invalidate { key: StateKey::TallyBrightness }, .. } } if *request == id))
}

fn dispatch(engine: &mut ProtocolEngine, id: RequestId, now: Instant) -> Vec<Effect> {
    let at = match phase_of(engine, id).unwrap() {
        Phase::Backoff { ready_at, .. } => ready_at,
        Phase::Ready => now,
        other => panic!("unexpected history phase {other:?}"),
    };
    engine.advance(at)
}

#[test]
fn terminal_paths_and_cache_agree_for_every_reachable_prior_history() {
    for history in [
        History::NeverWritten,
        History::ConclusivelyRejected,
        History::NotExecutable,
        History::Unconfirmed,
        History::UnconfirmedThenRejected,
    ] {
        for boundary in 0..6 {
            let (mut engine, id, now) = fixture(history);
            let certainty = expected(history);
            let entry = engine.entry(id).unwrap();
            assert_eq!(
                entry.certainty(AttemptEvidence::Unwritten),
                certainty,
                "{history:?}"
            );
            assert_eq!(
                entry.certainty(AttemptEvidence::Lifecycle),
                certainty,
                "{history:?}"
            );
            assert_eq!(
                entry.may_have_effect(AttemptEvidence::Unwritten),
                certainty != Certainty::NotAccepted
            );
            let due_certainty = match entry.due(id).unwrap().action {
                DueAction::RetryBudget { certainty, .. }
                | DueAction::RetryReady { certainty, .. } => certainty,
                other => panic!("unexpected queued deadline {other:?}"),
            };
            assert_eq!(
                due_certainty, certainty,
                "budget and retry-ready use the same certainty"
            );
            let effects = match boundary {
                0 => {
                    let turn = engine.begin_input_turn(now + Duration::from_millis(50));
                    engine.halt_in_turn(&turn, camera(1), crate::AffectedAxes::PAN_TILT, 2)
                }
                1 => {
                    let write = dispatch(&mut engine, id, now);
                    let (transmission, request, _) = request_transmit(&write);
                    engine.handle(
                        Input::TransmissionFinished {
                            request,
                            transmission,
                            result: WriteOutcome::NotSent(Error::InvalidRequest {
                                reason: "refused before write".into(),
                            }),
                        },
                        now + Duration::from_millis(50),
                    )
                }
                2 => {
                    let write = dispatch(&mut engine, id, now);
                    let (transmission, request, _) = request_transmit(&write);
                    engine
                        .expire_unwritten_halt(
                            request,
                            transmission,
                            now + Duration::from_millis(100),
                        )
                        .unwrap()
                }
                3 => engine.advance(now + Duration::from_millis(200)),
                4 => engine.handle(engine.cancel_input(id), now + Duration::from_millis(50)),
                _ => {
                    let mut at = now;
                    loop {
                        if let Phase::Backoff { ready_at, .. } = phase_of(&engine, id).unwrap() {
                            at = ready_at;
                        }
                        let write = dispatch(&mut engine, id, at);
                        send_ok(&mut engine, &write, Some(0x1001), at);
                        let effects = engine.handle(
                            frame(
                                1,
                                Some(0x1001),
                                DecodedResponse::Error {
                                    socket: None,
                                    code: 0x03.into(),
                                },
                            ),
                            at,
                        );
                        if terminal_failure(&effects, id).is_some() {
                            break effects;
                        }
                    }
                }
            };
            if boundary == 4 && certainty == Certainty::NotAccepted {
                assert!(matches!(
                    terminal_outcome(&effects, id),
                    Some(RuntimeOutcome::Command(CommandOutcome::Cancelled))
                ));
                assert!(effects.iter().any(|effect| matches!(
                    effect,
                    Effect::CancellationObservation {
                        observation: CancellationObservation::Concluded(
                            CancellationConclusion::Cancelled
                        ),
                        ..
                    }
                )));
            } else {
                let error = terminal_failure(&effects, id).unwrap();
                assert_eq!(
                    error
                        .failure_context()
                        .map_or(Certainty::NotAccepted, |context| context.certainty),
                    certainty,
                    "{history:?}, boundary {boundary}"
                );
                if certainty == Certainty::Unconfirmed {
                    assert_eq!(error.recovery(), crate::Recovery::Reconcile);
                    if boundary == 4 {
                        assert!(effects.iter().any(|effect| matches!(
                            effect,
                            Effect::CancellationObservation {
                                observation: CancellationObservation::Concluded(
                                    CancellationConclusion::Failed(crate::ErrorKind::Unconfirmed)
                                ),
                                ..
                            }
                        )));
                    }
                }
            }
            assert_eq!(
                invalidates(&effects, id),
                certainty == Certainty::Unconfirmed,
                "{history:?}, boundary {boundary}"
            );
            engine.assert_invariants().unwrap();
        }
    }
}

#[test]
fn physical_write_evidence_invalidates_without_mutating_retry_history() {
    for late_sent in [false, true] {
        let (mut engine, id, now) = fixture(History::NeverWritten);
        let write = dispatch(&mut engine, id, now);
        assert!(engine.entry(id).unwrap().prior.is_none());
        let (transmission, request, _) = request_transmit(&write);
        let result = if late_sent {
            WriteOutcome::Sent(TransmissionMeta::Allocated { sequence: 0x1001 })
        } else {
            // A transport may return an arbitrary typed error. Its physical
            // failure must not be mistaken for a pre-ACK camera rejection.
            WriteOutcome::Failed(Error::CommandBufferFull)
        };
        let effects = engine.handle(
            Input::TransmissionFinished {
                request,
                transmission,
                result,
            },
            if late_sent {
                now + Duration::from_millis(201)
            } else {
                now
            },
        );
        assert!(invalidates(&effects, id));
        if late_sent {
            assert_eq!(
                terminal_failure(&effects, id)
                    .unwrap()
                    .failure_context()
                    .unwrap()
                    .certainty,
                Certainty::Unconfirmed
            );
        } else {
            assert!(matches!(terminal_failure(&effects, id),
                Some(Error::RequestUnconfirmed { source, .. }) if matches!(source.as_ref(), Error::CommandBufferFull)));
            assert_eq!(
                terminal_failure(&effects, id).unwrap().recovery(),
                crate::Recovery::Reconcile
            );
        }
        engine.assert_invariants().unwrap();
    }
}

#[test]
fn queued_previous_attempt_must_agree_with_retry_history() {
    let (mut engine, id, _) = fixture(History::ConclusivelyRejected);
    engine
        .entries
        .transition(
            id,
            EntryChange::Lifecycle(UnsocketedLifecycle::Ready {
                previous_attempt: None,
            }),
        )
        .unwrap();
    assert!(engine
        .assert_invariants()
        .unwrap_err()
        .contains("retry history"));
}

#[test]
fn an_acknowledged_attempt_cannot_be_made_conclusive_by_later_refusal_evidence() {
    let (mut engine, id, now) = fixture(History::NeverWritten);
    let write = dispatch(&mut engine, id, now);
    send_ok(&mut engine, &write, Some(0x1001), now);
    engine.handle(
        frame(
            1,
            Some(0x1001),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    );
    let entry = engine.entry(id).unwrap();
    for evidence in [
        AttemptEvidence::Lifecycle,
        AttemptEvidence::Written,
        AttemptEvidence::Unwritten,
        AttemptEvidence::Rejected,
    ] {
        assert_eq!(entry.certainty(evidence), Certainty::Unconfirmed);
    }
}

#[test]
fn halt_racing_a_conclusive_rejection_keeps_terminal_and_cache_certainty_equal() {
    for envelope in [EnvelopeKind::Sony, EnvelopeKind::Raw] {
        let (mut engine, id, now) = fixture_for(History::NeverWritten, envelope);
        let write = dispatch(&mut engine, id, now);
        let sequence = (envelope == EnvelopeKind::Sony).then_some(0x1001);
        send_ok(&mut engine, &write, sequence, now);
        let turn = engine.begin_input_turn(now);
        assert!(engine
            .halt_in_turn(&turn, camera(1), crate::AffectedAxes::PAN_TILT, 2)
            .is_empty());
        let effects = engine.handle(
            frame(
                1,
                sequence,
                DecodedResponse::Error {
                    socket: None,
                    code: 0x03.into(),
                },
            ),
            now,
        );
        assert!(matches!(
            terminal_failure(&effects, id),
            Some(Error::MotionSuperseded { .. })
        ));
        assert_eq!(
            terminal_failure(&effects, id)
                .unwrap()
                .failure_context()
                .unwrap()
                .certainty,
            Certainty::NotAccepted
        );
        assert!(
            !invalidates(&effects, id),
            "a conclusively rejected attempt cannot have changed the camera"
        );
        engine.assert_invariants().unwrap();
    }
}

#[test]
fn transport_fault_variants_cannot_claim_a_camera_rejection() {
    for error in [
        Error::SyntaxError,
        Error::CommandBufferFull,
        Error::NoSocket,
        Error::CommandNotExecutable,
    ] {
        let (mut engine, id, now) = fixture(History::NeverWritten);
        let write = dispatch(&mut engine, id, now);
        send_ok(&mut engine, &write, Some(0x1001), now);
        let effects = engine.handle(Input::ReceiveFault { error }, now);
        assert!(retry_scheduled(&effects).is_some());
        let entry = engine.entry(id).unwrap();
        assert_eq!(
            entry.certainty(AttemptEvidence::Unwritten),
            Certainty::Unconfirmed
        );
        assert!(entry.prior.as_ref().unwrap().possibly_applied);
        let turn = engine.begin_input_turn(now);
        let superseded = engine.halt_in_turn(&turn, camera(1), crate::AffectedAxes::PAN_TILT, 2);
        assert_eq!(
            terminal_failure(&superseded, id)
                .unwrap()
                .failure_context()
                .unwrap()
                .certainty,
            Certainty::Unconfirmed
        );
        assert!(invalidates(&superseded, id));
        engine.assert_invariants().unwrap();
    }
}

#[test]
fn a_later_camera_rejection_cannot_erase_an_earlier_possible_effect() {
    for code in [0x03, 0x41] {
        let (mut engine, id, now) = fixture(History::Unconfirmed);
        let at = engine.next_wake(now).unwrap();
        let retry = dispatch(&mut engine, id, now);
        send_ok(&mut engine, &retry, Some(0x1001), at);
        engine.handle(
            frame(
                1,
                Some(0x1001),
                DecodedResponse::Error {
                    socket: None,
                    code: code.into(),
                },
            ),
            at,
        );
        let entry = engine.entry(id).unwrap();
        assert_eq!(entry.retry_count(), 2);
        assert!(entry.prior.as_ref().unwrap().possibly_applied);
        assert_eq!(
            entry.certainty(AttemptEvidence::Unwritten),
            Certainty::Unconfirmed
        );
        let turn = engine.begin_input_turn(at);
        let effects = engine.halt_in_turn(&turn, camera(1), crate::AffectedAxes::PAN_TILT, 2);
        assert_eq!(
            terminal_failure(&effects, id)
                .unwrap()
                .failure_context()
                .unwrap()
                .certainty,
            Certainty::Unconfirmed
        );
        assert!(invalidates(&effects, id));
        engine.assert_invariants().unwrap();
    }
}

#[test]
fn sequence_slots_keep_the_request_sequence_across_retries_and_cancellation() {
    const RETRIES: u32 = 10;
    let (mut engine, id, start) = fixture_with_retry(
        History::NeverWritten,
        EnvelopeKind::Sony,
        RetryPolicy {
            max_retries: RETRIES,
            initial_backoff: Duration::from_millis(1),
            maximum_backoff: Duration::from_millis(1),
            ..retrying()
        },
    );
    let sequence = 0x1234_1001;
    let first = dispatch(&mut engine, id, start);
    send_ok(&mut engine, &first, Some(sequence), start);
    let mut now = start;
    for _ in 0..RETRIES {
        let rejected = engine.handle(
            frame(
                1,
                Some(sequence),
                DecodedResponse::Error {
                    socket: None,
                    code: 0x03.into(),
                },
            ),
            now,
        );
        let (_, _, ready_at) = retry_scheduled(&rejected).unwrap();
        let retry = engine.advance(ready_at);
        send_ok(&mut engine, &retry, Some(sequence), ready_at);
        now = ready_at;
        let entry = engine.entry(id).unwrap();
        assert_eq!(entry.current_sequence(), Some(sequence));
        assert_eq!(entry.sequences.records().count(), 1);
    }
    engine.handle(
        frame(
            1,
            Some(sequence & 0xFFFF),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    );
    let cancellation = engine.handle(engine.cancel_input(id), now);
    let (transmission, request, _) = cancel_transmit(&cancellation);
    engine.handle(
        Input::TransmissionFinished {
            request,
            transmission,
            result: WriteOutcome::Sent(TransmissionMeta::Allocated {
                sequence: sequence + 1,
            }),
        },
        now,
    );
    for _ in 0..3 {
        let duplicate = engine.handle(engine.cancel_input(id), now);
        assert!(duplicate
            .iter()
            .all(|effect| !matches!(effect, Effect::Transmit { .. })));
        let entry = engine.entry(id).unwrap();
        assert_eq!(entry.current_sequence(), Some(sequence));
        assert_eq!(
            entry.sequences[CorrelationKind::Cancellation],
            Some(sequence + 1)
        );
        assert_eq!(entry.sequences.records().count(), 2);
        engine.assert_invariants().unwrap();
    }
}

#[test]
fn rejection_during_cancellation_cannot_erase_the_prior_lost_ack() {
    let (mut engine, id, now) = fixture(History::Unconfirmed);
    let Phase::Backoff { ready_at: at, .. } = phase_of(&engine, id).unwrap() else {
        unreachable!();
    };
    let write = dispatch(&mut engine, id, now);
    send_ok(&mut engine, &write, Some(0x1001), at);
    engine.handle(engine.cancel_input(id), at);
    let effects = engine.handle(
        frame(
            1,
            Some(0x1001),
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
            },
        ),
        at,
    );
    assert!(matches!(
        terminal_failure(&effects, id),
        Some(Error::CancellationUnconfirmed)
    ));
    assert!(invalidates(&effects, id));
    engine.assert_invariants().unwrap();
}
