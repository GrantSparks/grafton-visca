//! A write refusal proves only that this attempt sent no bytes.
use super::*;
use crate::{KnownState, Recovery, StateKey};

fn projected_command() -> RuntimeRequest {
    let mut request = command(1, CancellationPolicy::Supported);
    let RuntimeRequest::Command { applied_state, .. } = &mut request else {
        unreachable!();
    };
    *applied_state = Some(AppliedStateProjection::set(KnownState::TallyBrightness(
        true,
    )));
    request
}

fn result(
    engine: &mut ProtocolEngine,
    effects: &[Effect],
    outcome: WriteOutcome,
    now: Instant,
) -> Vec<Effect> {
    let (transmission, request, _) = request_transmit(effects);
    engine.handle(
        Input::TransmissionFinished {
            request,
            transmission,
            result: outcome,
        },
        now,
    )
}

fn invalidations(effects: &[Effect]) -> Vec<usize> {
    effects
        .iter()
        .enumerate()
        .filter_map(|(index, effect)| match effect {
            Effect::AppliedState {
                effect:
                    AppliedStateEffect {
                        projection:
                            AppliedStateProjection::Invalidate {
                                key: StateKey::TallyBrightness,
                            },
                        ..
                    },
            } => Some(index),
            _ => None,
        })
        .collect()
}

fn assert_invalidated_before_terminal(effects: &[Effect], request: RequestId) {
    let terminal = effects
        .iter()
        .position(|effect| matches!(effect, Effect::Terminal { id, .. } if *id == request))
        .unwrap();
    let invalidations = invalidations(effects);
    assert_eq!(invalidations.len(), 1, "{effects:?}");
    assert!(
        invalidations[0] < terminal,
        "cache commit precedes terminal delivery"
    );
}

#[test]
fn refused_first_write_is_not_accepted_and_never_poisons_or_invalidates() {
    for envelope in [EnvelopeKind::Raw, EnvelopeKind::Sony] {
        for transport in [TransportKind::Datagram, TransportKind::Stream] {
            let now = Instant::now();
            let mut engine = engine(envelope, transport);
            let (write, request) = admit(&mut engine, 1, projected_command(), now);
            let effects = result(
                &mut engine,
                &write,
                WriteOutcome::NotSent(Error::InvalidRequest("pre-write refusal".into())),
                now,
            );
            let error = terminal_failure(&effects, request).unwrap();
            assert!(matches!(&error, Error::TransmissionNotSent { source, .. }
                if matches!(source.as_ref(), Error::InvalidRequest(_))));
            assert_eq!(
                error.failure_context(),
                Some(FailureContext::new(
                    FailureStage::Terminal,
                    Certainty::NotAccepted
                ))
            );
            assert_eq!(error.recovery(), Recovery::Abandon);
            assert_eq!(engine.state(), SessionState::Running);
            assert!(invalidations(&effects).is_empty());
            assert_eq!(
                engine.ledger.lane_state(camera(1), Lane::Command),
                LaneState::Clear
            );
            let (next, _) = admit(&mut engine, 2, projected_command(), now);
            assert!(
                request_transmit_optional(&next).is_some(),
                "refusal leaves no false hold"
            );
            engine.assert_invariants().unwrap();
        }
    }
}

#[test]
fn refused_retry_preserves_prior_effect_and_invalidates_before_observation() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (write, request) = admit(&mut engine, 1, projected_command(), now);
    send_ok(&mut engine, &write, Some(0x1001), now);
    let timeout = engine.advance(now + Duration::from_millis(20));
    let (_, _, ready) = retry_scheduled(&timeout).unwrap();
    let retry = engine.advance(ready);
    let effects = result(
        &mut engine,
        &retry,
        WriteOutcome::NotSent(Error::InvalidRequest("mode changed before retry".into())),
        ready,
    );
    let error = terminal_failure(&effects, request).unwrap();
    assert_eq!(
        error.failure_context(),
        Some(FailureContext::new(
            FailureStage::Terminal,
            Certainty::Unconfirmed
        ))
    );
    assert_eq!(error.recovery(), Recovery::Reconcile);
    assert_invalidated_before_terminal(&effects, request);
    assert_eq!(engine.state(), SessionState::Running);
    engine.assert_invariants().unwrap();
}

#[test]
fn conclusive_pre_ack_rejection_keeps_previously_known_projection() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let mut request = projected_command();
    request.context_mut().retry.max_retries = 0;
    let (write, id) = admit(&mut engine, 1, request, now);
    send_ok(&mut engine, &write, Some(0x1001), now);
    let effects = engine.handle(
        frame(
            1,
            Some((0x1001, SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: None,
                code: 0x03,
            },
        ),
        now,
    );
    assert!(terminal_failure(&effects, id).is_some());
    assert!(invalidations(&effects).is_empty());
}

#[test]
fn every_uncertain_terminal_invalidates_from_attempt_facts() {
    for envelope in [EnvelopeKind::Raw, EnvelopeKind::Sony] {
        for terminal in 0..4 {
            let now = Instant::now();
            let mut engine = engine(envelope, TransportKind::Datagram);
            let mut request = projected_command();
            request.context_mut().retry.max_retries = 0;
            let (write, id) = admit(&mut engine, 1, request, now);
            send_ok(
                &mut engine,
                &write,
                (envelope == EnvelopeKind::Sony).then_some(0x1001),
                now,
            );
            let sequence =
                (envelope == EnvelopeKind::Sony).then_some((0x1001, SequenceWidth::Full32));
            let effects = match terminal {
                0 => engine.advance(now + Duration::from_millis(20)),
                1 => {
                    engine.handle(
                        frame(
                            1,
                            sequence,
                            DecodedResponse::Ack {
                                socket: Some(ViscaSocket::S1),
                            },
                        ),
                        now,
                    );
                    engine.handle(
                        frame(
                            1,
                            sequence,
                            DecodedResponse::Error {
                                socket: Some(ViscaSocket::S1),
                                code: 0x41,
                            },
                        ),
                        now,
                    )
                }
                2 => {
                    engine.handle(
                        frame(
                            1,
                            sequence,
                            DecodedResponse::Ack {
                                socket: Some(ViscaSocket::S1),
                            },
                        ),
                        now,
                    );
                    engine.advance(now + Duration::from_millis(40))
                }
                _ => engine.handle(
                    Input::Shutdown(ShutdownReason::TransportClosed {
                        reason: Some("peer disappeared".into()),
                    }),
                    now,
                ),
            };
            assert_invalidated_before_terminal(&effects, id);
            engine.assert_invariants().unwrap();
        }
    }
}

#[test]
fn arbitrary_inquiry_write_causes_are_conclusive_and_preserved() {
    for cause in [
        Error::io_timeout(),
        Error::UnsequencedCommandUnconfirmed,
        Error::CommandFailedAfterAck {
            source: Box::new(Error::CommandNotExecutable),
        },
    ] {
        let now = Instant::now();
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (write, id) = admit(&mut engine, 1, inquiry(1), now);
        let expected = cause.to_string();
        let effects = result(&mut engine, &write, WriteOutcome::Failed(cause), now);
        let error = terminal_failure(&effects, id).unwrap();
        let Error::InquiryFailed { source } = &error else {
            panic!("{error:?}");
        };
        assert_eq!(source.to_string(), expected);
        assert_eq!(
            error.failure_context(),
            Some(FailureContext::new(
                FailureStage::Terminal,
                Certainty::FailedConclusively
            ))
        );
        assert_ne!(error.recovery(), Recovery::Reconcile);
    }
}

#[test]
fn refusing_a_retry_after_conclusive_rejection_preserves_known_state() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (write, id) = admit(&mut engine, 1, projected_command(), now);
    send_ok(&mut engine, &write, Some(0x1001), now);
    let rejected = engine.handle(
        frame(
            1,
            Some((0x1001, SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: None,
                code: 0x03,
            },
        ),
        now,
    );
    let (_, _, ready) = retry_scheduled(&rejected).unwrap();
    let retry = engine.advance(ready);
    let effects = result(
        &mut engine,
        &retry,
        WriteOutcome::NotSent(Error::InvalidRequest("pre-write refusal".into())),
        ready,
    );
    assert_eq!(
        terminal_failure(&effects, id).unwrap().failure_context(),
        Some(FailureContext::new(
            FailureStage::Terminal,
            Certainty::NotAccepted
        ))
    );
    assert!(invalidations(&effects).is_empty());
}

#[test]
fn refused_cancellation_keeps_the_written_operation_and_its_socket_live() {
    for transport in [TransportKind::Datagram, TransportKind::Stream] {
        let now = Instant::now();
        let mut engine = engine(EnvelopeKind::Raw, transport);
        let (write, id) = admit(&mut engine, 1, projected_command(), now);
        send_ok(&mut engine, &write, None, now);
        engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            now,
        );
        let cancellation = engine.handle(Input::Cancel { id }, now);
        let (transmission, request, _) = cancel_transmit(&cancellation);
        let effects = engine.handle(
            Input::TransmissionFinished {
                request,
                transmission,
                result: WriteOutcome::NotSent(Error::InvalidRequest(
                    "cancel framing refused".into(),
                )),
            },
            now,
        );
        assert!(terminal_outcome(&effects, id).is_none());
        assert!(effects.iter().any(|effect| matches!(effect,
            Effect::CancellationObservation { id: observed, observation: CancellationObservation::Failed(_) } if *observed == id)));
        assert_eq!(engine.state(), SessionState::Running);
        assert_eq!(
            engine.ledger.lane_state(camera(1), Lane::Command),
            LaneState::Clear,
            "the unsent cancellation leaves no owed first answer"
        );
        let completed = engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            now,
        );
        assert!(matches!(
            terminal_outcome(&completed, id),
            Some(RuntimeOutcome::Applied)
        ));
        assert!(invalidations(&completed).is_empty());
        engine.assert_invariants().unwrap();
    }
}

#[test]
fn a_result_with_another_requests_identity_cannot_consume_the_active_write() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    engine
        .register_target(camera(2), TargetPolicy::test_default())
        .unwrap();
    let (first, first_id) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &first, Some(0x1001), now);
    let (second, second_id) = admit(
        &mut engine,
        2,
        command(2, CancellationPolicy::Supported),
        now,
    );
    let (transmission, _, _) = request_transmit(&second);
    let wrong = engine.handle_turn(
        Input::TransmissionFinished {
            request: first_id,
            transmission,
            result: WriteOutcome::NotSent(Error::InvalidRequest("wrong request".into())),
        },
        now,
        EngineTurn::INPUT_ONLY,
    );
    assert_eq!(
        ignored_reasons(&wrong),
        [IgnoreReason::IncompatibleTransmissionResult]
    );
    assert!(terminal_outcome(&wrong, first_id).is_none());
    assert!(terminal_outcome(&wrong, second_id).is_none());
    send_ok(&mut engine, &second, Some(0x1002), now);
    assert!(matches!(
        phase_of(&engine, first_id),
        Some(Phase::AwaitingAck { .. })
    ));
    assert!(matches!(
        phase_of(&engine, second_id),
        Some(Phase::AwaitingAck { .. })
    ));
    engine.assert_invariants().unwrap();
}
