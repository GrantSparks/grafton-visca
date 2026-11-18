//! A transport callback must leave its own worker free to finish the turn.

use super::*;
use crate::observability::DiagnosticCapacity;

fn refuses_reentry(during_write: bool) {
    let clock = ManualClock::new();
    let _participant = clock.enter();
    let profile = ProfileSpec::from_compile_time::<SonyFr7>().unwrap();
    let (owner, peer) = Setup::datagram_for(profile.clone()).on(&clock).spawn();
    let mut operation = owner
        .submit_operation(
            prepare_operation::<AppliedOnly, _>(
                &ZoomStop,
                &crate::TargetRegistration::from_profile(
                    crate::target::TargetSlot::CAMERA_1,
                    profile.clone(),
                ),
                OperationalTuning::new(),
                ClassSelection::Request,
            )
            .unwrap(),
        )
        .unwrap();
    let written = peer.next_write();
    peer.reply(&sony_reply_to(&written, ACK));
    let diagnostics = owner
        .subscribe_diagnostics(DiagnosticCapacity::new(16).expect("diagnostic capacity"))
        .unwrap();

    let callback_owner = owner.clone();
    let callback_profile = profile.clone();
    let (answered, answers) = flume::bounded(1);
    let callback: TransportCallback = Box::new(move || {
        // A subscription created before the callback retains the worker's
        // identity. Non-waiting reads remain usable from the callback.
        let _ = diagnostics.try_recv();
        assert!(diagnostics.recv_timeout(Duration::ZERO).is_ok());
        let deadline = callback_owner.now() + PROMPTLY;
        let tuning = crate::session_config::ValidatedTuning::validate(
            OperationalTuning::new(),
            [&callback_profile],
        )
        .unwrap();
        let halt = crate::prepared::prepare_halt(
            &crate::TargetRegistration::from_profile(
                crate::target::TargetSlot::CAMERA_1,
                callback_profile.clone(),
            ),
            tuning,
        );
        let results = [
            ("diagnostics recv", diagnostics.recv_blocking().map(|_| ())),
            (
                "diagnostics recv_with_timeout",
                diagnostics.recv_timeout(PROMPTLY).map(|_| ()),
            ),
            ("metrics", callback_owner.metrics().map(|_| ())),
            ("set_tuning", callback_owner.reconfigure(Ok(tuning))),
            (
                "subscribe_diagnostics",
                callback_owner
                    .subscribe_diagnostics(DiagnosticCapacity::MIN)
                    .map(|_| ()),
            ),
            (
                "submit_command",
                callback_owner
                    .submit_command(focus_manual_for(&callback_profile))
                    .map(|_| ()),
            ),
            (
                "submit_inquiry_until",
                callback_owner
                    .submit_with_timeout_until::<InquiryOutcome>(
                        raw_inquiry(PROMPTLY).into(),
                        PROMPTLY,
                        deadline,
                    )
                    .map(|_| ()),
            ),
            (
                "stop_all_motion",
                callback_owner.halt(halt, callback_owner.now()).map(|_| ()),
            ),
            (
                "receipt wait",
                callback_owner
                    .wait_core_until(&operation.observation.core, deadline)
                    .map(|_| ()),
            ),
            ("applied", operation.applied(None)),
            ("cancel", operation.cancel(None).map(|_| ())),
        ];
        answered.send(results).unwrap();
    });
    if during_write {
        *peer.write_callback.lock().unwrap() = Some(callback);
        drop(owner.submit_command(focus_manual_for(&profile)).unwrap());
    } else {
        peer.send(Read::Callback(callback));
    }
    // Waiting on the same clock keeps selection pacing and the next write
    // live. A real-time channel wait would leave this participant runnable,
    // preventing the owner from reaching its next virtual receive deadline.
    let answers = recv_within(&Clock::Manual(clock.clone()), &answers, PROMPTLY)
        .expect("every worker re-entry returns without waiting");
    for (call, result) in answers {
        assert!(
            matches!(result, Err(Error::InvalidState { .. })),
            "{call}: {result:?}"
        );
    }
    assert_eq!(
        owner.metrics().unwrap().admitted,
        if during_write { 2 } else { 1 }
    );
    owner.close().unwrap();
    assert!(peer.transport_dropped());
}

#[test]
fn read_callback_refuses_every_owner_wait_without_enqueuing_work() {
    refuses_reentry(false);
}

#[test]
fn write_callback_refuses_every_owner_wait_without_enqueuing_work() {
    refuses_reentry(true);
}
