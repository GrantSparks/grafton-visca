//! #842: prepared timing must cover a healthy attempt with a late valid ACK.
use super::*;

#[test]
fn prepared_retry_budget_preserves_healthy_late_ack_attempts() {
    let sony = crate::ProfileSpec::from_compile_time::<crate::profiles::SonyFR7>().unwrap();
    let raw = crate::ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().unwrap();
    for (profile, envelope, ack_window, ack_at, complete_at) in [
        (
            sony,
            EnvelopeKind::Sony,
            10,
            9,
            Duration::from_millis(16_500),
        ),
        (raw, EnvelopeKind::Raw, 8, 7, Duration::from_millis(11_500)),
    ] {
        let start = Instant::now();
        let command = crate::raw::Plain::new(
            [0x81, 0x01, 0x04, 0x00, 0x02, 0xff],
            crate::TimeoutClass::Quick,
            crate::RetryClass::Standard,
            crate::ControlClass::Normal,
        )
        .unwrap();
        let prepared = crate::prepared::test_support::prepare_command(
            &command,
            &crate::TargetRegistration::new((camera(1)).camera(), profile.clone())
                .expect("valid target registration"),
            crate::OperationalTuning::new().ack_timeout(Duration::from_secs(ack_window)),
            crate::prepared::ClassSelection::Request,
        )
        .unwrap();
        let request = prepared.admit_with(|request, _observer_timeout| request);
        let context = *request.context();
        let legacy_budget = crate::profile::MINIMUM_RETRY_BUDGET
            .max(context.timeout.completion.get() * 2)
            .max(profile.timing().busy_timeout());
        let full_attempt = context.timeout.ack.get() + context.timeout.completion.get();
        assert!(context.retry.total_budget.unwrap().get() >= full_attempt);
        assert!(Duration::from_secs(ack_at) < context.timeout.ack.get());
        assert!(complete_at > legacy_budget);
        assert!(complete_at < Duration::from_secs(ack_at) + context.timeout.completion.get());

        let mut engine = engine_with_target(
            envelope,
            TransportKind::Datagram,
            profile.maximum_command_sockets(),
            context.cancellation,
        );
        let (admission, id) = admit(&mut engine, 1, request, start);
        let sequence = (envelope == EnvelopeKind::Sony).then(|| sony_sequence(id));
        send_ok(&mut engine, &admission, sequence, start);
        let sequence = sequence.map(|sequence| (sequence, SequenceWidth::Full32));
        let acked = engine.handle(
            frame(
                1,
                sequence,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start + Duration::from_secs(ack_at),
        );
        assert!(terminal_outcome(&acked, id).is_none());
        engine.assert_invariants().unwrap();

        // The former completion-only budget expired here even though the
        // ACK was within its window and the completion was not due yet.
        let old_expiry = engine.advance(start + legacy_budget + Duration::from_millis(1));
        assert!(
            terminal_outcome(&old_expiry, id).is_none(),
            "{old_expiry:?}"
        );
        assert!(engine.entry(id).is_some());
        engine.assert_invariants().unwrap();

        let completed = engine.handle(
            frame(
                1,
                sequence,
                DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start + complete_at,
        );
        assert!(
            matches!(
                terminal_outcome(&completed, id),
                Some(RuntimeOutcome::Applied)
            ),
            "{}: {completed:?}",
            profile.name()
        );
        assert_eq!(engine.state(), SessionState::Running);
        engine.assert_invariants().unwrap();
    }
}
