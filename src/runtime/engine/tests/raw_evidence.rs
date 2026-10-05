//! Raw named-error evidence, post-ACK replay, the raw byte-stream correlation
//! ledger, and halt-supersession certainty (#795, PTZOptics G2 bench, three
//! firmware builds, 2026-10-04).

use super::*;

/// [`engine`] under a name a test's own `engine` binding does not shadow.
fn fresh(envelope: EnvelopeKind, transport: TransportKind) -> ProtocolEngine {
    engine(envelope, transport)
}

fn named_error(target: u8, socket: ViscaSocket, code: u8) -> Input {
    frame(
        target,
        None,
        DecodedResponse::Error {
            socket: Some(socket),
            code,
        },
    )
}

fn ack(target: u8, socket: ViscaSocket) -> Input {
    frame(
        target,
        None,
        DecodedResponse::Ack {
            socket: Some(socket),
        },
    )
}

fn completion(target: u8, socket: ViscaSocket) -> Input {
    frame(
        target,
        None,
        DecodedResponse::Completion {
            socket: Some(socket),
        },
    )
}

/// A movement-class retry policy without the `0x41` retry, so a camera
/// rejection is observable as a terminal error.
fn no_not_executable_retry() -> RetryPolicy {
    RetryPolicy {
        movement_not_executable: false,
        ..retrying()
    }
}

/// Admits one ordinary raw command on target 1, writes it, and ACKs it on
/// `socket`, leaving it `Executing`.
fn executing_on(
    engine: &mut ProtocolEngine,
    ticket: u64,
    socket: ViscaSocket,
    now: Instant,
) -> RequestId {
    let (effects, id) = admit(
        engine,
        ticket,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(engine, &effects, None, now);
    engine.handle(ack(1, socket), now);
    assert!(
        matches!(phase_of(engine, id), Some(Phase::Executing { socket: owned, .. }) if owned == socket)
    );
    id
}

/// Answers the stream owes `target` for requests that already ended.
fn raw_owed(engine: &ProtocolEngine, target: u8) -> usize {
    usize::try_from(engine.ledger.debts(camera(target))).unwrap()
}

fn command_lane(engine: &ProtocolEngine, target: u8) -> LaneState {
    engine.ledger.lane_state(camera(target), OwedLane::Command)
}

// ---------------------------------------------------------------------------
// Defect 1: a named error on a free, unheld socket.
// ---------------------------------------------------------------------------

/// The G2 rejects a command it cannot start with `90 6y 41 FF`, naming the
/// free socket it allocated. That is exact evidence for the target's unique
/// unacknowledged command, exactly as an ACK naming a free socket is.
#[test]
fn named_error_on_a_free_socket_rejects_the_unique_unacknowledged_command() {
    let now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let (effects, id) = admit(
        &mut engine,
        1,
        command_with_retry(1, no_not_executable_retry()),
        now,
    );
    send_ok(&mut engine, &effects, None, now);

    let rejected = engine.handle(named_error(1, ViscaSocket::S1, 0x41), now);
    assert!(matches!(
        terminal_failure(&rejected, id),
        Some(Error::CommandNotExecutable)
    ));
    assert!(retry_scheduled(&rejected).is_none());
    assert_eq!(raw_owed(&engine, 1), 0, "a rejected command owes nothing");
    engine.assert_invariants().unwrap();

    // A movement-class command keeps its #566 pre-ACK retry: the rejected
    // attempt provably did not start.
    let (effects, id) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &effects, None, now);
    let refused = engine.handle(named_error(1, ViscaSocket::S2, 0x41), now);
    assert_eq!(retry_scheduled(&refused).map(|retry| retry.0), Some(id));
    engine.assert_invariants().unwrap();
}

/// An inquiry is never allocated a socket: it is answered on socket 0
/// (`docs/visca_reference.md` §6.2), so its rejection is socketless (§6.3).
/// A named rejection on a free socket is therefore the command's even while
/// an inquiry is live, on either transport.
#[test]
fn named_error_on_a_free_socket_rejects_the_command_while_an_inquiry_is_live() {
    for transport in [TransportKind::Datagram, TransportKind::Stream] {
        let now = Instant::now();
        let mut engine = fresh(EnvelopeKind::Raw, transport);
        let (inquiry_effects, inquiry_id) = admit(&mut engine, 1, inquiry(1, POWER), now);
        send_ok(&mut engine, &inquiry_effects, None, now);
        let (command_effects, command_id) = admit(
            &mut engine,
            2,
            command_with_retry(1, no_not_executable_retry()),
            now,
        );
        send_ok(&mut engine, &command_effects, None, now);
        let rejected = engine.handle(named_error(1, ViscaSocket::S1, 0x41), now);
        assert!(
            matches!(
                terminal_failure(&rejected, command_id),
                Some(Error::CommandNotExecutable)
            ),
            "{transport:?}"
        );
        assert!(matches!(
            phase_of(&engine, inquiry_id),
            Some(Phase::AwaitingReply { .. })
        ));
        engine.assert_invariants().unwrap();
    }
}

#[test]
fn named_error_on_a_free_socket_is_ignored_when_it_could_belong_to_another_request() {
    let now = Instant::now();

    // Two positional candidates (#714): bind to neither.
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Datagram);
    let (ordinary, ordinary_id) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &ordinary, None, now);
    let (urgent, urgent_id) = admit(
        &mut engine,
        2,
        urgent_command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &urgent, None, now);
    let ignored = engine.handle(named_error(1, ViscaSocket::S2, 0x41), now);
    assert_eq!(
        ignored_reasons(&ignored),
        vec![IgnoreReason::UnmatchedFrame]
    );
    assert!(phase_of(&engine, ordinary_id).is_some() && phase_of(&engine, urgent_id).is_some());
    engine.assert_invariants().unwrap();

    // A socket still quarantined for a predecessor's lost completion is not
    // free evidence.
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Datagram);
    let lost = executing_on(&mut engine, 1, ViscaSocket::S2, now);
    let expired = engine.advance(now + Duration::from_millis(40));
    assert!(matches!(
        terminal_failure(&expired, lost),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    let later = now + Duration::from_millis(41);
    let (successor, successor_id) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        later,
    );
    send_ok(&mut engine, &successor, None, later);
    let ignored = engine.handle(named_error(1, ViscaSocket::S2, 0x41), later);
    assert_eq!(
        ignored_reasons(&ignored),
        vec![IgnoreReason::UnmatchedFrame]
    );
    assert!(matches!(
        phase_of(&engine, successor_id),
        Some(Phase::AwaitingAck { .. })
    ));
    engine.assert_invariants().unwrap();
}

// ---------------------------------------------------------------------------
// Defect 2: busy-socket ambiguity and no replay after ACK.
// ---------------------------------------------------------------------------

#[test]
fn named_error_on_a_busy_socket_is_ambiguous_while_a_command_is_unacknowledged() {
    let now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Datagram);
    let executing = executing_on(&mut engine, 1, ViscaSocket::S1, now);
    let (effects, pending) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &effects, None, now);

    let ambiguous = engine.handle(named_error(1, ViscaSocket::S1, 0x41), now);
    assert_eq!(
        ignored_reasons(&ambiguous),
        vec![IgnoreReason::UnmatchedFrame]
    );
    assert!(terminal_id(&ambiguous).is_none());
    assert!(retry_scheduled(&ambiguous).is_none());
    assert!(
        request_transmit_optional(&ambiguous).is_none(),
        "nothing is replayed"
    );
    assert!(matches!(
        phase_of(&engine, executing),
        Some(Phase::Executing { .. })
    ));
    assert!(matches!(
        phase_of(&engine, pending),
        Some(Phase::AwaitingAck { .. })
    ));

    // `0x04` answers only a cancellation packet; none was emitted for S1.
    let stale = engine.handle(named_error(1, ViscaSocket::S1, 0x04), now);
    assert!(terminal_id(&stale).is_none());
    assert!(matches!(
        phase_of(&engine, executing),
        Some(Phase::Executing { .. })
    ));
    engine.assert_invariants().unwrap();
}

#[test]
fn acknowledged_commands_are_never_rewritten_after_a_rejection() {
    let now = Instant::now();
    for code in [0x41, 0x03] {
        let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Datagram);
        let id = executing_on(&mut engine, 1, ViscaSocket::S1, now);
        let rejected = engine.handle(named_error(1, ViscaSocket::S1, code), now);
        assert!(retry_scheduled(&rejected).is_none(), "{code:#04x}");
        assert!(
            request_transmit_optional(&rejected).is_none(),
            "{code:#04x}"
        );
        assert!(
            matches!(
                terminal_failure(&rejected, id),
                Some(Error::CommandFailedAfterAck { source, .. })
                    if fixture_error_code(&source) == format!("0x{code:02X}")
            ),
            "{code:#04x}: {rejected:?}"
        );
        engine.assert_invariants().unwrap();
    }

    // The Sony envelope's exact sequence does not make a replay safe either.
    let mut engine = fresh(EnvelopeKind::Sony, TransportKind::Datagram);
    let (effects, id) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        now,
    );
    let sequence = sony_sequence(id);
    send_ok(&mut engine, &effects, Some(sequence), now);
    engine.handle(
        frame(
            1,
            Some((sequence, SequenceWidth::Full32)),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    );
    let rejected = engine.handle(
        frame(
            1,
            Some((sequence, SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: Some(ViscaSocket::S1),
                code: 0x41,
            },
        ),
        now,
    );
    assert!(retry_scheduled(&rejected).is_none());
    let error = terminal_failure(&rejected, id).expect("terminal");
    assert!(matches!(
        &error,
        Error::CommandFailedAfterAck { source, .. }
            if matches!(**source, Error::CommandNotExecutable)
    ));
    assert!(!error.is_retryable());
    assert_eq!(
        error.failure_context(),
        Some(FailureContext::new(
            FailureStage::Terminal,
            Certainty::Unconfirmed
        ))
    );
    engine.assert_invariants().unwrap();
}

// ---------------------------------------------------------------------------
// Defect 3: byte-stream answers owed by unconfirmed commands.
// ---------------------------------------------------------------------------

/// A stalled stream delivers the unconfirmed command's ACK and completion
/// after its one ambiguity interval. They are discarded as that command's
/// owed answer and never bind to the STOP written after it.
#[test]
fn stream_owed_ack_never_binds_a_later_command() {
    let now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let (effects, stalled) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &effects, None, now);
    let unconfirmed = engine.advance(now + Duration::from_millis(20));
    assert!(matches!(
        terminal_failure(&unconfirmed, stalled),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert_eq!(raw_owed(&engine, 1), 1);

    // Inside the owing command's ambiguity window ordinary motion waits.
    let later = now + Duration::from_millis(60);
    engine.advance(later);
    let (ordinary, ordinary_id) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        later,
    );
    assert!(request_transmit_optional(&ordinary).is_none());
    // A STOP is still delivered.
    let (urgent, stop) = admit(
        &mut engine,
        3,
        urgent_command(1, CancellationPolicy::Supported),
        later,
    );
    send_ok(&mut engine, &urgent, None, later);
    assert!(matches!(
        phase_of(&engine, stop),
        Some(Phase::AwaitingAck { .. })
    ));

    // The stall lifts: the owed ACK and completion arrive first.
    let stale_ack = engine.handle(ack(1, ViscaSocket::S1), later);
    assert_eq!(
        ignored_reasons(&stale_ack),
        vec![IgnoreReason::UnmatchedFrame]
    );
    assert!(matches!(
        phase_of(&engine, stop),
        Some(Phase::AwaitingAck { .. })
    ));
    assert_eq!(raw_owed(&engine, 1), 0);
    let stale_completion = engine.handle(completion(1, ViscaSocket::S1), later);
    assert!(terminal_id(&stale_completion).is_none());

    // The STOP's own answer binds to it, and ordinary motion resumes.
    let stop_ack = engine.handle(ack(1, ViscaSocket::S2), later);
    assert!(matches!(
        phase_of(&engine, stop),
        Some(Phase::Executing {
            socket: ViscaSocket::S2,
            ..
        })
    ));
    let (_, dispatched, _) = request_transmit(&stop_ack);
    assert_eq!(dispatched, ordinary_id);
    let applied = engine.handle(completion(1, ViscaSocket::S2), later);
    assert!(matches!(
        terminal_outcome(&applied, stop),
        Some(RuntimeOutcome::Applied)
    ));
    engine.assert_invariants().unwrap();
}

/// An owed rejection (`90 6y 41 FF` on a free socket) pays the debt too.
#[test]
fn stream_owed_rejection_settles_the_debt() {
    let now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let (effects, _) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &effects, None, now);
    engine.advance(now + Duration::from_millis(20));
    assert_eq!(raw_owed(&engine, 1), 1);
    let later = now + Duration::from_millis(80);
    // A prepared typed STOP never retries `0x41` (see `prepared.rs`).
    let mut stop_request = urgent_command(1, CancellationPolicy::Supported);
    stop_request.context_mut().retry = no_not_executable_retry();
    let (urgent, stop) = admit(&mut engine, 2, stop_request, later);
    send_ok(&mut engine, &urgent, None, later);

    let owed = engine.handle(named_error(1, ViscaSocket::S1, 0x41), later);
    assert!(terminal_id(&owed).is_none());
    assert_eq!(raw_owed(&engine, 1), 0);
    let own = engine.handle(named_error(1, ViscaSocket::S2, 0x41), later);
    assert!(matches!(
        terminal_failure(&own, stop),
        Some(Error::CommandNotExecutable)
    ));
    engine.assert_invariants().unwrap();
}

/// On a byte stream the #714 two-candidate state is not ambiguous: the
/// earlier-written command's ACK arrives first, so each ACK binds in write
/// order, and nothing is owed.
#[test]
fn stream_order_resolves_two_open_candidates() {
    let now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let (ordinary, ordinary_id) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &ordinary, None, now);
    let (urgent, urgent_id) = admit(
        &mut engine,
        2,
        urgent_command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &urgent, None, now);
    engine.handle(ack(1, ViscaSocket::S1), now);
    assert_eq!(socket_of(&engine, ordinary_id), Some(ViscaSocket::S1));
    assert!(matches!(
        phase_of(&engine, urgent_id),
        Some(Phase::AwaitingAck { .. })
    ));
    engine.handle(ack(1, ViscaSocket::S2), now);
    assert_eq!(socket_of(&engine, urgent_id), Some(ViscaSocket::S2));
    assert_eq!(raw_owed(&engine, 1), 0);
    engine.assert_invariants().unwrap();
}

/// An owed answer that outlives its window latches only the command lane:
/// ordinary commands fail unwritten with `CommandCorrelationLost`, STOPs are
/// still written, and the late answer reopens the lane. The session is never
/// poisoned (the same endgame as an owed inquiry reply).
#[test]
fn stream_owed_answer_past_its_window_latches_only_the_command_lane() {
    let now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let (effects, _) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &effects, None, now);
    let unconfirmed_at = now + Duration::from_millis(20);
    engine.advance(unconfirmed_at);
    assert_eq!(command_lane(&engine, 1), LaneState::Window);
    let window_end = unconfirmed_at + Duration::from_millis(50);
    assert!(engine.next_wake().is_some_and(|wake| wake <= window_end));
    let (queued, queued_id) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        now + Duration::from_millis(30),
    );
    assert!(request_transmit_optional(&queued).is_none());

    let latched = engine.advance(window_end);
    assert_eq!(command_lane(&engine, 1), LaneState::Latched);
    assert_eq!(engine.state(), SessionState::Running);
    assert!(matches!(
        terminal_failure(&latched, queued_id),
        Some(Error::CommandCorrelationLost { camera: c }) if c == camera(1)
    ));
    let rejected = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: command(1, CancellationPolicy::Supported),
            slot: AdmissionSlot::Ordinary,
        },
        window_end,
    );
    assert!(rejected.iter().any(|effect| matches!(
        effect,
        Effect::AdmissionRejected {
            error: Error::CommandCorrelationLost { .. },
            ..
        }
    )));
    // Another camera, and a STOP to this one, are unaffected.
    let (other, other_id) = admit(
        &mut engine,
        4,
        command(2, CancellationPolicy::Supported),
        window_end,
    );
    assert_eq!(request_transmit(&other).1, other_id);
    let mut stop_request = urgent_command(1, CancellationPolicy::Supported);
    stop_request.context_mut().retry = no_not_executable_retry();
    let (stop_effects, stop) = admit(&mut engine, 5, stop_request, window_end);
    assert_eq!(request_transmit(&stop_effects).1, stop);
    send_ok(&mut engine, &stop_effects, None, window_end);
    // A latched lane neither wakes the owner nor expires.
    assert!(engine.ledger.wake().is_none());

    // The stall lifts: the owed ACK arrives first and reopens the lane; the
    // STOP's own ACK then binds to it.
    // (Before the STOP's own 20 ms ACK deadline.)
    let later = window_end + Duration::from_millis(10);
    engine.handle(ack(1, ViscaSocket::S1), later);
    assert_eq!(command_lane(&engine, 1), LaneState::Clear);
    engine.handle(ack(1, ViscaSocket::S2), later);
    assert!(matches!(
        phase_of(&engine, stop),
        Some(Phase::Executing {
            socket: ViscaSocket::S2,
            ..
        })
    ));
    engine.assert_invariants().unwrap();
}

#[test]
fn datagram_unconfirmed_command_owes_nothing() {
    let now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Datagram);
    let (effects, _) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &effects, None, now);
    engine.advance(now + Duration::from_millis(20));
    assert_eq!(raw_owed(&engine, 1), 0);
    let later = now + Duration::from_millis(80);
    engine.advance(later);
    let (ordinary, ordinary_id) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        later,
    );
    assert_eq!(request_transmit(&ordinary).1, ordinary_id);
    engine.assert_invariants().unwrap();
}

// ---------------------------------------------------------------------------
// Defect 4: halt supersession certainty.
// ---------------------------------------------------------------------------

fn superseded(error: Option<Error>) -> FailureContext {
    match error {
        Some(error @ Error::MotionSuperseded { .. }) => error.failure_context().unwrap(),
        other => panic!("expected MotionSuperseded, got {other:?}"),
    }
}

#[test]
fn halt_superseding_never_written_motion_reports_not_accepted() {
    let now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Datagram);
    let (blocking, _) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &blocking, None, now);
    let (queued, queued_id) = admit(
        &mut engine,
        2,
        declared_motion(1, crate::AffectedAxes::ZOOM, 1),
        now,
    );
    assert!(request_transmit_optional(&queued).is_none());

    let halted = engine.halt(camera(1), crate::AffectedAxes::ZOOM, 2);
    assert_eq!(
        superseded(terminal_failure(&halted, queued_id)),
        FailureContext::new(FailureStage::Terminal, Certainty::NotAccepted)
    );

    // A submission ordered before the halt but admitted after it never
    // became a request at all.
    let rejected = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: declared_motion(1, crate::AffectedAxes::ZOOM, 2),
            slot: AdmissionSlot::Ordinary,
        },
        now,
    );
    let error = rejected.iter().find_map(|effect| match effect {
        Effect::AdmissionRejected { error, .. } => Some(error.clone()),
        _ => None,
    });
    assert_eq!(
        superseded(error),
        FailureContext::new(FailureStage::PreAdmission, Certainty::NotAccepted)
    );
    engine.assert_invariants().unwrap();
}

/// Every written attempt was conclusively rejected before ACK, so the
/// superseded request had no effect.
#[test]
fn halt_after_conclusive_rejections_reports_not_accepted() {
    let now = Instant::now();

    // Superseded while waiting in backoff.
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Datagram);
    let (effects, id) = admit(
        &mut engine,
        1,
        declared_motion(1, crate::AffectedAxes::PAN_TILT, 1),
        now,
    );
    send_ok(&mut engine, &effects, None, now);
    let rejected = engine.handle(
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
    assert_eq!(retry_scheduled(&rejected).map(|retry| retry.0), Some(id));
    let halted = engine.halt(camera(1), crate::AffectedAxes::PAN_TILT, 2);
    assert_eq!(
        superseded(terminal_failure(&halted, id)),
        FailureContext::new(FailureStage::Terminal, Certainty::NotAccepted)
    );
    engine.assert_invariants().unwrap();

    // Written before the halt, then rejected: the retry is suppressed.
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Datagram);
    let (effects, id) = admit(
        &mut engine,
        1,
        declared_motion(1, crate::AffectedAxes::PAN_TILT, 1),
        now,
    );
    send_ok(&mut engine, &effects, None, now);
    assert!(engine
        .halt(camera(1), crate::AffectedAxes::PAN_TILT, 2)
        .is_empty());
    let rejected = engine.handle(
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
    assert!(retry_scheduled(&rejected).is_none());
    assert_eq!(
        superseded(terminal_failure(&rejected, id)),
        FailureContext::new(FailureStage::Terminal, Certainty::NotAccepted)
    );
    engine.assert_invariants().unwrap();
}

/// A lost ACK leaves the attempt possibly applied, and that stays true even
/// after a later attempt is conclusively rejected.
#[test]
fn halt_after_a_possibly_applied_attempt_reports_unconfirmed() {
    let now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Sony, TransportKind::Datagram);
    let (effects, id) = admit(
        &mut engine,
        1,
        declared_motion(1, crate::AffectedAxes::PAN_TILT, 1),
        now,
    );
    let (_, _, retry_at) = ack_timeout_retry(&mut engine, &effects, now);
    let resent = engine.advance(retry_at);
    let (transmission, resent_id, _) = request_transmit(&resent);
    assert_eq!(resent_id, id);
    engine.handle(
        Input::TransmissionFinished {
            transmission,
            result: Ok(TransmissionMeta {
                sequence: Some(sony_sequence(id)),
            }),
        },
        retry_at,
    );
    let rejected = engine.handle(
        frame(
            1,
            Some((sony_sequence(id), SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: None,
                code: 0x03,
            },
        ),
        retry_at,
    );
    assert_eq!(retry_scheduled(&rejected).map(|retry| retry.0), Some(id));
    let halted = engine.halt(camera(1), crate::AffectedAxes::PAN_TILT, 2);
    assert_eq!(
        superseded(terminal_failure(&halted, id)),
        FailureContext::new(FailureStage::Terminal, Certainty::Unconfirmed)
    );
    engine.assert_invariants().unwrap();
}

// ---------------------------------------------------------------------------
// One owed-answer model: completion-only commands, socketless completions,
// and the inquiry half (#795).
// ---------------------------------------------------------------------------

fn socketless_completion(target: u8) -> Input {
    frame(target, None, DecodedResponse::Completion { socket: None })
}

fn socketless_error(target: u8, code: u8) -> Input {
    frame(target, None, DecodedResponse::Error { socket: None, code })
}

fn stop_command(target: u8) -> RuntimeRequest {
    let mut request = urgent_command(target, CancellationPolicy::Supported);
    request.context_mut().retry = no_not_executable_retry();
    request
}

/// A `CompletionOnly` command that ends unconfirmed on a stream owes its
/// completion and, until then, keeps its target exclusive: ordinary commands
/// and inquiries wait, and past its window both lanes latch (STOPs still
/// flow). The late completion is discarded and reopens them.
#[test]
fn stream_owed_completion_only_answer_holds_back_ordinary_work() {
    let now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let (effects, id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
        now,
    );
    send_ok(&mut engine, &effects, None, now);
    let expired = engine.advance(now + Duration::from_millis(40));
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert_eq!(raw_owed(&engine, 1), 1);
    assert_eq!(command_lane(&engine, 1), LaneState::Window);
    assert_eq!(inquiry_lane(&engine, 1), LaneState::Window);

    let window_end = now + Duration::from_millis(90);
    engine.advance(window_end);
    assert_eq!(command_lane(&engine, 1), LaneState::Latched);
    assert_eq!(inquiry_lane(&engine, 1), LaneState::Latched);
    for (ticket, request, inquiry_lost) in [
        (2, command(1, CancellationPolicy::Supported), false),
        (
            3,
            command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
            true,
        ),
        (6, inquiry(1, POWER), true),
    ] {
        let rejected = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(ticket),
                request,
                slot: AdmissionSlot::Ordinary,
            },
            window_end,
        );
        assert!(rejected.iter().any(|effect| match effect {
            Effect::AdmissionRejected { error, .. } if inquiry_lost =>
                matches!(error, Error::InquiryCorrelationLost { .. }),
            Effect::AdmissionRejected { error, .. } =>
                matches!(error, Error::CommandCorrelationLost { .. }),
            _ => false,
        }));
    }
    let (stop_effects, stop) = admit(&mut engine, 4, stop_command(1), window_end);
    assert_eq!(request_transmit(&stop_effects).1, stop);
    send_ok(&mut engine, &stop_effects, None, window_end);

    // The late completion (no socket nibble, no live socket holder) is the
    // owed one: it never completes the STOP.
    let late = engine.handle(socketless_completion(1), window_end);
    assert!(terminal_id(&late).is_none());
    assert_eq!(command_lane(&engine, 1), LaneState::Clear);
    assert_eq!(inquiry_lane(&engine, 1), LaneState::Clear);
    assert!(matches!(
        phase_of(&engine, stop),
        Some(Phase::AwaitingAck { .. })
    ));
    engine.handle(ack(1, ViscaSocket::S1), window_end);
    assert!(matches!(
        phase_of(&engine, stop),
        Some(Phase::Executing { .. })
    ));
    engine.assert_invariants().unwrap();
}

/// Once an owed ACK arrives, its command executes on the camera. A camera
/// that omits the socket nibble would otherwise complete a live STOP with that
/// command's late `90 50 FF`.
#[test]
fn socketless_late_completion_never_completes_a_live_command() {
    let now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let (effects, _) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &effects, None, now);
    engine.advance(now + Duration::from_millis(20));
    let later = now + Duration::from_millis(30);
    let (stop_effects, stop) = admit(&mut engine, 2, stop_command(1), later);
    send_ok(&mut engine, &stop_effects, None, later);

    // A camera that omits the socket nibble: its ACKs name no socket.
    let socketless_ack = || frame(1, None, DecodedResponse::Ack { socket: None });
    engine.handle(socketless_ack(), later);
    assert!(engine
        .raw_hold(camera(1), RawHoldScope::Socket(ViscaSocket::S1))
        .is_some());
    engine.handle(socketless_ack(), later);
    assert!(matches!(
        phase_of(&engine, stop),
        Some(Phase::Executing {
            socket: ViscaSocket::S2,
            ..
        })
    ));
    let ambiguous = engine.handle(socketless_completion(1), later);
    assert!(
        terminal_id(&ambiguous).is_none(),
        "a socketless completion is ambiguous while the owed command executes"
    );
    let applied = engine.handle(completion(1, ViscaSocket::S2), later);
    assert!(matches!(
        terminal_outcome(&applied, stop),
        Some(RuntimeOutcome::Applied)
    ));
    // With no live socket holder the socketless completion is the owed one.
    let settled = engine.handle(socketless_completion(1), later);
    assert!(terminal_id(&settled).is_none());
    assert!(engine
        .raw_hold(camera(1), RawHoldScope::Socket(ViscaSocket::S1))
        .is_none());
    engine.assert_invariants().unwrap();
}

/// A named error is never an inquiry's (an inquiry answers on socket 0,
/// `docs/visca_reference.md` §6.2–6.3), so an error naming an executing
/// command's socket is that command's execution error even while an inquiry
/// is live, on either transport; the inquiry keeps waiting for its reply.
#[test]
fn an_error_naming_a_busy_socket_is_its_commands_while_an_inquiry_is_live() {
    for transport in [TransportKind::Datagram, TransportKind::Stream] {
        let now = Instant::now();
        let mut engine = fresh(EnvelopeKind::Raw, transport);
        let executing = executing_on(&mut engine, 1, ViscaSocket::S1, now);
        let (inquiry_effects, inquiry_id) = admit(&mut engine, 2, inquiry(1, POWER), now);
        send_ok(&mut engine, &inquiry_effects, None, now);
        let failed = engine.handle(named_error(1, ViscaSocket::S1, 0x41), now);
        assert!(
            matches!(
                terminal_failure(&failed, executing),
                Some(Error::CommandFailedAfterAck { source, .. })
                    if matches!(*source, Error::CommandNotExecutable)
            ),
            "{transport:?}"
        );
        assert!(matches!(
            phase_of(&engine, inquiry_id),
            Some(Phase::AwaitingReply { .. })
        ));
        engine.assert_invariants().unwrap();
    }
}

/// An owed inquiry reply, an owed command answer, and a live STOP on one
/// target, written in that order: every frame binds in write order to the
/// oldest entry of its class, so each debt is paid by its own answer and the
/// STOP's answer reaches only the STOP.
#[test]
fn stream_order_settles_debts_across_lanes() {
    let start = Instant::now();
    let mut engine = single_flight_raw_stream_engine();
    let (_, timeout_at) = time_out_stream_inquiry(&mut engine, start);
    let (effects, _) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        timeout_at,
    );
    send_ok(&mut engine, &effects, None, timeout_at);
    let unconfirmed_at = timeout_at + Duration::from_millis(20);
    engine.advance(unconfirmed_at);
    assert_eq!(raw_owed(&engine, 1), 2);
    assert_eq!(owed_state(&engine, 1), Some(LaneState::Window));
    assert_eq!(command_lane(&engine, 1), LaneState::Window);

    let (stop_effects, stop) = admit(&mut engine, 3, stop_command(1), unconfirmed_at);
    send_ok(&mut engine, &stop_effects, None, unconfirmed_at);
    // The oldest entry is the inquiry: a socketless error is its rejection.
    let reply = engine.handle(socketless_error(1, 0x41), unconfirmed_at);
    assert!(terminal_id(&reply).is_none());
    assert_eq!(owed_state(&engine, 1), Some(LaneState::Clear));
    // A named rejection is a command's: the owed command came first.
    let owed = engine.handle(named_error(1, ViscaSocket::S1, 0x41), unconfirmed_at);
    assert!(terminal_id(&owed).is_none());
    assert_eq!(command_lane(&engine, 1), LaneState::Clear);
    assert_eq!(raw_owed(&engine, 1), 0);
    engine.handle(ack(1, ViscaSocket::S2), unconfirmed_at);
    assert!(matches!(
        phase_of(&engine, stop),
        Some(Phase::Executing {
            socket: ViscaSocket::S2,
            ..
        })
    ));
    engine.assert_invariants().unwrap();
}

/// A latched inquiry lane's owed reply was written before any later command,
/// so a socketless error is that reply's late answer: it settles the inquiry
/// lane, and the command's own ACK still binds to the command.
#[test]
fn a_latched_owed_reply_is_settled_by_stream_order_before_a_later_command() {
    let start = Instant::now();
    let mut engine = single_flight_raw_stream_engine();
    let (_, timeout_at) = time_out_stream_inquiry(&mut engine, start);
    let window_end = timeout_at + Duration::from_millis(50);
    engine.advance(window_end);
    assert_eq!(owed_state(&engine, 1), Some(LaneState::Latched));

    let (effects, id) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        window_end,
    );
    send_ok(&mut engine, &effects, None, window_end);
    let late = engine.handle(socketless_error(1, 0x02), window_end);
    assert!(terminal_id(&late).is_none());
    assert_eq!(owed_state(&engine, 1), Some(LaneState::Clear));
    engine.handle(ack(1, ViscaSocket::S1), window_end);
    assert_eq!(socket_of(&engine, id), Some(ViscaSocket::S1));
    assert_eq!(command_lane(&engine, 1), LaneState::Clear);
    engine.assert_invariants().unwrap();
}

// ---------------------------------------------------------------------------
// Review findings on the raw byte-stream ledger (#795).
// ---------------------------------------------------------------------------

/// Executes a command on `socket` and emits and writes its cancellation.
fn executing_with_cancel(
    engine: &mut ProtocolEngine,
    ticket: u64,
    socket: ViscaSocket,
    now: Instant,
) -> RequestId {
    let id = executing_on(engine, ticket, socket, now);
    let cancel = engine.handle(Input::Cancel { id }, now);
    let (transmission, _, cancelled) = cancel_transmit(&cancel);
    assert_eq!(cancelled, socket);
    engine.handle(
        Input::TransmissionFinished {
            transmission,
            result: Ok(TransmissionMeta { sequence: None }),
        },
        now,
    );
    id
}

fn command_awaiting_ack(engine: &mut ProtocolEngine, ticket: u64, now: Instant) -> RequestId {
    let (effects, id) = admit(
        engine,
        ticket,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(engine, &effects, None, now);
    assert!(matches!(
        phase_of(engine, id),
        Some(Phase::AwaitingAck { .. })
    ));
    id
}

/// H1: a cancellation's late `0x04`/`0x05` on a now-free socket answers that
/// cancellation only. It never rejects (and so never retries or fails) a
/// command written afterwards.
#[test]
fn cancellation_reply_on_a_free_socket_never_reaches_another_command() {
    for transport in [TransportKind::Datagram, TransportKind::Stream] {
        let now = Instant::now();
        let mut engine = fresh(EnvelopeKind::Raw, transport);
        let cancelled = executing_with_cancel(&mut engine, 1, ViscaSocket::S1, now);
        let completed = engine.handle(completion(1, ViscaSocket::S1), now);
        assert!(
            matches!(
                terminal_outcome(&completed, cancelled),
                Some(RuntimeOutcome::Applied)
            ),
            "{transport:?}"
        );
        let next = command_awaiting_ack(&mut engine, 2, now);
        for code in [0x05, 0x04] {
            let stale = engine.handle(named_error(1, ViscaSocket::S1, code), now);
            assert!(terminal_id(&stale).is_none(), "{transport:?} {code:#04x}");
            assert!(
                retry_scheduled(&stale).is_none(),
                "{transport:?} {code:#04x}"
            );
            assert!(
                matches!(phase_of(&engine, next), Some(Phase::AwaitingAck { .. })),
                "{transport:?} {code:#04x}"
            );
        }
        engine.handle(ack(1, ViscaSocket::S2), now);
        assert_eq!(
            socket_of(&engine, next),
            Some(ViscaSocket::S2),
            "{transport:?}"
        );
        engine.assert_invariants().unwrap();
    }
}

/// H1: a `0x04`/`0x05` naming an executing command's socket answers only a
/// cancellation emitted for it; none was, so it is never a false
/// `CommandCanceled`.
#[test]
fn stale_cancellation_reply_never_fails_a_live_owner() {
    for transport in [TransportKind::Datagram, TransportKind::Stream] {
        let now = Instant::now();
        let mut engine = fresh(EnvelopeKind::Raw, transport);
        let owner = executing_on(&mut engine, 1, ViscaSocket::S1, now);
        for code in [0x04, 0x05] {
            let stale = engine.handle(named_error(1, ViscaSocket::S1, code), now);
            assert!(terminal_id(&stale).is_none(), "{transport:?} {code:#04x}");
        }
        assert_eq!(socket_of(&engine, owner), Some(ViscaSocket::S1));
        engine.assert_invariants().unwrap();
    }
}

/// H2: frames that are not a first answer never pay a debt: an error naming
/// an executing command's socket is that command's execution error, and a
/// cancellation reply answers only a cancellation. The owed ACK, when it
/// finally arrives, still pays the debt, and a later command binds only its
/// own answer.
#[test]
fn non_first_answers_never_pay_a_stream_debt() {
    let now = Instant::now();
    let mut engine = numbering_camera(now);
    let executing = executing_on(&mut engine, 1, ViscaSocket::S2, now);
    command_awaiting_ack(&mut engine, 2, now);
    engine.advance(now + Duration::from_millis(20));
    assert_eq!(raw_owed(&engine, 1), 1);

    let noise_at = now + Duration::from_millis(21);
    let cancel_reply = engine.handle_turn(
        named_error(1, ViscaSocket::S1, 0x05),
        noise_at,
        EngineTurn::INPUT_ONLY,
    );
    assert!(terminal_id(&cancel_reply).is_none());
    assert_eq!(raw_owed(&engine, 1), 1);
    let execution_error = engine.handle_turn(
        named_error(1, ViscaSocket::S2, 0x41),
        noise_at,
        EngineTurn::INPUT_ONLY,
    );
    assert!(matches!(
        terminal_failure(&execution_error, executing),
        Some(Error::CommandFailedAfterAck { .. })
    ));
    assert_eq!(raw_owed(&engine, 1), 1);

    engine.handle_turn(ack(1, ViscaSocket::S1), noise_at, EngineTurn::INPUT_ONLY);
    assert_eq!(raw_owed(&engine, 1), 0);
    let later = command_awaiting_ack(&mut engine, 3, noise_at);
    engine.handle(ack(1, ViscaSocket::S2), noise_at);
    assert_eq!(socket_of(&engine, later), Some(ViscaSocket::S2));
    engine.assert_invariants().unwrap();
}

/// F2: whether a camera names its sockets in completions is learned from its
/// completions. Until it is, the engine cannot know an executing command's
/// socket is still busy, so a rejection code naming it that a waiting command
/// could have earned disputes that command instead of failing the owner. A
/// completion naming its socket teaches it; a socketless completion with no
/// `CompletionOnly` command outstanding proves the camera omits the nibble,
/// for the rest of the session.
#[test]
fn completion_socket_naming_is_learned_from_completions() {
    let now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    assert_eq!(engine.completion_sockets[1], CompletionSockets::Unknown);
    let owner = executing_on(&mut engine, 1, ViscaSocket::S2, now);
    let waiting = command_awaiting_ack(&mut engine, 2, now);
    let disputed = engine.handle(named_error(1, ViscaSocket::S2, 0x41), now);
    assert!(terminal_id(&disputed).is_none());
    assert_eq!(socket_of(&engine, owner), Some(ViscaSocket::S2));
    assert!(engine
        .ledger
        .entries()
        .any(|(_, entry)| entry.request == waiting && entry.disputed));
    engine.handle(completion(1, ViscaSocket::S2), now);
    assert_eq!(engine.completion_sockets[1], CompletionSockets::Named);

    let mut engine = numbering_camera(now);
    let socketless = engine.handle(socketless_completion(1), now);
    assert!(terminal_id(&socketless).is_none());
    assert_eq!(engine.completion_sockets[1], CompletionSockets::Omitted);
    let id = executing_on(&mut engine, 2, ViscaSocket::S1, now);
    engine.handle(completion(1, ViscaSocket::S1), now);
    assert!(phase_of(&engine, id).is_none());
    assert_eq!(
        engine.completion_sockets[1],
        CompletionSockets::Omitted,
        "permanent for the session"
    );
    engine.assert_invariants().unwrap();
}

/// A socketless error is a rejection (an execution error names its socket),
/// so it is the oldest outstanding request's first answer even while another
/// command executes. Discarding it instead would leave a phantom debt that
/// shifts every later answer onto the wrong request.
#[test]
fn a_socketless_error_pays_the_oldest_debt_while_a_command_executes() {
    let now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let executing = executing_on(&mut engine, 1, ViscaSocket::S2, now);
    command_awaiting_ack(&mut engine, 2, now);
    engine.advance(now + Duration::from_millis(20));
    let paid = engine.handle(socketless_error(1, 0x41), now + Duration::from_millis(21));
    assert!(terminal_id(&paid).is_none());
    assert_eq!(raw_owed(&engine, 1), 0);
    assert_eq!(socket_of(&engine, executing), Some(ViscaSocket::S2));
    let later = command_awaiting_ack(&mut engine, 3, now + Duration::from_millis(21));
    engine.handle(ack(1, ViscaSocket::S1), now + Duration::from_millis(21));
    assert_eq!(socket_of(&engine, later), Some(ViscaSocket::S1));
    engine.assert_invariants().unwrap();
}

/// H2: an inquiry reply answers only an inquiry. A live inquiry written
/// before an owed command gets its own reply; an owed reply is paid by a
/// later reply; neither touches the command's debt.
#[test]
fn inquiry_replies_answer_only_inquiry_entries() {
    let start = Instant::now();
    let mut engine = single_flight_raw_stream_engine();
    let (send, inquiry_id) = admit(&mut engine, 1, inquiry(1, POWER), start);
    send_ok(&mut engine, &send, None, start);
    command_awaiting_ack(&mut engine, 2, start);
    engine.advance(start + Duration::from_millis(20));
    assert_eq!(raw_owed(&engine, 1), 1);
    let reply = engine.handle(unkeyed_reply(1, 0x07), start + Duration::from_millis(21));
    assert!(matches!(
        terminal_outcome(&reply, inquiry_id),
        Some(RuntimeOutcome::Reply { payload, .. }) if payload.as_slice() == [0x07]
    ));
    assert_eq!(raw_owed(&engine, 1), 1);
    assert_eq!(command_lane(&engine, 1), LaneState::Window);

    // An owed reply and an owed command answer: the reply pays only the
    // inquiry's debt.
    let mut engine = single_flight_raw_stream_engine();
    let (_, timeout_at) = time_out_stream_inquiry(&mut engine, start);
    command_awaiting_ack(&mut engine, 2, timeout_at);
    engine.advance(timeout_at + Duration::from_millis(20));
    assert_eq!(raw_owed(&engine, 1), 2);
    engine.handle(
        unkeyed_reply(1, 0x02),
        timeout_at + Duration::from_millis(21),
    );
    assert_eq!(owed_state(&engine, 1), Some(LaneState::Clear));
    assert_eq!(raw_owed(&engine, 1), 1);
    assert_eq!(command_lane(&engine, 1), LaneState::Window);
    engine.assert_invariants().unwrap();
}

/// M1(a): a request whose staged write never left owes nothing; one whose
/// write result proves the bytes left after its budget owes its answer.
#[test]
fn only_a_write_that_left_owes_its_answer() {
    let now = Instant::now();
    let budgeted = || command_with_retry(1, immediate_retry_budget(Duration::from_millis(10)));

    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let (staged, id) = admit(&mut engine, 1, budgeted(), now);
    assert!(request_transmit_optional(&staged).is_some());
    let expired = engine.advance(now + Duration::from_millis(10));
    assert!(terminal_failure(&expired, id).is_some());
    assert_eq!(raw_owed(&engine, 1), 0);
    assert_eq!(command_lane(&engine, 1), LaneState::Clear);
    engine.assert_invariants().unwrap();

    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let (staged, id) = admit(&mut engine, 1, budgeted(), now);
    let (transmission, _, _) = request_transmit(&staged);
    let late = engine.handle(
        Input::TransmissionFinished {
            transmission,
            result: Ok(TransmissionMeta { sequence: None }),
        },
        now + Duration::from_millis(11),
    );
    assert!(matches!(
        terminal_failure(&late, id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert_eq!(raw_owed(&engine, 1), 1);
    engine.assert_invariants().unwrap();
}

/// Times out a written `CompletionOnly` command on target 1, leaving its
/// answer owed, and returns the instant after its window.
fn owed_completion_only(engine: &mut ProtocolEngine, now: Instant) -> Instant {
    let (effects, completion_only) = admit(
        engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
        now,
    );
    send_ok(engine, &effects, None, now);
    let expired = engine.advance(now + Duration::from_millis(40));
    assert!(terminal_failure(&expired, completion_only).is_some());
    let later = now + Duration::from_millis(91);
    engine.advance(later);
    later
}

/// A completed command on target 1, so the engine has seen it name its
/// sockets.
fn numbering_camera(now: Instant) -> ProtocolEngine {
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let id = executing_on(&mut engine, 100, ViscaSocket::S1, now);
    let done = engine.handle(completion(1, ViscaSocket::S1), now);
    assert_eq!(terminal_id(&done), Some(id));
    engine
}

/// Writes a `CompletionOnly` command, then a STOP behind it, and delivers a
/// socketless `0x41` either could have sent. It binds to neither; returns the
/// two and the inquiry queued behind the `CompletionOnly` command.
fn disputed(engine: &mut ProtocolEngine, now: Instant) -> (RequestId, RequestId, RequestId) {
    let (effects, completion_only) = admit(
        engine,
        1,
        command_with_reply_shape_and_retry(
            1,
            CancellationPolicy::Supported,
            ReplyShape::CompletionOnly,
            no_not_executable_retry(),
        ),
        now,
    );
    send_ok(engine, &effects, None, now);
    // A STOP is never held back by it; ordinary work is.
    let (queued, inquiry_id) = admit(engine, 2, inquiry(1, POWER), now);
    assert!(request_transmit_optional(&queued).is_none());
    let (stop_effects, stop) = admit(engine, 3, stop_command(1), now);
    assert_eq!(request_transmit(&stop_effects).1, stop);
    send_ok(engine, &stop_effects, None, now);
    let disputed = engine.handle_turn(socketless_error(1, 0x41), now, EngineTurn::INPUT_ONLY);
    assert!(terminal_id(&disputed).is_none() && retry_scheduled(&disputed).is_none());
    assert!(engine
        .ledger
        .entries()
        .any(|(_, entry)| entry.owes == Owes::AcceptedCompletion));
    (completion_only, stop, inquiry_id)
}

/// M1(c): the STOP's own ACK settles the dispute. While it is live nothing
/// crosses it, so its ACK is decisive: the error was the `CompletionOnly`
/// command's rejection, which it now receives. Nothing stays owed, and no
/// lane latches.
#[test]
fn a_disputed_stop_is_settled_by_its_own_ack() {
    let now = Instant::now();
    let mut engine = numbering_camera(now);
    let (completion_only, stop, inquiry_id) = disputed(&mut engine, now);
    // Nothing crosses the disputed STOP.
    assert!(engine.ledger.forbids_crossing(camera(1)));
    let (queued, _) = admit(&mut engine, 4, stop_command(1), now);
    assert!(request_transmit_optional(&queued).is_none());
    let proven = engine.handle(ack(1, ViscaSocket::S2), now);
    assert_eq!(socket_of(&engine, stop), Some(ViscaSocket::S2));
    assert!(matches!(
        terminal_failure(&proven, completion_only),
        Some(Error::CommandNotExecutable)
    ));
    assert_eq!(raw_owed(&engine, 1), 0);
    assert_eq!(command_lane(&engine, 1), LaneState::Clear);
    assert!(!engine.ledger.forbids_crossing(camera(1)));
    // The work queued behind the dispute is written again.
    for _ in 0..2 {
        let next = engine.advance(now);
        if request_transmit_optional(&next).is_some() {
            send_ok(&mut engine, &next, None, now);
        }
    }
    assert!(phase_of(&engine, inquiry_id).is_none_or(|phase| !matches!(phase, Phase::Ready { .. })));
    engine.assert_invariants().unwrap();
}

/// M1(c): the `CompletionOnly` command's completion settles the dispute: it
/// was accepted, so the error was the STOP's rejection, which the STOP now
/// receives.
#[test]
fn a_disputed_completion_only_command_is_settled_by_its_completion() {
    let now = Instant::now();
    let mut engine = numbering_camera(now);
    let (completion_only, stop, _) = disputed(&mut engine, now);
    let done = engine.handle(socketless_completion(1), now);
    assert!(matches!(
        terminal_outcome(&done, completion_only),
        Some(RuntimeOutcome::Applied)
    ));
    assert!(matches!(
        terminal_failure(&done, stop),
        Some(Error::CommandNotExecutable)
    ));
    assert_eq!(raw_owed(&engine, 1), 0);
    engine.assert_invariants().unwrap();
}

/// R2 for `CompletionOnly`: once a later request's first answer arrives, a
/// rejection of the earlier `CompletionOnly` command would have come first,
/// so it owes only its completion. An error after that is never ambiguous,
/// and the late completion still pays the debt.
#[test]
fn a_later_first_answer_proves_a_completion_only_debt_accepted() {
    let now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let later = owed_completion_only(&mut engine, now);
    let (stop_effects, stop) = admit(&mut engine, 2, stop_command(1), later);
    send_ok(&mut engine, &stop_effects, None, later);
    engine.handle(ack(1, ViscaSocket::S2), later);
    assert_eq!(socket_of(&engine, stop), Some(ViscaSocket::S2));
    let done = engine.handle(completion(1, ViscaSocket::S2), later);
    assert_eq!(terminal_id(&done), Some(stop));
    assert!(engine
        .ledger
        .entries()
        .any(|(_, entry)| entry.owes == Owes::AcceptedCompletion));

    let (stop_effects, second) = admit(&mut engine, 3, stop_command(1), later);
    send_ok(&mut engine, &stop_effects, None, later);
    let rejected = engine.handle(socketless_error(1, 0x41), later);
    assert!(matches!(
        terminal_failure(&rejected, second),
        Some(Error::CommandNotExecutable)
    ));
    let late = engine.handle(socketless_completion(1), later);
    assert!(terminal_id(&late).is_none());
    assert_eq!(raw_owed(&engine, 1), 0);
    assert_eq!(command_lane(&engine, 1), LaneState::Clear);
    engine.assert_invariants().unwrap();
}

/// M2: `0x05` answering an emitted cancellation after the ACK ends the
/// command unconfirmed on both envelopes; only the cancellation observation
/// keeps the camera's `NoSocket`.
#[test]
fn a_cancellation_rejected_after_ack_is_unconfirmed_on_both_envelopes() {
    let now = Instant::now();
    let assert_after_ack = |effects: &[Effect], id: RequestId| {
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::CancellationObservation {
                id: observed,
                observation: CancellationObservation::Failed(Error::NoSocket),
            } if *observed == id
        )));
        let error = terminal_failure(effects, id).expect("terminal");
        assert!(matches!(
            &error,
            Error::CommandFailedAfterAck { source, .. } if matches!(**source, Error::NoSocket)
        ));
        assert!(!error.is_retryable());
    };

    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Datagram);
    let id = executing_with_cancel(&mut engine, 1, ViscaSocket::S1, now);
    let rejected = engine.handle(named_error(1, ViscaSocket::S1, 0x05), now);
    assert_after_ack(&rejected, id);
    engine.assert_invariants().unwrap();

    let mut engine = fresh(EnvelopeKind::Sony, TransportKind::Datagram);
    let (effects, id) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        now,
    );
    let sequence = sony_sequence(id);
    send_ok(&mut engine, &effects, Some(sequence), now);
    engine.handle(
        frame(
            1,
            Some((sequence, SequenceWidth::Full32)),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    );
    let cancel = engine.handle(Input::Cancel { id }, now);
    let (transmission, _, _) = cancel_transmit(&cancel);
    let cancel_sequence = sequence + 0x100;
    engine.handle(
        Input::TransmissionFinished {
            transmission,
            result: Ok(TransmissionMeta {
                sequence: Some(cancel_sequence),
            }),
        },
        now,
    );
    let rejected = engine.handle(
        frame(
            1,
            Some((cancel_sequence, SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: Some(ViscaSocket::S1),
                code: 0x05,
            },
        ),
        now,
    );
    assert_after_ack(&rejected, id);
    engine.assert_invariants().unwrap();
}

/// L5: on a byte stream an error naming an executing command's socket is
/// that command's when no command could still be rejected: it is reported
/// at once instead of waiting for the completion deadline.
#[test]
fn stream_attributes_a_busy_socket_error_when_no_rejection_is_possible() {
    let now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let owner = executing_on(&mut engine, 1, ViscaSocket::S1, now);
    let failed = engine.handle(named_error(1, ViscaSocket::S1, 0x41), now);
    assert!(matches!(
        terminal_failure(&failed, owner),
        Some(Error::CommandFailedAfterAck { source, .. })
            if matches!(*source, Error::CommandNotExecutable)
    ));
    engine.assert_invariants().unwrap();
}

// ---------------------------------------------------------------------------
// The per-camera debt cap (#795).
// ---------------------------------------------------------------------------

use crate::runtime::engine::stream_ledger::MAX_DEBTS_PER_TARGET;

/// Writes a STOP to target 1 at `now` and lets its ACK deadline pass, leaving
/// one more owed ACK. Returns the instant it ended.
fn unanswered_stop(engine: &mut ProtocolEngine, ticket: u64, now: Instant) -> Instant {
    let (effects, stop) = admit(engine, ticket, stop_command(1), now);
    assert_eq!(
        request_transmit(&effects).1,
        stop,
        "a STOP is always written"
    );
    send_ok(engine, &effects, None, now);
    let ended = now + Duration::from_millis(20);
    let expired = engine.advance(ended);
    assert!(terminal_failure(&expired, stop).is_some());
    ended
}

fn inquiry_lane(engine: &ProtocolEngine, target: u8) -> LaneState {
    engine.ledger.lane_state(camera(target), OwedLane::Inquiry)
}

/// Reaching the cap means the camera answers nothing: every lane of that
/// camera latches, and no debt is ever dropped. STOPs are still written past
/// it, no other camera is affected, the session is never poisoned, and paying
/// debts reopens the lanes.
#[test]
fn reaching_the_debt_cap_latches_every_lane_without_dropping_a_debt() {
    let mut now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    for ticket in 1..MAX_DEBTS_PER_TARGET as u64 {
        now = unanswered_stop(&mut engine, ticket, now);
    }
    // Below the cap only the lane that owes answers is affected.
    assert_eq!(raw_owed(&engine, 1), MAX_DEBTS_PER_TARGET - 1);
    assert_eq!(inquiry_lane(&engine, 1), LaneState::Clear);

    now = unanswered_stop(&mut engine, 100, now);
    assert_eq!(raw_owed(&engine, 1), MAX_DEBTS_PER_TARGET);
    assert_eq!(inquiry_lane(&engine, 1), LaneState::Latched);
    assert_eq!(command_lane(&engine, 1), LaneState::Latched);
    assert_eq!(engine.state(), SessionState::Running);
    for (ticket, request, inquiry_lost) in [
        (101, inquiry(1, POWER), true),
        (102, command(1, CancellationPolicy::Supported), false),
    ] {
        let rejected = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(ticket),
                request,
                slot: AdmissionSlot::Ordinary,
            },
            now,
        );
        assert!(rejected.iter().any(|effect| match effect {
            Effect::AdmissionRejected { error, .. } if inquiry_lost =>
                matches!(error, Error::InquiryCorrelationLost { camera: c } if *c == camera(1)),
            Effect::AdmissionRejected { error, .. } =>
                matches!(error, Error::CommandCorrelationLost { camera: c } if *c == camera(1)),
            _ => false,
        }));
    }
    // STOPs are still written at the cap, and their debts are kept.
    now = unanswered_stop(&mut engine, 103, now);
    assert_eq!(raw_owed(&engine, 1), MAX_DEBTS_PER_TARGET + 1);
    // Another camera is unaffected.
    let (other, other_id) = admit(&mut engine, 104, inquiry(2, POWER), now);
    assert_eq!(request_transmit(&other).1, other_id);

    // The camera recovers: its late ACKs pay the oldest debts first. Back
    // below the cap the inquiry lane reopens; the command lane stays latched
    // until its own debts are paid.
    engine.handle(ack(1, ViscaSocket::S1), now);
    assert_eq!(raw_owed(&engine, 1), MAX_DEBTS_PER_TARGET);
    assert_eq!(inquiry_lane(&engine, 1), LaneState::Latched);
    engine.handle(ack(1, ViscaSocket::S2), now);
    assert_eq!(raw_owed(&engine, 1), MAX_DEBTS_PER_TARGET - 1);
    assert_eq!(inquiry_lane(&engine, 1), LaneState::Clear);
    assert_eq!(command_lane(&engine, 1), LaneState::Latched);
    for _ in 0..MAX_DEBTS_PER_TARGET - 1 {
        engine.handle(ack(1, ViscaSocket::S1), now);
    }
    assert_eq!(raw_owed(&engine, 1), 0);
    assert_eq!(command_lane(&engine, 1), LaneState::Clear);
    // The acknowledged STOPs hold their sockets until they complete.
    engine.handle(completion(1, ViscaSocket::S1), now);
    engine.handle(completion(1, ViscaSocket::S2), now);
    let (effects, id) = admit(
        &mut engine,
        105,
        command(1, CancellationPolicy::Supported),
        now,
    );
    assert_eq!(request_transmit(&effects).1, id);
    assert_eq!(engine.state(), SessionState::Running);
    engine.assert_invariants().unwrap();
}

/// Halts against a camera that answers nothing keep the ledger at a constant
/// size: the STOPs' owed ACKs form one run. When the camera recovers, its
/// answers pay every member and the lanes reopen.
#[test]
fn ten_thousand_halts_against_a_silent_camera_keep_the_ledger_bounded() {
    const HALTS: u64 = 10_000;
    let mut now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    for ticket in 1..=HALTS {
        now = unanswered_stop(&mut engine, ticket, now);
        assert_eq!(engine.ledger.len(camera(1)), 1);
    }
    assert_eq!(raw_owed(&engine, 1), usize::try_from(HALTS).unwrap());
    assert_eq!(command_lane(&engine, 1), LaneState::Latched);
    assert_eq!(inquiry_lane(&engine, 1), LaneState::Latched);
    assert_eq!(engine.state(), SessionState::Running);
    engine.assert_invariants().unwrap();

    for _ in 0..HALTS {
        engine.handle(ack(1, ViscaSocket::S1), now);
    }
    assert_eq!(raw_owed(&engine, 1), 0);
    assert_eq!(engine.ledger.len(camera(1)), 0);
    assert_eq!(command_lane(&engine, 1), LaneState::Clear);
    assert_eq!(inquiry_lane(&engine, 1), LaneState::Clear);
    engine.assert_invariants().unwrap();
}

/// Runs keep write order across answer classes: a burst of answers pays the
/// first STOP run, then the inquiry's owed reply written between the runs,
/// then the second run, and only then binds a live STOP.
#[test]
fn a_burst_of_answers_pays_runs_in_write_order() {
    let start = Instant::now();
    let mut engine = single_flight_raw_stream_engine();
    let mut now = start;
    for ticket in 10..13 {
        now = unanswered_stop(&mut engine, ticket, now);
    }
    let (_, timeout_at) = time_out_stream_inquiry(&mut engine, now);
    now = timeout_at;
    for ticket in 20..60 {
        now = unanswered_stop(&mut engine, ticket, now);
    }
    // [3 owed ACKs] [owed reply] [40 owed ACKs]
    assert_eq!(engine.ledger.len(camera(1)), 3);
    assert_eq!(raw_owed(&engine, 1), 44);
    assert!(engine.ledger.owes_reply(camera(1)));

    // Socketless rejections answer any kind, so only write order decides.
    for _ in 0..3 {
        let paid = engine.handle(socketless_error(1, 0x02), now);
        assert!(terminal_id(&paid).is_none());
        assert!(engine.ledger.owes_reply(camera(1)));
    }
    engine.handle(socketless_error(1, 0x02), now);
    assert!(!engine.ledger.owes_reply(camera(1)));
    assert_eq!(raw_owed(&engine, 1), 40);
    assert_eq!(engine.ledger.len(camera(1)), 1);

    let (effects, stop) = admit(&mut engine, 70, stop_command(1), now);
    send_ok(&mut engine, &effects, None, now);
    for _ in 0..40 {
        engine.handle(ack(1, ViscaSocket::S1), now);
        assert!(matches!(
            phase_of(&engine, stop),
            Some(Phase::AwaitingAck { .. })
        ));
    }
    engine.handle(ack(1, ViscaSocket::S2), now);
    assert_eq!(socket_of(&engine, stop), Some(ViscaSocket::S2));
    assert_eq!(raw_owed(&engine, 1), 0);
    engine.assert_invariants().unwrap();
}

// ---------------------------------------------------------------------------
// Round-2 review: stream holds, write-order settlement, and the gate (#795).
// ---------------------------------------------------------------------------

/// Advances in small steps until `id` ends; returns its terminal effects and
/// the instant.
fn run_until_terminal(
    engine: &mut ProtocolEngine,
    id: RequestId,
    mut now: Instant,
) -> (Vec<Effect>, Instant) {
    for _ in 0..1_000 {
        now += Duration::from_millis(5);
        let effects = engine.advance(now);
        if terminal_id(&effects) == Some(id) || phase_of(engine, id).is_none() {
            return (effects, now);
        }
    }
    panic!("{id:?} never ended");
}

/// R1: on a stream, a command that ends while its cancellation is unanswered
/// keeps its socket held until evidence arrives — not until a deadline, which
/// a stall can outlast: until then an error naming the socket is that
/// command's late execution error, never a later command's rejection. The
/// cancellation's late `0x04`/`0x05` pays its debt and releases the hold
/// (the camera answers it before it can allocate the socket again), so a new
/// command the camera allocates that socket is rejected by its own
/// `90 6y 41 FF` rather than leaving a phantom debt.
#[test]
fn stream_unanswered_cancel_holds_its_socket_until_evidence() {
    let now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let cancelled = executing_with_cancel(&mut engine, 1, ViscaSocket::S1, now);
    let (_, ended) = run_until_terminal(&mut engine, cancelled, now);
    let stalled = ended + Duration::from_secs(5);
    engine.advance(stalled);
    assert!(engine
        .raw_hold(camera(1), RawHoldScope::Socket(ViscaSocket::S1))
        .is_some());
    assert_eq!(raw_owed(&engine, 1), 1, "the cancellation's answer is owed");

    // Long after any deadline, a late execution error naming the socket is
    // the cancelled command's, even with a later command awaiting its ACK.
    let pending = command_awaiting_ack(&mut engine, 2, stalled);
    let late = engine.handle(named_error(1, ViscaSocket::S1, 0x41), stalled);
    assert!(terminal_id(&late).is_none());
    assert!(matches!(
        phase_of(&engine, pending),
        Some(Phase::AwaitingAck { .. })
    ));
    engine.handle(ack(1, ViscaSocket::S2), stalled);
    assert_eq!(socket_of(&engine, pending), Some(ViscaSocket::S2));

    // Another command ends with its cancellation unanswered, on S2.
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let cancelled = executing_with_cancel(&mut engine, 1, ViscaSocket::S1, now);
    let (_, ended) = run_until_terminal(&mut engine, cancelled, now);
    // The camera cancels it, then allocates S1 to a new command it rejects.
    let reply = engine.handle(named_error(1, ViscaSocket::S1, 0x04), ended);
    assert!(terminal_id(&reply).is_none());
    assert!(engine
        .raw_hold(camera(1), RawHoldScope::Socket(ViscaSocket::S1))
        .is_none());
    assert_eq!(raw_owed(&engine, 1), 0);
    let (effects, id) = admit(
        &mut engine,
        2,
        command_with_retry(1, no_not_executable_retry()),
        ended,
    );
    send_ok(&mut engine, &effects, None, ended);
    let rejected = engine.handle(named_error(1, ViscaSocket::S1, 0x41), ended);
    assert!(matches!(
        terminal_failure(&rejected, id),
        Some(Error::CommandNotExecutable)
    ));
    assert_eq!(command_lane(&engine, 1), LaneState::Clear);
    engine.assert_invariants().unwrap();
}

/// R1: a camera ACK that displaces a stale socket owner, or a socket held for
/// an owed command, leaves no `PreAck` hold on a stream, so the new owner's
/// completion still completes it.
#[test]
fn stream_socket_displacement_leaves_no_preack_hold() {
    let now = Instant::now();

    // A stale owner.
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let stale = executing_on(&mut engine, 1, ViscaSocket::S1, now);
    let (effects, successor) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &effects, None, now);
    let displaced = engine.handle(ack(1, ViscaSocket::S1), now);
    assert!(matches!(
        terminal_failure(&displaced, stale),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert!(engine.raw_hold(camera(1), RawHoldScope::PreAck).is_none());
    let done = engine.handle(completion(1, ViscaSocket::S1), now);
    assert_eq!(terminal_id(&done), Some(successor));
    engine.assert_invariants().unwrap();

    // A socket held for an owed command's completion.
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let ended = unanswered_stop(&mut engine, 1, now);
    engine.handle(ack(1, ViscaSocket::S1), ended);
    assert!(engine
        .raw_hold(camera(1), RawHoldScope::Socket(ViscaSocket::S1))
        .is_some());
    let (effects, successor) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        ended,
    );
    send_ok(&mut engine, &effects, None, ended);
    engine.handle(ack(1, ViscaSocket::S1), ended);
    assert_eq!(socket_of(&engine, successor), Some(ViscaSocket::S1));
    assert!(engine.raw_hold(camera(1), RawHoldScope::PreAck).is_none());
    let done = engine.handle(completion(1, ViscaSocket::S1), ended);
    assert_eq!(terminal_id(&done), Some(successor));
    engine.assert_invariants().unwrap();
}

/// R2: an inquiry the camera never answers latches only the inquiry lane;
/// the first later command's ACK proves (by write order) that its reply will
/// never come, so the debt is retired and inquiries work again.
#[test]
fn a_later_first_answer_retires_a_never_answered_inquiry() {
    let start = Instant::now();
    let mut engine = single_flight_raw_stream_engine();
    let (_, timeout_at) = time_out_stream_inquiry(&mut engine, start);
    let window_end = timeout_at + Duration::from_millis(50);
    engine.advance(window_end);
    assert_eq!(inquiry_lane(&engine, 1), LaneState::Latched);

    let command_id = command_awaiting_ack(&mut engine, 2, window_end);
    engine.handle(ack(1, ViscaSocket::S1), window_end);
    assert_eq!(socket_of(&engine, command_id), Some(ViscaSocket::S1));
    assert!(!engine.ledger.owes_reply(camera(1)));
    assert_eq!(inquiry_lane(&engine, 1), LaneState::Clear);

    let later = window_end + Duration::from_millis(100);
    engine.advance(later);
    let (effects, inquiry_id) = admit(&mut engine, 3, inquiry(1, POWER), later);
    send_ok(&mut engine, &effects, None, later);
    let reply = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: None,
                payload: SmallVec::from_slice(&[2]),
            },
        ),
        later,
    );
    assert_eq!(terminal_id(&reply), Some(inquiry_id));
    engine.assert_invariants().unwrap();
}

/// R2: a STOP the camera never answers starts a cascade — each later STOP's
/// ACK pays the previous debt — until a first answer of another class binds
/// to a live request written after the debts: the inquiry's reply retires
/// them, and the next STOP binds its own ACK.
#[test]
fn an_inquiry_reply_ends_a_stop_debt_cascade() {
    let mut now = Instant::now();
    let mut engine = single_flight_raw_stream_engine();
    now = unanswered_stop(&mut engine, 1, now);
    let (effects, second) = admit(&mut engine, 2, stop_command(1), now);
    send_ok(&mut engine, &effects, None, now);
    engine.handle(ack(1, ViscaSocket::S1), now);
    assert!(
        matches!(phase_of(&engine, second), Some(Phase::AwaitingAck { .. })),
        "its ACK paid the first STOP's debt"
    );
    let (_, ended) = run_until_terminal(&mut engine, second, now);
    now = ended;
    assert_eq!(raw_owed(&engine, 1), 1);

    let (effects, inquiry_id) = admit(&mut engine, 3, inquiry(1, POWER), now);
    send_ok(&mut engine, &effects, None, now);
    let reply = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: None,
                payload: SmallVec::from_slice(&[2]),
            },
        ),
        now,
    );
    assert_eq!(terminal_id(&reply), Some(inquiry_id));
    assert_eq!(raw_owed(&engine, 1), 0, "the reply retired the STOP debt");

    let (effects, third) = admit(&mut engine, 4, stop_command(1), now);
    send_ok(&mut engine, &effects, None, now);
    engine.handle(ack(1, ViscaSocket::S2), now);
    assert_eq!(socket_of(&engine, third), Some(ViscaSocket::S2));
    engine.assert_invariants().unwrap();
}

/// The gate blocks a queued ordinary command on a latched lane, not only one
/// that waits in a window: a lane that latches before the due pass (which
/// fails such work) never lets the command be written in between.
#[test]
fn a_latched_command_lane_never_dispatches_a_queued_command() {
    let start = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let now = unanswered_stop(&mut engine, 1, start);
    let (queued, id) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        now,
    );
    assert!(request_transmit_optional(&queued).is_none());
    assert!(engine.queued_dispatch_at(id).is_none());
    let window_end = now + Duration::from_millis(50);
    // The lane latches with the command still queued.
    engine.ledger.latch_due(window_end);
    assert_eq!(command_lane(&engine, 1), LaneState::Latched);
    assert!(engine.queued_dispatch_at(id).is_none());
    let failed = engine.advance(window_end);
    assert!(request_transmit_optional(&failed).is_none());
    assert!(matches!(
        terminal_failure(&failed, id),
        Some(Error::CommandCorrelationLost { .. })
    ));
    engine.assert_invariants().unwrap();
}

// ---------------------------------------------------------------------------
// STOPs bypass the `NoReply` hold (#700, #795).
// ---------------------------------------------------------------------------

/// Writes a `NoReply` command on target 1, queues an inquiry and an ordinary
/// command behind its hold, and writes a STOP, which the hold never holds
/// back. Returns the STOP and the two queued requests.
fn stop_inside_no_reply_hold(
    engine: &mut ProtocolEngine,
    now: Instant,
) -> (RequestId, RequestId, RequestId) {
    let (effects, no_reply) = admit(
        engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::NoReply),
        now,
    );
    let written = send_ok(engine, &effects, None, now);
    assert!(matches!(
        terminal_outcome(&written, no_reply),
        Some(RuntimeOutcome::Written)
    ));
    let (queued, inquiry_id) = admit(engine, 2, inquiry(1, POWER), now);
    assert!(request_transmit_optional(&queued).is_none());
    let (queued, command_id) = admit(engine, 3, command(1, CancellationPolicy::Supported), now);
    assert!(request_transmit_optional(&queued).is_none());
    let (stop_effects, stop) = admit(engine, 4, stop_command(1), now);
    assert_eq!(request_transmit(&stop_effects).1, stop);
    send_ok(engine, &stop_effects, None, now);
    (stop, inquiry_id, command_id)
}

/// A STOP is written at once inside a `NoReply` command's hold on either
/// transport, while ordinary work keeps waiting for the hold. Its ACK (which
/// the `NoReply` command never sends) settles it, and the completion naming
/// its socket completes it.
#[test]
fn a_stop_inside_a_no_reply_hold_is_written_at_once_and_settled_by_its_ack() {
    let now = Instant::now();
    for transport in [TransportKind::Stream, TransportKind::Datagram] {
        let mut engine = fresh(EnvelopeKind::Raw, transport);
        let (stop, inquiry_id, command_id) = stop_inside_no_reply_hold(&mut engine, now);
        let acked = engine.handle(ack(1, ViscaSocket::S1), now);
        assert!(ignored_reasons(&acked).is_empty(), "{transport:?}");
        assert_eq!(
            socket_of(&engine, stop),
            Some(ViscaSocket::S1),
            "{transport:?}"
        );
        assert!(matches!(
            phase_of(&engine, inquiry_id),
            Some(Phase::Ready { .. })
        ));
        assert!(matches!(
            phase_of(&engine, command_id),
            Some(Phase::Ready { .. })
        ));
        let done = engine.handle(completion(1, ViscaSocket::S1), now);
        assert!(
            matches!(terminal_outcome(&done, stop), Some(RuntimeOutcome::Applied)),
            "{transport:?}"
        );
        assert_eq!(raw_owed(&engine, 1), 0);
        engine.assert_invariants().unwrap();
    }
}

/// A socketless error inside the hold, with a STOP written there, is the
/// `NoReply` command's rejection or the STOP's, and binds to neither. On a
/// stream the STOP's own ACK, which nothing else could have sent, then
/// proves the error was the `NoReply` command's.
#[test]
fn an_ambiguous_error_inside_a_no_reply_hold_binds_to_neither() {
    let now = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let (stop, _, _) = stop_inside_no_reply_hold(&mut engine, now);
    let disputed = engine.handle_turn(socketless_error(1, 0x41), now, EngineTurn::INPUT_ONLY);
    assert!(terminal_id(&disputed).is_none() && retry_scheduled(&disputed).is_none());
    assert!(engine.ledger.awaits_dispute(camera(1), stop));
    let proven = engine.handle(ack(1, ViscaSocket::S2), now);
    assert!(terminal_id(&proven).is_none());
    assert_eq!(socket_of(&engine, stop), Some(ViscaSocket::S2));
    assert!(!engine.ledger.forbids_crossing(camera(1)));
    assert_eq!(raw_owed(&engine, 1), 0);
    engine.assert_invariants().unwrap();
}

/// If the STOP's own answer never comes, nothing proves which request the
/// error was, and the STOP honestly ends unconfirmed: on a stream past its
/// extended ACK deadline, on a datagram transport (where the hold discards
/// the error) past its own.
#[test]
fn a_stop_whose_error_is_ambiguous_ends_unconfirmed() {
    let now = Instant::now();
    for transport in [TransportKind::Stream, TransportKind::Datagram] {
        let mut engine = fresh(EnvelopeKind::Raw, transport);
        let (stop, _, _) = stop_inside_no_reply_hold(&mut engine, now);
        let disputed = engine.handle_turn(socketless_error(1, 0x41), now, EngineTurn::INPUT_ONLY);
        assert!(
            terminal_id(&disputed).is_none() && retry_scheduled(&disputed).is_none(),
            "{transport:?}"
        );
        let mut ended = None;
        for step in 1..=200 {
            let effects = engine.advance(now + Duration::from_millis(step));
            if let Some(error) = terminal_failure(&effects, stop) {
                ended = Some(error);
                break;
            }
        }
        assert!(
            matches!(ended, Some(Error::UnsequencedCommandUnconfirmed)),
            "{transport:?}: {ended:?}"
        );
        engine.assert_invariants().unwrap();
    }
}

/// N1: a stall can outlast any window, so a `NoReply` command's late
/// rejection may arrive after its hold has ended and an ordinary command was
/// written behind it. The ledger keeps owing that possible rejection until
/// write order settles it, so the frame never reaches the later command: a
/// movement command whose `0x41` is retried would otherwise be written twice
/// (its real ACK dropped while it waits to retry).
#[test]
fn a_late_no_reply_rejection_never_reaches_a_later_command() {
    let start = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let (effects, no_reply) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::NoReply),
        start,
    );
    let written = send_ok(&mut engine, &effects, None, start);
    assert!(matches!(
        terminal_outcome(&written, no_reply),
        Some(RuntimeOutcome::Written)
    ));
    // Long past the hold, the link still stalled.
    let later = start + Duration::from_secs(5);
    engine.advance(later);
    let (effects, moving) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        later,
    );
    assert_eq!(request_transmit(&effects).1, moving);
    send_ok(&mut engine, &effects, None, later);
    let mut writes = 1;
    // The stall ends: the `NoReply` command's rejection, then the command's
    // own ACK and completion.
    let late = engine.handle(socketless_error(1, 0x41), later);
    let misbound = terminal_id(&late).is_some() || retry_scheduled(&late).is_some();
    engine.handle(ack(1, ViscaSocket::S1), later);
    let mut done = None;
    for step in 1..=200 {
        let now = later + Duration::from_millis(step);
        let mut effects = engine.advance(now);
        if step == 5 {
            effects.extend(engine.handle(completion(1, ViscaSocket::S1), now));
        }
        if let Some((_, id, _)) = request_transmit_optional(&effects) {
            if id == moving {
                writes += 1;
            }
            send_ok(&mut engine, &effects, None, now);
        }
        if let Some(outcome) = terminal_outcome(&effects, moving) {
            done = Some(outcome);
        }
    }
    assert_eq!(writes, 1, "the command was written again");
    assert!(!misbound);
    assert!(matches!(done, Some(RuntimeOutcome::Applied)), "{done:?}");
    assert_eq!(raw_owed(&engine, 1), 0);
    engine.assert_invariants().unwrap();
}

/// N3: socketless completions dropped while socket naming was unknown pay
/// only the `CompletionOnly` commands owed when the camera is learned to name
/// its sockets. Any left over (here, an executing command's completion that
/// omitted its socket) are forgotten, never kept to complete a later
/// `CompletionOnly` command at once.
#[test]
fn leftover_unlearned_completions_never_complete_a_later_command() {
    let start = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let executing = executing_on(&mut engine, 1, ViscaSocket::S1, start);
    let expired = engine.advance(start + Duration::from_millis(40));
    assert!(terminal_failure(&expired, executing).is_some());
    let now = start + Duration::from_millis(100);
    engine.advance(now);
    let completion_only_request =
        || command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly);
    let (effects, first) = admit(&mut engine, 2, completion_only_request(), now);
    assert_eq!(request_transmit(&effects).1, first);
    send_ok(&mut engine, &effects, None, now);
    // Two socketless completions while socket S1 is still held: either could
    // be the held command's, so both are dropped as ambiguous.
    for _ in 0..2 {
        let dropped = engine.handle(socketless_completion(1), now);
        assert!(terminal_id(&dropped).is_none());
    }
    // The held command's late completion names its socket: the camera names
    // sockets, so one dropped completion was the `CompletionOnly` command's.
    let learned = engine.handle(completion(1, ViscaSocket::S1), now);
    assert!(matches!(
        terminal_outcome(&learned, first),
        Some(RuntimeOutcome::Applied)
    ));
    let later = now + Duration::from_millis(1);
    let (effects, second) = admit(&mut engine, 3, completion_only_request(), later);
    assert_eq!(request_transmit(&effects).1, second);
    let written = send_ok(&mut engine, &effects, None, later);
    let next = engine.advance(later + Duration::from_millis(1));
    assert!(terminal_outcome(&written, second).is_none());
    assert!(terminal_outcome(&next, second).is_none());
    assert!(matches!(
        phase_of(&engine, second),
        Some(Phase::AwaitingCompletion { .. })
    ));
    let done = engine.handle(socketless_completion(1), later);
    assert!(matches!(
        terminal_outcome(&done, second),
        Some(RuntimeOutcome::Applied)
    ));
    engine.assert_invariants().unwrap();
}

/// N2: a tracked dispute whose requests have all ended (here neither the
/// `CompletionOnly` command's completion nor the STOP's ACK ever comes) ends
/// one ambiguity interval later, falling to the documented latch: queued and
/// new inquiries to that camera then fail fast with
/// `InquiryCorrelationLost` instead of waiting out their own deadlines.
#[test]
fn an_unanswered_tracked_dispute_falls_to_the_latch() {
    let now = Instant::now();
    let mut engine = numbering_camera(now);
    let (completion_only, stop, _) = disputed(&mut engine, now);
    let mut outcomes = BTreeMap::new();
    let mut step = 0;
    while engine.ledger.forbids_crossing(camera(1)) {
        step += 1;
        assert!(step <= 1_000, "the dispute never ended");
        let at = now + Duration::from_millis(step);
        // An inquiry queued just before the dispute ends.
        let effects = if step == 115 {
            let (effects, _) = admit(&mut engine, 8, inquiry(1, POWER), at);
            assert!(request_transmit_optional(&effects).is_none());
            effects
        } else {
            engine.advance(at)
        };
        for effect in effects {
            if let Effect::Terminal { id, outcome } = effect {
                outcomes.insert(id, (step, outcome));
            }
        }
    }
    let at = now + Duration::from_millis(step);
    assert!(outcomes.contains_key(&completion_only) && outcomes.contains_key(&stop));
    // Never proven accepted, the command holds both lanes like any whose
    // completion never came: the queued inquiry fails as the dispute ends,
    // and a new one is refused.
    assert_eq!(
        engine.ledger.lane_state(camera(1), OwedLane::Inquiry),
        LaneState::Latched
    );
    assert!(
        outcomes.values().any(|(ended, outcome)| *ended == step
            && matches!(
                outcome,
                RuntimeOutcome::Failed(Error::InquiryCorrelationLost { .. })
            )),
        "{outcomes:?}"
    );
    let rejected = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(9),
            request: inquiry(1, POWER),
            slot: AdmissionSlot::Ordinary,
        },
        at,
    );
    assert!(rejected.iter().any(|effect| matches!(
        effect,
        Effect::AdmissionRejected {
            error: Error::InquiryCorrelationLost { .. },
            ..
        }
    )));
    engine.assert_invariants().unwrap();
}

/// A STOP that the crossing rules hold back on a byte stream waits at most
/// one ACK plus one ambiguity interval (here 20 + 50 ms), and never past half
/// its dispatch budget (here 60 ms), then is written anyway: the ledger still
/// binds every frame by write order, so the cost is only that a frame it
/// makes ambiguous binds to neither request.
#[test]
fn a_held_back_stop_is_written_at_its_wait_bound() {
    let now = Instant::now();
    let mut engine = numbering_camera(now);
    let (_, disputed_stop, _) = disputed(&mut engine, now);
    let mut request = stop_command(1);
    request.context_mut().dispatch_deadline = Some(now + Duration::from_millis(60));
    let (queued, stop) = admit(&mut engine, 4, request, now);
    assert!(request_transmit_optional(&queued).is_none());
    let bound = now + Duration::from_millis(30);
    let mut at = now;
    while at < bound - Duration::from_millis(1) {
        assert!(engine.next_wake().is_some_and(|wake| wake <= bound));
        at += Duration::from_millis(1);
        let early = engine.advance(at);
        assert!(request_transmit_optional(&early).is_none());
    }
    // The disputed STOP still waits, and the dispute holds everything else.
    assert!(matches!(
        phase_of(&engine, disputed_stop),
        Some(Phase::AwaitingAck { .. })
    ));
    assert!(engine.ledger.forbids_crossing(camera(1)));
    let written = engine.advance(bound);
    assert_eq!(request_transmit(&written).1, stop);
    send_ok(&mut engine, &written, None, bound);
    engine.assert_invariants().unwrap();
}

/// Round 5 (1): a STOP written at its wait bound behind the disputed STOP
/// makes a second ambiguity. The tracked dispute is kept, accept-only: no
/// STOP's ACK can prove the `CompletionOnly` command rejected any more, but
/// its completion still proves it accepted, and the disputed error then
/// reaches the STOP that sent it.
#[test]
fn a_second_ambiguity_keeps_the_dispute_accept_only() {
    let now = Instant::now();
    let mut engine = numbering_camera(now);
    let (completion_only, first_stop, _) = disputed(&mut engine, now);
    let mut request = stop_command(1);
    request.context_mut().dispatch_deadline = Some(now + Duration::from_millis(20));
    let (queued, second_stop) = admit(&mut engine, 4, request, now);
    assert!(request_transmit_optional(&queued).is_none());
    let at = now + Duration::from_millis(10);
    let written = engine.advance(at);
    assert_eq!(request_transmit(&written).1, second_stop);
    send_ok(&mut engine, &written, None, at);
    // Either STOP's answer: binds to neither, and the dispute is kept.
    let ambiguous = engine.handle_turn(socketless_error(1, 0x41), at, EngineTurn::INPUT_ONLY);
    assert!(terminal_id(&ambiguous).is_none() && retry_scheduled(&ambiguous).is_none());
    assert!(engine.ledger.awaits_dispute(camera(1), first_stop));
    // An ACK no longer proves the command rejected.
    let ack_at = at + Duration::from_millis(1);
    let acked = engine.handle_turn(ack(1, ViscaSocket::S2), ack_at, EngineTurn::INPUT_ONLY);
    assert!(terminal_outcome(&acked, completion_only).is_none());
    assert!(engine.ledger.awaits_dispute(camera(1), first_stop));
    // Its completion proves it accepted: the first error was the first STOP's.
    let done = engine.handle(socketless_completion(1), ack_at);
    assert!(matches!(
        terminal_outcome(&done, completion_only),
        Some(RuntimeOutcome::Applied)
    ));
    assert!(matches!(
        terminal_failure(&done, first_stop),
        Some(Error::CommandNotExecutable)
    ));
    assert!(!engine.ledger.awaits_dispute(camera(1), first_stop));
    engine.assert_invariants().unwrap();
}

/// Round 5 (2): a `NoReply` command then only `CompletionOnly` work. Nothing
/// answers after the `NoReply` command, so its possible rejection stays owed
/// and the `CompletionOnly` command waits; past the `NoReply` command's
/// window it fails unwritten with `CommandCorrelationLost` (as do new ones)
/// rather than timing out. Any later first answer — here an inquiry's
/// reply — settles the owed rejection, and `CompletionOnly` work flows again.
#[test]
fn completion_only_work_behind_an_unsettled_no_reply_fails_fast() {
    let start = Instant::now();
    let mut engine = fresh(EnvelopeKind::Raw, TransportKind::Stream);
    let (effects, _) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::NoReply),
        start,
    );
    send_ok(&mut engine, &effects, None, start);
    let completion_only_request =
        || command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly);
    let (queued, waiting) = admit(&mut engine, 2, completion_only_request(), start);
    assert!(request_transmit_optional(&queued).is_none());
    let window_end = start + Duration::from_millis(50);
    assert!(engine.next_wake().is_some_and(|wake| wake <= window_end));
    let failed = engine.advance(window_end);
    assert!(request_transmit_optional(&failed).is_none());
    assert!(matches!(
        terminal_failure(&failed, waiting),
        Some(Error::CommandCorrelationLost { .. })
    ));
    let refused = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: completion_only_request(),
            slot: AdmissionSlot::Ordinary,
        },
        window_end,
    );
    assert!(refused.iter().any(|effect| matches!(
        effect,
        Effect::AdmissionRejected {
            error: Error::CommandCorrelationLost { .. },
            ..
        }
    )));
    // An inquiry's reply settles the `NoReply` command's possible rejection.
    let (effects, inquiry_id) = admit(&mut engine, 4, inquiry(1, POWER), window_end);
    assert_eq!(request_transmit(&effects).1, inquiry_id);
    send_ok(&mut engine, &effects, None, window_end);
    let replied = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: None,
                payload: SmallVec::from_slice(&[0x02]),
            },
        ),
        window_end,
    );
    assert_eq!(terminal_id(&replied), Some(inquiry_id));
    let later = window_end + Duration::from_millis(60);
    engine.advance(later);
    let (effects, flowing) = admit(&mut engine, 5, completion_only_request(), later);
    assert_eq!(request_transmit(&effects).1, flowing);
    engine.assert_invariants().unwrap();
}
