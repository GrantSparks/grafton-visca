//! Sequenced ACKs without a socket name prove acceptance and consume capacity.
use super::*;

const FIRST: u32 = 0x1101;
const SECOND: u32 = 0x1102;

fn reply(sequence: u32, response: DecodedResponse) -> Input {
    frame(1, Some(sequence), response)
}

fn assert_cancel_refused(effects: &[Effect], expected: RequestId) {
    assert!(
        effects.iter().any(|effect| matches!(
            effect,
            Effect::CancellationObservation {
                id,
                observation: CancellationObservation::AttemptFailed(Error::CancellationUnconfirmed),
            } if *id == expected
        )),
        "{effects:?}"
    );
    assert!(cancel_transmit_optional(effects).is_none(), "{effects:?}");
    assert!(terminal_id(effects).is_none(), "{effects:?}");
}

#[test]
fn sequenced_unnamed_ack_keeps_capacity_until_its_own_terminal() {
    for transport in [TransportKind::Datagram, TransportKind::Stream] {
        for socket in [None, Some(ViscaSocket::S1), Some(ViscaSocket::S2)] {
            let start = Instant::now();
            let mut engine = engine(EnvelopeKind::Sony, transport);
            let (first, first_id) = admit(&mut engine, 1, command(1), start);
            send_ok(&mut engine, &first, Some(FIRST), start);
            let accepted =
                engine.handle(reply(FIRST, DecodedResponse::Ack { socket: None }), start);
            assert!(matches!(
                phase_of(&engine, first_id),
                Some(Phase::ExecutingUnassigned { .. })
            ));
            assert!(terminal_id(&accepted).is_none());
            assert_eq!(engine.command_capacity_used(camera(1)), 1);
            assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S1), None);
            assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S2), None);

            let later = start + Duration::from_millis(1);
            let (second, second_id) = admit(&mut engine, 2, command(1), later);
            send_ok(&mut engine, &second, Some(SECOND), later);
            engine.handle(
                reply(
                    SECOND,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                later,
            );
            let (queued, third_id) = admit(&mut engine, 3, command(1), later);
            assert!(matches!(phase_of(&engine, third_id), Some(Phase::Ready)));
            assert!(!queued
                .iter()
                .any(|effect| matches!(effect, Effect::Transmit { .. })));
            assert_cancel_refused(
                &engine.handle(engine.cancel_input(first_id), later),
                first_id,
            );

            let wrong = engine.handle(
                reply(FIRST + 100, DecodedResponse::Completion { socket }),
                later,
            );
            assert_eq!(
                ignored_reasons(&wrong),
                vec![IgnoreReason::UnmatchedSequencedFrame]
            );
            assert!(terminal_id(&wrong).is_none());
            assert_eq!(engine.command_capacity_used(camera(1)), 2);

            let completion = reply(FIRST, DecodedResponse::Completion { socket });
            let done = engine.handle(completion, later);
            assert_eq!(
                done.iter()
                    .filter(|effect| matches!(
                        effect,
                        Effect::Terminal {
                            id,
                            outcome: RuntimeOutcome::Command(CommandOutcome::Applied),
                        } if *id == first_id
                    ))
                    .count(),
                1,
                "{done:?}"
            );
            assert_eq!(request_transmit(&done).1, third_id);
            assert_eq!(
                engine.socket_owner(camera(1), ViscaSocket::S1),
                Some(second_id)
            );
            engine.assert_invariants().unwrap();
        }
    }
}

#[test]
fn sequenced_unnamed_ack_refuses_pending_cancel_but_keeps_the_completion_deadline() {
    for terminal in [
        Some(DecodedResponse::Error {
            socket: Some(ViscaSocket::S2),
            code: CameraErrorCode::BufferFull,
        }),
        None,
    ] {
        let start = Instant::now();
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let (first, id) = admit(&mut engine, 1, command(1), start);
        send_ok(&mut engine, &first, Some(FIRST), start);
        let cancelling = engine.handle(engine.cancel_input(id), start);
        assert!(cancelling
            .iter()
            .any(|effect| matches!(effect, Effect::CancellationRecorded { id: expected } if *expected == id)));
        assert!(cancel_transmit_optional(&cancelling).is_none());

        let ack_at = start + Duration::from_millis(10);
        let acked = engine.handle(reply(FIRST, DecodedResponse::Ack { socket: None }), ack_at);
        assert_cancel_refused(&acked, id);
        let deadline = ack_at + Duration::from_millis(40);
        assert_eq!(
            phase_of(&engine, id),
            Some(Phase::ExecutingUnassigned {
                started_at: ack_at,
                deadline
            })
        );
        let before_deadline = deadline - Duration::from_millis(1);
        let pending = engine.advance(before_deadline);
        assert!(pending.is_empty(), "{pending:?}");
        let answered = terminal.is_some();
        let done = if let Some(response) = terminal {
            engine.handle(reply(FIRST, response), before_deadline)
        } else {
            engine.advance(deadline)
        };
        let error = terminal_failure(&done, id).expect("one accepted terminal failure");
        if answered {
            assert!(matches!(
                error,
                Error::CommandFailedAfterAck { source, .. } if matches!(*source, Error::CommandBufferFull)
            ));
        } else {
            assert!(matches!(error, Error::Timeout { .. }));
        }
        assert!(
            !done.iter().any(|effect| matches!(
                effect,
                Effect::Transmit { .. }
                    | Effect::RetryScheduled { .. }
                    | Effect::CancellationObservation { .. }
            )),
            "{done:?}"
        );
        assert_eq!(engine.command_capacity_used(camera(1)), 0);
        let duplicate = engine.handle(
            reply(FIRST, DecodedResponse::Completion { socket: None }),
            deadline,
        );
        assert!(terminal_id(&duplicate).is_none());
        assert_eq!(engine.command_capacity_used(camera(1)), 0);
        engine.assert_invariants().unwrap();
    }
}
