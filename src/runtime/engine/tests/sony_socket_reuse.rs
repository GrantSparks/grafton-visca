//! A Sony sequence retains acceptance after the camera reassigns its socket.
use super::*;

const FIRST: u32 = 0x1001;
const SECOND: u32 = 0x1002;

fn response(sequence: u32, response: DecodedResponse) -> Input {
    frame(1, Some(sequence), response)
}

fn ack(sequence: u32, socket: ViscaSocket) -> Input {
    response(
        sequence,
        DecodedResponse::Ack {
            socket: Some(socket),
        },
    )
}

fn completion(sequence: u32) -> Input {
    response(
        sequence,
        DecodedResponse::Completion {
            socket: Some(ViscaSocket::S1),
        },
    )
}

fn projected_command(state: crate::KnownState) -> RuntimeRequest {
    let mut request = command(1);
    let RuntimeRequest::Command { applied_state, .. } = &mut request else {
        unreachable!()
    };
    *applied_state = Some(AppliedStateProjection::set(state));
    request
}

fn projection(effects: &[Effect], request: RequestId) -> Option<AppliedStateProjection> {
    effects.iter().find_map(|effect| match effect {
        Effect::AppliedState { effect } if effect.request == request => Some(effect.projection),
        _ => None,
    })
}

fn reused_socket(
    start: Instant,
    transport: TransportKind,
) -> (ProtocolEngine, RequestId, RequestId) {
    let mut engine = engine(EnvelopeKind::Sony, transport);
    let first = projected_command(crate::KnownState::TallyBrightness(
        crate::types::TallyBrightness::High,
    ));
    let (first, first_id) = admit(&mut engine, 1, first, start);
    send_ok(&mut engine, &first, Some(FIRST), start);
    engine.handle(ack(FIRST, ViscaSocket::S1), start);
    engine.assert_invariants().unwrap();
    let later = start + Duration::from_millis(10);
    let second = projected_command(crate::KnownState::NdFilterAuto(true));
    let (second, second_id) = admit(&mut engine, 2, second, later);
    send_ok(&mut engine, &second, Some(SECOND), later);
    let reused = engine.handle(ack(SECOND, ViscaSocket::S1), later);
    assert!(ignored_reasons(&reused).is_empty(), "{reused:?}");
    assert!(terminal_id(&reused).is_none());
    assert!(matches!(
        phase_of(&engine, first_id),
        Some(Phase::ExecutingDisplaced { .. })
    ));
    assert_eq!(socket_of(&engine, first_id), None);
    assert_eq!(socket_of(&engine, second_id), Some(ViscaSocket::S1));
    assert_eq!(
        engine.socket_owner(camera(1), ViscaSocket::S1),
        Some(second_id)
    );
    engine.assert_invariants().unwrap();
    (engine, first_id, second_id)
}

fn assert_applied(effects: &[Effect], expected: RequestId) {
    assert!(effects.iter().any(|effect| matches!(effect,
        Effect::Terminal { id, outcome: RuntimeOutcome::Command(CommandOutcome::Applied) } if *id == expected
    )), "{effects:?}");
}

#[test]
fn sony_socket_reuse_preserves_both_sequence_terminals_and_allows_the_other_socket() {
    for transport in [TransportKind::Datagram, TransportKind::Stream] {
        let start = Instant::now();
        let (mut engine, first, second) = reused_socket(start, transport);
        let at = start + Duration::from_millis(11);
        let (third, third_id) = admit(&mut engine, 3, command(1), at);
        assert_eq!(
            request_transmit(&third).1,
            third_id,
            "displaced predecessor owns no capacity"
        );
        send_ok(&mut engine, &third, Some(0x1003), at);
        let accepted = engine.handle(ack(0x1003, ViscaSocket::S2), at);
        assert!(ignored_reasons(&accepted).is_empty());
        assert_eq!(socket_of(&engine, third_id), Some(ViscaSocket::S2));
        engine.assert_invariants().unwrap();
        // The old sequence's late completion cannot release its successor's socket.
        let first_completion = engine.handle(completion(FIRST), at);
        assert_applied(&first_completion, first);
        assert_eq!(
            projection(&first_completion, first),
            Some(AppliedStateProjection::set(
                crate::KnownState::TallyBrightness(crate::types::TallyBrightness::High,)
            ))
        );
        assert_eq!(
            engine.socket_owner(camera(1), ViscaSocket::S1),
            Some(second)
        );
        engine.assert_invariants().unwrap();
        let second_completion = engine.handle(completion(SECOND), at);
        assert_applied(&second_completion, second);
        assert_eq!(
            projection(&second_completion, second),
            Some(AppliedStateProjection::set(
                crate::KnownState::NdFilterAuto(true)
            ))
        );
        assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S1), None);
        assert_eq!(socket_of(&engine, third_id), Some(ViscaSocket::S2));
        engine.assert_invariants().unwrap();
    }
}

#[test]
fn sony_socket_reuse_cancels_only_the_camera_named_successor_socket() {
    let start = Instant::now();
    let (mut engine, first, second) = reused_socket(start, TransportKind::Datagram);
    let at = start + Duration::from_millis(11);
    let refused = engine.handle(engine.cancel_input(first), at);
    assert!(refused.iter().any(|effect| matches!(effect,
        Effect::CancellationObservation { id, observation: CancellationObservation::AttemptFailed(Error::CancellationUnconfirmed) } if *id == first
    )));
    assert!(!refused
        .iter()
        .any(|effect| matches!(effect, Effect::Transmit { .. })));
    assert!(engine.entry(first).is_some());
    let cancelled = engine.handle(engine.cancel_input(second), at);
    let (_, cancelled_id, socket) = cancel_transmit(&cancelled);
    assert_eq!((cancelled_id, socket), (second, ViscaSocket::S1));
    engine.assert_invariants().unwrap();
}

#[test]
fn sony_socket_reuse_retains_the_predecessor_completion_deadline_and_certainty() {
    let start = Instant::now();
    let (mut engine, first, second) = reused_socket(start, TransportKind::Datagram);
    let deadline = start + Duration::from_millis(40);
    let effects = engine.advance(deadline);
    let error = terminal_failure(&effects, first).expect("displaced predecessor times out");
    assert_eq!(
        error.failure_context().unwrap().certainty,
        Certainty::Unconfirmed
    );
    assert_eq!(error.recovery(), crate::error::Recovery::Reconcile);
    assert!(matches!(error, Error::Timeout { .. }));
    assert!(matches!(
        projection(&effects, first),
        Some(AppliedStateProjection::Invalidate { .. })
    ));
    assert_eq!(
        engine.socket_owner(camera(1), ViscaSocket::S1),
        Some(second)
    );
    assert!(engine.entry(second).is_some());
    engine.assert_invariants().unwrap();
}

#[test]
fn sony_displaced_camera_errors_keep_acceptance_instead_of_retrying() {
    let start = Instant::now();
    let (mut engine, first, second) = reused_socket(start, TransportKind::Datagram);
    let effects = engine.handle(
        response(
            FIRST,
            DecodedResponse::Error {
                socket: Some(ViscaSocket::S1),
                code: 0x03.into(),
            },
        ),
        start + Duration::from_millis(11),
    );
    let error = terminal_failure(&effects, first).expect("camera terminal for predecessor");
    assert!(matches!(error, Error::CommandFailedAfterAck { .. }));
    assert_eq!(error.recovery(), crate::error::Recovery::Reconcile);
    assert!(matches!(
        projection(&effects, first),
        Some(AppliedStateProjection::Invalidate { .. })
    ));
    assert!(!effects
        .iter()
        .any(|effect| matches!(effect, Effect::RetryScheduled { .. })));
    assert_eq!(
        engine.socket_owner(camera(1), ViscaSocket::S1),
        Some(second)
    );
    engine.assert_invariants().unwrap();
}

#[test]
fn sony_displaced_late_completion_and_due_order_agree() {
    for transport in [TransportKind::Datagram, TransportKind::Stream] {
        let run = |due_first| {
            let start = Instant::now();
            let (mut engine, first, second) = reused_socket(start, transport);
            let at = start + Duration::from_millis(41);
            let mut effects = Vec::new();
            if due_first {
                effects.extend(engine.advance(at));
            }
            effects.extend(engine.handle(completion(FIRST), at));
            effects.extend(engine.advance(at));
            assert!(matches!(
                terminal_failure(&effects, first),
                Some(Error::Timeout { .. })
            ));
            assert_eq!(
                engine.socket_owner(camera(1), ViscaSocket::S1),
                Some(second)
            );
            engine.assert_invariants().unwrap();
            effects
                .into_iter()
                .filter_map(|effect| match effect {
                    Effect::Terminal { outcome, .. } => Some(format!("{outcome:?}")),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(run(false), run(true));
    }
}

#[test]
fn sony_socket_reuse_resolves_an_open_cancellation_without_losing_the_request_sequence() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (first, first_id) = admit(&mut engine, 1, command(1), start);
    send_ok(&mut engine, &first, Some(FIRST), start);
    engine.handle(ack(FIRST, ViscaSocket::S1), start);
    let cancellation = engine.handle(engine.cancel_input(first_id), start);
    let (transmission, request, _) = cancel_transmit(&cancellation);
    engine.handle(
        Input::TransmissionFinished {
            request,
            transmission,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(0x2001))),
        },
        start,
    );
    let (second, second_id) = admit(&mut engine, 2, command(1), start);
    send_ok(&mut engine, &second, Some(SECOND), start);
    let reused = engine.handle(ack(SECOND, ViscaSocket::S1), start);
    assert!(reused.iter().any(|effect| matches!(
        effect,
        Effect::Transition { id, from, to }
            if *id == first_id
                && matches!(from.phase(), Phase::AwaitingCancellationResolution { .. })
                && matches!(to.phase(), Phase::ExecutingDisplaced { .. })
    )));
    assert!(reused.iter().any(|effect| matches!(effect,
        Effect::CancellationObservation { id, observation: CancellationObservation::AttemptFailed(Error::CancellationUnconfirmed) } if *id == first_id
    )));
    let stale_cancel = engine.handle(
        response(
            0x2001,
            DecodedResponse::Error {
                socket: Some(ViscaSocket::S1),
                code: 0x04.into(),
            },
        ),
        start,
    );
    assert!(terminal_id(&stale_cancel).is_none());
    assert_applied(&engine.handle(completion(FIRST), start), first_id);
    assert_eq!(
        engine.socket_owner(camera(1), ViscaSocket::S1),
        Some(second_id)
    );
    engine.assert_invariants().unwrap();
}

#[test]
fn same_key_setters_keep_admission_order_across_priorities_and_retry_backoff() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let mut first = projected_command(crate::KnownState::NdFilterAuto(false));
    first.context_mut().control.class = ControlClass::Background;
    let mut second = projected_command(crate::KnownState::NdFilterAuto(true));
    second.context_mut().control.class = ControlClass::User;
    let mut pending = [first, second]
        .into_iter()
        .enumerate()
        .map(|(index, request)| {
            admitted(&engine.handle_turn(
                Input::Admit {
                    ticket: AdmissionTicket(index as u64 + 1),
                    request,
                },
                now,
                EngineTurn::InputOnly,
            ))
        });
    let first = pending.next().unwrap();
    let second = pending.next().unwrap();

    let writing = engine.advance(now);
    assert_eq!(request_transmit(&writing).1, first);
    let sent = send_ok(&mut engine, &writing, Some(FIRST), now);
    assert!(request_transmit_optional(&sent).is_none());

    let rejected = engine.handle(
        response(
            FIRST,
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
            },
        ),
        now,
    );
    let (retried, _, ready_at) = retry_scheduled(&rejected).unwrap();
    assert_eq!(retried, first);
    assert!(ready_at > now);
    assert!(projection(&rejected, first).is_none());
    assert!(request_transmit_optional(&rejected).is_none());

    let writing = engine.advance(ready_at);
    assert_eq!(request_transmit(&writing).1, first);
    send_ok(&mut engine, &writing, Some(FIRST), ready_at);
    let accepted = engine.handle(ack(FIRST, ViscaSocket::S1), ready_at);
    assert!(request_transmit_optional(&accepted).is_none());
    let completed = engine.handle(completion(FIRST), ready_at);
    assert_applied(&completed, first);
    assert_eq!(
        projection(&completed, first),
        Some(AppliedStateProjection::set(
            crate::KnownState::NdFilterAuto(false)
        ))
    );
    assert_eq!(request_transmit(&completed).1, second);
    send_ok(&mut engine, &completed, Some(SECOND), ready_at);
    let completed = engine.handle(completion(SECOND), ready_at);
    assert_applied(&completed, second);
    assert_eq!(
        projection(&completed, second),
        Some(AppliedStateProjection::set(
            crate::KnownState::NdFilterAuto(true)
        ))
    );
}

#[test]
fn failed_setter_releases_its_key_with_the_correct_cache_evidence() {
    for acknowledged in [false, true] {
        let now = Instant::now();
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let mut first = projected_command(crate::KnownState::NdFilterAuto(false));
        first.context_mut().retry = RetryPolicy::NEVER;
        let (writing, first) = admit(&mut engine, 1, first, now);
        send_ok(&mut engine, &writing, Some(FIRST), now);
        let socket = acknowledged.then_some(ViscaSocket::S1);
        if let Some(socket) = socket {
            engine.handle(ack(FIRST, socket), now);
        }
        let (pending, second) = admit(
            &mut engine,
            2,
            projected_command(crate::KnownState::NdFilterAuto(true)),
            now,
        );
        assert!(request_transmit_optional(&pending).is_none());

        let failure = engine.handle(
            response(
                FIRST,
                DecodedResponse::Error {
                    socket,
                    code: 0x03.into(),
                },
            ),
            now,
        );
        assert!(matches!(
            terminal_failure(&failure, first),
            Some(Error::CommandBufferFull | Error::CommandFailedAfterAck { .. })
        ));
        assert_eq!(
            projection(&failure, first),
            acknowledged.then_some(AppliedStateProjection::invalidate(
                crate::StateKey::NdFilterAuto
            ))
        );
        assert_eq!(request_transmit(&failure).1, second);

        send_ok(&mut engine, &failure, Some(SECOND), now);
        let completed = engine.handle(completion(SECOND), now);
        assert_applied(&completed, second);
        assert_eq!(
            projection(&completed, second),
            Some(AppliedStateProjection::set(
                crate::KnownState::NdFilterAuto(true)
            ))
        );
    }
}

fn assert_cancel_refused(effects: &[Effect], request: RequestId) {
    assert!(
        effects.iter().any(|effect| matches!(
            effect,
            Effect::CancellationObservation {
                id,
                observation: CancellationObservation::AttemptFailed(Error::CancellationUnconfirmed)
            } if *id == request
        )),
        "{effects:?}"
    );
    assert!(cancel_transmit_optional(effects).is_none(), "{effects:?}");
}

#[derive(Clone, Copy)]
enum AckOrder {
    BeforeSuccessorWrite,
    AfterSuccessorAck,
    BeforeSuccessorAck,
    SuccessorAckPending,
}

#[test]
fn sony_datagram_cancel_requires_a_unique_socket_in_the_camera_occupancy_model() {
    for order in [
        AckOrder::BeforeSuccessorWrite,
        AckOrder::AfterSuccessorAck,
        AckOrder::BeforeSuccessorAck,
        AckOrder::SuccessorAckPending,
    ] {
        for first_socket in [ViscaSocket::S1, ViscaSocket::S2] {
            for second_socket in [ViscaSocket::S1, ViscaSocket::S2] {
                let now = Instant::now();
                let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
                let (write, first) = admit(&mut engine, 1, command(1), now);
                send_ok(&mut engine, &write, Some(FIRST), now);

                // The camera may complete A without the owner seeing its terminal.
                let mut camera_occupants = [None, None];
                camera_occupants[first_socket.as_index()] = Some(first);
                if matches!(
                    order,
                    AckOrder::BeforeSuccessorWrite | AckOrder::SuccessorAckPending
                ) {
                    engine.handle(ack(FIRST, first_socket), now);
                }
                let (write, second) = admit(&mut engine, 2, command(1), now);
                send_ok(&mut engine, &write, Some(SECOND), now);
                camera_occupants[second_socket.as_index()] = Some(second);

                if matches!(order, AckOrder::BeforeSuccessorAck) {
                    engine.handle(ack(FIRST, first_socket), now);
                }
                if !matches!(order, AckOrder::SuccessorAckPending) {
                    engine.handle(ack(SECOND, second_socket), now);
                }
                if matches!(order, AckOrder::AfterSuccessorAck) {
                    engine.handle(ack(FIRST, first_socket), now);
                }
                assert_occupancy_agrees(&engine, now);

                let first_can_cancel = first_socket != second_socket
                    && !matches!(order, AckOrder::SuccessorAckPending);
                let first_effects = engine.handle(engine.cancel_input(first), now);
                if first_can_cancel {
                    let (transmission, id, socket) = cancel_transmit(&first_effects);
                    assert_eq!(camera_occupants[socket.as_index()], Some(id));
                    camera_occupants[socket.as_index()] = None;
                    engine.handle(
                        Input::TransmissionFinished {
                            request: id,
                            transmission,
                            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(
                                0x2001,
                            ))),
                        },
                        now,
                    );
                    engine.handle(
                        response(
                            0x2001,
                            DecodedResponse::Error {
                                socket: Some(socket),
                                code: 0x04.into(),
                            },
                        ),
                        now,
                    );
                } else {
                    assert_cancel_refused(&first_effects, first);
                    assert!(engine.entry(first).is_some());
                }

                if matches!(order, AckOrder::SuccessorAckPending) {
                    engine.handle(ack(SECOND, second_socket), now);
                }
                if first_socket == second_socket
                    && !matches!(
                        order,
                        AckOrder::BeforeSuccessorWrite | AckOrder::SuccessorAckPending
                    )
                {
                    // Same-socket ACKs with overlapping writes do not prove
                    // succession, but together cost only one unit of capacity.
                    assert_eq!(engine.commands_inflight(camera(1)), 1);
                    let second_effects = engine.handle(engine.cancel_input(second), now);
                    assert_cancel_refused(&second_effects, second);
                }
                if !first_can_cancel {
                    let done = engine.handle(
                        response(
                            FIRST,
                            DecodedResponse::Completion {
                                socket: Some(first_socket),
                            },
                        ),
                        now,
                    );
                    assert_applied(&done, first);
                    if camera_occupants[first_socket.as_index()] == Some(first) {
                        camera_occupants[first_socket.as_index()] = None;
                    }
                }

                let second_effects = engine.handle(engine.cancel_input(second), now);
                let (_, id, socket) = cancel_transmit(&second_effects);
                assert_eq!(camera_occupants[socket.as_index()], Some(id));
                assert_eq!((id, socket), (second, second_socket));
            }
        }
    }
}

#[test]
fn sony_datagram_waiting_cancel_rechecks_a_competing_write_after_pacing() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    engine.policy.command_spacing = Duration::from_millis(2);
    engine.targets[camera(1)]
        .policy
        .as_mut()
        .unwrap()
        .pan_tilt_drive_cancellation_ms = std::num::NonZeroU16::new(15);
    let mut drive = command(1);
    drive.context_mut().pan_tilt_drive = true;
    let (write, first) = admit(&mut engine, 1, drive, now);
    send_ok(&mut engine, &write, Some(FIRST), now);
    engine.handle(ack(FIRST, ViscaSocket::S1), now);
    let requested = engine.handle(engine.cancel_input(first), now);
    assert!(cancel_transmit_optional(&requested).is_none());

    let later = now + Duration::from_millis(2);
    let (write, _) = admit(&mut engine, 2, command(1), later);
    send_ok(&mut engine, &write, Some(SECOND), later);
    let refused = engine.advance(now + Duration::from_millis(15));
    assert_cancel_refused(&refused, first);
    assert_applied(
        &engine.handle(completion(FIRST), later + Duration::from_millis(15)),
        first,
    );
}

#[test]
fn sony_datagram_contender_blocks_cancellation_in_backoff_and_after_timeout() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (write, first) = admit(&mut engine, 1, command(1), now);
    send_ok(&mut engine, &write, Some(FIRST), now);
    engine.handle(ack(FIRST, ViscaSocket::S1), now);
    let mut contender = command_with_retry(
        1,
        RetryPolicy {
            max_retries: 1,
            initial_backoff: Duration::from_millis(10),
            maximum_backoff: Duration::from_millis(10),
            total_budget: None,
        },
    );
    let ack_interval = Duration::from_millis(3);
    contender.context_mut().timeout.ack = NonZeroDuration::new(ack_interval).unwrap();
    let (write, second) = admit(&mut engine, 2, contender, now);
    send_ok(&mut engine, &write, Some(SECOND), now);

    let backoff_at = now + ack_interval;
    let effects = engine.advance(backoff_at);
    let (_, _, ready_at) =
        retry_scheduled(&effects).expect("an unacknowledged write waits to retry");
    let effects = engine.handle(engine.cancel_input(first), backoff_at);
    assert_cancel_refused(&effects, first);
    let replay = engine.advance(ready_at);
    assert_eq!(request_transmit(&replay).1, second);
    send_ok(&mut engine, &replay, Some(SECOND), ready_at);
    let timed_out_at = ready_at + ack_interval;
    let effects = engine.advance(timed_out_at);
    assert!(terminal_failure(&effects, second).is_some());
    let effects = engine.handle(engine.cancel_input(first), timed_out_at);
    assert_cancel_refused(&effects, first);
    assert_applied(&engine.handle(completion(FIRST), timed_out_at), first);
}

#[test]
fn sony_datagram_pre_ack_intent_cannot_cancel_an_unassigned_contender() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (writing, first) = admit(&mut engine, 1, command(1), now);
    send_ok(&mut engine, &writing, Some(FIRST), now);
    let requested = engine.handle(engine.cancel_input(first), now);
    assert!(cancel_transmit_optional(&requested).is_none());
    let (writing, second) = admit(&mut engine, 2, command(1), now);
    send_ok(&mut engine, &writing, Some(SECOND), now);

    let acked = engine.handle(ack(FIRST, ViscaSocket::S1), now);
    assert_cancel_refused(&acked, first);
    assert!(engine.entry(first).is_some());
    assert!(engine.entry(second).is_some());
    assert_applied(&engine.handle(completion(FIRST), now), first);
}

#[test]
fn sony_datagram_conclusive_rejection_ends_socket_evidence_until_retry_writes() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (writing, first) = admit(&mut engine, 1, command(1), now);
    send_ok(&mut engine, &writing, Some(FIRST), now);
    let rejection = engine.handle(
        response(
            FIRST,
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
            },
        ),
        now,
    );
    let (_, _, ready_at) = retry_scheduled(&rejection).expect("buffer full is retryable");
    let contrary = engine.handle(ack(FIRST, ViscaSocket::S1), now);
    assert_eq!(
        ignored_reasons(&contrary),
        vec![IgnoreReason::UnmatchedFrame]
    );
    assert!(matches!(
        phase_of(&engine, first),
        Some(Phase::Backoff { .. })
    ));

    let writing = engine.advance(ready_at);
    send_ok(&mut engine, &writing, Some(FIRST), ready_at);
    let accepted = engine.handle(ack(FIRST, ViscaSocket::S1), ready_at);
    assert!(ignored_reasons(&accepted).is_empty());
    assert_eq!(socket_of(&engine, first), Some(ViscaSocket::S1));
}
