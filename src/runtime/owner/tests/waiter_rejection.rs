//! Boundary-only camera harness for the production two-axis waiters.
//!
//! Local fake: decoded position replies replace hardware. Admission, writes,
//! engine correlation, terminal retirement, handle waits and sampling remain
//! the production paths; no caller timeout or queue failure is fabricated.

use super::*;
use crate::runtime::engine::{DecodedFrame, DecodedResponse, TransmissionMeta};

pub(in crate::runtime::owner) const ITERATIONS: usize = 10_000;
pub(in crate::runtime::owner) const BUDGET: Duration = Duration::from_secs(1);
pub(in crate::runtime::owner) const INTERVAL: Duration = Duration::from_millis(25);
pub(in crate::runtime::owner) const AXES: crate::AffectedAxes =
    crate::AffectedAxes::ZOOM.union(crate::AffectedAxes::FOCUS);

pub(in crate::runtime::owner) fn policy() -> OwnerPolicy {
    OwnerPolicy::single_target(
        ProtocolPolicy {
            capacity: std::num::NonZeroUsize::MIN,
            inquiry_cooldown: Duration::ZERO,
            raw_inquiry_release_hold: Duration::ZERO,
            ..ProtocolPolicy::test_default()
        },
        TargetSlot::CAMERA_1,
        TargetPolicy {
            cancellation: CancellationPolicy::Unsupported,
            ..TargetPolicy::test_default()
        },
    )
    .unwrap()
}

pub(in crate::runtime::owner) fn queries(
    tuning: crate::session_config::ValidatedTuning,
) -> crate::prepared::PositionQueryPlan {
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().unwrap();
    let target = crate::TargetRegistration::new(crate::IndividualCameraId::CAMERA_1, profile);
    let queries = crate::prepared::prepare_position_queries(&target, tuning, AXES).unwrap();
    assert!(queries.zoom.is_some() && queries.focus.is_some());
    assert!(queries.pan_tilt.is_none());
    queries
}

pub(in crate::runtime::owner) fn wait() -> crate::camera::IdleWait {
    crate::camera::IdleWait::new(AXES, BUDGET).with_interval(INTERVAL)
}

pub(in crate::runtime::owner) fn assert_axis(
    admission: &boundary::AdmissionBoundary,
    register: u8,
) {
    let RuntimeRequest::Inquiry { wire, .. } = admission.request() else {
        panic!("a production waiter submits position inquiries");
    };
    assert_eq!(wire.as_bytes(), &[0x81, 0x09, 0x04, register, 0xff]);
}

fn drive(state: &mut OwnerState, mut effects: VecDeque<Effect>, now: Instant) {
    while let Some(effect) = effects.pop_front() {
        if let AppliedEffect::Transmit(staged) = state.apply_effect(effect, now) {
            let next =
                state.finish_write(&staged, WriteOutcome::Sent(TransmissionMeta::Known), now);
            // Write results precede any effects remaining in their owner turn.
            for effect in next.into_iter().rev() {
                effects.push_front(effect);
            }
        }
    }
}

pub(in crate::runtime::owner) fn answer_position(
    state: &mut OwnerState,
    mut admission: boundary::AdmissionBoundary,
    register: u8,
    clock: &crate::testing::manual_clock::ManualClock,
) {
    let now = clock.now();
    let writes_before = state.metrics().writes;
    assert_axis(&admission, register);
    admission.claim_for_admission(now).unwrap();
    let (request, permit, observer, reply) = admission.into_parts();
    let turn = state.begin_input_turn(now);
    let admitted = state
        .admit_in_turn(&turn, request, permit, observer)
        .unwrap();
    reply.try_send(Ok(admitted)).unwrap();
    let effects = state.finish_input_turn(turn, EngineTurn::Complete);
    drive(state, effects, now);
    // Honor the actual prepared inquiry spacing, including between axes.
    // The fake camera cannot reply to a request the engine has not written.
    while state.metrics().writes == writes_before {
        let next = state
            .next_wake(clock.now())
            .expect("paced inquiry has a wake");
        assert!(
            next > clock.now(),
            "paced dispatch must make clock progress"
        );
        clock.advance(next.duration_since(clock.now()));
        let effects = state.input(Input::Wake, clock.now());
        drive(state, effects, clock.now());
    }
    assert_eq!(state.metrics().writes, writes_before + 1);
    let now = clock.now();
    let effects = state.input(
        Input::Frame(DecodedFrame {
            target: TargetSlot::CAMERA_1,
            sequence: None,
            response: DecodedResponse::InquiryReply {
                payload: smallvec::smallvec![0, 0, 0, 1],
            },
        }),
        now,
    );
    drive(state, effects, now);
    assert_eq!(state.active_len(), 0);
}
