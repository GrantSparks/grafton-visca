//! Each freshly selected due action makes the same request no longer due.
use super::*;

#[derive(Clone, Copy)]
enum Scenario {
    Ack,
    Completion,
    Inquiry,
    CancellationResolution,
    CancelledAck,
    CancellationObservation,
    Budget,
    RetryReady,
}

fn fixture(scenario: Scenario) -> (ProtocolEngine, RequestId, Instant) {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let mut retry = if matches!(scenario, Scenario::RetryReady | Scenario::Budget) {
        retrying()
    } else {
        RetryPolicy::NEVER
    };
    if matches!(scenario, Scenario::Budget) {
        retry.total_budget = Some(NonZeroDuration::new(Duration::from_millis(10)).unwrap());
    }
    let request = if matches!(scenario, Scenario::Inquiry) {
        inquiry_with_retry(1, retry)
    } else {
        command_with_retry(1, retry)
    };
    let (write, id) = admit(&mut engine, 1, request, start);
    send_ok(&mut engine, &write, Some(0x1001), start);
    let at = match scenario {
        Scenario::Ack => start + Duration::from_millis(20),
        Scenario::Inquiry => start + Duration::from_millis(30),
        Scenario::Budget => start + Duration::from_millis(10),
        Scenario::RetryReady => {
            let timeout = engine.advance(start + Duration::from_millis(20));
            retry_scheduled(&timeout).unwrap().2
        }
        Scenario::CancelledAck => {
            engine.handle(Input::Cancel { id }, start);
            engine.advance(start + Duration::from_millis(20));
            start + Duration::from_millis(50)
        }
        Scenario::Completion
        | Scenario::CancellationObservation
        | Scenario::CancellationResolution => {
            engine.handle(
                frame(
                    1,
                    Some((0x1001, SequenceWidth::Full32)),
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                start,
            );
            if matches!(scenario, Scenario::Completion) {
                start + Duration::from_millis(40)
            } else {
                let cancel = engine.handle(Input::Cancel { id }, start);
                let (transmission, request, _) = cancel_transmit(&cancel);
                finish_write_input_only(
                    &mut engine,
                    request,
                    transmission,
                    Ok(TransmissionMeta::Sony { sequence: 0x2001 }),
                    start,
                );
                if matches!(scenario, Scenario::CancellationResolution) {
                    engine.advance(start + Duration::from_millis(10));
                    start + Duration::from_millis(50)
                } else {
                    start + Duration::from_millis(10)
                }
            }
        }
    };
    (engine, id, at)
}

fn assert_progress(scenario: Scenario, expected: fn(DueAction) -> bool) {
    let (mut engine, id, now) = fixture(scenario);
    let due = engine.next_due().unwrap();
    assert_eq!(due.request, id);
    assert!(due.at <= now);
    assert!(expected(due.action), "unexpected action: {:?}", due.action);

    engine.apply_due(due, now, &mut Decision::new(&mut Vec::new()));
    assert!(engine
        .entry(id)
        .and_then(|entry| entry.due(id))
        .is_none_or(|next| next.at > now));
    engine.assert_invariants().unwrap();
}

#[test]
fn ack_action_progresses() {
    assert_progress(Scenario::Ack, |action| {
        matches!(action, DueAction::Ack { .. })
    });
}

#[test]
fn completion_action_progresses() {
    assert_progress(Scenario::Completion, |action| {
        matches!(action, DueAction::Completion(_))
    });
}

#[test]
fn inquiry_reply_action_progresses() {
    assert_progress(Scenario::Inquiry, |action| {
        matches!(action, DueAction::InquiryReply { .. })
    });
}

#[test]
fn cancellation_resolution_action_progresses() {
    assert_progress(Scenario::CancellationResolution, |action| {
        matches!(action, DueAction::CancellationResolution { .. })
    });
}

#[test]
fn cancelled_ack_action_progresses() {
    assert_progress(Scenario::CancelledAck, |action| {
        matches!(
            action,
            DueAction::Ack {
                cancellation: Some(_),
                ..
            }
        )
    });
}

#[test]
fn cancellation_observation_action_progresses() {
    assert_progress(Scenario::CancellationObservation, |action| {
        matches!(action, DueAction::CancellationObservation { .. })
    });
}

#[test]
fn retry_budget_action_progresses() {
    assert_progress(Scenario::Budget, |action| {
        matches!(action, DueAction::RetryBudget { .. })
    });
}

#[test]
fn retry_ready_action_progresses() {
    assert_progress(Scenario::RetryReady, |action| {
        matches!(action, DueAction::RetryReady { .. })
    });
}

#[test]
fn late_observation_and_retry_ready_actions_finish_without_another_due_iteration() {
    for scenario in [Scenario::CancellationObservation, Scenario::RetryReady] {
        let (mut engine, id, _) = fixture(scenario);
        let now = Instant::now() + Duration::from_secs(20);
        let due = engine.next_due().unwrap();

        engine.apply_due(due, now, &mut Decision::new(&mut Vec::new()));
        assert!(engine.entry(id).is_none());
    }
}

#[test]
fn immediate_timeout_retries_end_the_action_ready() {
    for family in 0..3 {
        let now = Instant::now();
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let mut retry = retrying();
        retry.initial_backoff = Duration::ZERO;
        retry.maximum_backoff = Duration::ZERO;
        let request = match family {
            0 => command_with_retry(1, retry),
            1 => command_with_reply_shape_and_retry(
                1,
                CancellationPolicy::Supported,
                ReplyShape::CompletionOnly,
                retry,
            ),
            _ => inquiry_with_retry(1, retry),
        };
        let (write, id) = admit(&mut engine, 1, request, now);
        send_ok(&mut engine, &write, Some(0x1001), now);
        let due = engine.next_due().unwrap();

        engine.apply_due(due, due.at, &mut Decision::new(&mut Vec::new()));
        assert!(matches!(phase_of(&engine, id), Some(Phase::Ready)));
        assert!(engine
            .entry(id)
            .unwrap()
            .due(id)
            .is_none_or(|next| next.at > due.at));
        engine.assert_invariants().unwrap();
    }
}

fn raw_pair(minimum_ambiguity: bool) -> (ProtocolEngine, RequestId, RequestId, Instant) {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    let request = command_with_reply_shape_and_retry(
        1,
        CancellationPolicy::Supported,
        ReplyShape::CompletionOnly,
        RetryPolicy::NEVER,
    );
    let (write, first) = admit(&mut engine, 1, request, now);
    send_ok(&mut engine, &write, None, now);
    let mut stop = command_with_retry(1, RetryPolicy::NEVER);
    stop.context_mut().control.class = ControlClass::Urgent;
    if minimum_ambiguity {
        stop.context_mut().timeout.ambiguity =
            NonZeroDuration::new(Duration::from_nanos(1)).unwrap();
    }
    let (write, second) = admit(&mut engine, 2, stop, now);
    send_ok(&mut engine, &write, None, now);
    (engine, first, second, now)
}

#[test]
fn a_disputed_ack_at_equality_gets_a_strictly_later_minimum_ambiguity_deadline() {
    let (mut engine, first, stop, now) = raw_pair(true);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x41,
            },
        ),
        now,
    );
    assert!(engine.correlation.ledger().awaits_dispute(camera(1), stop));
    let equality = now + Duration::from_millis(20);
    engine.advance(equality);
    assert!(engine.entry(first).is_some());
    assert!(
        matches!(phase_of(&engine, stop), Some(Phase::AwaitingAck { deadline, .. }) if deadline == equality + Duration::from_nanos(1))
    );
    engine.assert_invariants().unwrap();
}

#[test]
fn ambiguous_wire_completion_counter_saturates() {
    let (mut engine, first, stop, now) = raw_pair(false);
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
    engine
        .correlation
        .stream_mut()
        .unwrap()
        .update_learning(camera(1), |learning| {
            learning.unlearned_completions = u32::MAX
        });
    for _ in 0..4 {
        engine.handle(
            frame(1, None, DecodedResponse::Completion { socket: None }),
            now,
        );
    }
    assert_eq!(
        engine.correlation.learning(camera(1)).unlearned_completions,
        u32::MAX
    );
    assert!(engine.entry(first).is_some() && engine.entry(stop).is_some());
    engine.assert_invariants().unwrap();
}

#[test]
fn preexisting_requests_make_progress_when_the_clock_reaches_its_last_instant() {
    let latest = crate::timeout::saturating_deadline(Instant::now(), Duration::MAX);
    assert_eq!(
        crate::timeout::saturating_deadline(latest, Duration::from_nanos(1)),
        latest
    );
    for scenario in [
        Scenario::Ack,
        Scenario::Completion,
        Scenario::Inquiry,
        Scenario::CancellationResolution,
        Scenario::CancelledAck,
        Scenario::CancellationObservation,
        Scenario::Budget,
        Scenario::RetryReady,
    ] {
        let (mut engine, id, _) = fixture(scenario);
        let due = engine.next_due().unwrap();

        engine.apply_due(due, latest, &mut Decision::new(&mut Vec::new()));
        assert!(
            engine.entry(id).is_none(),
            "a finite wait cannot extend beyond the final instant"
        );
        engine.assert_invariants().unwrap();
    }
    // This path extends a disputed ACK deadline from the sampled clock,
    // rather than an already retained absolute cancellation deadline.
    let (mut engine, _, stop, now) = raw_pair(true);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x41,
            },
        ),
        now,
    );
    assert!(engine.correlation.ledger().awaits_dispute(camera(1), stop));
    engine.advance(latest);
    assert!(engine.entries.is_empty());
    engine.advance(latest);
    engine.assert_invariants().unwrap();
}

#[test]
fn engine_construction_retuning_and_admission_refuse_unrepresentable_intervals() {
    let now = Instant::now();
    let mut invalid = policy(EnvelopeKind::Raw, TransportKind::Datagram);
    invalid.command_spacing = Duration::MAX;
    assert!(ProtocolEngine::new(invalid, now).is_err());
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let spacing = engine.policy.command_spacing;
    assert!(engine
        .retune(Duration::MAX, Duration::ZERO, PerTarget::default(), now)
        .is_err());
    assert_eq!(
        engine.policy.command_spacing, spacing,
        "failed retuning is atomic"
    );
    let mut invalid = command(1, CancellationPolicy::Supported);
    invalid.context_mut().timeout.ambiguity = NonZeroDuration::new(Duration::MAX).unwrap();
    let rejected = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: invalid,
            slot: AdmissionSlot::Ordinary,
        },
        now,
    );
    assert!(matches!(
        rejected.as_slice(),
        [Effect::AdmissionRejected { .. }]
    ));
    assert!(engine.entries.is_empty());
    let (_, id) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        now,
    );
    assert_eq!(
        id.get(),
        1,
        "an invalid context consumes no request identity"
    );
    engine.assert_invariants().unwrap();
}
