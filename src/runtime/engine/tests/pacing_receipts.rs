use super::*;

#[test]
fn request_pacing_uses_the_matching_write_receipt() {
    let command_spacing = Duration::from_millis(100);
    let inquiry_spacing = Duration::from_millis(150);
    for lane in [Lane::Command, Lane::Inquiry] {
        let spacing = match lane {
            Lane::Command => command_spacing,
            Lane::Inquiry => inquiry_spacing,
        };
        let request = |target, retry| match lane {
            Lane::Command => {
                command_with_reply_shape_and_retry(target, CommandReply::NoReply, retry)
            }
            Lane::Inquiry => inquiry_with_retry(target, retry),
        };
        // The final duration exceeds the whole-operation deadline. A receipt
        // can establish pacing even when it arrives too late to apply.
        for write_ms in [0, 40, 250, 450] {
            for result in [
                WriteOutcome::Sent(TransmissionMeta::Known),
                WriteOutcome::Failed(Error::io_timeout()),
                WriteOutcome::NotSent(Error::InvalidState {
                    reason: "refused before I/O".into(),
                }),
            ] {
                let start = Instant::now();
                let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
                engine.policy.command_spacing = command_spacing;
                engine.policy.inquiry_spacing = inquiry_spacing;
                let retry = RetryPolicy {
                    total_budget: Some(NonZeroDuration::new(Duration::from_millis(400)).unwrap()),
                    ..RetryPolicy::NEVER
                };
                let (staged, first_id) = admit(&mut engine, 1, request(1, retry), start);
                let (transmission, _, _) = request_transmit(&staged);
                assert_eq!(engine.pacing.last_request(), None);

                let io_attempted = !matches!(result, WriteOutcome::NotSent(_));
                let written = start + Duration::from_millis(write_ms);
                engine.handle(
                    Input::TransmissionFinished {
                        request: first_id,
                        transmission,
                        result,
                    },
                    written,
                );
                let last_write = io_attempted.then_some(written);
                assert_eq!(engine.pacing.last_request(), last_write);

                let late = if io_attempted {
                    written + Duration::from_millis(1)
                } else {
                    written
                };
                let stale = engine.handle(
                    Input::TransmissionFinished {
                        request: first_id,
                        transmission,
                        result: WriteOutcome::Sent(TransmissionMeta::Known),
                    },
                    late,
                );
                assert_eq!(
                    ignored_reasons(&stale),
                    vec![IgnoreReason::StaleTransmission]
                );
                assert_eq!(engine.pacing.last_request(), last_write);

                let (queued, next_id) = admit(&mut engine, 2, request(2, RetryPolicy::NEVER), late);
                if io_attempted {
                    assert!(request_transmit_optional(&queued).is_none());
                    let eligible = written + spacing;
                    let early = engine.advance(eligible - Duration::from_nanos(1));
                    assert!(request_transmit_optional(&early).is_none());
                    assert_eq!(request_transmit(&engine.advance(eligible)).1, next_id);
                } else {
                    assert_eq!(request_transmit(&queued).1, next_id);
                }
            }
        }
    }
}

#[test]
fn retry_receipt_starts_a_new_command_spacing_interval() {
    let start = Instant::now();
    let spacing = Duration::from_millis(100);
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    engine.policy.command_spacing = spacing;
    let (initial, id) = admit(
        &mut engine,
        1,
        command_with_retry(1, immediate_retry_budget(Duration::from_secs(1))),
        start,
    );
    let sequence = sony_sequence(id);
    let written = start + Duration::from_millis(40);
    send_ok(&mut engine, &initial, Some(sequence), written);
    let rejected = engine.handle(
        frame(
            1,
            Some(sequence),
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
            },
        ),
        written,
    );
    assert!(retry_scheduled(&rejected).is_some());
    assert!(request_transmit_optional(&rejected).is_none());
    let retry_at = written + spacing;
    let early = engine.advance(retry_at - Duration::from_nanos(1));
    assert!(request_transmit_optional(&early).is_none());
    let retry = engine.advance(retry_at);
    assert_eq!(request_transmit(&retry).1, id);
    let retry_written = retry_at + Duration::from_millis(250);
    send_ok(&mut engine, &retry, Some(sequence), retry_written);

    let (queued, stop_id) = admit(&mut engine, 2, urgent_command(2), retry_written);
    assert!(request_transmit_optional(&queued).is_none());
    let stop_at = retry_written + spacing;
    let early = engine.advance(stop_at - Duration::from_nanos(1));
    assert!(request_transmit_optional(&early).is_none());
    assert_eq!(request_transmit(&engine.advance(stop_at)).1, stop_id);
}

#[test]
fn expired_unwritten_halt_does_not_start_command_spacing() {
    let start = Instant::now();
    let deadline = start + Duration::from_millis(10);
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    engine.policy.command_spacing = Duration::from_millis(100);
    let mut stop = urgent_command(1);
    stop.context_mut().dispatch_deadline = Some(deadline);
    let (staged, id) = admit(&mut engine, 1, stop, start);
    let (transmission, _, _) = request_transmit(&staged);

    let mut effects = Vec::new();
    assert!(engine.expire_unwritten_halt_into(id, transmission, deadline, &mut effects));
    assert!(matches!(
        terminal_failure(&effects, id),
        Some(Error::Timeout { .. })
    ));
    assert_eq!(engine.pacing.last_request(), None);
    let (following, next_id) = admit(&mut engine, 2, urgent_command(1), deadline);
    assert_eq!(request_transmit(&following).1, next_id);
}
