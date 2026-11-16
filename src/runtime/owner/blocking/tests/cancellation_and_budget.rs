//! Owner regressions for #835 and #842 using the real blocking worker.
use super::*;

#[test]
fn completion_only_cancel_is_unsupported_without_ending_application() {
    let command = crate::raw::AppliedOnlyRequest::with_policy(
        [0x81, 0x01, 0x04, 0x07, 0x02, 0xff],
        crate::AffectedAxes::ZOOM,
        crate::raw::OperationPolicy::new(
            crate::CommandTimeoutClass::Quick,
            crate::RetryClass::Never,
            crate::SubmissionClass::Normal,
        )
        .with_reply_shape(crate::raw::OperationReplyShape::CompletionOnly),
    )
    .unwrap();
    let prepared = prepare_operation::<AppliedOnly, _>(
        &command,
        &crate::TargetRegistration::new(
            (crate::target::TargetSlot::CAMERA_1).individual_camera(),
            generic_profile().clone(),
        )
        .expect("valid target registration"),
        OperationalTuning::new(),
        ClassSelection::Request,
    )
    .unwrap();
    let (owner, peer) = Setup::datagram().spawn();
    let mut operation = owner.submit_operation(prepared).unwrap();
    peer.next_write();
    assert!(matches!(
        operation.cancel(None),
        Err(Error::CancellationNotSupported)
    ));
    assert!(
        peer.writes.is_empty(),
        "socketless operations never send cancellation frames"
    );
    peer.reply(&[0x90, 0x50, 0xff]);
    operation
        .applied(None)
        .expect("unsupported cancellation keeps the original completion live");
    owner.close().unwrap();
}

#[test]
fn prepared_retry_budget_covers_late_ack_and_completion_on_the_worker() {
    for (sony, ack_override, ack_at, completed_at) in [
        (true, 10, 9, Duration::from_millis(16_500)),
        (false, 8, 7, Duration::from_millis(11_500)),
    ] {
        let profile = if sony {
            ProfileSpec::from_compile_time::<SonyFr7>().unwrap()
        } else {
            ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().unwrap()
        };
        let clock = ManualClock::new();
        let _driver = clock.enter();
        let (owner, peer) = Setup::datagram_for(profile.clone()).on(&clock).spawn();
        let command = crate::raw::AppliedOnlyRequest::new(
            [0x81, 0x01, 0x04, 0x07, 0x02, 0xff],
            crate::AffectedAxes::ZOOM,
            crate::CommandTimeoutClass::Quick,
            crate::RetryClass::Standard,
            crate::SubmissionClass::Normal,
        )
        .unwrap();
        let prepared = prepare_operation::<AppliedOnly, _>(
            &command,
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).individual_camera(),
                profile.clone(),
            )
            .expect("valid target registration"),
            OperationalTuning::new().with_ack_timeout(Duration::from_secs(ack_override)),
            ClassSelection::Request,
        )
        .unwrap();
        let mut operation = owner.submit_operation(prepared).unwrap();
        let written = peer.next_write();
        let start = clock.now();
        clock.sleep(Duration::from_secs(ack_at));
        peer.reply(&if sony {
            sony_reply_to(&written, ACK)
        } else {
            ACK.to_vec()
        });
        clock.sleep((start + completed_at).saturating_duration_since(clock.now()));
        peer.reply(&if sony {
            sony_reply_to(&written, COMPLETION)
        } else {
            COMPLETION.to_vec()
        });
        operation
            .applied(None)
            .expect("the full healthy attempt fits the prepared owner budget");
        assert!(
            peer.writes.is_empty(),
            "healthy late ACKs do not trigger replay"
        );
        owner.close().unwrap();
    }
}
