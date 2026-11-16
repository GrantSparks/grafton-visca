//! Exercise the production sampling methods with an authoritative delivery time.

use super::super::super::{
    boundary::{AdmissionBoundaryClaim, AdmissionClaim},
    TerminalSlot,
};
use super::*;

#[derive(Clone, Copy, Debug)]
enum Observation {
    Sample,
    Idle,
    Motion,
    Settled,
    Applied,
}

/// Local fake: only the owner boundary is scripted, so delivery can precede
/// caller wake-up deterministically. Actual handle admission, receipt waiting,
/// decoding and position polling all run through their production methods.
#[test]
fn position_observations_use_delivery_time_even_when_the_caller_wakes_late() {
    for kind in [
        Observation::Sample,
        Observation::Idle,
        Observation::Motion,
        Observation::Settled,
        Observation::Applied,
    ] {
        for offset in [-1_i8, 0, 1] {
            let clock = ManualClock::new();
            let _participant = clock.enter();
            let (transport, _peer, profile, _) = Setup::datagram_for(
                ProfileSpec::from_compile_time::<crate::profiles::SonyEviH100>().unwrap(),
            )
            .on(&clock)
            .transport();
            let (_, policy) = BlockingTransportAdapter::new(
                transport,
                &profile,
                crate::target::TargetSlot::CAMERA_1,
            )
            .unwrap();
            let mut state = OwnerState::new(policy, clock.now()).unwrap();
            let (core, ends) = OwnerHandleCore::new(&state);
            let owner = BlockingOwnerHandle {
                core,
                clock: Clock::Manual(clock.clone()),
                worker_id: thread::spawn(|| thread::current().id()).join().unwrap(),
                worker: Arc::new(Mutex::new(None)),
            };
            let registration =
                crate::TargetRegistration::new(crate::IndividualCameraId::CAMERA_1, profile);
            let queries = crate::prepared::prepare_position_queries(
                &registration,
                owner.validated_tuning(),
                crate::AffectedAxes::ZOOM,
            )
            .unwrap();
            let window = crate::prepared::MotionWindow::new(crate::camera::MotionQuery::new(
                crate::AffectedAxes::ZOOM,
            ))
            .unwrap();
            let budget = match kind {
                Observation::Motion => window.budget(&queries).unwrap(),
                _ => Duration::from_secs(1),
            };
            let deadline = clock.now() + budget;
            let delivered = match offset {
                -1 => deadline - Duration::from_nanos(1),
                0 => deadline,
                _ => deadline + Duration::from_nanos(1),
            };
            let clock_for_owner = clock.clone();
            let participant = clock.register();
            let mock_owner = thread::spawn(move || {
                let _participant = participant.enter();
                let clock = Clock::Manual(clock_for_owner.clone());
                let samples = match kind {
                    Observation::Sample | Observation::Applied => 1,
                    Observation::Settled => 3,
                    _ => 2,
                };
                for index in 0..samples {
                    let admission = clock
                        .select()
                        .recv(&ends.receivers.admissions, |value| value.unwrap())
                        .wait();
                    if let AdmissionBoundaryClaim::Pending(validity) = &admission.claim {
                        assert!(matches!(
                            validity.claim_for_admission(clock.now()),
                            AdmissionClaim::Claimed { .. }
                        ));
                    }
                    let cell = match &admission.observer {
                        TerminalSlot::Inquiry(cell) => TerminalSlot::Inquiry(Arc::clone(cell)),
                        TerminalSlot::Command(cell) => TerminalSlot::Command(Arc::clone(cell)),
                        _ => panic!("typed observations use their own terminal lane"),
                    };
                    let turn = state.begin_input_turn(clock.now());
                    let admitted = state
                        .admit_in_turn(
                            &turn,
                            admission.request,
                            admission.permit,
                            admission.observer,
                        )
                        .unwrap();
                    let last = index + 1 == samples;
                    if last {
                        clock_for_owner.advance(delivered.saturating_duration_since(clock.now()));
                    }
                    let value = match &cell {
                        TerminalSlot::Command(_) => {
                            RuntimeOutcome::Command(CommandOutcome::Applied)
                        }
                        _ => RuntimeOutcome::Inquiry(InquiryOutcome::Reply {
                            payload: smallvec::smallvec![0, 0, 0, 1],
                        }),
                    };
                    cell.resolve_at(value, clock.now());
                    if last {
                        // The result is already delivered when the caller resumes
                        // after the observation budget. Its timestamp alone decides.
                        clock_for_owner.advance(
                            (deadline + Duration::from_millis(10))
                                .saturating_duration_since(clock.now()),
                        );
                    }
                    admission.reply.send(Ok(admitted)).unwrap();
                }
            });
            let result = match kind {
                Observation::Sample => owner
                    .sample_before(queries.zoom.as_ref().unwrap(), deadline)
                    .map(|_| ()),
                Observation::Idle => owner.wait_until_idle(
                    crate::camera::IdleWait::new(crate::AffectedAxes::ZOOM, budget)
                        .with_interval(Duration::from_millis(25)),
                    &queries,
                ),
                Observation::Motion => owner.observe_motion(window, &queries).map(|moving| {
                    assert!(!moving, "the two delivered positions agree");
                }),
                Observation::Settled => {
                    let prepared = prepare_operation::<crate::completion::Targeted, _>(
                        &crate::request::builtin::ZoomTarget::new(
                            crate::types::ZoomPosition::new(1).unwrap(),
                        ),
                        &registration,
                        OperationalTuning::new(),
                        ClassSelection::Request,
                    )
                    .unwrap();
                    let mut receipt = owner.submit_operation(prepared).unwrap();
                    receipt.settled(Some(budget)).map(|_| ())
                }
                Observation::Applied => {
                    let mut receipt = owner.submit_operation(zoom_stop()).unwrap();
                    receipt.applied_until(deadline)
                }
            };
            assert_eq!(
                result.is_ok(),
                offset <= 0,
                "{kind:?}, offset {offset}: {result:?}"
            );
            assert!(clock.now() > deadline);
            mock_owner.join().unwrap();
        }
    }
}
