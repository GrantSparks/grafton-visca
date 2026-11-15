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
fn sequence_owners_need_target_and_reply_phase_compatibility() {
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
        assert!(
            engine.assert_invariants().is_err(),
            "target mismatch must be audited in both indexes"
        );
        engine
            .entries
            .get_mut(&id)
            .unwrap()
            .request
            .context_mut()
            .target = camera(1);
        engine.entries.get_mut(&id).unwrap().phase = Phase::AwaitingReply {
            sent_at: start,
            deadline: start + Duration::from_secs(1),
        };
        assert!(
            engine.assert_invariants().is_err(),
            "reply shape mismatch must be audited in both indexes"
        );
    }
}

#[test]
fn representation_size_baseline() {
    assert!(size_of::<Entry>() <= 560);
    assert!(size_of::<ProtocolEngine>() <= 3880);
    eprintln!(
        "Entry size={} ProtocolEngine size={}",
        size_of::<Entry>(),
        size_of::<ProtocolEngine>()
    );
}
