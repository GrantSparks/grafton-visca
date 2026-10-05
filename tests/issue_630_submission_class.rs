//! Issue #630: the engine's four control-class lanes.
//!
//! A submission is admitted into the owner's queue (#780), so class ordering
//! is observable at the transport boundary: while both command sockets are
//! busy, the queued requests wait, and each freed socket goes to the highest
//! class waiting for it. The camera has two sockets, assigned round-robin; it
//! acknowledges every write at once and completes a socket only when the test
//! says so, which is what makes "the socket freed, and this is what the owner
//! wrote next" observable.
//!
//! Every scenario in the `facade_matrix!` block runs on the blocking facade
//! and on the async facade under each enabled runtime. Two are single-facade:
//!
//! * `a_camera_session_default_reaches_the_views_it_hands_out` is blocking
//!   only: it opens a single-camera `CameraSession`, whose async `open` takes
//!   an executor a `facade_matrix!` body cannot name (`open!` opens only a
//!   multi-camera `Session`).
//! * `dyn_projection_has_the_same_surface` is async only and needs
//!   `dyn-api`: it drives `submit_applied` and the submission-class setters of
//!   the async erased projection, which the blocking erased projection does
//!   not offer.

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

use std::{thread, time::Duration};

#[cfg(all(
    feature = "dyn-api",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
use grafton_visca::Executor;
use grafton_visca::{
    completion::AppliedOnly,
    profile::ProfileSpec,
    profiles::PtzOpticsG2,
    request::builtin::{FocusDrive, FocusModeCommand, ZoomDrive, ZoomStop},
    SessionConfig, SubmissionClass,
};

use fake_camera::{frames, FakeCamera, ZOOM_STOP, ZOOM_TELE};

const ZOOM_WIDE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x03, 0xff];
const FOCUS_FAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x02, 0xff];
const FOCUS_NEAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x03, 0xff];

/// A two-socket camera that acknowledges every write at once, assigning
/// sockets round-robin exactly as a real camera would as each one becomes
/// free again (the first write takes socket 2). It completes nothing on its
/// own: the test frees a socket with [`complete_socket`].
fn lane_camera() -> FakeCamera {
    let mut sends = 0usize;
    FakeCamera::new(move |_, answer| {
        sends = sends.saturating_add(1);
        let socket = u8::try_from(sends % 2).unwrap_or(1) + 1;
        answer.reply(frames::ack(socket));
    })
}

/// Completes one command socket, freeing it for the next dispatch.
fn complete_socket(camera: &FakeCamera, socket: u8) {
    camera.push(frames::complete(socket));
}

/// Completes `socket` from a helper thread once `writes` writes have been
/// made, so a call that waits for its completion can be driven on either
/// facade.
fn complete_after_writes(camera: &FakeCamera, writes: usize, socket: u8) -> thread::JoinHandle<()> {
    let camera = camera.clone();
    thread::spawn(move || {
        camera.wait_for_writes(writes);
        complete_socket(&camera, socket);
    })
}

fn g2_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("built-in G2 profile"),
    )
}

/// The message every starved-socket check carries.
const STARVED: &str = "no queued request may be written while every socket is busy";

/// Lets the owner run for the facade's settle window while every socket is
/// busy: one 10 ms pause on the blocking facade, five 2 ms pauses on the
/// async one. Usable only inside a `facade_matrix!` body.
macro_rules! settle {
    () => {
        if FACADE == "blocking" {
            pause!(Duration::from_millis(10));
        } else {
            for _ in 0..5 {
                pause!(Duration::from_millis(2));
            }
        }
    };
}

facade_matrix! {
    /// A background-class submission yields the freed socket to a user-class
    /// submission made after it.
    fn background_yields_to_a_later_user_submission() {
        let fake = lane_camera();
        let session = open!(fake, g2_config()).expect("owner session");
        let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

        let first =
            wait!(camera.submit::<AppliedOnly, _>(&ZoomDrive::Tele)).expect("first socket");
        wait_for_writes!(fake, 1);
        let second =
            wait!(camera.submit::<AppliedOnly, _>(&FocusDrive::Far)).expect("second socket");
        wait_for_writes!(fake, 2);

        let background = wait!(camera
            .with_submission_class(SubmissionClass::Background)
            .submit::<AppliedOnly, _>(&ZoomDrive::Wide))
        .expect("background submission is admitted and queued");
        let user = wait!(camera
            .with_submission_class(SubmissionClass::User)
            .submit::<AppliedOnly, _>(&FocusDrive::Near))
        .expect("user submission is admitted and queued");
        settle!();
        assert_eq!(fake.write_count(), 2, "{STARVED}");

        complete_socket(&fake, 2);
        wait_for_writes!(fake, 3);
        assert_eq!(
            fake.writes()[2],
            FOCUS_NEAR.to_vec(),
            "issue #630: the later user-class request takes the freed socket",
        );

        complete_socket(&fake, 1);
        wait_for_writes!(fake, 4);
        assert_eq!(
            fake.writes()[3],
            ZOOM_WIDE.to_vec(),
            "the background request is written once nothing outranks it",
        );
        assert_eq!(
            fake.writes()[0..2].to_vec(),
            vec![ZOOM_TELE.to_vec(), FOCUS_FAR.to_vec()],
        );

        first.detach();
        second.detach();
        user.detach();
        background.detach();
        session.shutdown().expect("owner shutdown");
    }

    /// The handle default demotes one handle's traffic; a sibling clone keeps
    /// the built-in classification, and a derived view can outrank the
    /// default.
    fn handle_default_and_derived_view_override() {
        let fake = lane_camera();
        let session = open!(fake, g2_config()).expect("owner session");
        let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

        let first =
            wait!(camera.submit::<AppliedOnly, _>(&ZoomDrive::Tele)).expect("first socket");
        wait_for_writes!(fake, 1);
        let second =
            wait!(camera.submit::<AppliedOnly, _>(&FocusDrive::Far)).expect("second socket");
        wait_for_writes!(fake, 2);

        let mut poller = camera.clone();
        assert_eq!(poller.submission_class(), None);
        poller.set_submission_class(Some(SubmissionClass::Background));
        assert_eq!(poller.submission_class(), Some(SubmissionClass::Background));
        assert_eq!(
            camera.submission_class(),
            None,
            "a clone diverges rather than sharing the default",
        );

        // Both are `SubmissionClass::User` built-ins, so only the handle they
        // came from can separate them.
        let demoted =
            wait!(poller.submit::<AppliedOnly, _>(&ZoomDrive::Wide)).expect("demoted submission");
        let ordinary = wait!(camera.submit::<AppliedOnly, _>(&FocusDrive::Near))
            .expect("ordinary submission");
        settle!();
        assert_eq!(fake.write_count(), 2, "{STARVED}");

        complete_socket(&fake, 2);
        wait_for_writes!(fake, 3);
        assert_eq!(
            fake.writes()[2],
            FOCUS_NEAR.to_vec(),
            "issue #630: the demoted handle's earlier request yields the socket",
        );

        complete_socket(&fake, 1);
        wait_for_writes!(fake, 4);
        assert_eq!(fake.writes()[3], ZOOM_WIDE.to_vec());

        // Now the override: the demoted handle submits first and still wins.
        let raised = wait!(poller
            .with_submission_class(SubmissionClass::User)
            .submit::<AppliedOnly, _>(&ZoomDrive::Tele))
        .expect("raised submission");
        let later = wait!(camera.submit::<AppliedOnly, _>(&FocusDrive::Far))
            .expect("later ordinary submission");
        settle!();
        assert_eq!(fake.write_count(), 4, "{STARVED}");
        assert_eq!(
            poller.submission_class(),
            Some(SubmissionClass::Background),
            "a derived view does not change the source handle default",
        );

        complete_socket(&fake, 2);
        wait_for_writes!(fake, 5);
        assert_eq!(
            fake.writes()[4],
            ZOOM_TELE.to_vec(),
            "issue #630: the derived view's class wins over the handle default",
        );

        first.detach();
        second.detach();
        demoted.detach();
        ordinary.detach();
        raised.detach();
        later.detach();
        session.shutdown().expect("owner shutdown");
    }

    /// A typed stop retains its urgent safety class under both QoS override
    /// forms.
    fn urgent_cannot_be_demoted_by_any_submission_override() {
        let fake = lane_camera();
        let session = open!(fake, g2_config()).expect("owner session");
        let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

        let first =
            wait!(camera.submit::<AppliedOnly, _>(&ZoomDrive::Tele)).expect("first socket");
        wait_for_writes!(fake, 1);
        let second =
            wait!(camera.submit::<AppliedOnly, _>(&FocusDrive::Far)).expect("second socket");
        wait_for_writes!(fake, 2);

        let ordinary = wait!(camera.submit::<AppliedOnly, _>(&FocusDrive::Near))
            .expect("ordinary user-class submission");
        let mut poller = camera.clone();
        poller.set_submission_class(Some(SubmissionClass::Background));
        let stop = wait!(poller.submit::<AppliedOnly, _>(&ZoomStop))
            .expect("stop from a demoted handle");
        settle!();
        assert_eq!(fake.write_count(), 2, "{STARVED}");

        complete_socket(&fake, 2);
        wait_for_writes!(fake, 3);
        assert_eq!(
            fake.writes()[2],
            ZOOM_STOP.to_vec(),
            "issue #630: an urgent stop keeps its class under a demoted handle",
        );

        complete_socket(&fake, 1);
        wait_for_writes!(fake, 4);
        assert_eq!(fake.writes()[3], FOCUS_NEAR.to_vec());

        // A derived view's QoS value is also unable to weaken the stop's
        // intrinsic urgent safety floor.
        let protected_stop = wait!(camera
            .with_submission_class(SubmissionClass::Background)
            .submit::<AppliedOnly, _>(&ZoomStop))
        .expect("stop with background ordinary-traffic QoS");
        let later = wait!(camera.submit::<AppliedOnly, _>(&FocusDrive::Far))
            .expect("later user-class submission");
        settle!();
        assert_eq!(fake.write_count(), 4, "{STARVED}");

        complete_socket(&fake, 2);
        wait_for_writes!(fake, 5);
        assert_eq!(
            fake.writes()[4],
            ZOOM_STOP.to_vec(),
            "issue #630: a derived view cannot demote an urgent stop",
        );

        first.detach();
        second.detach();
        ordinary.detach();
        stop.detach();
        protected_stop.detach();
        later.detach();
        session.shutdown().expect("owner shutdown");
    }

    /// A class-selected view's `execute` reaches the same owner path as its
    /// source-view twin.
    fn classified_execute_reaches_the_wire() {
        let fake = lane_camera();
        let session = open!(fake, g2_config()).expect("owner session");
        let mut camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

        let completion = complete_after_writes(&fake, 1, 2);
        wait!(camera
            .with_submission_class(SubmissionClass::Background)
            .execute(&FocusModeCommand::Manual))
        .expect("background plain command still executes");
        completion.join().expect("completion thread");

        camera.set_submission_class(Some(SubmissionClass::User));
        let completion = complete_after_writes(&fake, 2, 1);
        wait!(camera.execute(&FocusModeCommand::Auto))
            .expect("handle default plain command still executes");
        completion.join().expect("completion thread");

        assert_eq!(fake.write_count(), 2);
        session.shutdown().expect("owner shutdown");
    }
}

/// The single-camera session carries the default into every view it hands
/// out.
#[cfg(feature = "blocking")]
#[test]
fn a_camera_session_default_reaches_the_views_it_hands_out() {
    let mut session = grafton_visca::blocking::CameraSession::<PtzOpticsG2>::open(
        lane_camera().blocking_wire(),
        &grafton_visca::blocking::CameraConfig::<PtzOpticsG2>::tcp("127.0.0.1"),
    )
    .expect("single-camera session");

    assert_eq!(session.submission_class(), None);
    assert_eq!(session.camera().submission_class(), None);

    session.set_submission_class(Some(SubmissionClass::Background));
    assert_eq!(
        session.submission_class(),
        Some(SubmissionClass::Background)
    );
    assert_eq!(
        session.camera().submission_class(),
        Some(SubmissionClass::Background),
        "issue #630: a view taken after the call carries the session default",
    );

    session.set_submission_class(None);
    assert_eq!(session.camera().submission_class(), None);

    session.close().expect("owner shutdown");
}

/// The erased projection carries the same submission-class surface.
#[cfg(all(
    feature = "dyn-api",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
async fn dyn_projection_has_the_same_surface<E: Executor>(executor: E) {
    let fake = lane_camera();
    let session = grafton_visca::Session::open(fake.async_wire(), g2_config(), executor.clone())
        .await
        .expect("owner session");
    let dynamic = session.camera_dyn().expect("dynamic view");

    let first = dynamic
        .submit_applied(&ZoomDrive::Tele)
        .await
        .expect("first socket");
    fake.wait_for_writes_async(&executor, 1).await;
    let second = dynamic
        .submit_applied(&FocusDrive::Far)
        .await
        .expect("second socket");
    fake.wait_for_writes_async(&executor, 2).await;

    let mut poller = dynamic.clone();
    assert_eq!(poller.submission_class(), None);
    poller.set_submission_class(Some(SubmissionClass::Background));
    assert_eq!(poller.submission_class(), Some(SubmissionClass::Background));
    assert_eq!(dynamic.submission_class(), None);

    let demoted = poller
        .submit_applied(&ZoomDrive::Wide)
        .await
        .expect("demoted submission");
    let ordinary = dynamic
        .with_submission_class(SubmissionClass::User)
        .submit_applied(&FocusDrive::Near)
        .await
        .expect("ordinary submission");
    for _ in 0..5 {
        executor.sleep(Duration::from_millis(2)).await;
    }
    assert_eq!(fake.write_count(), 2, "{STARVED}");

    complete_socket(&fake, 2);
    fake.wait_for_writes_async(&executor, 3).await;
    assert_eq!(
        fake.writes()[2],
        FOCUS_NEAR.to_vec(),
        "issue #630: the dynamic projection uses the same lanes",
    );

    complete_socket(&fake, 1);
    fake.wait_for_writes_async(&executor, 4).await;
    assert_eq!(fake.writes()[3], ZOOM_WIDE.to_vec());

    // A typed view projected out of the dynamic one inherits the default.
    let typed = poller.camera::<PtzOpticsG2>().expect("typed projection");
    assert_eq!(typed.submission_class(), Some(SubmissionClass::Background));

    first.detach();
    second.detach();
    demoted.detach();
    ordinary.detach();
    session.shutdown().expect("owner shutdown");
}

#[cfg(all(
    feature = "dyn-api",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
runtime_matrix!(dyn_projection_has_the_same_surface);
