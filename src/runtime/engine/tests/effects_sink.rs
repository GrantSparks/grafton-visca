//! Appended effects stay local to their decision even with a nonempty sink.
use super::*;

#[test]
fn a_lent_decision_indexes_only_its_own_appended_segment() {
    let mut sink = vec![Effect::Ignored(IgnoreReason::MalformedFrame)];
    let mut decision = Decision::new(&mut sink);
    assert_eq!(decision.len(), 0);
    assert!(decision.as_slice().is_empty());
    decision.push(Effect::Ignored(IgnoreReason::StaleTransmission));
    decision.insert(0, Effect::Ignored(IgnoreReason::SessionNotRunning));
    assert_eq!(decision.len(), 2);
    assert!(matches!(
        decision.as_slice(),
        [
            Effect::Ignored(IgnoreReason::SessionNotRunning),
            Effect::Ignored(IgnoreReason::StaleTransmission)
        ]
    ));
    assert!(matches!(
        sink.as_slice(),
        [
            Effect::Ignored(IgnoreReason::MalformedFrame),
            Effect::Ignored(IgnoreReason::SessionNotRunning),
            Effect::Ignored(IgnoreReason::StaleTransmission)
        ]
    ));
}

#[test]
fn deadline_cause_insertion_preserves_the_sink_prefix_and_collecting_verdict() {
    let now = Instant::now();
    for envelope in [EnvelopeKind::Raw, EnvelopeKind::Sony] {
        let mut collecting = engine(envelope, TransportKind::Datagram);
        let mut appending = engine(envelope, TransportKind::Datagram);
        for engine in [&mut collecting, &mut appending] {
            let mut request = command(1, CancellationPolicy::Supported);
            request.context_mut().retry = RetryPolicy::NEVER;
            let (effects, _) = admit(engine, 1, request, now);
            send_ok(
                engine,
                &effects,
                (envelope == EnvelopeKind::Sony).then_some(7),
                now,
            );
        }
        let at = now + Duration::from_millis(20);
        let expected = collecting.advance(at);
        let mut sink = vec![Effect::Ignored(IgnoreReason::MalformedFrame)];
        appending.advance_into(at, &mut sink);
        assert!(matches!(
            sink[0],
            Effect::Ignored(IgnoreReason::MalformedFrame)
        ));
        assert!(matches!(sink[1], Effect::DeadlineExpired { .. }));
        assert_eq!(format!("{:?}", &sink[1..]), format!("{expected:?}"));
    }
}

#[test]
fn complete_input_appends_input_and_dispatch_effects_after_existing_work() {
    let now = Instant::now();
    let mut collecting = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let mut appending = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let input = || Input::Admit {
        ticket: AdmissionTicket(1),
        request: command(1, CancellationPolicy::Supported),
        slot: AdmissionSlot::Ordinary,
    };
    let expected = collecting.handle(input(), now);
    let mut sink = vec![Effect::Ignored(IgnoreReason::MalformedFrame)];
    appending.handle_into(input(), now, &mut sink);
    assert!(matches!(
        sink[0],
        Effect::Ignored(IgnoreReason::MalformedFrame)
    ));
    assert_eq!(format!("{:?}", &sink[1..]), format!("{expected:?}"));
}
