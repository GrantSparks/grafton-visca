//! Closed table mutations keep lookup indexes in step with one entry.
use super::*;

#[test]
fn displaced_entry_removal_cannot_release_the_successors_socket() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (first, predecessor) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        now,
    );
    send_ok(&mut engine, &first, Some(0x1001), now);
    engine.handle(
        frame(
            1,
            Some((0x1001, SequenceWidth::Full32)),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    );
    let (second, successor) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        now,
    );
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
    engine
        .entries
        .transition(successor, EntryChange::Lifecycle(logical))
        .unwrap();
    engine.entries.remove(&predecessor).unwrap();
    assert_eq!(
        engine.entries.socket_owner(camera(1), ViscaSocket::S1),
        Some(successor)
    );

    let late = engine.handle(
        frame(
            1,
            Some((0x1001, SequenceWidth::Full32)),
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
            Some((0x1002, SequenceWidth::Full32)),
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
fn evicting_a_full_sequence_keeps_the_other_historys_lower16_owner() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (_, id) = admit(&mut engine, 1, inquiry(1), now);
    for sequence in [
        0x0001_beef,
        0x0002_beef,
        0x1003,
        0x1004,
        0x1005,
        0x1006,
        0x1007,
        0x1008,
        0x1009,
    ] {
        engine
            .entries
            .transition(
                id,
                EntryChange::Written {
                    receipt: TransmissionMeta::Sony { sequence },
                    kind: CorrelationKind::Request,
                },
            )
            .unwrap();
    }
    assert_eq!(
        engine.entry(id).unwrap().sequence_history.len(),
        MAX_SEQUENCE_HISTORY
    );
    assert!(engine.entries.exact_owners(0x0001_beef).is_empty());
    assert_eq!(engine.entries.exact_owners(0x0002_beef).len(), 1);
    assert_eq!(engine.entries.lower_owners(0xbeef).len(), 1);
    assert_eq!(
        engine.resolve_sequence(
            camera(1),
            EnvelopeSequence {
                value: 0xbeef,
                width: SequenceWidth::Lower16
            }
        ),
        Ok((id, CorrelationKind::Request))
    );
    engine.entries.remove(&id).unwrap();
    assert!(engine.entries.lower_owners(0xbeef).is_empty());
}

#[test]
fn admission_and_transitions_never_duplicate_a_ready_position() {
    let now = Instant::now();
    let mut table = EntryTable::new();
    let id = RequestId::from_nonzero(NonZeroU64::new(1).unwrap());
    let facts = AdmissionFacts {
        order: 0,
        submitted_at: now,
        generation: GenerationTicket(1),
        slot: AdmissionSlot::Ordinary,
    };
    assert!(table.admit(id, inquiry(1), facts));
    assert!(!table.admit(id, inquiry(1), facts));
    for _ in 0..3 {
        table
            .transition(
                id,
                EntryChange::Lifecycle(Lifecycle::Ready {
                    previous_attempt: None,
                }),
            )
            .unwrap();
    }
    assert_eq!(
        table
            .ready(Lane::Inquiry, ControlClass::Normal.priority_index())
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [id]
    );
    table.remove(&id).unwrap();
    assert!(table
        .ready(Lane::Inquiry, ControlClass::Normal.priority_index())
        .is_empty());
}
