//! A typed STOP is admitted while ordinary admission is saturated (D26, #778).
//!
//! `SessionConfig::admission_capacity` bounds ordinary requests. Each
//! registered camera also reserves one admission slot per typed STOP its
//! profile supports, which only an urgent typed STOP may use. These tests
//! saturate a capacity-1 session with a running zoom and show that a second
//! ordinary request is refused while the STOP is admitted, written, and
//! applied.
//!
//! The zoom must be acknowledged before the STOP is submitted. A raw STOP that
//! crosses a still-unacknowledged command creates the documented two-candidate
//! state (#714), in which neither ACK binds and the STOP may end
//! `UnsequencedCommandUnconfirmed`. Each test therefore waits until the owner
//! has read the zoom's ACK, never for an elapsed time.

#![cfg(any(
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]
#![allow(clippy::expect_used)]

use grafton_visca_test_support::{facade_matrix, fake_camera, profile_fixtures};

use std::{num::NonZeroUsize, time::Duration};

use grafton_visca::{
    config::{OperationalTuning, SessionConfig},
    operation::AppliedOnly,
    profiles::ProfileSpec,
    Error,
};
#[cfg(all(
    any(feature = "async", feature = "blocking"),
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
use grafton_visca::{r#async::Session, runtime::Executor};

use fake_camera::{frames, FakeCamera, WAIT_BUDGET, ZOOM_STOP, ZOOM_TELE};
use profile_fixtures::NonDefaultCompileTimeProfile as Raw;

/// A capacity-1 session for the raw three-axis fixture.
///
/// Every scripted ACK is pushed only after the owner has written its frame, so
/// the test never races the camera. The fixture's 100 ms ACK deadline would
/// still race the owner's scheduling on a loaded host; the deadline is not what
/// these tests exercise, so it is widened to the wait budget the helpers share.
fn saturated_config() -> SessionConfig {
    SessionConfig::new(ProfileSpec::from_compile_time::<Raw>().expect("raw three-axis profile"))
        .with_admission_capacity(NonZeroUsize::MIN)
        .with_tuning(OperationalTuning::new().with_ack_timeout(WAIT_BUDGET))
}

facade_matrix! {
    fn stop_passes_a_saturated_session() {
        let fake = FakeCamera::silent();
        let session = open!(fake, saturated_config()).expect("owner session");
        let camera = session.camera::<Raw>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("raw camera");

        // The running zoom holds the session's only ordinary slot. Its ACK is
        // answered after the owner writes it and read before the STOP is
        // submitted, so the zoom is executing on socket one when the STOP is
        // written.
        let mut zoom = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Tele))
            .expect("ordinary zoom admitted");
        wait_for_writes!(fake, 1);
        fake.push(frames::ack(1));
        wait_for_reads!(fake, 1);
        assert!(matches!(
            wait!(zoom.applied_with_timeout(Duration::from_millis(20))),
            Err(Error::ObservationTimeout { .. })
        ));

        let refused = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Wide))
            .expect_err("ordinary admission is full");
        assert!(matches!(
            refused,
            Error::RuntimeQueueFull { capacity: 1, .. }
        ));

        let mut stop = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop))
            .expect("the typed STOP uses camera 1's control reserve");
        wait_for_writes!(fake, 2);
        fake.push(frames::ack(2));
        fake.push(frames::complete(2));
        wait!(stop.applied()).expect("the STOP applies");
        assert_eq!(fake.writes(), vec![ZOOM_TELE.to_vec(), ZOOM_STOP.to_vec()]);

        let metrics = wait!(session.metrics()).expect("metrics");
        assert_eq!(metrics.control_reserve_admitted, 1);
        assert_eq!(metrics.control_reserve_rejected, 0);
        assert_eq!(metrics.admission_rejected, 1);

        zoom.detach();
        drop(stop);
        session.shutdown().expect("owner shutdown");
    }
}

// Virtual time keeps the ten-thousand-request regression independent of the
// worker's bounded receive probes and the host scheduler's timer granularity.
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
        fn completed_requests_free_capacity_before_immediate_resubmission() {
            let fake = FakeCamera::acking(1);
            let session = open!(fake, saturated_config()).expect("capacity-one session");
            let camera = session.camera::<Raw>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("camera");
            for _ in 0..10_000 {
                let mut ordinary = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Tele))
                    .expect("ordinary resubmission");
                wait!(ordinary.applied()).expect("ordinary completion");
                let mut urgent = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop))
                    .expect("urgent resubmission");
                wait!(urgent.applied()).expect("urgent completion");
            }
            assert_eq!(fake.write_count(), 20_000);
            let metrics = wait!(session.metrics()).expect("metrics");
            assert_eq!(metrics.admission_rejected, 0);
            assert_eq!(metrics.control_reserve_rejected, 0);
            wait!(session.close()).expect("close");
        }
    }
}

/// The dynamic projection reaches the same reserve. The blocking dynamic
/// camera has no `submit_applied`, so this scenario is async-only.
#[cfg(all(
    any(feature = "async", feature = "blocking"),
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
async fn dyn_stop_passes_a_saturated_session<E: Executor>(executor: E) {
    let fake = FakeCamera::silent();
    let session = Session::open(fake.async_wire(), &saturated_config(), executor.clone())
        .await
        .expect("owner session");
    let camera = session
        .camera::<grafton_visca::profiles::AnyProfile>(
            grafton_visca::types::IndividualCameraId::CAMERA_1,
        )
        .expect("dynamic camera");
    let zoom = camera
        .submit(&grafton_visca::command::zoom::Tele)
        .await
        .expect("ordinary zoom admitted");
    fake.wait_for_writes_async(&executor, 1).await;
    fake.push(frames::ack(1));
    fake.wait_for_reads_async(&executor, 1).await;

    assert!(matches!(
        camera.submit(&grafton_visca::command::zoom::Wide).await,
        Err(Error::RuntimeQueueFull { capacity: 1, .. })
    ));
    let mut stop = camera
        .submit(&grafton_visca::command::zoom::Stop)
        .await
        .expect("the typed STOP uses camera 1's control reserve");
    fake.wait_for_writes_async(&executor, 2).await;
    fake.push(frames::ack(2));
    fake.push(frames::complete(2));
    stop.applied().await.expect("the STOP applies");

    zoom.detach();
    session.shutdown().expect("owner shutdown");
}

#[cfg(all(
    any(feature = "async", feature = "blocking"),
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
grafton_visca_test_support::runtime_matrix!(dyn_stop_passes_a_saturated_session);

/// Every built-in's halt uses exactly the reserve derived from its STOP rows.
/// An outstanding inquiry fills ordinary capacity without being superseded by
/// the halt's motion fence. The camera answers STOPs in each profile's framing.
#[cfg(feature = "blocking")]
#[test]
fn every_builtin_halt_passes_saturated_ordinary_admission() {
    use grafton_visca::{
        blocking::Session,
        profiles::{AnyProfile, ProfileId},
        types::{HighResolutionZoomSpeed, IndividualCameraId},
    };
    for id in ProfileId::all() {
        let profile = id.spec().expect("built-in profile");
        let expected = usize::from(profile.capabilities().pan_tilt.is_some())
            + usize::from(profile.capabilities().zoom.is_some())
            + usize::from(profile.capabilities().focus.is_some());
        let fake = FakeCamera::visca(|request, answer| {
            if request.get(1) == Some(&0x01) {
                answer.reply(frames::ack(1)).reply(frames::complete(1));
            }
        });
        let config = SessionConfig::new(profile)
            .with_admission_capacity(NonZeroUsize::MIN)
            .with_tuning(OperationalTuning::new().with_inquiry_timeout(WAIT_BUDGET));
        let session = Session::open(fake.blocking_wire(), &config).expect("session");
        let camera = session
            .camera::<AnyProfile>(IndividualCameraId::CAMERA_1)
            .expect("camera");
        if *id == ProfileId::SonyFr7 {
            camera
                .zoom()
                .wide_high_resolution(HighResolutionZoomSpeed::MIN)
                .expect("FR7 high-resolution drive")
                .applied()
                .expect("drive applied");
            assert_eq!(fake.take_payloads().len(), 1);
        }
        std::thread::scope(|scope| {
            let querying = camera.clone();
            let inquiry = scope
                .spawn(move || querying.inquire(&grafton_visca::command::zoom::PositionInquiry));
            fake.wait_for_writes(1);
            assert!(
                matches!(
                    camera.inquire(&grafton_visca::command::zoom::PositionInquiry),
                    Err(Error::RuntimeQueueFull { capacity: 1, .. })
                ),
                "{id:?}"
            );
            camera
                .motion()
                .stop_all()
                .expect("halt accepted")
                .into_result()
                .expect("every supported STOP applies");
            assert_eq!(fake.write_count(), 1 + expected, "{id:?}");
            if *id == ProfileId::SonyFr7 {
                let packets = fake.take_payloads();
                let high_resolution_stop = [0x81, 1, 0x7e, 4, 0x17, 0, 0, 0, 0, 0, 0xff];
                assert_eq!(
                    packets
                        .iter()
                        .filter(|packet| packet.as_slice() == high_resolution_stop)
                        .count(),
                    1
                );
                assert!(!packets.iter().any(|packet| packet == ZOOM_STOP));
            }
            let metrics = session.metrics().expect("metrics");
            assert_eq!(metrics.control_reserve_admitted, expected as u64, "{id:?}");
            assert_eq!(metrics.control_reserve_rejected, 0, "{id:?}");
            session.shutdown().expect("shutdown");
            assert!(inquiry.join().expect("query thread").is_err());
        });
    }
}
