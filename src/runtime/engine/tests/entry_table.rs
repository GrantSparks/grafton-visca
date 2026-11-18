//! Closed table mutations keep lookup indexes in step with one entry.
use super::*;

#[test]
fn displaced_entry_removal_cannot_release_the_successors_socket() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (first, predecessor) = admit(&mut engine, 1, command(1), now);
    send_ok(&mut engine, &first, Some(0x1001), now);
    engine.handle(
        frame(
            1,
            Some(0x1001),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    );
    let (second, successor) = admit(&mut engine, 2, command(1), now);
    send_ok(&mut engine, &second, Some(0x1002), now);
    let logical = engine.entry(predecessor).unwrap().lifecycle();
    engine
        .entries
        .transition(predecessor, EntryChange::DisplaceSocket)
        .unwrap();
    assert_eq!(engine.entry(predecessor).unwrap().lifecycle(), logical);
    assert!(engine
        .entry(predecessor)
        .unwrap()
        .due(predecessor)
        .is_none());
    assert_eq!(
        engine.entries.socket_owner(camera(1), ViscaSocket::S1),
        None
    );
    // One frame operation can assign its ACK to the successor before retiring
    // the displaced predecessor. No external input crosses this window.
    let Lifecycle::Executing {
        started_at,
        deadline,
        attempt,
        cancellation,
        ..
    } = logical
    else {
        unreachable!("executing predecessor")
    };
    let claim = engine
        .entries
        .claim_socket(successor, [ViscaSocket::S1])
        .unwrap();
    claim
        .install(SocketedLifecycle::Executing {
            started_at,
            deadline,
            attempt,
            cancellation,
        })
        .unwrap();
    engine.entries.remove(&predecessor).unwrap();
    assert_eq!(
        engine.entries.socket_owner(camera(1), ViscaSocket::S1),
        Some(successor)
    );

    let late = engine.handle(
        frame(
            1,
            Some(0x1001),
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    );
    assert_eq!(
        ignored_reasons(&late),
        [IgnoreReason::UnmatchedSequencedFrame]
    );
    assert!(engine.entry(successor).is_some());
    let completed = engine.handle(
        frame(
            1,
            Some(0x1002),
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    );
    assert_eq!(terminal_id(&completed), Some(successor));
    engine.assert_invariants().unwrap();
}

#[test]
fn occupied_socket_cannot_mint_an_installation_claim() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (first, predecessor) = admit(&mut engine, 1, command(1), now);
    send_ok(&mut engine, &first, Some(0x1001), now);
    engine.handle(
        frame(
            1,
            Some(0x1001),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    );
    let (_, successor) = admit(&mut engine, 2, command(1), now);
    assert!(engine
        .entries
        .claim_socket(successor, [ViscaSocket::S1])
        .is_none());
    assert!(engine
        .entries
        .claim_socket(predecessor, [ViscaSocket::S1])
        .is_some());
    assert!(engine
        .entries
        .claim_socket(successor, [ViscaSocket::S2])
        .is_some());
    engine.assert_invariants().unwrap();
}

#[test]
fn admission_and_transitions_never_duplicate_a_ready_position() {
    let now = Instant::now();
    let mut table = EntryTable::new();
    let id = RequestId::from_nonzero(NonZeroU64::new(1).unwrap());
    table.admit(id, inquiry(1), now);
    for _ in 0..3 {
        table
            .transition(
                id,
                EntryChange::Lifecycle(UnsocketedLifecycle::Ready {
                    previous_attempt: None,
                }),
            )
            .unwrap();
    }
    assert_eq!(
        table
            .ready(ControlClass::Normal.priority_index())
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [id]
    );
    table.remove(&id).unwrap();
    assert!(table
        .ready(ControlClass::Normal.priority_index())
        .is_empty());
}

#[test]
fn stop_class_alone_selects_the_highest_priority_ready_index() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let mut ordinary = command(1);
    ordinary.context_mut().control.class = ControlClass::User;
    let mut stop = urgent_command(1);
    // Even a lowest-priority context cannot demote the immutable STOP class.
    stop.context_mut().control.class = ControlClass::Background;
    let turn = engine.begin_input_turn(now);
    let user = engine.admit_in_turn(&turn, ordinary).unwrap();
    let safety = engine.admit_in_turn(&turn, stop).unwrap();
    assert_eq!(engine.entries.ready(3), &VecDeque::from([safety]));
    assert_eq!(engine.entries.ready(2), &VecDeque::from([user]));
    assert!(engine.entries.ready(0).is_empty());
    assert_eq!(engine.entry(safety).unwrap().request.priority_index(), 3);
    engine.assert_invariants().unwrap();
}

#[test]
fn retry_promotion_follows_waiting_work_of_either_protocol_lane() {
    let request_kinds: [fn(u8) -> RuntimeRequest; 2] = [command, inquiry];
    for make_first in request_kinds {
        for make_second in request_kinds {
            let start = Instant::now();
            let spacing = Duration::from_millis(100);
            let mut engine = ProtocolEngine::new(
                ProtocolPolicy {
                    command_spacing: spacing,
                    ..policy(EnvelopeKind::Sony, TransportKind::Datagram)
                },
                target_policies(&[camera(1)], TargetPolicy::test_default()),
                start,
            )
            .unwrap();
            let (first, first_id) = admit(&mut engine, 1, make_first(1), start);
            let sequence = sony_sequence(first_id);
            send_ok(&mut engine, &first, Some(sequence), start);

            let (second, second_id) = admit(
                &mut engine,
                2,
                make_second(1),
                start + Duration::from_millis(1),
            );
            assert!(request_transmit_optional(&second).is_none());
            let rejected = engine.handle(
                frame(
                    1,
                    Some(sequence),
                    DecodedResponse::Error {
                        socket: None,
                        code: 0x03.into(),
                    },
                ),
                start + Duration::from_millis(2),
            );
            let (retry_id, _, ready_at) = retry_scheduled(&rejected).expect("buffer-full retry");
            assert_eq!(retry_id, first_id);
            assert!(ready_at < start + spacing);
            assert!(request_transmit_optional(&engine.advance(ready_at)).is_none());

            let priority = ControlClass::Normal.priority_index();
            assert_eq!(
                engine.entries.ready(priority),
                &VecDeque::from([second_id, first_id])
            );
            let released = engine.advance(start + spacing);
            assert_eq!(request_transmit(&released).1, second_id);
            assert_eq!(engine.entries.ready(priority), &VecDeque::from([first_id]));
            engine.assert_invariants().unwrap();
        }
    }
}
