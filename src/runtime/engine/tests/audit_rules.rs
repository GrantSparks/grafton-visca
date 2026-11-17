//! Retained semantic safety audits and boundary hooks (#856, #859).
use super::*;

#[test]
fn ended_session_cannot_retain_work() {
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    admit(&mut engine, 1, command(1), Instant::now());
    engine.session = EngineSession::Ended(SessionEnd {
        status: EndStatus::Closed,
        cause: Error::ConnectionClosed { source: None },
    });
    assert_eq!(
        engine.assert_invariants().unwrap_err().as_ref(),
        "ended session retains lifecycle or correlation work"
    );
}

#[test]
fn representation_size_baseline() {
    // Fixed sequence slots reduce the prior 536-byte entry without losing facts.
    assert!(size_of::<Entry>() <= 456, "Entry={}", size_of::<Entry>());
    assert!(
        size_of::<ProtocolEngine>() <= 2968,
        "ProtocolEngine={}",
        size_of::<ProtocolEngine>()
    );
    eprintln!(
        "Entry size={} ProtocolEngine size={} TargetState size={}",
        size_of::<Entry>(),
        size_of::<ProtocolEngine>(),
        size_of::<TargetState>()
    );
}

#[test]
fn request_pacing_follows_the_latest_lane_write() {
    let start = Instant::now();
    let mut pacing = Pacing::default();
    assert_eq!(pacing.last_request(), None);
    pacing.record(Lane::Command, start);
    pacing.record(Lane::Inquiry, start + Duration::from_millis(1));
    assert_eq!(
        pacing.last_request(),
        Some(start + Duration::from_millis(1))
    );
    assert_eq!(pacing.last_command, Some(start));
    pacing.record(Lane::Command, start + Duration::from_millis(2));
    assert_eq!(
        pacing.last_request(),
        Some(start + Duration::from_millis(2))
    );
    assert_eq!(pacing.last_inquiry, Some(start + Duration::from_millis(1)));
}

#[cfg(debug_assertions)]
fn corrupt_capacity() -> ProtocolEngine {
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let turn = engine.begin_input_turn(Instant::now());
    for _ in 0..2 {
        engine
            .admit_in_turn(&turn, command(1), AdmissionSlot::Ordinary)
            .unwrap();
    }
    // Lower an existing valid proof beneath the admitted population.
    engine.policy.capacity = std::num::NonZeroUsize::MIN;
    engine
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "engine invariant violated")]
fn halt_runs_the_production_invariant_hook() {
    let mut engine = corrupt_capacity();
    engine.halt_in_turn(
        &engine.begin_input_turn(Instant::now()),
        camera(1),
        crate::AffectedAxes::PAN_TILT,
        0,
    );
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "engine invariant violated")]
fn expire_unwritten_halt_runs_the_hook_even_on_an_unknown_transmission() {
    let mut engine = corrupt_capacity();
    engine.expire_unwritten_halt(
        RequestId::from_nonzero(NonZeroU64::new(999).unwrap()),
        TransmissionId::from_nonzero(NonZeroU64::new(999).unwrap()),
        Instant::now(),
    );
}

#[test]
fn ready_and_backoff_have_no_active_attempt() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (write, id) = admit(&mut engine, 1, command(1), start);
    assert!(engine.entry(id).unwrap().lifecycle().attempt().is_some());
    send_ok(&mut engine, &write, Some(0x1001), start);
    engine.handle(
        frame(
            1,
            Some(0x1001),
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
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
                .lifecycle()
                .previous_attempt()
                .unwrap();
            engine.promote_retry(id, previous, start, &mut Decision::new(&mut Vec::new()));
        }
        let entry = engine.entry(id).unwrap();
        assert!(entry.lifecycle().attempt().is_none());
        assert!(entry.lifecycle().previous_attempt().is_some());
        assert!(entry.dispatched_socket_capacity().is_none());
        assert!(entry.stream_order().is_none());
        engine.assert_invariants().unwrap();
    }
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "engine invariant violated")]
fn raw_release_gate_runs_the_hook_even_when_no_input_is_retained() {
    let mut engine = corrupt_capacity();
    engine.resolve_raw_release_gate(Instant::now(), None);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "no write may be staged at an input boundary")]
fn unrelated_input_cannot_cross_an_unfinished_write() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (_, id) = admit(&mut engine, 1, command(1), now);
    engine.handle(Input::Cancel { id }, now);
}
