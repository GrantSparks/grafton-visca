//! The C16 declaration cap is a union of actual retained engine facts.
use super::*;

fn reservation_engine(transport: TransportKind, capacity: usize, now: Instant) -> ProtocolEngine {
    let mut configured = policy(EnvelopeKind::Raw, transport);
    configured.capacity = std::num::NonZeroUsize::new(capacity).unwrap();
    ProtocolEngine::new(
        configured,
        target_policies(&TargetSlot::ALL, TargetPolicy::test_default()),
        now,
    )
    .unwrap()
}

fn submit(
    engine: &mut ProtocolEngine,
    ticket: u64,
    target: u8,
    shape: PayloadShape,
    now: Instant,
    turn: EngineTurn,
) -> Vec<Effect> {
    engine.handle_turn(
        Input::Admit {
            ticket: AdmissionTicket(ticket),
            request: inquiry_with_shape(target, shape),
        },
        now,
        turn,
    )
}

fn assert_shape_capacity(effects: &[Effect], target: u8) {
    assert!(effects.iter().any(|effect| matches!(effect,
        Effect::AdmissionRejected { error: Error::InquiryShapeCapacityExhausted { camera: refused, capacity: 16 }, .. }
            if *refused == camera(target).individual_camera()
    )), "{effects:?}");
    assert!(!effects
        .iter()
        .any(|effect| matches!(effect, Effect::Admitted { .. } | Effect::Transmit { .. })));
}

#[test]
fn queued_exact_declarations_reserve_c16_independently_on_all_seven_targets() {
    let now = Instant::now();
    let mut engine = reservation_engine(TransportKind::Datagram, 128, now);
    let declarations: Vec<_> = (1..=14)
        .map(PayloadShape::Bytes)
        .chain([PayloadShape::Nibbles(4), PayloadShape::Unknown])
        .collect();
    let mut ticket = 0;
    for target in 1..=7 {
        for shape in declarations.iter().copied() {
            ticket += 1;
            let effects = submit(
                &mut engine,
                ticket,
                target,
                shape,
                now,
                EngineTurn::InputOnly,
            );
            assert!(matches!(
                phase_of(&engine, admitted(&effects)),
                Some(Phase::Ready)
            ));
            assert!(request_transmit_optional(&effects).is_none());
        }
        assert_eq!(
            engine.tracked_inquiry_shapes(camera(target)).unwrap().len(),
            16
        );
        ticket += 1;
        assert_shape_capacity(
            &submit(
                &mut engine,
                ticket,
                target,
                PayloadShape::Bytes(15),
                now,
                EngineTurn::InputOnly,
            ),
            target,
        );
        // Unknown and the byte/nibble overlap above reserve distinct facts;
        // an already reserved declaration still fits without merging classes.
        ticket += 1;
        let duplicate = submit(
            &mut engine,
            ticket,
            target,
            PayloadShape::Nibbles(4),
            now,
            EngineTurn::InputOnly,
        );
        assert!(engine.entry(admitted(&duplicate)).is_some());
        assert_eq!(
            engine.tracked_inquiry_shapes(camera(target)).unwrap().len(),
            16
        );
        ticket += 1;
        let command = engine.handle_turn(
            Input::Admit {
                ticket: AdmissionTicket(ticket),
                request: command(target),
            },
            now,
            EngineTurn::InputOnly,
        );
        assert!(engine.entry(admitted(&command)).is_some());
    }
    assert_eq!(engine.active_len(), 126);
    engine.assert_invariants().unwrap();
}

#[test]
fn sending_waiting_backoff_ready_and_skew_holds_keep_the_same_reservation() {
    let now = Instant::now();
    let mut engine = reservation_engine(TransportKind::Datagram, 64, now);
    let mut ids = Vec::new();
    for length in 1..=16 {
        let write = submit(
            &mut engine,
            length as u64,
            1,
            PayloadShape::Bytes(length),
            now,
            EngineTurn::Complete,
        );
        let id = admitted(&write);
        ids.push(id);
        assert!(matches!(phase_of(&engine, id), Some(Phase::Sending { .. })));
        assert_eq!(
            engine.tracked_inquiry_shapes(camera(1)).unwrap().len(),
            length
        );
        send_ok(&mut engine, &write, None, now);
        assert!(matches!(
            phase_of(&engine, id),
            Some(Phase::AwaitingReply { .. })
        ));
    }
    assert_shape_capacity(
        &submit(
            &mut engine,
            17,
            1,
            PayloadShape::Bytes(17),
            now,
            EngineTurn::InputOnly,
        ),
        1,
    );

    let timed_out = engine.advance(now + Duration::from_millis(30));
    assert!(request_transmit_optional(&timed_out).is_none());
    assert!(ids
        .iter()
        .all(|id| matches!(phase_of(&engine, *id), Some(Phase::Backoff { .. }))));
    assert_eq!(engine.tracked_inquiry_shapes(camera(1)).unwrap().len(), 16);
    assert_eq!(engine.raw_inquiry_holds(camera(1)).count(), 16);
    assert_shape_capacity(
        &submit(
            &mut engine,
            18,
            1,
            PayloadShape::Bytes(17),
            now + Duration::from_millis(30),
            EngineTurn::InputOnly,
        ),
        1,
    );

    let ready_at = now + Duration::from_millis(45);
    let ready = engine.advance(ready_at);
    assert!(
        request_transmit_optional(&ready).is_none(),
        "a retry cannot cross its own skew hold"
    );
    assert!(ids
        .iter()
        .all(|id| matches!(phase_of(&engine, *id), Some(Phase::Ready))));
    assert_shape_capacity(
        &submit(
            &mut engine,
            19,
            1,
            PayloadShape::Bytes(17),
            ready_at,
            EngineTurn::InputOnly,
        ),
        1,
    );
    engine.assert_invariants().unwrap();

    // A separate no-retry session isolates declarations retained only by
    // finite datagram skew holds, after their admitted entries have ended.
    let mut engine = reservation_engine(TransportKind::Datagram, 64, now);
    for length in 1..=16 {
        let mut request = inquiry_with_shape(1, PayloadShape::Bytes(length));
        request.context_mut().retry = RetryPolicy::NEVER;
        let write = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(length as u64),
                request,
            },
            now,
        );
        send_ok(&mut engine, &write, None, now);
    }
    engine.advance(now + Duration::from_millis(30));
    assert!(engine.entries.is_empty());
    assert_eq!(engine.raw_inquiry_holds(camera(1)).count(), 16);
    assert_eq!(engine.tracked_inquiry_shapes(camera(1)).unwrap().len(), 16);
    assert_shape_capacity(
        &submit(
            &mut engine,
            19,
            1,
            PayloadShape::Bytes(17),
            ready_at,
            EngineTurn::InputOnly,
        ),
        1,
    );

    let released_at = now + Duration::from_millis(80);
    engine.advance(released_at);
    assert!(engine.tracked_inquiry_shapes(camera(1)).unwrap().is_empty());
    let admitted = submit(
        &mut engine,
        20,
        1,
        PayloadShape::Bytes(17),
        released_at,
        EngineTurn::Complete,
    );
    assert!(request_transmit_optional(&admitted).is_some());
    engine.assert_invariants().unwrap();
}

#[test]
fn unpaid_stream_declarations_survive_time_and_release_only_after_evidence_and_skew() {
    let now = Instant::now();
    let mut engine = reservation_engine(TransportKind::Stream, 32, now);
    for length in 1..=16 {
        let write = submit(
            &mut engine,
            length as u64,
            1,
            PayloadShape::Bytes(length),
            now,
            EngineTurn::Complete,
        );
        send_ok(&mut engine, &write, None, now);
    }
    engine.advance(now + Duration::from_millis(30));
    assert!(engine.entries.is_empty());
    assert_eq!(
        engine
            .correlation
            .ledger()
            .inquiry_shapes(camera(1))
            .count(),
        16
    );
    assert_eq!(engine.raw_inquiry_holds(camera(1)).count(), 16);
    assert_eq!(engine.tracked_inquiry_shapes(camera(1)).unwrap().len(), 16);

    let latched_at = now + Duration::from_millis(80);
    engine.advance(latched_at);
    assert_eq!(
        engine
            .correlation
            .ledger()
            .inquiry_shapes(camera(1))
            .count(),
        16,
        "time cannot discard an unpaid answer"
    );
    assert_shape_capacity(
        &submit(
            &mut engine,
            17,
            1,
            PayloadShape::Bytes(17),
            latched_at,
            EngineTurn::InputOnly,
        ),
        1,
    );
    let existing = submit(
        &mut engine,
        18,
        1,
        PayloadShape::Bytes(1),
        latched_at,
        EngineTurn::InputOnly,
    );
    assert!(existing.iter().any(|effect| matches!(
        effect,
        Effect::AdmissionRejected {
            error: Error::InquiryCorrelationLost { .. },
            ..
        }
    )));
    let other = submit(
        &mut engine,
        19,
        2,
        PayloadShape::Bytes(17),
        latched_at,
        EngineTurn::Complete,
    );
    send_ok(&mut engine, &other, None, latched_at);

    // A structurally legal answer for the last write proves every earlier
    // first answer passed it on this ordered stream. All declarations remain
    // reserved by their exact skew holds until that finite window ends.
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                payload: SmallVec::from_slice(&[1; 16]),
            },
        ),
        latched_at,
    );
    assert_eq!(
        engine
            .correlation
            .ledger()
            .inquiry_shapes(camera(1))
            .count(),
        0
    );
    assert_eq!(engine.tracked_inquiry_shapes(camera(1)).unwrap().len(), 16);
    assert_shape_capacity(
        &submit(
            &mut engine,
            20,
            1,
            PayloadShape::Bytes(17),
            latched_at,
            EngineTurn::InputOnly,
        ),
        1,
    );
    let released_at = latched_at + Duration::from_millis(50);
    engine.advance(released_at);
    assert!(engine.tracked_inquiry_shapes(camera(1)).unwrap().is_empty());
    let new_class = submit(
        &mut engine,
        21,
        1,
        PayloadShape::Bytes(17),
        released_at,
        EngineTurn::Complete,
    );
    assert!(request_transmit_optional(&new_class).is_some());
    engine.assert_invariants().unwrap();
}

#[test]
fn a_retry_cannot_share_its_overlap_with_a_queued_successor_at_wide_capacity() {
    let now = Instant::now();
    let mut engine = reservation_engine(TransportKind::Datagram, 32, now);
    let first = submit(
        &mut engine,
        1,
        1,
        PayloadShape::Bytes(1),
        now,
        EngineTurn::Complete,
    );
    let first_id = admitted(&first);
    send_ok(&mut engine, &first, None, now);
    let second = submit(
        &mut engine,
        2,
        1,
        PayloadShape::Nibbles(1),
        now,
        EngineTurn::Complete,
    );
    let second_id = admitted(&second);
    assert!(request_transmit_optional(&second).is_none());
    let mut disjoint = inquiry_with_shape(1, PayloadShape::Bytes(4));
    disjoint.context_mut().timeout.inquiry =
        NonZeroDuration::new(Duration::from_millis(300)).unwrap();
    let disjoint_write = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: disjoint,
        },
        now,
    );
    let disjoint_id = admitted(&disjoint_write);
    send_ok(&mut engine, &disjoint_write, None, now);

    let timed_out = engine.advance(now + Duration::from_millis(30));
    assert!(retry_scheduled(&timed_out).is_some());
    assert!(request_transmit_optional(&timed_out).is_none());
    assert!(matches!(
        phase_of(&engine, first_id),
        Some(Phase::Backoff { .. })
    ));
    assert!(matches!(phase_of(&engine, second_id), Some(Phase::Ready)));
    assert!(matches!(
        phase_of(&engine, disjoint_id),
        Some(Phase::AwaitingReply { .. })
    ));

    let released_at = now + Duration::from_millis(80);
    let released = engine.advance(released_at);
    assert_eq!(request_transmit(&released).1, second_id);
    let sent = send_ok(&mut engine, &released, None, released_at);
    assert!(
        request_transmit_optional(&sent).is_none(),
        "the retry must wait for its overlapping successor"
    );
    assert!(matches!(phase_of(&engine, first_id), Some(Phase::Ready)));
    assert!(matches!(
        phase_of(&engine, second_id),
        Some(Phase::AwaitingReply { .. })
    ));
    let answered = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                payload: smallvec![1],
            },
        ),
        released_at,
    );
    assert_eq!(terminal_id(&answered), Some(second_id));
    assert_eq!(request_transmit(&answered).1, first_id);
    assert!(matches!(
        phase_of(&engine, disjoint_id),
        Some(Phase::AwaitingReply { .. })
    ));
    engine.assert_invariants().unwrap();
}
