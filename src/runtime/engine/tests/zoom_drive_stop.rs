use super::*;

const HIGH_RESOLUTION_DRIVE: &[u8] = &[0x81, 1, 0x7e, 4, 0x17, 2, 0, 1, 0, 0, 0xff];
const STANDARD_DRIVE: &[u8] = &[0x81, 1, 4, 7, 2, 0xff];
const HIGH_RESOLUTION_STOP: &[u8] = &[0x81, 1, 0x7e, 4, 0x17, 0, 0, 0, 0, 0, 0xff];
const STANDARD_STOP: &[u8] = &[0x81, 1, 4, 7, 0, 0xff];

fn zoom_request(bytes: &[u8], reply_shape: CommandReply) -> RuntimeRequest {
    RuntimeRequest::Command {
        wire: wire_for_test(bytes),
        context: context(1),
        reply_shape,
        not_executable: false,
        applied_state: None,
    }
}

fn finish_command(engine: &mut ProtocolEngine, sequence: u32, now: Instant) {
    engine.handle(
        frame(
            1,
            Some(sequence),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    );
    engine.handle(
        frame(
            1,
            Some(sequence),
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    );
}

fn buffer_full(sequence: u32) -> Input {
    frame(
        1,
        Some(sequence),
        DecodedResponse::Error {
            socket: None,
            code: CameraErrorCode::BufferFull,
        },
    )
}

#[test]
fn halt_resolves_the_drive_present_at_dispatch_and_pins_the_wire_across_retry() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (effects, _) = admit(
        &mut engine,
        1,
        zoom_request(HIGH_RESOLUTION_DRIVE, CommandReply::AckThenCompletion),
        start,
    );
    send_ok(&mut engine, &effects, Some(1), start);
    finish_command(&mut engine, 1, start);

    let mut newer_drive = zoom_request(STANDARD_DRIVE, CommandReply::AckThenCompletion);
    // A conclusive error can arrive while the halt is admitted but not staged.
    newer_drive.context_mut().retry = RetryPolicy::NEVER;
    let (effects, drive) = admit(&mut engine, 2, newer_drive, start);
    send_ok(&mut engine, &effects, Some(2), start);

    let mut stop = zoom_request(STANDARD_STOP, CommandReply::Stop);
    stop.context_mut().dispatch_deadline = Some(start + Duration::from_secs(1));
    let turn = engine.begin_input_turn(start);
    let halt = engine.admit_in_turn(&turn, stop).expect("halt STOP admits");
    engine.offer_high_resolution_zoom_stop(halt, wire_for_test(HIGH_RESOLUTION_STOP));
    let rejected = engine.handle_turn(buffer_full(2), start, EngineTurn::InputOnly);
    assert_eq!(terminal_id(&rejected), Some(drive));
    let effects = engine.advance(start);
    let (_, written, payload) = request_transmit(&effects);
    assert_eq!(written, halt);
    assert_eq!(payload.as_bytes(), HIGH_RESOLUTION_STOP);
    send_ok(&mut engine, &effects, Some(3), start);
    let rejected = engine.handle(buffer_full(3), start);
    let (retry, _, retry_at) = retry_scheduled(&rejected).expect("STOP retry scheduled");
    assert_eq!(retry, halt);

    // This raw-equivalent request declares no motion axes. It can reach the
    // camera while the halt is in backoff and change the latest drive family.
    let (effects, _) = admit(
        &mut engine,
        3,
        zoom_request(STANDARD_DRIVE, CommandReply::AckThenCompletion),
        start,
    );
    send_ok(&mut engine, &effects, Some(4), start);
    finish_command(&mut engine, 4, start);
    assert!(!engine.targets[TargetSlot::CAMERA_1]
        .zoom_drive
        .use_high_resolution_stop());

    let retried = engine.advance(retry_at);
    let (_, written, payload) = request_transmit(&retried);
    assert_eq!(written, halt);
    assert_eq!(payload.as_bytes(), HIGH_RESOLUTION_STOP);
    assert!(retried.iter().any(|effect| matches!(
        effect,
        Effect::Transmit { kind: Transmission::Request { framing, .. }, .. }
            if framing.sony_sequence() == Some(SonyRequestSequence::Replay(3))
    )));
}
