//! An unnamed acknowledgement proves acceptance independently of the
//! camera's physical socket allocation.

use super::*;

fn ack(engine: &mut ProtocolEngine, target: u8, socket: Option<ViscaSocket>, at: Instant) {
    engine.handle(frame(target, None, DecodedResponse::Ack { socket }), at);
}

fn complete(
    engine: &mut ProtocolEngine,
    target: u8,
    socket: Option<ViscaSocket>,
    at: Instant,
) -> Vec<Effect> {
    engine.handle(
        frame(target, None, DecodedResponse::Completion { socket }),
        at,
    )
}

fn executing(
    engine: &mut ProtocolEngine,
    target: u8,
    socket: Option<ViscaSocket>,
    at: Instant,
) -> RequestId {
    let (write, id) = admit(engine, u64::from(target), command(target), at);
    send_ok(engine, &write, None, at);
    ack(engine, target, socket, at);
    id
}

#[test]
fn anonymous_ack_never_assigns_a_socket_and_serializes_terminal_candidates() {
    for transport in [TransportKind::Datagram, TransportKind::Stream] {
        for (first_socket, second_socket) in [
            (ViscaSocket::S1, ViscaSocket::S2),
            (ViscaSocket::S2, ViscaSocket::S1),
        ] {
            let now = Instant::now();
            let mut engine = engine(EnvelopeKind::Raw, transport);
            let first = executing(&mut engine, 1, None, now);
            assert!(matches!(
                phase_of(&engine, first),
                Some(Phase::ExecutingUnassigned { .. })
            ));
            assert_eq!(socket_of(&engine, first), None);
            let effects = engine.handle(engine.cancel_input(first), now);
            assert!(effects.iter().any(|effect| matches!(
                effect,
                Effect::CancellationObservation { id, observation: CancellationObservation::AttemptFailed(Error::CancellationUnconfirmed) } if *id == first
            )));
            assert!(request_transmit_optional(&effects).is_none());

            let (queued, second) = admit(&mut engine, 3, command(1), now);
            assert!(request_transmit_optional(&queued).is_none());
            let unrelated = executing(&mut engine, 2, Some(first_socket), now);
            assert_eq!(
                terminal_id(&complete(&mut engine, 2, Some(first_socket), now)),
                Some(unrelated)
            );
            assert!(request_transmit_optional(&engine.advance(now)).is_none());

            let completed = complete(&mut engine, 1, Some(first_socket), now);
            assert_eq!(terminal_id(&completed), Some(first));
            assert_eq!(request_transmit(&completed).1, second);
            send_ok(&mut engine, &completed, None, now);
            ack(&mut engine, 1, None, now);
            assert_eq!(
                terminal_id(&complete(&mut engine, 1, Some(second_socket), now)),
                Some(second)
            );
            engine.assert_invariants().unwrap();
        }
    }
}

#[test]
fn unassigned_terminal_does_not_override_named_socket_ownership() {
    for transport in [TransportKind::Datagram, TransportKind::Stream] {
        let now = Instant::now();
        let mut engine = engine(EnvelopeKind::Raw, transport);
        let first = executing(&mut engine, 1, Some(ViscaSocket::S1), now);
        let second = executing(&mut engine, 1, None, now);
        assert_eq!(socket_of(&engine, first), Some(ViscaSocket::S1));
        assert_eq!(socket_of(&engine, second), None);

        // An earlier named holder could have completed invisibly and the
        // unassigned command could have reused S1, or it could be on S2.
        assert!(terminal_id(&complete(&mut engine, 1, Some(ViscaSocket::S1), now)).is_none());
        assert!(terminal_id(&complete(&mut engine, 1, None, now)).is_none());
        assert!(engine.entry(first).is_some());
        assert!(engine.entry(second).is_some());

        let error = engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Error {
                    socket: Some(ViscaSocket::S2),
                    code: CameraErrorCode::NotExecutable,
                },
            ),
            now,
        );
        assert_eq!(terminal_id(&error), Some(second));
        assert!(engine.entry(first).is_some());
        engine.assert_invariants().unwrap();
    }
}

#[test]
fn urgent_stop_crosses_anonymous_execution_without_inventing_a_terminal() {
    for transport in [TransportKind::Datagram, TransportKind::Stream] {
        for socket in [ViscaSocket::S1, ViscaSocket::S2] {
            let now = Instant::now();
            let mut engine = engine(EnvelopeKind::Raw, transport);
            let first = executing(&mut engine, 1, None, now);
            let (write, stop) = admit(&mut engine, 2, urgent_command(1), now);
            assert_eq!(request_transmit(&write).1, stop);
            send_ok(&mut engine, &write, None, now);
            assert!(terminal_id(&complete(&mut engine, 1, Some(socket), now)).is_none());
            ack(&mut engine, 1, Some(socket), now);
            assert!(terminal_id(&complete(&mut engine, 1, Some(socket), now)).is_none());
            assert!(engine.entry(first).is_some());
            assert!(engine.entry(stop).is_some());
            engine.assert_invariants().unwrap();
        }
    }
}

#[test]
fn a_timed_out_anonymous_execution_cannot_complete_an_urgent_successor() {
    for transport in [TransportKind::Datagram, TransportKind::Stream] {
        let now = Instant::now();
        let mut engine = engine(EnvelopeKind::Raw, transport);
        let first = executing(&mut engine, 1, None, now);
        let Phase::ExecutingUnassigned { deadline, .. } = phase_of(&engine, first).unwrap() else {
            unreachable!();
        };
        let expired = engine.advance(deadline);
        assert!(matches!(
            terminal_failure(&expired, first),
            Some(Error::UnsequencedCommandUnconfirmed)
        ));
        let (queued, _) = admit(&mut engine, 3, command(1), deadline);
        assert!(request_transmit_optional(&queued).is_none());
        let (write, stop) = admit(&mut engine, 4, urgent_command(1), deadline);
        assert_eq!(request_transmit(&write).1, stop);
        send_ok(&mut engine, &write, None, deadline);
        ack(&mut engine, 1, Some(ViscaSocket::S2), deadline);
        let late = complete(&mut engine, 1, Some(ViscaSocket::S2), deadline);
        assert!(terminal_id(&late).is_none());
        assert!(engine.entry(stop).is_some());
        engine.assert_invariants().unwrap();
    }
}

#[test]
fn an_overdue_stream_terminal_fails_the_waiting_command_lane_until_evidence_arrives() {
    let now = Instant::now();
    let target = camera(1);
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    let first = executing(&mut engine, 1, None, now);
    let (queued, waiting) = admit(&mut engine, 2, command(1), now);
    assert!(request_transmit_optional(&queued).is_none());
    let Phase::ExecutingUnassigned { deadline, .. } = phase_of(&engine, first).unwrap() else {
        unreachable!();
    };
    let expired = engine.advance(deadline);
    assert!(matches!(
        terminal_failure(&expired, first),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    let until = engine
        .raw_hold(target, RawHoldScope::UnassignedExecution)
        .unwrap()
        .until;
    assert_eq!(engine.next_wake(deadline), Some(until));
    let latched = engine.advance(until);
    assert!(matches!(
        terminal_failure(&latched, waiting),
        Some(Error::CommandCorrelationLost { .. })
    ));
    assert!(request_transmit_optional(&latched).is_none());

    // A terminal received after the window proves the lane safe again.
    let late = complete(&mut engine, 1, Some(ViscaSocket::S2), until);
    assert!(terminal_id(&late).is_none());
    assert!(engine
        .raw_hold(target, RawHoldScope::UnassignedExecution)
        .is_none());
    let (write, fresh) = admit(&mut engine, 3, command(1), until);
    assert_eq!(request_transmit(&write).1, fresh);
}

#[test]
fn a_stops_ack_proves_a_prior_ambiguous_error_paid_the_unassigned_terminal() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    let first = executing(&mut engine, 1, None, now);
    let Phase::ExecutingUnassigned { deadline, .. } = phase_of(&engine, first).unwrap() else {
        unreachable!();
    };
    engine.advance(deadline);
    let (write, stop) = admit(&mut engine, 2, urgent_command(1), deadline);
    send_ok(&mut engine, &write, None, deadline);

    // This error could be the original execution's terminal or the STOP's
    // pre-ACK rejection. The later ACK proves that the STOP was accepted.
    let error = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: Some(ViscaSocket::S1),
                code: CameraErrorCode::NotExecutable,
            },
        ),
        deadline,
    );
    assert!(terminal_id(&error).is_none());
    assert!(engine
        .raw_hold(camera(1), RawHoldScope::UnassignedExecution)
        .is_some());
    ack(&mut engine, 1, None, deadline);
    assert!(engine
        .raw_hold(camera(1), RawHoldScope::UnassignedExecution)
        .is_none());
    assert_eq!(
        terminal_id(&complete(&mut engine, 1, Some(ViscaSocket::S1), deadline)),
        Some(stop)
    );
}

#[test]
fn an_unassigned_ack_revokes_a_pending_named_cancellation_at_dispatch() {
    for transport in [TransportKind::Datagram, TransportKind::Stream] {
        let now = Instant::now();
        let step = Duration::from_millis(5);
        let mut engine = engine(EnvelopeKind::Raw, transport);
        engine
            .retune(step, Duration::ZERO, PerTarget::default(), now)
            .unwrap();
        let named = executing(&mut engine, 1, Some(ViscaSocket::S1), now);

        let at = now + step;
        let (write, _) = admit(&mut engine, 2, command(1), at);
        send_ok(&mut engine, &write, None, at);
        let cancel = engine.handle(engine.cancel_input(named), at);
        assert!(request_transmit_optional(&cancel).is_none());
        ack(&mut engine, 1, None, at);

        let refused = engine.advance(at + step);
        assert!(refused.iter().any(|effect| matches!(
            effect,
            Effect::CancellationObservation { id, observation: CancellationObservation::AttemptFailed(Error::CancellationUnconfirmed) } if *id == named
        )));
        assert!(request_transmit_optional(&refused).is_none());
        assert_eq!(socket_of(&engine, named), Some(ViscaSocket::S1));
        engine.assert_invariants().unwrap();
    }
}
