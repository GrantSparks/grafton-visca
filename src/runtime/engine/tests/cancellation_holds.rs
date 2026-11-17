//! Regression oracles for unconfirmed raw cancellation windows (#835).
use super::*;

fn completion_only() -> RuntimeRequest {
    let mut request =
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly);
    request.context_mut().timeout.completion =
        NonZeroDuration::new(Duration::from_secs(5)).unwrap();
    request
}

fn refused(effects: &[Effect], id: RequestId) -> bool {
    effects.iter().any(|effect| {
        matches!(effect, Effect::CancellationObservation {
        id: observed, observation: CancellationObservation::Failed(Error::CancellationNotSupported)
    } if *observed == id)
    })
}

#[test]
fn completion_only_cancel_preserves_window_and_late_completion_cannot_apply_successor() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (write, id) = admit(&mut engine, 1, completion_only(), now);
    send_ok(&mut engine, &write, None, now);
    let cancelled = engine.handle(Input::Cancel { id }, now + Duration::from_millis(1));
    assert!(refused(&cancelled, id));
    assert_eq!(engine.entry(id).unwrap().cancellation(), CancelState::None);
    assert!(engine
        .advance(now + Duration::from_millis(51))
        .iter()
        .all(|effect| !matches!(effect, Effect::Terminal { id: ended, .. } if *ended == id)));
    let (queued, successor) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        now + Duration::from_millis(52),
    );
    assert!(request_transmit_optional(&queued).is_none());
    let completion = engine.handle(
        frame(1, None, DecodedResponse::Completion { socket: None }),
        now + Duration::from_millis(60),
    );
    assert!(completion.iter().any(|effect| matches!(effect, Effect::Terminal { id: ended, outcome: RuntimeOutcome::Command(CommandOutcome::Applied) } if *ended == id)));
    assert!(!completion
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { id: ended, .. } if *ended == successor)));
    engine.assert_invariants().unwrap();
}

#[test]
fn completion_only_unconfirmed_still_holds_all_responses() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (write, id) = admit(&mut engine, 1, completion_only(), now);
    send_ok(&mut engine, &write, None, now);
    assert!(refused(&engine.handle(Input::Cancel { id }, now), id));
    let terminal = now + Duration::from_secs(5);
    assert!(terminal_failure(&engine.advance(terminal), id).is_some());
    assert!(
        engine
            .raw_hold(camera(1), RawHoldScope::AllResponses)
            .unwrap()
            .until
            > terminal
    );
}

#[test]
fn sending_command_cannot_record_cancellation_intent() {
    for shape in [
        ReplyShape::AckThenCompletion,
        ReplyShape::CompletionOnly,
        ReplyShape::NoReply,
    ] {
        let now = Instant::now();
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (write, id) = admit(
            &mut engine,
            1,
            command_with_reply_shape(1, CancellationPolicy::Supported, shape),
            now,
        );
        assert!(engine.assert_input_boundary().is_err());
        let mut sink = Vec::new();
        let mut effects = Decision::new(&mut sink);
        assert!(matches!(
            engine.cancel(id, now, &mut effects),
            Err(Error::CancellationNotSupported)
        ));
        assert!(!effects
            .as_slice()
            .iter()
            .any(|effect| matches!(effect, Effect::CancellationRecorded { .. })));
        assert!(!engine.entry(id).unwrap().cancellation_observation_open());
        send_ok(&mut engine, &write, None, now);
        assert!(engine.assert_input_boundary().is_ok());
    }
}

#[test]
fn unconfirmed_pre_ack_cancel_retains_a_future_pre_ack_hold() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (write, id) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &write, None, now);
    engine.handle(Input::Cancel { id }, now);
    let terminal = now + Duration::from_millis(50);
    assert!(terminal_failure(&engine.advance(terminal), id).is_some());
    assert!(
        engine
            .raw_hold(camera(1), RawHoldScope::PreAck)
            .unwrap()
            .until
            > terminal
    );
}

#[test]
fn unconfirmed_cancel_write_failure_and_resolution_expiry_retain_future_socket_hold() {
    for success in [false, true] {
        let now = Instant::now();
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (write, id) = admit(
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
                    socket: Some(ViscaSocket::S1),
                },
            ),
            now,
        );
        let cancel = engine.handle(Input::Cancel { id }, now);
        let (transmission, _, _) = cancel_transmit(&cancel);
        engine.handle(
            Input::TransmissionFinished {
                request: id,
                transmission,
                result: if success {
                    WriteOutcome::Sent(TransmissionMeta::for_sequence(None))
                } else {
                    WriteOutcome::Failed(Error::transport_error(std::io::Error::other(
                        "cancel write failed",
                    )))
                },
            },
            now,
        );
        let terminal = now + Duration::from_millis(50);
        assert!(terminal_failure(&engine.advance(terminal), id).is_some());
        assert!(
            engine
                .raw_hold(camera(1), RawHoldScope::Socket(ViscaSocket::S1))
                .unwrap()
                .until
                > terminal
        );
        engine.assert_invariants().unwrap();
    }
}

/// Cancellation must retain every ACK that the original response window admits,
/// even when the ambiguity interval elapsed long before that ACK arrives (#978).
#[test]
fn pre_ack_cancel_never_shortens_a_long_ack_window() {
    for (envelope, transport) in [
        (EnvelopeKind::Raw, TransportKind::Datagram),
        (EnvelopeKind::Raw, TransportKind::Stream),
        (EnvelopeKind::Sony, TransportKind::Datagram),
    ] {
        let now = Instant::now();
        let mut engine = engine(envelope, transport);
        let mut request = command(1, CancellationPolicy::Supported);
        request.context_mut().timeout.ack = NonZeroDuration::new(Duration::from_secs(1)).unwrap();
        let (write, id) = admit(&mut engine, 1, request, now);
        let sequence = (envelope == EnvelopeKind::Sony).then_some(0x1001);
        send_ok(&mut engine, &write, sequence, now);
        let requested = engine.handle(Input::Cancel { id }, now + Duration::from_millis(1));
        assert!(requested.iter().any(|effect| matches!(effect,
            Effect::CancellationObservation { id: observed, observation: CancellationObservation::Recorded } if *observed == id)));

        let early = engine.advance(now + Duration::from_millis(60));
        assert!(
            terminal_outcome(&early, id).is_none(),
            "{envelope:?}/{transport:?}"
        );
        assert_eq!(engine.next_due().unwrap().at, now + Duration::from_secs(1));
        assert!(engine
            .raw_correlation_releases_due(now + Duration::from_millis(60))
            .is_empty());

        let ack_at = now + Duration::from_millis(500);
        let ack = engine.handle(
            frame(
                1,
                sequence.map(|value| (value, SequenceWidth::Full32)),
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            ack_at,
        );
        assert!(
            ignored_reasons(&ack).is_empty(),
            "{envelope:?}/{transport:?}: {ack:?}"
        );
        assert!(terminal_outcome(&ack, id).is_none());
        let (transmission, cancelled_id, socket) = cancel_transmit(&ack);
        assert_eq!((cancelled_id, socket), (id, ViscaSocket::S1));
        assert_eq!(
            ack.iter()
                .filter(|effect| matches!(
                    effect,
                    Effect::Transmit {
                        kind: Transmission::Cancel { .. },
                        ..
                    }
                ))
                .count(),
            1
        );
        let cancel_sequence = sequence.map(|_| 0x2001);
        let sent = engine.handle(
            Input::TransmissionFinished {
                request: id,
                transmission,
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(cancel_sequence)),
            },
            ack_at,
        );
        assert!(terminal_outcome(&sent, id).is_none());
        let cancelled = engine.handle(
            frame(
                1,
                cancel_sequence.map(|value| (value, SequenceWidth::Full32)),
                DecodedResponse::Error {
                    socket: Some(ViscaSocket::S1),
                    code: 0x04.into(),
                },
            ),
            ack_at + Duration::from_millis(1),
        );
        assert!(matches!(
            terminal_outcome(&cancelled, id),
            Some(RuntimeOutcome::Command(CommandOutcome::Cancelled))
        ));
        engine.assert_invariants().unwrap();
    }
}

#[test]
fn pre_ack_cancel_expires_at_the_later_deadline_and_retains_a_full_raw_window() {
    for ack_ms in [20, 1_000] {
        for (envelope, transport) in [
            (EnvelopeKind::Raw, TransportKind::Datagram),
            (EnvelopeKind::Raw, TransportKind::Stream),
            (EnvelopeKind::Sony, TransportKind::Datagram),
        ] {
            let now = Instant::now();
            let mut engine = engine(envelope, transport);
            let mut request = command(1, CancellationPolicy::Supported);
            request.context_mut().timeout.ack =
                NonZeroDuration::new(Duration::from_millis(ack_ms)).unwrap();
            let (write, id) = admit(&mut engine, 1, request, now);
            send_ok(
                &mut engine,
                &write,
                (envelope == EnvelopeKind::Sony).then_some(0x1001),
                now,
            );
            engine.handle(Input::Cancel { id }, now + Duration::from_millis(1));
            let terminal = now + Duration::from_millis(ack_ms.max(51));
            assert!(
                terminal_outcome(&engine.advance(terminal - Duration::from_nanos(1)), id).is_none()
            );
            let expired = engine.advance(terminal);
            match envelope {
                EnvelopeKind::Raw => assert!(matches!(
                    terminal_failure(&expired, id),
                    Some(Error::UnsequencedCommandUnconfirmed)
                )),
                EnvelopeKind::Sony => assert!(matches!(
                    terminal_failure(&expired, id),
                    Some(Error::CancellationUnconfirmed)
                )),
            }
            assert!(engine.entry(id).is_none());
            let retained_until = terminal + Duration::from_millis(50);
            if envelope == EnvelopeKind::Raw {
                match transport {
                    TransportKind::Datagram => assert_eq!(
                        engine
                            .raw_hold(camera(1), RawHoldScope::PreAck)
                            .unwrap()
                            .until,
                        retained_until
                    ),
                    TransportKind::Stream => {
                        let debt = engine
                            .correlation
                            .ledger()
                            .entries()
                            .find(|(_, debt)| debt.request == id)
                            .unwrap()
                            .1;
                        assert!(
                            matches!(debt.standing, Standing::Owed { window_end, .. } if window_end == retained_until)
                        );
                    }
                }
            }
            engine.assert_invariants().unwrap();
        }
    }
}
