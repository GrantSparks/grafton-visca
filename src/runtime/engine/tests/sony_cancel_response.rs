//! Sony gives an emitted cancellation and its original request separate replies.
use super::*;

const COMMAND_SEQUENCE: u32 = 0x1001;
const CANCEL_SEQUENCE: u32 = 0x1002;

fn completion() -> Input {
    frame(
        1,
        Some(COMMAND_SEQUENCE),
        DecodedResponse::Completion {
            socket: Some(ViscaSocket::S1),
        },
    )
}

fn cancel_reply(response: DecodedResponse) -> Input {
    frame(1, Some(CANCEL_SEQUENCE), response)
}

fn cancel_error(socket: Option<ViscaSocket>, code: CameraErrorCode) -> Input {
    cancel_reply(DecodedResponse::Error { socket, code })
}

fn start_cancellation(now: Instant) -> (ProtocolEngine, RequestId) {
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (write, id) = admit(&mut engine, 1, command(1), now);
    send_ok(&mut engine, &write, Some(COMMAND_SEQUENCE), now);
    engine.handle(
        frame(
            1,
            Some(COMMAND_SEQUENCE),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    );
    let effect = engine.handle(engine.cancel_input(id), now);
    let (transmission, cancelling, socket) = cancel_transmit(&effect);
    assert_eq!((cancelling, socket), (id, ViscaSocket::S1));
    finish_write_input_only(
        &mut engine,
        id,
        transmission,
        Ok(TransmissionMeta::Allocated {
            sequence: CANCEL_SEQUENCE,
        }),
        now,
    );
    (engine, id)
}

#[test]
fn sony_no_socket_and_natural_completion_agree_across_reply_order() {
    for refusal_first in [true, false] {
        let now = Instant::now();
        let (mut engine, id) = start_cancellation(now);
        let refused = || cancel_error(Some(ViscaSocket::S1), CameraErrorCode::NoSocket);
        let applied = if refusal_first {
            let response = engine.handle(refused(), now);
            assert!(terminal_id(&response).is_none());
            assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S1), None);

            let duplicate = engine.handle(refused(), now);
            assert!(terminal_id(&duplicate).is_none());
            assert!(request_transmit_optional(&duplicate).is_none());
            assert!(cancel_transmit_optional(&duplicate).is_none());
            engine.assert_invariants().unwrap();
            engine.handle(completion(), now)
        } else {
            let applied = engine.handle(completion(), now);
            assert!(terminal_id(&engine.handle(refused(), now)).is_none());
            applied
        };
        assert!(matches!(
            terminal_outcome(&applied, id),
            Some(RuntimeOutcome::Command(CommandOutcome::Applied))
        ));
        assert!(applied.iter().any(|effect| matches!(
            effect,
            Effect::CancellationObservation {
                id: observed,
                observation: CancellationObservation::Concluded(CancellationConclusion::Completed)
            } if *observed == id
        )));
        assert!(terminal_id(&engine.handle(completion(), now)).is_none());
        assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S1), None);
        engine.assert_invariants().unwrap();
    }
}

#[test]
fn sony_cancel_sequence_requires_a_matching_named_cancel_error() {
    let now = Instant::now();
    let (mut engine, id) = start_cancellation(now);
    for input in [
        cancel_reply(DecodedResponse::Ack {
            socket: Some(ViscaSocket::S1),
        }),
        cancel_reply(DecodedResponse::Completion {
            socket: Some(ViscaSocket::S1),
        }),
        cancel_reply(DecodedResponse::Completion {
            socket: Some(ViscaSocket::S2),
        }),
        cancel_reply(DecodedResponse::InquiryReply {
            payload: smallvec![0x02],
        }),
        cancel_error(Some(ViscaSocket::S2), CameraErrorCode::Cancelled),
        cancel_error(Some(ViscaSocket::S2), CameraErrorCode::NoSocket),
        cancel_error(Some(ViscaSocket::S1), CameraErrorCode::Syntax),
        cancel_error(None, CameraErrorCode::Cancelled),
        cancel_error(None, CameraErrorCode::NoSocket),
    ] {
        let ignored = engine.handle(input, now);
        assert_eq!(ignored_reasons(&ignored), [IgnoreReason::UnmatchedFrame]);
        assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S1), Some(id));
        assert!(terminal_id(&ignored).is_none());
    }
    let confirm = || cancel_error(Some(ViscaSocket::S1), CameraErrorCode::Cancelled);
    let cancelled = engine.handle(confirm(), now);
    assert!(matches!(
        terminal_outcome(&cancelled, id),
        Some(RuntimeOutcome::Command(CommandOutcome::Cancelled))
    ));
    assert!(terminal_id(&engine.handle(confirm(), now)).is_none());
    assert!(terminal_id(&engine.handle(completion(), now)).is_none());
    assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S1), None);
    engine.assert_invariants().unwrap();
}

#[test]
fn sony_no_socket_keeps_the_original_error_or_completion_deadline() {
    for original_error in [true, false] {
        let now = Instant::now();
        let (mut engine, id) = start_cancellation(now);
        let refusal = engine.handle(
            cancel_error(Some(ViscaSocket::S1), CameraErrorCode::NoSocket),
            now + Duration::from_millis(1),
        );
        assert!(terminal_id(&refusal).is_none());
        assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S1), None);
        assert_eq!(engine.next_wake(now), Some(now + Duration::from_millis(40)));

        // The released physical socket can be assigned before the original
        // command settles, and a late original terminal cannot release it.
        let at = now + Duration::from_millis(20);
        let (successor, successor_id) = admit(&mut engine, 2, command(1), at);
        let successor_sequence = 0x1003;
        send_ok(&mut engine, &successor, Some(successor_sequence), at);
        engine.handle(
            frame(
                1,
                Some(successor_sequence),
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            at,
        );
        let terminal = if original_error {
            engine.handle(
                frame(
                    1,
                    Some(COMMAND_SEQUENCE),
                    DecodedResponse::Error {
                        socket: Some(ViscaSocket::S1),
                        code: CameraErrorCode::NotExecutable,
                    },
                ),
                at,
            )
        } else {
            engine.advance(now + Duration::from_millis(40))
        };
        assert!(
            matches!(
                (original_error, terminal_failure(&terminal, id)),
                (true, Some(Error::CommandFailedAfterAck { source }))
                    if matches!(*source, Error::CommandNotExecutable)
            ) || matches!(
                (original_error, terminal_failure(&terminal, id)),
                (false, Some(Error::Timeout { .. }))
            )
        );
        assert_eq!(
            engine.socket_owner(camera(1), ViscaSocket::S1),
            Some(successor_id)
        );
        assert!(request_transmit_optional(&terminal).is_none());
        assert!(cancel_transmit_optional(&terminal).is_none());
        assert!(!terminal
            .iter()
            .any(|effect| matches!(effect, Effect::RetryScheduled { .. })));
        assert!(
            terminal_id(&engine.handle(completion(), now + Duration::from_millis(41))).is_none()
        );
        engine.assert_invariants().unwrap();
    }
}
