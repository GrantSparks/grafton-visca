//! Deadlines retire requests; they never erase acceptance or socket evidence.

use super::*;
use proptest::prelude::*;

const SEQUENCE: u32 = 0x1234_1001;

fn answer(envelope: EnvelopeKind, response: DecodedResponse) -> Input {
    frame(
        1,
        (envelope == EnvelopeKind::Sony).then_some((SEQUENCE, SequenceWidth::Full32)),
        response,
    )
}

fn ack(envelope: EnvelopeKind, socket: ViscaSocket) -> Input {
    answer(
        envelope,
        DecodedResponse::Ack {
            socket: Some(socket),
        },
    )
}

fn complete(envelope: EnvelopeKind, socket: Option<ViscaSocket>) -> Input {
    answer(envelope, DecodedResponse::Completion { socket })
}

fn assert_applied(effects: &[Effect], request: RequestId) {
    assert!(
        effects.iter().any(|effect| matches!(effect,
            Effect::Terminal { id, outcome: RuntimeOutcome::Command(CommandOutcome::Applied) } if *id == request
        )),
        "{effects:?}"
    );
}

fn assert_no_rewrite(effects: &[Effect], request: RequestId) {
    assert!(!effects.iter().any(|effect| matches!(effect,
        Effect::Transmit { request: id, kind: Transmission::Request { .. }, .. } if *id == request
    )), "acceptance evidence must cancel the retry: {effects:?}");
}

fn sony_waiting(start: Instant) -> (ProtocolEngine, RequestId) {
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (write, id) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        start,
    );
    send_ok(&mut engine, &write, Some(SEQUENCE), start);
    (engine, id)
}

#[test]
fn sony_late_ack_adopts_acceptance_before_any_retry_write() {
    for waiting in 0..3 {
        let start = Instant::now();
        let (mut engine, id) = sony_waiting(start);
        let mut at = start + Duration::from_millis(21);
        if waiting != 0 {
            engine.advance(start + Duration::from_millis(20));
            let Some(Phase::Backoff { ready_at, .. }) = phase_of(&engine, id) else {
                panic!("expected retry backoff");
            };
            if waiting == 2 {
                at = ready_at;
                engine.run_due(at, &mut Decision::new(&mut Vec::new()));
                assert!(matches!(phase_of(&engine, id), Some(Phase::Ready)));
            }
        }
        let accepted = engine.handle(ack(EnvelopeKind::Sony, ViscaSocket::S1), at);
        assert_no_rewrite(&accepted, id);
        assert!(
            matches!(phase_of(&engine, id), Some(Phase::Executing { .. })),
            "{accepted:?}"
        );
        let duplicate = engine.handle(ack(EnvelopeKind::Sony, ViscaSocket::S1), at);
        assert_no_rewrite(&duplicate, id);
        assert!(duplicate
            .iter()
            .any(|effect| matches!(effect, Effect::Ignored(IgnoreReason::UnmatchedFrame))));
        let completed = engine.handle(complete(EnvelopeKind::Sony, Some(ViscaSocket::S1)), at);
        assert_applied(&completed, id);
        assert_no_rewrite(&completed, id);
        assert_no_rewrite(&engine.advance(at + Duration::from_secs(1)), id);
        engine.assert_invariants().unwrap();
    }
}

#[test]
fn sony_late_completion_applies_each_unacknowledged_retry_path() {
    for cause in 0..3 {
        for ready in [false, true] {
            let start = Instant::now();
            let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
            let request = if cause == 1 {
                command_with_reply_shape(
                    1,
                    CancellationPolicy::Supported,
                    ReplyShape::CompletionOnly,
                )
            } else {
                command(1, CancellationPolicy::Supported)
            };
            let (write, id) = admit(&mut engine, 1, request, start);
            send_ok(&mut engine, &write, Some(SEQUENCE), start);
            let mut at = start
                + Duration::from_millis(match cause {
                    1 => 40,
                    2 => 1,
                    _ => 20,
                });
            if cause == 2 {
                engine.handle(
                    Input::ReceiveFault {
                        error: Error::transport_error(std::io::Error::other("transient receive")),
                    },
                    at,
                );
            } else {
                engine.advance(at);
            }
            let Some(Phase::Backoff { ready_at, .. }) = phase_of(&engine, id) else {
                panic!("expected retry backoff for cause {cause}");
            };
            if ready {
                at = ready_at;
                engine.run_due(at, &mut Decision::new(&mut Vec::new()));
            }
            let completed = engine.handle(complete(EnvelopeKind::Sony, Some(ViscaSocket::S1)), at);
            assert_applied(&completed, id);
            assert_no_rewrite(&completed, id);
            assert_no_rewrite(&engine.advance(at + Duration::from_secs(1)), id);
            engine.assert_invariants().unwrap();
        }
    }
}

#[test]
fn sony_lower16_ack_keeps_the_written_attempt_socket_capacity() {
    let start = Instant::now();
    let (mut engine, id) = sony_waiting(start);
    engine.advance(start + Duration::from_millis(20));
    engine
        .retune(
            Duration::ZERO,
            Duration::ZERO,
            PerTarget::from_fn(|_| Some(CommandSocketCount::ONE)),
            Instant::now(),
        )
        .unwrap();
    let accepted = engine.handle(
        frame(
            1,
            Some((SEQUENCE & 0xffff, SequenceWidth::Lower16)),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S2),
            },
        ),
        start + Duration::from_millis(21),
    );
    assert!(
        matches!(
            phase_of(&engine, id),
            Some(Phase::Executing {
                socket: ViscaSocket::S2,
                ..
            })
        ),
        "{accepted:?}"
    );
    engine.assert_invariants().unwrap();
}

#[test]
fn sony_late_camera_error_does_not_spend_another_retry() {
    let start = Instant::now();
    let (mut engine, id) = sony_waiting(start);
    engine.advance(start + Duration::from_millis(20));
    let attempt = engine.entry(id).unwrap().retry_count();
    let ignored = engine.handle(
        answer(
            EnvelopeKind::Sony,
            DecodedResponse::Error {
                socket: Some(ViscaSocket::S1),
                code: 0x03.into(),
            },
        ),
        start + Duration::from_millis(21),
    );
    assert_eq!(engine.entry(id).unwrap().retry_count(), attempt);
    assert!(!ignored
        .iter()
        .any(|effect| matches!(effect, Effect::RetryScheduled { .. })));
    engine.assert_invariants().unwrap();
}

#[test]
fn acknowledged_lifecycle_cannot_supply_retry_eligibility() {
    let start = Instant::now();
    let (mut engine, id) = sony_waiting(start);
    assert!(engine
        .entry(id)
        .unwrap()
        .lifecycle()
        .retry_eligibility(true)
        .is_some());
    engine.handle(ack(EnvelopeKind::Sony, ViscaSocket::S1), start);
    assert!(engine
        .entry(id)
        .unwrap()
        .lifecycle()
        .retry_eligibility(true)
        .is_none());
    engine.handle(Input::Cancel { id }, start);
    assert!(engine
        .entry(id)
        .unwrap()
        .lifecycle()
        .retry_eligibility(true)
        .is_none());
}

#[test]
fn late_sony_completion_does_not_outlive_the_total_budget_in_backoff() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let mut request = command(1, CancellationPolicy::Supported);
    request.context_mut().retry.total_budget =
        Some(NonZeroDuration::new(Duration::from_millis(22)).unwrap());
    request.context_mut().retry.initial_backoff = Duration::from_millis(1);
    request.context_mut().retry.maximum_backoff = Duration::from_millis(1);
    let (write, id) = admit(&mut engine, 1, request, start);
    send_ok(&mut engine, &write, Some(SEQUENCE), start);
    engine.advance(start + Duration::from_millis(20));

    let late = engine.handle(
        complete(EnvelopeKind::Sony, Some(ViscaSocket::S1)),
        start + Duration::from_millis(23),
    );
    assert!(late.iter().any(|effect| matches!(effect, Effect::Terminal { id: ended, outcome: RuntimeOutcome::Command(CommandOutcome::Failed(_)) } if *ended == id)));
    assert!(!late.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            outcome: RuntimeOutcome::Command(CommandOutcome::Applied),
            ..
        }
    )));
    assert_no_rewrite(&late, id);
    engine.assert_invariants().unwrap();
}

#[test]
fn raw_late_socketless_completion_releases_the_retired_holder_without_a_cascade() {
    for due_first in [false, true] {
        let start = Instant::now();
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
        let mut at = start;
        for (index, socket) in [ViscaSocket::S1, ViscaSocket::S2, ViscaSocket::S1]
            .into_iter()
            .enumerate()
        {
            let (write, id) = admit(
                &mut engine,
                u64::try_from(index + 1).unwrap(),
                command(1, CancellationPolicy::Supported),
                at,
            );
            send_ok(&mut engine, &write, None, at);
            engine.handle(ack(EnvelopeKind::Raw, socket), at);
            if index == 0 {
                at += Duration::from_millis(41);
            }
            let mut effects = Vec::new();
            if due_first {
                effects.extend(engine.advance(at));
            }
            effects.extend(engine.handle(complete(EnvelopeKind::Raw, None), at));
            if index == 0 {
                assert!(effects.iter().any(|effect| matches!(effect,
                    Effect::Terminal { id: terminal, outcome: RuntimeOutcome::Command(CommandOutcome::Failed(Error::UnsequencedCommandUnconfirmed)) } if *terminal == id
                )));
            } else {
                assert_applied(&effects, id);
            }
            assert!(
                engine.correlation.holds().is_empty(),
                "command {index}, due_first={due_first}: {:?}",
                engine.correlation.holds()
            );
            engine.assert_invariants().unwrap();
        }
    }
}

fn held_and_live(start: Instant) -> (ProtocolEngine, RequestId, RequestId) {
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    let (write, held) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        start,
    );
    send_ok(&mut engine, &write, None, start);
    engine.handle(ack(EnvelopeKind::Raw, ViscaSocket::S1), start);
    let later = start + Duration::from_millis(10);
    let (write, live) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        later,
    );
    send_ok(&mut engine, &write, None, later);
    engine.handle(ack(EnvelopeKind::Raw, ViscaSocket::S2), later);
    engine.advance(start + Duration::from_millis(40));
    assert!(engine
        .raw_hold(camera(1), RawHoldScope::Socket(ViscaSocket::S1))
        .is_some());
    (engine, held, live)
}

#[test]
fn ambiguous_socketless_terminals_are_accounted_once_count_proves_both_candidates_ended() {
    let start = Instant::now();
    let (mut engine, _, live) = held_and_live(start);
    let at = start + Duration::from_millis(41);
    let first = engine.handle(complete(EnvelopeKind::Raw, None), at);
    assert!(terminal_id(&first).is_none());
    assert!(engine
        .correlation
        .learning(camera(1))
        .pending_socketless_completions
        .is_some());
    let second = engine.handle(complete(EnvelopeKind::Raw, None), at);
    assert_applied(&second, live);
    assert!(engine.correlation.holds().is_empty());
    assert!(engine
        .correlation
        .learning(camera(1))
        .pending_socketless_completions
        .is_none());
    engine.assert_invariants().unwrap();
}

#[test]
fn held_socket_reuse_does_not_assign_an_ambiguous_predecessor_completion_to_a_live_command() {
    let start = Instant::now();
    let (mut engine, _, live) = held_and_live(start);
    let at = start + Duration::from_millis(41);
    engine.handle(complete(EnvelopeKind::Raw, None), at);
    let (write, successor) = admit(
        &mut engine,
        3,
        command(1, CancellationPolicy::Supported),
        at,
    );
    send_ok(&mut engine, &write, None, at);
    let reused = engine.handle(ack(EnvelopeKind::Raw, ViscaSocket::S1), at);
    assert!(
        terminal_id(&reused).is_none(),
        "an ACK proves socket reuse, not whose earlier completion arrived"
    );
    assert!(engine.entry(live).is_some() && engine.entry(successor).is_some());
    assert!(engine
        .correlation
        .learning(camera(1))
        .pending_socketless_completions
        .is_none());
    engine.assert_invariants().unwrap();
}

fn order_snapshot(
    envelope: EnvelopeKind,
    transport: TransportKind,
    completion: bool,
    offset: u64,
    due_first: bool,
) -> (Vec<String>, Vec<String>, String, String, String) {
    let start = Instant::now();
    // One shared epoch makes snapshots directly comparable despite Instant's
    // opaque representation; only relative lifecycle deadlines are projected.
    let mut engine = engine(envelope, transport);
    let (write, id) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        start,
    );
    send_ok(
        &mut engine,
        &write,
        (envelope == EnvelopeKind::Sony).then_some(SEQUENCE),
        start,
    );
    if completion {
        engine.handle(ack(envelope, ViscaSocket::S1), start);
    }
    let late = start
        + Duration::from_millis(if completion { 40 } else { 20 })
        + Duration::from_micros(offset);
    let mut effects = Vec::new();
    if due_first {
        effects.extend(engine.advance_turn(late, EngineTurn::InputOnly));
        effects.extend(due_effects(&mut engine, late));
    }
    let response = if completion {
        DecodedResponse::Completion { socket: None }
    } else {
        DecodedResponse::Ack {
            socket: Some(ViscaSocket::S1),
        }
    };
    effects.extend(engine.handle_turn(answer(envelope, response), late, EngineTurn::InputOnly));
    effects.extend(due_effects(&mut engine, late));
    let outcomes = effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Terminal { id, outcome } => Some(format!("{id:?} {outcome:?}")),
            _ => None,
        })
        .collect();
    let writes = effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Transmit { request, .. } => Some(format!("{request:?}")),
            _ => None,
        })
        .collect();
    let phase = match phase_of(&engine, id) {
        Some(Phase::Executing {
            socket, deadline, ..
        }) => format!("executing {socket:?} {:?}", deadline.duration_since(late)),
        Some(Phase::Backoff { ready_at, .. }) => {
            format!("backoff {:?}", ready_at.duration_since(late))
        }
        other => format!("{other:?}"),
    };
    let holds = engine
        .correlation
        .holds()
        .iter()
        .map(|(key, hold)| {
            format!(
                "{key:?} {:?} {:?}",
                hold.until.duration_since(late),
                hold.owner
            )
        })
        .collect::<Vec<_>>();
    let ledger = engine
        .correlation
        .ledger()
        .entries()
        .map(|(target, entry)| {
            format!(
                "{target:?} {:?} {:?} {}",
                entry.request, entry.owes, entry.members
            )
        })
        .collect::<Vec<_>>();
    engine.assert_invariants().unwrap();
    (
        outcomes,
        writes,
        phase,
        format!("{holds:?}"),
        format!("{ledger:?}"),
    )
}

proptest! {
    #[test]
    fn correlated_frame_and_due_order_agree_on_every_envelope(
        sony in any::<bool>(), stream in any::<bool>(), completion in any::<bool>(), offset in 1u64..5_000,
    ) {
        let envelope = if sony { EnvelopeKind::Sony } else { EnvelopeKind::Raw };
        let transport = if stream { TransportKind::Stream } else { TransportKind::Datagram };
        prop_assert_eq!(order_snapshot(envelope, transport, completion, offset, true), order_snapshot(envelope, transport, completion, offset, false));
    }

    /// Camera truth is recorded before delivering each exact-sequence frame.
    /// Every actual transport write is then checked against that truth,
    /// including writes recursively produced by a result or a due turn.
    #[test]
    fn sony_camera_model_never_rewrites_after_delivered_acceptance(
        events in prop::collection::vec((0u8..5, 0u16..40_000), 1..160),
    ) {
        use std::collections::BTreeSet;
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let mut written = BTreeMap::<RequestId, u32>::new();
        let mut accepted = BTreeSet::<RequestId>::new();
        let mut now = Instant::now();
        let mut ticket = 0;
        for (action, advance) in events {
            now += Duration::from_micros(u64::from(advance));
            let input = match action {
                0 => {
                    ticket += 1;
                    Input::Admit { ticket: AdmissionTicket(ticket), request: command(1, CancellationPolicy::Supported), slot: AdmissionSlot::Ordinary }
                }
                1 => Input::ReceiveFault { error: Error::transport_error(std::io::Error::other("model receive fault")) },
                2 | 3 if !written.is_empty() => {
                    let index = usize::from(advance) % written.len();
                    let (id, sequence) = written.iter().nth(index).unwrap();
                    accepted.insert(*id);
                    frame(1, Some((*sequence, SequenceWidth::Full32)),
                        if action == 2 { DecodedResponse::Ack { socket: Some(ViscaSocket::S1) } }
                        else { DecodedResponse::Completion { socket: Some(ViscaSocket::S1) } })
                }
                _ => Input::Wake,
            };
            let mut effects: VecDeque<_> = engine.handle(input, now).into();
            while let Some(effect) = effects.pop_front() {
                if let Effect::Transmit { transmission, request, kind: Transmission::Request { framing, .. } } = effect {
                    prop_assert!(!accepted.contains(&request), "rewrote {request:?} after camera acceptance");
                    let sequence = *written.entry(request).or_insert_with(|| 0x1000 + u32::try_from(request.get()).unwrap());
                    if let RequestFraming::Sony { sequence: SonyRequestSequence::Replay(requested) } = framing.expectation() {
                        prop_assert_eq!(requested, sequence);
                    }
                    effects.extend(engine.handle_turn(Input::TransmissionFinished {
request, transmission, result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(sequence))) }, now, EngineTurn::InputOnly));
                }
            }
            prop_assert!(engine.assert_invariants().is_ok());
        }
    }
}
