//! A refused cancellation must never strand the caller (#612, #777).
//!
//! `PtzOpticsG2` is the one built-in profile without VISCA socket-cancel
//! support, so cancelling one of its already-written commands is refused with
//! [`Error::NotSupported`]. Dropping a handle is exactly `detach` — no STOP is
//! emitted — so a refusal that also swallowed the handle would leave a caller
//! watching a moving axis with nothing left to observe it through.
//!
//! `cancel` borrows the handle on both facades, so a refusal is a plain error
//! and the same handle keeps observing the original operation. These tests pin
//! that contract: the same handle still observes the original operation the
//! engine deliberately left running, a second cancel is refused the same way,
//! and the documented recourse — an explicit typed STOP — reaches the wire,
//! both while the original is still in flight and after it concluded.
//!
//! Every scenario runs on the blocking facade and on the async facade under
//! each enabled runtime, except one:
//!
//! * `dyn_refused_cancel_leaves_the_handle_observing` runs on the async
//!   runtimes only (and only with `dyn-api`). It pins the erased dynamic
//!   handle that `camera_dyn().submit_applied` returns; the blocking dynamic
//!   camera has no erased handle — its `submit` returns the typed blocking
//!   `Operation` the other scenarios already cover.

#![cfg(any(
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]

#[path = "common/fake_camera.rs"]
mod fake_camera;
#[macro_use]
#[path = "common/matrix.rs"]
mod matrix;

use std::time::Duration;

use grafton_visca::{
    profile::ProfileSpec, profiles::PtzOpticsG2, CancellationOutcome, Error, SessionConfig,
};

use fake_camera::{frames, FakeCamera, ZOOM_STOP, ZOOM_TELE};

fn g2_config() -> SessionConfig {
    let profile = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("built-in G2 profile");
    SessionConfig::new(profile)
}

facade_matrix! {
    /// The whole contract on one moving axis: refuse, keep observing, and stop
    /// it while the original is still in flight.
    fn refused_cancel_leaves_the_handle_observing() {
        let fake = FakeCamera::silent();
        let session = open!(fake, g2_config()).expect("owner session");
        let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

        // A continuous zoom: the axis keeps moving until something stops it.
        let mut moving = wait!(camera.zoom().tele()).expect("continuous zoom admitted");
        wait_for_writes!(fake, 1);
        fake.push(frames::ack(1));
        wait_for_reads!(fake, 1);

        // The G2 has no socket-cancel, so the owner refuses. `cancel` borrows
        // the handle, so the refusal leaves it in the caller's hands.
        let refused = wait!(moving.cancel())
            .expect_err("G2 sent cancellation must be rejected by profile policy");
        assert!(matches!(refused, Error::NotSupported));

        // A refusal installs no intent, so a retry is refused the same way.
        let refused = wait!(moving.cancel())
            .expect_err("the retry is refused on the same profile grounds");
        assert!(matches!(refused, Error::NotSupported));

        // No cancellation frame was ever written, and the original is still
        // live.
        assert_eq!(fake.writes(), vec![ZOOM_TELE.to_vec()]);

        // The documented recourse for a profile without socket-cancel: an
        // explicit typed STOP, which the second command socket dispatches
        // while the original is still in flight.
        let mut stop = wait!(camera.zoom().stop()).expect("typed stop admitted");
        wait_for_writes!(fake, 2);
        assert_eq!(
            fake.writes(),
            vec![ZOOM_TELE.to_vec(), ZOOM_STOP.to_vec()],
            "the STOP that ends physical movement must reach the wire"
        );
        fake.push(frames::ack(2));
        fake.push(frames::complete(2));
        within!(Duration::from_secs(2), stop.applied())
            .expect("the stop applies while the refused original is still running");

        // The handle still reports the original operation's own terminal
        // state, and a cancel after it concluded answers from that state.
        fake.push(frames::complete(1));
        within!(Duration::from_secs(2), moving.applied())
            .expect("the handle still observes the original operation");
        assert!(matches!(
            wait!(moving.cancel()),
            Ok(CancellationOutcome::Completed)
        ));
        assert_eq!(fake.write_count(), 2, "a concluded cancel sends nothing");

        session.shutdown().expect("owner shutdown");
    }

    /// A refused cancellation of an unacknowledged command leaves the handle
    /// observing it to its terminal state, after which a typed STOP is still
    /// available.
    fn refused_cancel_leaves_the_handle_observing_and_stop_available() {
        let fake = FakeCamera::silent();
        let session = open!(fake, g2_config()).expect("owner session");
        let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

        // A continuous zoom: the axis keeps moving until something stops it.
        let mut moving = wait!(camera.zoom().tele()).expect("continuous zoom admitted");
        wait_for_writes!(fake, 1);
        assert_eq!(fake.writes(), vec![ZOOM_TELE.to_vec()]);

        // The G2 has no socket-cancel, so the owner refuses. `cancel` borrows
        // the handle, so the refusal leaves it in the caller's hands.
        let refused = wait!(moving.cancel())
            .expect_err("G2 sent cancellation must be rejected by profile policy");
        assert!(matches!(refused, Error::NotSupported));

        // A refusal installs no intent, so a retry is refused the same way.
        let refused = wait!(moving.cancel())
            .expect_err("the retry is refused on the same profile grounds");
        assert!(matches!(refused, Error::NotSupported));

        // No cancellation frame was ever written.
        assert_eq!(fake.writes(), vec![ZOOM_TELE.to_vec()]);

        // The refused handle keeps the original operation's retained result:
        // it still reports the operation's own terminal state, and a cancel
        // after it concluded answers from that state.
        fake.push(fake_camera::ack_and_complete(1));
        within!(Duration::from_secs(2), moving.applied())
            .expect("the handle still observes the original operation");
        assert!(matches!(
            wait!(moving.cancel()),
            Ok(CancellationOutcome::Completed)
        ));

        // The documented recourse for a profile without socket-cancel: an
        // explicit typed STOP, which still reaches the wire.
        let mut stop = wait!(camera.zoom().stop()).expect("typed stop admitted");
        wait_for_writes!(fake, 2);
        fake.push(fake_camera::ack_and_complete(1));
        within!(Duration::from_secs(2), stop.applied()).expect("the stop applies");
        assert_eq!(
            fake.writes(),
            vec![ZOOM_TELE.to_vec(), ZOOM_STOP.to_vec()],
            "the STOP that ends physical movement must reach the wire"
        );

        session.shutdown().expect("owner shutdown");
    }

    /// A handle can be detached after a refused cancellation, and the refusal
    /// leaves the engine's view of the original request untouched.
    fn handle_can_be_detached_after_a_refused_cancel() {
        let fake = FakeCamera::silent();
        let session = open!(fake, g2_config()).expect("owner session");
        let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

        let mut moving = wait!(camera.zoom().tele()).expect("continuous zoom admitted");
        wait_for_writes!(fake, 1);

        assert!(matches!(wait!(moving.cancel()), Err(Error::NotSupported)));
        moving.detach();

        // Detaching relinquishes observation only, exactly as it does for a
        // handle that was never offered to `cancel`: the original still
        // completes.
        fake.push(frames::ack(1));
        fake.push(frames::complete(1));
        wait_for_reads!(fake, 2);
        let mut next = wait!(camera.zoom().stop()).expect("typed stop admitted");
        wait_for_writes!(fake, 2);
        fake.push(frames::ack(1));
        fake.push(frames::complete(1));
        within!(Duration::from_secs(2), next.applied())
            .expect("stop applied after the detached original terminalized");
        assert_eq!(
            fake.writes(),
            vec![ZOOM_TELE.to_vec(), ZOOM_STOP.to_vec()],
            "no cancellation frame is ever written for a profile without support"
        );

        session.shutdown().expect("owner shutdown");
    }

    /// A still-queued command cancels locally on every profile.
    fn queued_cancel_concludes_cancelled_on_every_profile() {
        let fake = FakeCamera::silent();
        let session = open!(fake, g2_config()).expect("owner session");
        let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

        // Occupy both G2 command sockets so the third submission is
        // unambiguously still queued when it is cancelled.
        let first = wait!(camera.zoom().tele()).expect("first operation");
        wait_for_writes!(fake, 1);
        fake.push(frames::ack(1));
        wait_for_reads!(fake, 1);

        let second = wait!(camera.focus().stop()).expect("second operation");
        wait_for_writes!(fake, 2);
        fake.push(frames::ack(2));
        wait_for_reads!(fake, 2);

        let mut queued = wait!(camera.pan_tilt().home()).expect("queued operation");
        assert!(matches!(
            wait!(queued.cancel_with_timeout(Duration::from_secs(1))),
            Ok(CancellationOutcome::Cancelled)
        ));
        assert_eq!(
            fake.write_count(),
            2,
            "the queued command never reached the wire"
        );

        first.detach();
        second.detach();
        session.shutdown().expect("owner shutdown");
    }
}

/// The dynamic projection erases the completion marker but not the contract:
/// a refused cancellation leaves the dynamic handle observing too.
#[cfg(all(
    feature = "dyn-api",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
async fn dyn_refused_cancel_leaves_the_handle_observing<E: grafton_visca::Executor>(executor: E) {
    use grafton_visca::request::builtin::ZoomDrive;

    let fake = FakeCamera::silent();
    let session = grafton_visca::Session::open(fake.async_wire(), g2_config(), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera_dyn().expect("dynamic G2 camera");

    let mut moving = camera
        .submit_applied(&ZoomDrive::Tele)
        .await
        .expect("continuous zoom admitted");
    fake.wait_for_writes_async(&executor, 1).await;
    fake.push(frames::ack(1));
    fake.wait_for_reads_async(&executor, 1).await;

    assert!(matches!(moving.cancel().await, Err(Error::NotSupported)));

    fake.push(frames::complete(1));
    executor
        .timeout(Duration::from_secs(2), moving.applied())
        .await
        .expect("handle observer deadline")
        .expect("the dynamic handle still observes the original operation");
    assert_eq!(fake.writes(), vec![ZOOM_TELE.to_vec()]);

    session.shutdown().expect("owner shutdown");
}

#[cfg(all(
    feature = "dyn-api",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
runtime_matrix!(dyn_refused_cancel_leaves_the_handle_observing);
