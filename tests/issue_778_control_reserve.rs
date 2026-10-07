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

#[path = "common/fake_camera.rs"]
mod fake_camera;
#[macro_use]
#[path = "common/matrix.rs"]
mod matrix;
#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{num::NonZeroUsize, time::Duration};

use grafton_visca::{
    completion::AppliedOnly,
    profile::ProfileSpec,
    request::builtin::{ZoomDrive, ZoomStop},
    Error, OperationalTuning, SessionConfig,
};
#[cfg(all(
    feature = "dyn-api",
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
use grafton_visca::{Executor, Session};

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
        .with_tuning(OperationalTuning::new().ack_timeout(WAIT_BUDGET))
}

facade_matrix! {
    fn stop_passes_a_saturated_session() {
        let fake = FakeCamera::silent();
        let session = open!(fake, saturated_config()).expect("owner session");
        let camera = session.camera::<Raw>().expect("raw camera");

        // The running zoom holds the session's only ordinary slot. Its ACK is
        // answered after the owner writes it and read before the STOP is
        // submitted, so the zoom is executing on socket one when the STOP is
        // written.
        let mut zoom = wait!(camera.submit::<AppliedOnly, _>(&ZoomDrive::Tele))
            .expect("ordinary zoom admitted");
        wait_for_writes!(fake, 1);
        fake.push(frames::ack(1));
        wait_for_reads!(fake, 1);
        assert!(matches!(
            wait!(zoom.applied_with_timeout(Duration::from_millis(20))),
            Err(Error::ObservationTimeout { .. })
        ));

        let refused = wait!(camera.submit::<AppliedOnly, _>(&ZoomDrive::Wide))
            .expect_err("ordinary admission is full");
        assert!(matches!(
            refused,
            Error::RuntimeQueueFull { capacity: 1, .. }
        ));

        let mut stop = wait!(camera.submit::<AppliedOnly, _>(&ZoomStop))
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

/// The dynamic projection reaches the same reserve. The blocking dynamic
/// camera has no `submit_applied`, so this scenario is async-only.
#[cfg(all(
    feature = "dyn-api",
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
async fn dyn_stop_passes_a_saturated_session<E: Executor>(executor: E) {
    let fake = FakeCamera::silent();
    let session = Session::open(fake.async_wire(), saturated_config(), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera_dyn().expect("dynamic camera");
    let zoom = camera
        .submit_applied(&ZoomDrive::Tele)
        .await
        .expect("ordinary zoom admitted");
    fake.wait_for_writes_async(&executor, 1).await;
    fake.push(frames::ack(1));
    fake.wait_for_reads_async(&executor, 1).await;

    assert!(matches!(
        camera.submit_applied(&ZoomDrive::Wide).await,
        Err(Error::RuntimeQueueFull { capacity: 1, .. })
    ));
    let mut stop = camera
        .submit_applied(&ZoomStop)
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
    feature = "dyn-api",
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
runtime_matrix!(dyn_stop_passes_a_saturated_session);
