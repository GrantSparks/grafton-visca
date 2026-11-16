//! Compatibility ACKs preserve physical execution evidence after an ambiguous
//! completion, including when an urgent STOP must cross occupied sockets.

use super::*;

fn socketless_ack(engine: &mut ProtocolEngine, at: Instant) -> Vec<Effect> {
    engine.handle(frame(1, None, DecodedResponse::Ack { socket: None }), at)
}

fn socketless_done(engine: &mut ProtocolEngine, at: Instant) -> Vec<Effect> {
    engine.handle(
        frame(1, None, DecodedResponse::Completion { socket: None }),
        at,
    )
}

fn executing(engine: &mut ProtocolEngine, ticket: u64, at: Instant) -> RequestId {
    let (write, id) = admit(
        engine,
        ticket,
        command(1, CancellationPolicy::Supported),
        at,
    );
    send_ok(engine, &write, None, at);
    socketless_ack(engine, at);
    assert!(matches!(
        phase_of(engine, id),
        Some(Phase::Executing { .. })
    ));
    id
}

fn crossing_stop(engine: &mut ProtocolEngine, at: Instant) -> (RequestId, Instant) {
    let mut stop = urgent_command(1, CancellationPolicy::Supported);
    stop.context_mut().dispatch_deadline = Some(at + Duration::from_millis(20));
    let (_, id) = admit(engine, 3, stop, at);
    let written_at = at + Duration::from_millis(10);
    let write = engine.advance(written_at);
    assert_eq!(request_transmit(&write).1, id);
    send_ok(engine, &write, None, written_at);
    (id, written_at)
}

fn assert_anonymous_socket_union(engine: &ProtocolEngine) {
    for socket in [ViscaSocket::S1, ViscaSocket::S2] {
        assert!(engine.socket_owner(camera(1), socket).is_none());
        assert_eq!(
            engine
                .raw_hold(camera(1), RawHoldScope::Socket(socket))
                .unwrap()
                .owner,
            None
        );
    }
    engine.assert_invariants().unwrap();
}

#[test]
fn socketless_ack_after_an_ambiguous_completion_preserves_every_possible_execution() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    let first = executing(&mut engine, 1, now);
    let second = executing(&mut engine, 2, now);
    // The camera frees S1, but this frame cannot identify which host owner
    // completed. The urgent STOP then reuses S1 before S2 finishes.
    assert!(terminal_id(&socketless_done(&mut engine, now)).is_none());
    let (stop, later) = crossing_stop(&mut engine, now);
    let acked = socketless_ack(&mut engine, later);
    for id in [first, second, stop] {
        assert!(
            matches!(
                terminal_failure(&acked, id),
                Some(Error::UnsequencedCommandUnconfirmed)
            ),
            "{id:?}: {acked:?}"
        );
    }
    assert_eq!(engine.state(), SessionState::Running);
    assert_anonymous_socket_union(&engine);

    // Neither the still-executing second command nor the STOP can give a
    // later command an Applied verdict through a forgotten execution.
    let (write, successor) = admit(
        &mut engine,
        4,
        urgent_command(1, CancellationPolicy::Supported),
        later,
    );
    send_ok(&mut engine, &write, None, later);
    let acked = socketless_ack(&mut engine, later);
    assert!(matches!(
        terminal_failure(&acked, successor),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert!(terminal_id(&socketless_done(&mut engine, later)).is_none());
    assert_anonymous_socket_union(&engine);

    // Named evidence recovers one exact physical socket without reviving a
    // guessed predecessor or poisoning the healthy default-policy session.
    let (write, exact) = admit(
        &mut engine,
        5,
        urgent_command(1, CancellationPolicy::Supported),
        later,
    );
    send_ok(&mut engine, &write, None, later);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        later,
    );
    assert_eq!(socket_of(&engine, exact), Some(ViscaSocket::S1));
    let completed = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        later,
    );
    assert!(matches!(
        terminal_outcome(&completed, exact),
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
    ));
    engine.assert_invariants().unwrap();
}

#[test]
fn strict_socketless_ack_conflict_ends_the_session_with_its_unconfirmed_cause() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    engine.policy.strict_unconfirmed_poison = true;
    let first = executing(&mut engine, 1, now);
    let second = executing(&mut engine, 2, now);
    socketless_done(&mut engine, now);
    let (stop, later) = crossing_stop(&mut engine, now);
    let ended = socketless_ack(&mut engine, later);
    for id in [first, second, stop] {
        assert!(
            matches!(terminal_failure(&ended, id), Some(Error::StreamPoisoned { source }) if matches!(source.as_ref(), Error::UnsequencedCommandUnconfirmed))
        );
    }
    assert_eq!(engine.state(), SessionState::Poisoned);
    assert_eq!(
        ended
            .iter()
            .filter(|effect| matches!(effect, Effect::SessionEnded(_)))
            .count(),
        1
    );
    assert!(engine.entries.is_empty());
    assert!(engine.correlation.holds().is_empty());
    assert!(engine.correlation.ledger().is_empty());
    engine.assert_invariants().unwrap();
}

#[test]
fn socketless_ack_cannot_guess_past_an_inert_predecessor_socket() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    extend_fixture_hold(
        &mut engine,
        camera(1),
        StreamScope::Socket(ViscaSocket::S1),
        now + Duration::from_millis(40),
        None,
    );
    let (write, request) = admit(
        &mut engine,
        1,
        urgent_command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &write, None, now);
    let acked = socketless_ack(&mut engine, now);
    assert!(matches!(
        terminal_failure(&acked, request),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert_anonymous_socket_union(&engine);
}

#[test]
fn an_owed_socketless_ack_keeps_the_same_union_as_a_live_ack() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    let first = executing(&mut engine, 1, now);
    let second = executing(&mut engine, 2, now);
    socketless_done(&mut engine, now);
    let (stop, written_at) = crossing_stop(&mut engine, now);
    let expired_at = written_at + Duration::from_millis(20);
    let expired = engine.advance(expired_at);
    assert!(matches!(
        terminal_failure(&expired, stop),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    let paid = socketless_ack(&mut engine, expired_at);
    for owner in [first, second] {
        assert!(matches!(
            terminal_failure(&paid, owner),
            Some(Error::UnsequencedCommandUnconfirmed)
        ));
    }
    assert_eq!(engine.correlation.ledger().debts(camera(1)), 0);
    assert_anonymous_socket_union(&engine);
    assert!(terminal_id(&socketless_done(&mut engine, expired_at)).is_none());
    assert_anonymous_socket_union(&engine);
}

#[test]
fn a_one_socket_union_recovers_without_waiting_for_a_phantom_second_socket() {
    let now = Instant::now();
    let mut engine = engine_with_target(
        EnvelopeKind::Raw,
        TransportKind::Stream,
        1,
        CancellationPolicy::Supported,
    );
    extend_fixture_hold(
        &mut engine,
        camera(1),
        StreamScope::Socket(ViscaSocket::S1),
        now + Duration::from_millis(40),
        None,
    );
    let (write, first) = admit(
        &mut engine,
        1,
        urgent_command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &write, None, now);
    assert!(matches!(
        terminal_failure(&socketless_ack(&mut engine, now), first),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert!(engine
        .raw_hold(camera(1), RawHoldScope::Socket(ViscaSocket::S1))
        .is_some());
    assert!(engine
        .raw_hold(camera(1), RawHoldScope::Socket(ViscaSocket::S2))
        .is_none());
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    );
    let (write, next) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &write, None, now);
    socketless_ack(&mut engine, now);
    assert_eq!(socket_of(&engine, next), Some(ViscaSocket::S1));
    assert!(matches!(
        terminal_outcome(&socketless_done(&mut engine, now), next),
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
    ));
    engine.assert_invariants().unwrap();
}

#[test]
fn lowering_capacity_cannot_forget_a_second_socket_from_an_ended_attempt() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    let (write, old) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &write, None, now);
    let later = now + Duration::from_millis(20);
    assert!(matches!(
        terminal_failure(&engine.advance(later), old),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    engine
        .retune(
            Duration::ZERO,
            Duration::ZERO,
            PerTarget::from_fn(|target| (target == camera(1)).then_some(1)),
            later,
        )
        .unwrap();
    extend_fixture_hold(
        &mut engine,
        camera(1),
        StreamScope::Socket(ViscaSocket::S1),
        later + Duration::from_millis(40),
        None,
    );
    // The old request's live AttemptFacts have gone, but its ACK still proves
    // execution under the former two-socket capacity.
    socketless_ack(&mut engine, later);
    assert_anonymous_socket_union(&engine);
}

#[test]
fn a_camera_proven_second_socket_survives_a_one_socket_dispatch_limit() {
    let now = Instant::now();
    let mut engine = engine_with_target(
        EnvelopeKind::Raw,
        TransportKind::Stream,
        1,
        CancellationPolicy::Supported,
    );
    let (write, old) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &write, None, now);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S2),
            },
        ),
        now,
    );
    assert_eq!(socket_of(&engine, old), Some(ViscaSocket::S2));
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S2),
            },
        ),
        now,
    );
    assert!(engine.entry(old).is_none());
    extend_fixture_hold(
        &mut engine,
        camera(1),
        StreamScope::Socket(ViscaSocket::S1),
        now + Duration::from_millis(40),
        None,
    );
    let (write, next) = admit(
        &mut engine,
        2,
        urgent_command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &write, None, now);
    assert!(matches!(
        terminal_failure(&socketless_ack(&mut engine, now), next),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert_anonymous_socket_union(&engine);
}
