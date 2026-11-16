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
