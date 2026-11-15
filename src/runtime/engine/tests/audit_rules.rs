//! Corrupt-engine oracles for cross-index rules (#856).
use super::*;

#[test]
fn ended_session_cannot_retain_work() {
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        Instant::now(),
    );
    engine.state = SessionState::Closed;
    assert_eq!(
        engine.assert_invariants().unwrap_err().as_ref(),
        "ended session retains lifecycle or correlation work"
    );
}

#[test]
fn collection_bounds_are_independent_auditor_rules() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (write, id) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        start,
    );
    let transmission = request_transmit(&write).0;
    let owner = engine.transmissions[&transmission];
    for value in [800, 801] {
        engine.transmissions.insert(
            TransmissionId::from_nonzero(NonZeroU64::new(value).unwrap()),
            owner,
        );
    }
    assert_eq!(
        engine.assert_invariants().unwrap_err().as_ref(),
        "transmission bound exceeded"
    );
    engine.transmissions.retain(|key, _| *key == transmission);
    let owner = CorrelationOwner {
        request: id,
        generation: engine.entry(id).unwrap().generation,
        kind: CorrelationKind::Request,
    };
    engine.targets[TargetSlot::ALL[0]].raw_inquiries =
        std::iter::repeat_n(owner, engine.policy.inquiry_capacity + 1).collect();
    assert_eq!(
        engine.assert_invariants().unwrap_err().as_ref(),
        "raw inquiry FIFO bound exceeded"
    );
    engine.targets[TargetSlot::ALL[0]].raw_inquiries.clear();
    engine.displaced_unseen.extend([id; 3]);
    assert_eq!(
        engine.assert_invariants().unwrap_err().as_ref(),
        "displaced socket owner bound exceeded"
    );
    engine.displaced_unseen.clear();
    let retro = Retro {
        request: id,
        generation: owner.generation,
        evidence: Evidence::Other,
    };
    engine.retro.extend(std::iter::repeat_n(
        retro,
        stream_ledger::MAX_DISPUTE_HANDOFFS + 3,
    ));
    assert_eq!(
        engine.assert_invariants().unwrap_err().as_ref(),
        "retroactive evidence bound exceeded"
    );
}

#[test]
fn sequence_owners_need_matching_targets() {
    for lower_only in [false, true] {
        let start = Instant::now();
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let (write, id) = admit(
            &mut engine,
            1,
            command(1, CancellationPolicy::Supported),
            start,
        );
        send_ok(&mut engine, &write, Some(0x1001), start);
        if lower_only {
            engine.sequences.clear();
        }
        engine
            .entries
            .get_mut(&id)
            .unwrap()
            .request
            .context_mut()
            .target = camera(2);
        let expected = if lower_only {
            "lower16 owner is stale"
        } else {
            "sequence owner is stale or phase-incompatible"
        };
        assert_eq!(
            engine.assert_invariants().unwrap_err().as_ref(),
            expected,
            "target mismatch"
        );
        engine
            .entries
            .get_mut(&id)
            .unwrap()
            .request
            .context_mut()
            .target = camera(1);
    }
}

#[test]
fn representation_size_baseline() {
    // Command-only reply shape costs one aligned word in the request variant;
    // inquiry contexts no longer carry a meaningless command shape.
    assert!(size_of::<Entry>() <= 568, "Entry={}", size_of::<Entry>());
    assert!(size_of::<ProtocolEngine>() <= 3880);
    eprintln!(
        "Entry size={} ProtocolEngine size={}",
        size_of::<Entry>(),
        size_of::<ProtocolEngine>()
    );
}

#[cfg(debug_assertions)]
fn corrupt_dispatch_index() -> ProtocolEngine {
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (_, id) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        Instant::now(),
    );
    engine.inject_queue_ticket(
        Lane::Command,
        0,
        QueueTicket {
            request: id,
            generation: GenerationTicket(999),
            queue_generation: 999,
        },
    );
    engine
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "engine invariant violated")]
fn halt_runs_the_production_invariant_hook() {
    let mut engine = corrupt_dispatch_index();
    engine.halt(camera(1), crate::AffectedAxes::PAN_TILT, 0);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "engine invariant violated")]
fn expire_unwritten_halt_runs_the_hook_even_on_an_unknown_transmission() {
    let mut engine = corrupt_dispatch_index();
    engine.expire_unwritten_halt(
        TransmissionId::from_nonzero(NonZeroU64::new(999).unwrap()),
        Instant::now(),
    );
}

#[test]
fn ready_and_backoff_have_no_active_attempt() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (write, id) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        start,
    );
    assert!(engine.entry(id).unwrap().lifecycle.attempt().is_some());
    send_ok(&mut engine, &write, Some(0x1001), start);
    engine.handle(
        frame(
            1,
            Some((0x1001, SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: None,
                code: 0x03,
            },
        ),
        start,
    );
    assert!(matches!(
        engine.entry(id).unwrap().phase(),
        Phase::Backoff { .. }
    ));
    for ready in [false, true] {
        if ready {
            let previous = engine
                .entry(id)
                .unwrap()
                .lifecycle
                .previous_attempt()
                .unwrap();
            engine.promote_retry(id, previous, &mut Vec::new());
        }
        let entry = engine.entry(id).unwrap();
        assert!(entry.lifecycle.attempt().is_none());
        assert!(entry.lifecycle.previous_attempt().is_some());
        assert!(entry.dispatched_socket_capacity().is_none());
        assert!(entry.stream_order().is_none());
        engine.assert_invariants().unwrap();
    }
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "engine invariant violated")]
fn raw_release_gate_runs_the_hook_even_when_no_input_is_retained() {
    let mut engine = corrupt_dispatch_index();
    engine.resolve_raw_release_gate(Instant::now(), None);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "no entry may be Sending at an input boundary")]
fn unrelated_input_cannot_cross_an_unfinished_write() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (_, id) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        now,
    );
    engine.handle(Input::Cancel { id }, now);
}
