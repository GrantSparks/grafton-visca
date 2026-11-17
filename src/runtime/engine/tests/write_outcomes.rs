//! A write refusal proves only that this attempt sent no bytes.
use super::*;
use crate::{KnownState, Recovery, StateKey};

fn projected_command() -> RuntimeRequest {
    let mut request = command(1);
    let RuntimeRequest::Command { applied_state, .. } = &mut request else {
        unreachable!();
    };
    *applied_state = Some(AppliedStateProjection::set(KnownState::TallyBrightness(
        crate::types::TallyBrightness::High,
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
                WriteOutcome::NotSent(Error::InvalidRequest {
                    reason: "pre-write refusal".into(),
                }),
                now,
            );
            let error = terminal_failure(&effects, request).unwrap();
            assert!(matches!(&error, Error::TransmissionNotSent { source, .. }
                if matches!(source.as_ref(), Error::InvalidRequest { .. })));
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
                engine
                    .correlation
                    .ledger()
                    .lane_state(camera(1), Lane::Command),
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
        WriteOutcome::NotSent(Error::InvalidRequest {
            reason: "mode changed before retry".into(),
        }),
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
            Some(0x1001),
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
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
            let sequence = (envelope == EnvelopeKind::Sony).then_some(0x1001);
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
                                code: 0x41.into(),
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
                        cause: Some(Arc::new(Error::from(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "peer disappeared",
                        )))),
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
            Some(0x1001),
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
            },
        ),
        now,
    );
    let (_, _, ready) = retry_scheduled(&rejected).unwrap();
    let retry = engine.advance(ready);
    let effects = result(
        &mut engine,
        &retry,
        WriteOutcome::NotSent(Error::InvalidRequest {
            reason: "pre-write refusal".into(),
        }),
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
                result: WriteOutcome::NotSent(Error::InvalidRequest {
                    reason: "cancel framing refused".into(),
                }),
            },
            now,
        );
        assert!(terminal_outcome(&effects, id).is_none());
        assert!(effects.iter().any(|effect| matches!(effect,
            Effect::CancellationObservation { id: observed, observation: CancellationObservation::Failed(_) } if *observed == id)));
        assert_eq!(engine.state(), SessionState::Running);
        assert_eq!(
            engine
                .correlation
                .ledger()
                .lane_state(camera(1), Lane::Command),
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
            Some(RuntimeOutcome::Command(CommandOutcome::Applied))
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
    let (first, first_id) = admit(&mut engine, 1, command(1), now);
    send_ok(&mut engine, &first, Some(0x1001), now);
    let (second, second_id) = admit(&mut engine, 2, command(2), now);
    let (transmission, _, _) = request_transmit(&second);
    let wrong = engine.handle_turn(
        Input::TransmissionFinished {
            request: first_id,
            transmission,
            result: WriteOutcome::NotSent(Error::InvalidRequest {
                reason: "wrong request".into(),
            }),
        },
        now,
        EngineTurn::InputOnly,
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

/// The same failed physical write must have the same terminal certainty,
/// cache projection and relative hold on both sides of the retry budget.
#[test]
fn failed_datagrams_have_one_evidence_rule_inside_and_after_the_budget() {
    for envelope in [EnvelopeKind::Raw, EnvelopeKind::Sony] {
        for kind in 0..4 {
            let mut early = None;
            for late in [false, true] {
                let now = Instant::now();
                let mut engine = engine(envelope, TransportKind::Datagram);
                let mut request = if kind == 3 {
                    inquiry(1)
                } else {
                    projected_command()
                };
                if let RuntimeRequest::Command { reply_shape, .. } = &mut request {
                    *reply_shape = match kind {
                        0 => CommandReply::AckThenCompletion,
                        1 => CommandReply::CompletionOnly,
                        _ => CommandReply::NoReply,
                    };
                }
                request.context_mut().retry.total_budget =
                    Some(NonZeroDuration::new(Duration::from_millis(100)).unwrap());
                let (write, id) = admit(&mut engine, 1, request, now);
                let at = if late {
                    now + Duration::from_millis(101)
                } else {
                    now
                };
                let effects = result(
                    &mut engine,
                    &write,
                    WriteOutcome::Failed(Error::transport_error(std::io::Error::other(
                        "ambiguous datagram write",
                    ))),
                    at,
                );
                let error = terminal_failure(&effects, id).unwrap();
                let certainty = if kind == 3 {
                    Certainty::FailedConclusively
                } else {
                    Certainty::Unconfirmed
                };
                assert_eq!(
                    error.failure_context(),
                    Some(FailureContext::new(FailureStage::Terminal, certainty))
                );
                if kind != 3 {
                    assert_eq!(error.recovery(), Recovery::Reconcile);
                    assert_invalidated_before_terminal(&effects, id);
                    if envelope == EnvelopeKind::Raw {
                        assert!(matches!(error, Error::UnsequencedCommandUnconfirmed));
                    } else {
                        assert!(
                            matches!(&error, Error::RequestUnconfirmed { source, .. } if matches!(source.as_ref(), Error::TransportError { .. }))
                        );
                    }
                } else {
                    assert!(invalidations(&effects).is_empty());
                    assert!(
                        matches!(&error, Error::InquiryFailed { source } if matches!(source.as_ref(), Error::TransportError { .. }))
                    );
                }
                let hold = if envelope == EnvelopeKind::Raw {
                    let scope = match kind {
                        0 => RawHoldScope::PreAck,
                        1 | 2 => RawHoldScope::AllResponses,
                        _ => RawHoldScope::Inquiry(PayloadShape::Unknown),
                    };
                    let hold = engine
                        .raw_hold(camera(1), scope)
                        .expect("uncertain raw write keeps correlation");
                    assert_eq!(hold.owner, Some(id));
                    Some(hold.until.duration_since(at))
                } else {
                    assert!(engine.correlation.holds().is_empty());
                    None
                };
                let summary = (
                    error.to_string(),
                    error.failure_context(),
                    error.recovery(),
                    hold,
                    invalidations(&effects).len(),
                );
                if let Some(early) = &early {
                    assert_eq!(&summary, early, "{envelope:?}, kind {kind}");
                } else {
                    early = Some(summary);
                }
                assert_eq!(engine.state(), SessionState::Running);
                engine.assert_invariants().unwrap();
            }
        }
    }
}

#[test]
fn failed_raw_datagram_holds_late_answers_away_from_a_successor() {
    for is_inquiry in [false, true] {
        let now = Instant::now();
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let request = || {
            if is_inquiry {
                inquiry(1)
            } else {
                projected_command()
            }
        };
        let (write, first) = admit(&mut engine, 1, request(), now);
        result(
            &mut engine,
            &write,
            WriteOutcome::Failed(Error::io_timeout()),
            now,
        );
        let queued = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(2),
                request: request(),
                slot: AdmissionSlot::Ordinary,
            },
            now,
        );
        let successor = admitted(&queued);
        assert!(!queued
            .iter()
            .any(|effect| matches!(effect, Effect::Transmit { .. })));
        let replies = if is_inquiry {
            vec![DecodedResponse::InquiryReply {
                payload: smallvec::smallvec![0x01],
            }]
        } else {
            vec![
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
                DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S1),
                },
            ]
        };
        for response in replies {
            let effects = engine.handle(frame(1, None, response), now);
            assert!(terminal_outcome(&effects, successor).is_none());
            assert!(!effects
                .iter()
                .any(|effect| matches!(effect, Effect::Transmit { .. })));
            assert!(matches!(phase_of(&engine, successor), Some(Phase::Ready)));
        }
        assert!(engine.entry(first).is_none());
        engine.assert_invariants().unwrap();
    }
}

#[test]
fn failed_raw_datagrams_use_strict_poison_on_either_side_of_the_budget() {
    for late in [false, true] {
        let now = Instant::now();
        let mut engine = strict_poison_engine();
        let mut request = projected_command();
        request.context_mut().retry.total_budget =
            Some(NonZeroDuration::new(Duration::from_millis(100)).unwrap());
        let (write, id) = admit(&mut engine, 1, request, now);
        let at = if late {
            now + Duration::from_millis(101)
        } else {
            now
        };
        let effects = result(
            &mut engine,
            &write,
            WriteOutcome::Failed(Error::io_timeout()),
            at,
        );
        assert!(matches!(
            terminal_failure(&effects, id),
            Some(Error::StreamPoisoned { .. })
        ));
        assert_eq!(engine.state(), SessionState::Poisoned);
        engine.assert_invariants().unwrap();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttemptEnding {
    NotSent,
    Failed,
    LateSuccess,
    Ack,
    Completion,
    Inquiry,
    Budget,
}

/// Recorded before consolidating the terminal paths. Each row pins the public
/// outcome together with correlation and cache consequences; deadlines use
/// deliberately different literal intervals so one expiry cannot stand in for
/// another.
#[test]
fn attempt_endings_preserve_error_certainty_recovery_holds_and_cache() {
    for (envelope, transport) in [
        (EnvelopeKind::Raw, TransportKind::Datagram),
        (EnvelopeKind::Raw, TransportKind::Stream),
        (EnvelopeKind::Sony, TransportKind::Datagram),
    ] {
        for family in 0..4 {
            for ending in [
                AttemptEnding::NotSent,
                AttemptEnding::Failed,
                AttemptEnding::LateSuccess,
                AttemptEnding::Ack,
                AttemptEnding::Completion,
                AttemptEnding::Inquiry,
                AttemptEnding::Budget,
            ] {
                if (ending == AttemptEnding::Ack && family != 0)
                    || (ending == AttemptEnding::Completion && family > 1)
                    || (ending == AttemptEnding::Inquiry && family != 3)
                    || (ending == AttemptEnding::Budget && family == 2)
                {
                    continue;
                }
                let start = Instant::now();
                let mut engine = engine(envelope, transport);
                let mut request = if family == 3 {
                    inquiry(1)
                } else {
                    projected_command()
                };
                if let RuntimeRequest::Command { reply_shape, .. } = &mut request {
                    *reply_shape = match family {
                        0 => CommandReply::AckThenCompletion,
                        1 => CommandReply::CompletionOnly,
                        _ => CommandReply::NoReply,
                    };
                }
                request.context_mut().retry = RetryPolicy {
                    total_budget: Some(NonZeroDuration::new(Duration::from_millis(100)).unwrap()),
                    ..RetryPolicy::NEVER
                };
                let (write, id) = admit(&mut engine, 1, request, start);
                let effects = match ending {
                    AttemptEnding::NotSent => result(
                        &mut engine,
                        &write,
                        WriteOutcome::NotSent(Error::InvalidRequest {
                            reason: "refused".into(),
                        }),
                        start,
                    ),
                    AttemptEnding::Failed => result(
                        &mut engine,
                        &write,
                        WriteOutcome::Failed(Error::transport_error(std::io::Error::other(
                            "failed",
                        ))),
                        start,
                    ),
                    AttemptEnding::LateSuccess => result(
                        &mut engine,
                        &write,
                        WriteOutcome::Sent(TransmissionMeta::for_sequence(
                            (envelope == EnvelopeKind::Sony).then_some(0x1001),
                        )),
                        start + Duration::from_millis(101),
                    ),
                    AttemptEnding::Budget => {
                        send_ok(
                            &mut engine,
                            &write,
                            (envelope == EnvelopeKind::Sony).then_some(0x1001),
                            start,
                        );
                        engine.advance(start + Duration::from_millis(100))
                    }
                    AttemptEnding::Ack | AttemptEnding::Completion | AttemptEnding::Inquiry => {
                        send_ok(
                            &mut engine,
                            &write,
                            (envelope == EnvelopeKind::Sony).then_some(0x1001),
                            start,
                        );
                        if ending == AttemptEnding::Completion && family == 0 {
                            engine.handle(
                                frame(
                                    1,
                                    (envelope == EnvelopeKind::Sony).then_some(0x1001),
                                    DecodedResponse::Ack {
                                        socket: Some(ViscaSocket::S1),
                                    },
                                ),
                                start,
                            );
                        }
                        let millis = match ending {
                            AttemptEnding::Ack => 20,
                            AttemptEnding::Completion => 40,
                            AttemptEnding::Inquiry => 30,
                            _ => unreachable!(),
                        };
                        engine.advance(start + Duration::from_millis(millis))
                    }
                };
                let error = terminal_failure(&effects, id).unwrap();
                let cause = terminal_cause(&effects, id).unwrap();
                let variant = match cause {
                    Error::TransmissionNotSent { .. } => "not_sent",
                    Error::StreamPoisoned { .. } => "poisoned",
                    Error::UnsequencedCommandUnconfirmed => "raw_unconfirmed",
                    Error::RequestUnconfirmed { .. } => "unconfirmed",
                    Error::TransportError { .. } => "transport",
                    Error::Timeout { .. } => "timeout",
                    ref other => panic!("unexpected cause {other:?}"),
                };
                let holds: Vec<_> = engine
                    .correlation
                    .holds()
                    .keys()
                    .map(|key| key.scope)
                    .collect();
                let summary = (
                    variant,
                    error.failure_context(),
                    error.recovery(),
                    holds,
                    invalidations(&effects).len(),
                );
                // Literal expectations recorded from the pre-refactor paths.
                let poisoned =
                    transport == TransportKind::Stream && ending == AttemptEnding::Failed;
                let expected_variant = if ending == AttemptEnding::NotSent {
                    "not_sent"
                } else if poisoned {
                    "poisoned"
                } else if envelope == EnvelopeKind::Raw && family != 3 {
                    "raw_unconfirmed"
                } else if ending == AttemptEnding::Failed {
                    if family == 3 {
                        "transport"
                    } else {
                        "unconfirmed"
                    }
                } else {
                    "timeout"
                };
                let expected_context = if poisoned && family != 3 {
                    None
                } else {
                    Some(FailureContext::new(
                        FailureStage::Terminal,
                        if family == 3 {
                            Certainty::FailedConclusively
                        } else if ending == AttemptEnding::NotSent {
                            Certainty::NotAccepted
                        } else {
                            Certainty::Unconfirmed
                        },
                    ))
                };
                let expected_recovery = if ending == AttemptEnding::NotSent {
                    Recovery::Abandon
                } else if poisoned {
                    Recovery::ReopenSession
                } else if family != 3 {
                    Recovery::Reconcile
                } else {
                    Recovery::Resubmit {
                        after: if ending == AttemptEnding::Failed {
                            Duration::from_millis(50)
                        } else {
                            Duration::from_secs(2)
                        },
                    }
                };
                let expected_holds = if envelope == EnvelopeKind::Sony
                    || ending == AttemptEnding::NotSent
                    || poisoned
                {
                    vec![]
                } else if family == 3 {
                    vec![RawHoldScope::Inquiry(PayloadShape::Unknown)]
                } else if family == 0 && ending == AttemptEnding::Completion {
                    vec![RawHoldScope::Socket(ViscaSocket::S1)]
                } else if transport == TransportKind::Datagram {
                    vec![if family == 0 {
                        RawHoldScope::PreAck
                    } else {
                        RawHoldScope::AllResponses
                    }]
                } else if family == 2 {
                    vec![RawHoldScope::AllResponses]
                } else {
                    vec![]
                };
                assert_eq!(
                    summary,
                    (
                        expected_variant,
                        expected_context,
                        expected_recovery,
                        expected_holds,
                        usize::from(family != 3 && ending != AttemptEnding::NotSent)
                    ),
                    "{envelope:?}/{transport:?}/{family}/{ending:?}"
                );
                engine.assert_invariants().unwrap();
            }
        }
    }
}

mod ending_properties {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn write_failures_and_deadline_orderings_keep_occupancy_consistent(
            sony in any::<bool>(),
            stream in any::<bool>(),
            family in 0_usize..4,
            verdict in 0_u8..3,
            elapsed_us in 0_u64..150_000,
            late_frame_us in 0_u64..150_000,
        ) {
            let envelope = if sony { EnvelopeKind::Sony } else { EnvelopeKind::Raw };
            let transport = if stream { TransportKind::Stream } else { TransportKind::Datagram };
            let start = Instant::now();
            let mut engine = engine(envelope, transport);
            let mut request = if family == 3 { inquiry(1) } else { projected_command() };
            if let RuntimeRequest::Command { reply_shape, .. } = &mut request {
                *reply_shape = match family {
                    0 => CommandReply::AckThenCompletion,
                    1 => CommandReply::CompletionOnly,
                    _ => CommandReply::NoReply,
                };
            }
            request.context_mut().retry.total_budget = Some(NonZeroDuration::new(Duration::from_millis(100)).unwrap());
            let (write, _) = admit(&mut engine, 1, request, start);
            engine.assert_invariants().unwrap();
            let at = start + Duration::from_micros(elapsed_us);
            let outcome = match verdict {
                0 => WriteOutcome::NotSent(Error::InvalidRequest { reason: "generated refusal".into() }),
                1 => WriteOutcome::Failed(Error::io_timeout()),
                _ => WriteOutcome::Sent(TransmissionMeta::for_sequence(sony.then_some(0x1001))),
            };
            result(&mut engine, &write, outcome, at);
            engine.assert_invariants().unwrap();
            assert_occupancy_agrees(&engine, at);
            let late_at = at + Duration::from_micros(late_frame_us);
            engine.handle(frame(1, sony.then_some(0x1001),
                DecodedResponse::Completion { socket: Some(ViscaSocket::S1) }), late_at);
            engine.assert_invariants().unwrap();
            assert_occupancy_agrees(&engine, late_at);
            engine.advance(late_at + Duration::from_secs(1));
            prop_assert!(engine.assert_invariants().is_ok());
            assert_occupancy_agrees(&engine, late_at + Duration::from_secs(1));
        }
    }
}
