//! Sony R7 p. 5 sets independent wire intervals before cancel and restart.
use super::*;

use crate::error::{ErrorKind, Recovery};

const DRIVE_SEQUENCE: u32 = 0x1001;
const CANCEL_SEQUENCE: u32 = 0x1002;

fn engine_with_drive_rule() -> ProtocolEngine {
    let mut protocol = policy(EnvelopeKind::Sony, TransportKind::Datagram);
    protocol.command_spacing = Duration::from_millis(35);
    ProtocolEngine::new(
        protocol,
        target_policies(
            &[camera(1), camera(2)],
            TargetPolicy {
                pan_tilt_drive_cancellation_ms: crate::profiles::ProfileId::SonyFr7
                    .pan_tilt_drive_cancellation_ms(),
                ..TargetPolicy::test_default()
            },
        ),
        Instant::now(),
    )
    .unwrap()
}

fn drive(target: u8, stop: bool) -> RuntimeRequest {
    let mut request = if stop {
        urgent_command(target)
    } else {
        command(target)
    };
    let context = request.context_mut();
    context.pan_tilt_drive = true;
    context.retry = RetryPolicy::NEVER;
    context.timeout.ack = NonZeroDuration::HALF_SECOND;
    context.timeout.completion = NonZeroDuration::from_secs::<5>();
    context.timeout.ambiguity = NonZeroDuration::from_secs::<1>();
    request
}

fn assert_restart_blocked(error: &Error, stage: FailureStage, certainty: Certainty) {
    let context = FailureContext::new(stage, certainty);
    assert!(
        matches!(
            error,
            Error::PanTiltDriveRestartBlocked { camera, context: actual }
                if *camera == crate::IndividualCameraId::CAMERA_1 && *actual == context
        ),
        "{error:?}"
    );
    assert_eq!(error.failure_context(), Some(context));
    assert_eq!(error.kind(), ErrorKind::NotExecutable);
    assert!(!error.requires_new_session());
    assert_eq!(
        error.recovery(),
        if certainty == Certainty::Unconfirmed {
            Recovery::Reconcile
        } else {
            Recovery::Abandon
        }
    );
}

fn start_drive(engine: &mut ProtocolEngine, start: Instant, stop: bool) -> RequestId {
    let (write, drive_id) = admit(engine, 1, drive(1, stop), start);
    send_ok(
        engine,
        &write,
        Some(DRIVE_SEQUENCE),
        start + Duration::from_millis(7),
    );
    drive_id
}

fn ack_drive(engine: &mut ProtocolEngine, now: Instant) -> Vec<Effect> {
    engine.handle(
        frame(
            1,
            Some(DRIVE_SEQUENCE),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    )
}

fn complete_drive(engine: &mut ProtocolEngine, now: Instant) -> Vec<Effect> {
    engine.handle(
        frame(
            1,
            Some(DRIVE_SEQUENCE),
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    )
}

fn finish_cancel_write(engine: &mut ProtocolEngine, effects: &[Effect], now: Instant) {
    let (transmission, request, _) = cancel_transmit(effects);
    finish_write_input_only(
        engine,
        request,
        transmission,
        Ok(TransmissionMeta::Allocated {
            sequence: CANCEL_SEQUENCE,
        }),
        now,
    );
}

fn cancel_error(engine: &mut ProtocolEngine, code: u8, now: Instant) -> Vec<Effect> {
    engine.handle(
        frame(
            1,
            Some(CANCEL_SEQUENCE),
            DecodedResponse::Error {
                socket: Some(ViscaSocket::S1),
                code: code.into(),
            },
        ),
        now,
    )
}

#[test]
fn fr7_drive_cancel_and_restart_each_need_timing_and_wire_evidence() {
    // The first case needs the 200 ms floors, the second needs the late ACK
    // and late cancellation response. Swap directional and STOP encodings.
    for (stop, ack_at, cancel_at, canceled_after, restart_after) in
        [(false, 120, 207, 60, 200), (true, 240, 240, 240, 240)]
    {
        let mut engine = engine_with_drive_rule();
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        let drive_id = start_drive(&mut engine, start, stop);
        let intent = engine.handle(engine.cancel_input(drive_id), at(15));
        assert!(intent.iter().any(|effect| matches!(
            effect,
            Effect::CancellationRecorded { id } if *id == drive_id
        )));
        assert!(cancel_transmit_optional(&intent).is_none());
        let (waiting, next) = admit(&mut engine, 2, drive(1, !stop), at(35));
        assert!(request_transmit_optional(&waiting).is_none());

        let (zoom, _) = admit(&mut engine, 3, urgent_command(1), at(50));
        assert!(request_transmit_optional(&zoom).is_some());
        send_ok(&mut engine, &zoom, Some(0x1003), at(50));
        engine.handle(
            frame(
                1,
                Some(0x1003),
                DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S2),
                },
            ),
            at(51),
        );
        let (other_target, _) = admit(&mut engine, 4, drive(2, false), at(90));
        assert!(request_transmit_optional(&other_target).is_some());
        send_ok(&mut engine, &other_target, Some(0x1004), at(90));

        let early_ack = if ack_at < cancel_at {
            let effects = ack_drive(&mut engine, at(ack_at));
            assert!(cancel_transmit_optional(&effects).is_none());
            assert_eq!(engine.next_wake(at(ack_at)), Some(at(cancel_at)));
            true
        } else {
            false
        };
        let before = engine.advance(at(cancel_at - 1));
        assert!(cancel_transmit_optional(&before).is_none());
        assert!(request_transmit_optional(&before).is_none());

        let cancel = if early_ack {
            engine.advance(at(cancel_at))
        } else {
            ack_drive(&mut engine, at(ack_at))
        };
        assert_eq!(cancel_transmit(&cancel).1, drive_id);
        // Completing the driver write can take time. Its receipt, not the
        // scheduler's selection timestamp, starts the restart interval.
        let cancel_written = cancel_at + 8;
        finish_cancel_write(&mut engine, &cancel, at(cancel_written));
        let duplicate = engine.handle(engine.cancel_input(drive_id), at(cancel_written + 1));
        assert!(cancel_transmit_optional(&duplicate).is_none());

        let unrelated = engine.handle(
            frame(
                1,
                Some(DRIVE_SEQUENCE),
                DecodedResponse::Error {
                    socket: Some(ViscaSocket::S1),
                    code: 0x04.into(),
                },
            ),
            at(cancel_written + 2),
        );
        assert!(terminal_id(&unrelated).is_none());

        if canceled_after < restart_after {
            let reply = cancel_error(&mut engine, 0x04, at(cancel_written + canceled_after));
            assert!(request_transmit_optional(&reply).is_none());
        }
        let before = engine.advance(at(cancel_written + restart_after - 1));
        assert!(request_transmit_optional(&before).is_none());
        assert!(cancel_transmit_optional(&before).is_none());
        let restart = if canceled_after == restart_after {
            cancel_error(&mut engine, 0x04, at(cancel_written + canceled_after))
        } else {
            engine.advance(at(cancel_written + restart_after))
        };
        assert_eq!(request_transmit(&restart).1, next);
    }
}

#[test]
fn natural_completion_releases_the_gate_only_before_cancel_is_written() {
    for cancel_written in [false, true] {
        let mut engine = engine_with_drive_rule();
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        let drive_id = start_drive(&mut engine, start, false);
        ack_drive(&mut engine, at(10));
        engine.handle(engine.cancel_input(drive_id), at(15));
        let (_, next) = admit(&mut engine, 2, drive(1, false), at(50));

        if cancel_written {
            let cancel = engine.advance(at(207));
            finish_cancel_write(&mut engine, &cancel, at(207));
            let completion = complete_drive(&mut engine, at(250));
            assert!(request_transmit_optional(&completion).is_none());
            assert!(completion.iter().any(|effect| matches!(
                effect, Effect::Terminal {
                    id,
                    outcome: RuntimeOutcome::Command(CommandOutcome::Applied),
                } if *id == drive_id
            )));
            assert_restart_blocked(
                &terminal_failure(&completion, next).unwrap(),
                FailureStage::Terminal,
                Certainty::NotAccepted,
            );
            assert!(request_transmit_optional(&engine.advance(at(407))).is_none());
        } else {
            let completion = complete_drive(&mut engine, at(100));
            assert_eq!(request_transmit(&completion).1, next);
            assert!(cancel_transmit_optional(&completion).is_none());
            send_ok(&mut engine, &completion, Some(0x1003), at(100));
            assert!(cancel_transmit_optional(&engine.advance(at(207))).is_none());
        }
    }
}

#[test]
fn inconclusive_cancellation_refuses_only_the_same_targets_drive_family() {
    let mut engine = engine_with_drive_rule();
    let start = Instant::now();
    let at = |ms| start + Duration::from_millis(ms);
    let drive_id = start_drive(&mut engine, start, false);
    ack_drive(&mut engine, at(10));
    engine.handle(engine.cancel_input(drive_id), at(15));
    let (_, next) = admit(&mut engine, 2, drive(1, true), at(50));
    let cancel = engine.advance(at(207));
    finish_cancel_write(&mut engine, &cancel, at(207));

    let refusal = cancel_error(&mut engine, 0x05, at(220));
    assert!(request_transmit_optional(&refusal).is_none());
    assert_restart_blocked(
        &terminal_failure(&refusal, next).unwrap(),
        FailureStage::Terminal,
        Certainty::NotAccepted,
    );
    let turn = engine.begin_input_turn(at(250));
    assert_restart_blocked(
        &engine.admit_in_turn(&turn, drive(1, false)).unwrap_err(),
        FailureStage::PreAdmission,
        Certainty::NotAccepted,
    );
    let (zoom, _) = admit(&mut engine, 3, command(1), at(250));
    assert!(request_transmit_optional(&zoom).is_some());
    send_ok(&mut engine, &zoom, Some(0x1003), at(250));
    let (other_target, _) = admit(&mut engine, 4, drive(2, false), at(285));
    assert!(request_transmit_optional(&other_target).is_some());
}

#[test]
fn refused_drive_retry_preserves_uncertainty_from_its_prior_write() {
    let mut engine = engine_with_drive_rule();
    let start = Instant::now();
    let at = |ms| start + Duration::from_millis(ms);
    let drive_id = start_drive(&mut engine, start, false);
    ack_drive(&mut engine, at(10));

    let mut retrying_drive = drive(1, false);
    let context = retrying_drive.context_mut();
    context.timeout.ack = NonZeroDuration::new(Duration::from_millis(20)).unwrap();
    context.retry = retrying();
    let (write, retry_id) = admit(&mut engine, 2, retrying_drive, at(50));
    send_ok(&mut engine, &write, Some(0x1003), at(50));
    engine.handle(engine.cancel_input(drive_id), at(60));

    let retry = engine.advance(at(70));
    assert!(retry.iter().any(|effect| matches!(
        effect,
        Effect::RetryScheduled { id, .. } if *id == retry_id
    )));
    assert!(request_transmit_optional(&retry).is_none());
    let cancel = engine.advance(at(207));
    assert_eq!(cancel_transmit(&cancel).1, drive_id);
    finish_cancel_write(&mut engine, &cancel, at(207));

    let refusal = cancel_error(&mut engine, 0x05, at(210));
    assert!(request_transmit_optional(&refusal).is_none());
    assert_restart_blocked(
        &terminal_failure(&refusal, retry_id).unwrap(),
        FailureStage::Terminal,
        Certainty::Unconfirmed,
    );
}

#[test]
fn another_sony_profile_uses_ordinary_cancel_spacing() {
    let mut engine = engine_with_drive_rule();
    engine.targets[camera(1)]
        .policy
        .as_mut()
        .unwrap()
        .pan_tilt_drive_cancellation_ms =
        crate::profiles::ProfileId::SonyBrcH900.pan_tilt_drive_cancellation_ms();
    let start = Instant::now();
    let drive_id = start_drive(&mut engine, start, false);
    ack_drive(&mut engine, start + Duration::from_millis(10));
    engine.handle(
        engine.cancel_input(drive_id),
        start + Duration::from_millis(15),
    );
    // The preceding write returned at 7 ms. The 35 ms spacing starts there.
    let eligible = start + Duration::from_millis(42);
    let early = engine.advance(eligible - Duration::from_nanos(1));
    assert!(cancel_transmit_optional(&early).is_none());
    assert_eq!(cancel_transmit(&engine.advance(eligible)).1, drive_id);
}

#[test]
fn socket_reassignment_without_confirmation_keeps_the_drive_family_restricted() {
    let mut engine = engine_with_drive_rule();
    let start = Instant::now();
    let at = |ms| start + Duration::from_millis(ms);
    let drive_id = start_drive(&mut engine, start, false);
    ack_drive(&mut engine, at(10));
    engine.handle(engine.cancel_input(drive_id), at(15));
    let (_, next) = admit(&mut engine, 2, drive(1, true), at(50));

    let mut other = command(1);
    other.context_mut().timeout.ack = NonZeroDuration::HALF_SECOND;
    let (other, other_id) = admit(&mut engine, 3, other, at(165));
    send_ok(&mut engine, &other, Some(0x1003), at(165));
    let cancel = engine.advance(at(207));
    finish_cancel_write(&mut engine, &cancel, at(207));

    let reassignment = engine.handle(
        frame(
            1,
            Some(0x1003),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        at(210),
    );
    assert_eq!(
        engine.socket_owner(camera(1), ViscaSocket::S1),
        Some(other_id)
    );
    assert!(matches!(
        phase_of(&engine, drive_id),
        Some(Phase::ExecutingDisplaced { .. })
    ));
    assert_restart_blocked(
        &terminal_failure(&reassignment, next).unwrap(),
        FailureStage::Terminal,
        Certainty::NotAccepted,
    );
    assert!(request_transmit_optional(&engine.advance(at(407))).is_none());
}
