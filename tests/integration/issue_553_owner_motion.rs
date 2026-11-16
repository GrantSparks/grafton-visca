//! Owner-backed motion surface coverage for the root session camera.
//!
//! The motion view answers its queries from position inquiries, bounds its
//! idle waits by their deadline, and reports each axis of a combined halt
//! separately. The camera answers each position inquiry from a per-axis
//! script and acknowledges and completes every command, except that the
//! first pan/tilt stop may be refused with a transient Syntax Error.
//!
//! Every scenario runs on the blocking facade and on the async facade under
//! each enabled runtime.

#![cfg(any(
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]

use grafton_visca_test_support::{facade_matrix, fake_camera, profile_fixtures};

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use grafton_visca::{
    config::SessionConfig,
    operation::HaltOutcome,
    profiles::{ProfileSpec, PtzOpticsG2},
    types::{AffectedAxes, IdleWait},
    Error,
};

#[cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
use grafton_visca::{
    error::{Certainty, FailureContext, FailureStage},
    types::MotionQuery,
};
#[cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use fake_camera::{frames, FakeCamera, FOCUS_STOP, ZOOM_STOP};
use profile_fixtures::MotionOwnerCompileTimeProfile;

/// An idle-wait budget no settling wait reaches: the wait returns as soon as
/// two samples agree, so the budget bounds only a failure and the outcome
/// never depends on scheduling.
#[cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
const SETTLE_BUDGET: Duration = Duration::from_secs(30);

/// The failure context of an idle wait that ran out of time (D20).
#[cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
const OBSERVATION_TIMEOUT: FailureContext =
    FailureContext::new(FailureStage::Observation, Certainty::NotAccepted);

/// The zoom position inquiry, camera address 1.
#[cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
const ZOOM_POSITION_INQUIRY: &[u8] = &[0x81, 0x09, 0x04, 0x47, 0xff];

/// The positions the camera reports, one per inquiry, per axis. An exhausted
/// axis reports zero.
#[derive(Debug, Default)]
struct PositionScript {
    pan_tilt: VecDeque<(i16, i16)>,
    zoom: VecDeque<u16>,
    focus: VecDeque<u16>,
}

impl PositionScript {
    #[cfg(any(
        feature = "runtime-tokio",
        all(
            feature = "test-utils",
            any(feature = "blocking", feature = "runtime-smol")
        )
    ))]
    fn zoom(positions: impl IntoIterator<Item = u16>) -> Self {
        Self {
            zoom: positions.into_iter().collect(),
            ..Self::default()
        }
    }
}

/// A camera that answers position inquiries from `script` and acknowledges
/// and completes every command on socket 1. With `fail_first_pan_stop`, the
/// first pan/tilt drive (a stop, in these scenarios) is refused with a
/// Syntax Error instead.
fn motion_camera(mut script: PositionScript, fail_first_pan_stop: bool) -> FakeCamera {
    let mut fail_pan_stop = fail_first_pan_stop;
    FakeCamera::visca(move |payload, answer| {
        if payload.starts_with(&[0x81, 0x09, 0x06, 0x12]) {
            let (pan, tilt) = script.pan_tilt.pop_front().unwrap_or((0, 0));
            answer.reply(pan_tilt_position(pan, tilt));
        } else if payload.starts_with(&[0x81, 0x09, 0x04, 0x47]) {
            answer.reply(nibble_position(script.zoom.pop_front().unwrap_or(0)));
        } else if payload.starts_with(&[0x81, 0x09, 0x04, 0x48]) {
            answer.reply(nibble_position(script.focus.pop_front().unwrap_or(0)));
        } else if payload.get(2) == Some(&0x06) && payload.get(3) == Some(&0x01) && fail_pan_stop {
            fail_pan_stop = false;
            answer.reply(frames::syntax_error());
        } else {
            answer.reply(frames::ack(1)).reply(frames::complete(1));
        }
    })
}

/// A camera whose zoom position alternates beyond tolerance at every inquiry
/// until `stopped` is set, then holds still. Every other command is
/// acknowledged and completed.
#[cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
fn moving_zoom_camera(stopped: Arc<AtomicBool>) -> FakeCamera {
    let mut position: u16 = 0;
    FakeCamera::visca(move |payload, answer| {
        if payload.starts_with(&[0x81, 0x09, 0x04, 0x47]) {
            if !stopped.load(Ordering::SeqCst) {
                position ^= 0x100;
            }
            answer.reply(nibble_position(position));
        } else {
            answer.reply(frames::ack(1)).reply(frames::complete(1));
        }
    })
}

fn pan_tilt_position(pan: i16, tilt: i16) -> Vec<u8> {
    let [pan_high, pan_low] = pan.to_be_bytes();
    let [tilt_high, tilt_low] = tilt.to_be_bytes();
    frames::inquiry_reply(&[pan_high, pan_low, tilt_high, tilt_low])
}

fn nibble_position(value: u16) -> Vec<u8> {
    frames::inquiry_reply(&[
        ((value >> 12) & 0x0f) as u8,
        ((value >> 8) & 0x0f) as u8,
        ((value >> 4) & 0x0f) as u8,
        (value & 0x0f) as u8,
    ])
}

fn motion_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<MotionOwnerCompileTimeProfile>()
            .expect("non-default runtime profile"),
    )
}

#[cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
mod paused {
    use super::*;
    facade_matrix! {
        paused:
        /// A settled zoom answers the motion query and the idle wait from zoom
        /// position inquiries alone.
        fn owner_motion_zoom_query_and_idle_wait_poll_only_zoom() {
            let fake = motion_camera(PositionScript::zoom([10, 10, 10, 10]), false);
            let session = open!(fake, motion_config()).expect("session");
            let camera = session
                .camera::<MotionOwnerCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1)
                .expect("camera");
            assert!(!wait!(camera
                .motion()
                .is_moving(MotionQuery::new(AffectedAxes::ZOOM)))
            .expect("zoom motion query"));
            wait!(camera.motion().wait_until_idle(
                IdleWait::new(AffectedAxes::ZOOM, SETTLE_BUDGET).with_interval(Duration::from_millis(25)),
            ))
            .expect("zoom idle wait");
            let writes = fake.writes();
            assert_eq!(writes.len(), 4);
            assert!(writes.iter().all(|bytes| bytes == ZOOM_POSITION_INQUIRY));
            session.shutdown().expect("shutdown");
        }

        /// A multi-axis operation samples each position sequentially after its
        /// application, while the session has exactly one ordinary permit.
        fn multi_axis_settlement_sampling_reuses_capacity_one() {
            let fake = FakeCamera::visca(|request, answer| {
                if request == [0x81, 0x09, 0x06, 0x12, 0xff] {
                    answer.reply(frames::inquiry_reply(&[0; 8]));
                } else if request == ZOOM_POSITION_INQUIRY
                    || request == [0x81, 0x09, 0x04, 0x48, 0xff] {
                    answer.reply(nibble_position(0));
                } else {
                    answer.reply(frames::ack(1)).reply(frames::complete(1));
                }
            });
            let config = SessionConfig::from_compile_time::<grafton_visca::profiles::GenericVisca>()
                .expect("polling profile")
                .with_admission_capacity(std::num::NonZeroUsize::MIN);
            let session = open!(fake, config).expect("capacity-one session");
            let camera = session.camera::<grafton_visca::profiles::GenericVisca>(
                grafton_visca::types::IndividualCameraId::CAMERA_1).expect("camera");
            let mut operation = wait!(camera.presets().recall(
                grafton_visca::types::PresetNumber::new(1).expect("preset")))
                .expect("recall admitted");
            let settled = wait!(operation.settled())
                .expect("pan/tilt, zoom and focus samples share one permit sequentially");
            assert!(matches!(settled, grafton_visca::operation::Settlement::ObservedStable { axes, .. }
                if axes == AffectedAxes::PAN_TILT | AffectedAxes::ZOOM | AffectedAxes::FOCUS));
            assert_eq!(fake.write_count(), 7, "recall followed by two samples per position inquiry");
            wait!(session.close()).expect("close");
        }

        /// An axis the profile cannot observe is refused before any wire I/O.
        fn owner_motion_unsupported_axis_is_refused_before_the_wire() {
            let fake = motion_camera(PositionScript::default(), false);
            let session = open!(fake, motion_config()).expect("unsupported-axis session");
            let camera = session
                .camera::<MotionOwnerCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1)
                .expect("camera");
            assert!(matches!(
                wait!(camera
                    .motion()
                    .is_moving(MotionQuery::new(AffectedAxes::ND_FILTER))),
                Err(Error::FeatureNotSupported { .. })
            ));
            assert!(matches!(
                wait!(camera.motion().wait_until_idle(IdleWait::new(
                    AffectedAxes::ND_FILTER,
                    Duration::from_secs(1),
                ))),
                Err(Error::FeatureNotSupported { .. })
            ));
            assert!(fake.writes().is_empty());
            session.shutdown().expect("shutdown");
        }

        /// An idle wait keeps polling while the position changes and returns once
        /// it settles.
        fn owner_motion_idle_wait_settles_after_motion() {
            let fake = motion_camera(PositionScript::zoom([0, 100, 100]), false);
            let session = open!(fake, motion_config()).expect("moving session");
            wait!(session
                .camera::<MotionOwnerCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1)
                .expect("camera")
                .motion()
                .wait_until_idle(
                    IdleWait::new(AffectedAxes::ZOOM, SETTLE_BUDGET)
                        .with_interval(Duration::from_millis(25)),
                ))
            .expect("moving then settled");
            assert_eq!(fake.write_count(), 3);
            session.shutdown().expect("shutdown");
        }

        /// An idle wait is a read-only query: its deadline is an
        /// `Observation`/`NotAccepted` timeout (D20), and repeating the wait
        /// answers again. The deadline bounds the whole wait, first sample
        /// included, so a wait whose deadline has already passed samples nothing.
        fn owner_motion_idle_wait_deadline_is_a_repeatable_observation() {
            let fake = motion_camera(PositionScript::zoom([0, 0]), false);
            let session = open!(fake, motion_config()).expect("deadline session");
            let camera = session
                .camera::<MotionOwnerCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1)
                .expect("camera");
            let expired = IdleWait::new(AffectedAxes::ZOOM, Duration::ZERO);
            for _ in 0..2 {
                assert_eq!(
                    wait!(camera.motion().wait_until_idle(expired))
                        .expect_err("the deadline has passed")
                        .failure_context(),
                    Some(OBSERVATION_TIMEOUT)
                );
            }
            assert_eq!(fake.write_count(), 0);
            wait!(camera.motion().wait_until_idle(
                IdleWait::new(AffectedAxes::ZOOM, SETTLE_BUDGET).with_interval(Duration::from_millis(25)),
            ))
            .expect("the repeated wait answers");
            let writes = fake.writes();
            assert_eq!(writes.len(), 2);
            assert!(writes.iter().all(|bytes| bytes == ZOOM_POSITION_INQUIRY));
            session.shutdown().expect("shutdown");
        }

        /// A deadline that passes while the axis keeps moving ends the wait with
        /// the same `Observation`/`NotAccepted` timeout wherever it lands: before
        /// an inquiry is admitted, while a reply is awaited, or in a pause. The
        /// wait writes only position inquiries, and a repeated wait settles once
        /// the axis stops.
        fn owner_motion_idle_wait_deadline_while_moving_is_an_observation() {
            let stopped = Arc::new(AtomicBool::new(false));
            let fake = moving_zoom_camera(Arc::clone(&stopped));
            let session = open!(fake, motion_config()).expect("moving session");
            let camera = session
                .camera::<MotionOwnerCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1)
                .expect("camera");
            assert_eq!(
                wait!(camera.motion().wait_until_idle(
                    IdleWait::new(AffectedAxes::ZOOM, Duration::from_millis(50))
                        .with_interval(Duration::from_millis(25)),
                ))
                .expect_err("the camera keeps moving")
                .failure_context(),
                Some(OBSERVATION_TIMEOUT)
            );
            stopped.store(true, Ordering::SeqCst);
            wait!(camera.motion().wait_until_idle(
                IdleWait::new(AffectedAxes::ZOOM, SETTLE_BUDGET).with_interval(Duration::from_millis(25)),
            ))
            .expect("the stopped axis settles");
            assert!(fake
                .writes()
                .iter()
                .all(|bytes| bytes == ZOOM_POSITION_INQUIRY));
            session.shutdown().expect("shutdown");
        }

    }
}

facade_matrix! {
    /// Zero intervals are invalid observation evidence and never reach I/O.
    fn owner_motion_zero_idle_interval_is_refused_before_io() {
        let fake = motion_camera(PositionScript::default(), false);
        let session = open!(fake, motion_config()).expect("session");
        let camera = session.camera::<MotionOwnerCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("camera");
        assert!(matches!(
            wait!(camera.motion().wait_until_idle(IdleWait::new(AffectedAxes::ZOOM, Duration::from_secs(1)).with_interval(Duration::ZERO))),
            Err(Error::InvalidParameter { parameter: "idle sampling interval", .. })
        ));
        assert_eq!(fake.write_count(), 0);
        session.shutdown().expect("shutdown");
    }

    /// A combined halt reports each axis on its own: a refused pan/tilt stop
    /// does not mask the zoom and focus stops that applied.
    fn owner_motion_stop_all_reports_each_axis() {
        let fake = motion_camera(PositionScript::default(), true);
        let session = open!(fake, motion_config()).expect("stop session");
        let report = wait!(session
            .camera::<MotionOwnerCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("camera")
            .motion()
            .stop_all())
        .expect("owner accepted halt");
        assert!(
            matches!(report.pan_tilt, HaltOutcome::Failed(Error::SyntaxError)),
            "{report:?}"
        );
        assert!(matches!(report.zoom, HaltOutcome::Applied));
        assert!(matches!(report.focus, HaltOutcome::Applied));
        let writes = fake.writes();
        assert_eq!(writes.len(), 3);
        assert!(writes[0].starts_with(&[0x81, 0x01, 0x06, 0x01]));
        assert_eq!(writes[1], ZOOM_STOP);
        assert_eq!(writes[2], FOCUS_STOP);
        session.shutdown().expect("shutdown");
    }

    /// #712: the raw profile's physical 150 ms inquiry spacing remains, but a
    /// successful reply adds no one-second hold and cannot delay the safety
    /// lane.
    fn successful_raw_inquiries_keep_polling_fast_and_urgent_stop_immediate() {
        let fake = motion_camera(PositionScript::default(), false);
        let session = open!(
            fake,
            SessionConfig::new(
                ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PtzOptics profile")
            )
        )
        .expect("raw session");
        let camera = session.camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("camera");

        let polling_started = Instant::now();
        for _ in 0..6 {
            wait!(camera.zoom().position()).expect("raw zoom position reply");
        }
        let polling_elapsed = polling_started.elapsed();
        assert!(
            polling_elapsed <= Duration::from_millis(1_200),
            "six replies exceeded the 5 Hz floor: {polling_elapsed:?}"
        );

        let stop_started = Instant::now();
        let mut stop = wait!(camera.zoom().stop()).expect("urgent stop reaches the wire");
        wait_for_writes!(fake, 7);
        let stop_latency = stop_started.elapsed();
        assert!(
            stop_latency < Duration::from_millis(50),
            "matched inquiry delayed urgent stop by {stop_latency:?}"
        );
        wait!(stop.applied()).expect("urgent stop applies");

        assert_eq!(fake.write_count(), 7);
        session.shutdown().expect("shutdown");
    }
}
