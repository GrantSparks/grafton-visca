//! Issue #561: blocking submission queues behind busy sockets.
//!
//! A blocking operation handle is returned once the owner has admitted the
//! request (D24). A request that finds every socket busy stays queued and is
//! written when one frees; a transport write failure is reported through the
//! operation's own outcome.

#![cfg(feature = "blocking")]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[path = "common/fake_camera.rs"]
mod fake_camera;

use fake_camera::{frames, Answer, FakeCamera, FOCUS_STOP, ZOOM_STOP};
use grafton_visca::{
    blocking::{Session, SessionConfig},
    completion::AppliedOnly,
    profile::ProfileSpec,
    profiles::SonyFR7,
    request::builtin::{FocusStop, PanTiltStop, ZoomStop},
    types::{PanSpeed, TiltSpeed},
    Error, OperationalTuning,
};

use profile_fixtures::{session_config, sony_session_config, NonDefaultCompileTimeProfile};

const PAN_TILT_STOP_PREFIX: &[u8] = &[0x81, 0x01, 0x06, 0x01];

/// Queue an ACK and a completion on alternating sockets 1 and 2, the way a
/// two-socket camera answers each accepted command in write order.
fn answer_on_alternating_sockets(next_socket: &mut u8, answer: &mut Answer) {
    let socket = (*next_socket % 2) + 1;
    *next_socket = next_socket.wrapping_add(1);
    answer
        .reply(frames::ack(socket))
        .reply(frames::complete(socket));
}

/// A two-socket camera: every accepted command is answered with an ACK and a
/// completion on alternating sockets, in the order the commands were written.
/// Replies follow the framing of each request.
fn two_socket_camera() -> FakeCamera {
    let mut next_socket = 0_u8;
    FakeCamera::visca(move |_, answer| answer_on_alternating_sockets(&mut next_socket, answer))
}

fn pan_tilt_stop() -> PanTiltStop {
    PanTiltStop::new(
        PanSpeed::new(1).expect("valid pan speed"),
        TiltSpeed::new(1).expect("valid tilt speed"),
    )
}

const FIRST_WRITE_ERROR: &str = "distinctive first-write transport failure";

/// The sends a camera accepted, as opposed to every attempted send (which is
/// `FakeCamera::writes`). A failed send accepts no bytes.
type AcceptedWrites = Arc<Mutex<Vec<Vec<u8>>>>;

fn accepted_count(accepted: &AcceptedWrites) -> usize {
    accepted.lock().expect("accepted lock").len()
}

/// A datagram camera whose first send fails before accepting bytes, then
/// recovers for a later submission. The camera records attempted sends and
/// `accepted` records the successful ones, so an operation cannot hide a failed
/// first write as a no-write path.
fn first_write_failure_camera() -> (FakeCamera, AcceptedWrites) {
    let accepted = AcceptedWrites::default();
    let log = Arc::clone(&accepted);
    let mut sends = 0_usize;
    let mut next_socket = 0_u8;
    let camera = FakeCamera::new(move |write, answer| {
        sends = sends.saturating_add(1);
        if sends == 1 {
            answer.fail_send(Error::TransportError(FIRST_WRITE_ERROR.into()));
            return;
        }
        log.lock().expect("accepted lock").push(write.to_vec());
        answer_on_alternating_sockets(&mut next_socket, answer);
    });
    (camera, accepted)
}

/// A datagram camera with a small independent pacing guard. The profile used
/// by the pacing test requires a longer interval, so a skipped profile wait is
/// rejected by this camera without relying on a wall-clock lower bound
/// assertion in the test itself. Replies follow the framing of each request.
fn pacing_camera(minimum_spacing: Duration) -> (FakeCamera, AcceptedWrites) {
    let accepted = AcceptedWrites::default();
    let log = Arc::clone(&accepted);
    let mut next_eligible: Option<Instant> = None;
    let mut next_socket = 0_u8;
    let camera = FakeCamera::visca(move |write, answer| {
        let now = Instant::now();
        if next_eligible.is_some_and(|eligible| now < eligible) {
            answer.fail_send(Error::TransportError("transport pacing violation".into()));
            return;
        }
        log.lock().expect("accepted lock").push(write.to_vec());
        next_eligible = Some(now + minimum_spacing);
        answer_on_alternating_sockets(&mut next_socket, answer);
    });
    (camera, accepted)
}

/// A third public operation over a two-socket target is admitted and queued
/// while both sockets are busy, then written once one frees. Nothing is
/// rejected and nothing is written twice.
#[test]
fn a_third_operation_queues_until_a_socket_frees() {
    let fake = two_socket_camera();
    let session =
        Session::open(fake.blocking_wire(), sony_session_config()).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    let mut first = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first submission");
    let mut second = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("second submission");
    let mut third = camera
        .submit::<AppliedOnly, _>(&pan_tilt_stop())
        .expect("a third operation is admitted and queued");

    first.applied().expect("first operation applied");
    second.applied().expect("second operation applied");
    third.applied().expect("queued operation applied");

    assert!(
        fake.writes()
            .iter()
            .all(|write| frames::sony_split(write).is_some()),
        "every Sony write carries its sequence envelope"
    );
    let written = fake.take_payloads();
    assert_eq!(written.len(), 3, "each operation writes exactly once");
    assert_eq!(written[0], ZOOM_STOP);
    assert_eq!(written[1], FOCUS_STOP);
    assert!(written[2].starts_with(PAN_TILT_STOP_PREFIX));

    let metrics = session.metrics().expect("metrics");
    assert_eq!(metrics.admitted, 3);
    assert_eq!(metrics.terminal, 3);
    assert_eq!(metrics.active, 0);
    session.shutdown().expect("owner shutdown");
}

/// The same holds when a target is tuned to one command socket: the second
/// operation waits in the owner's queue and is written only after the first
/// one completes.
#[test]
fn a_one_socket_target_queues_the_second_operation() {
    let config = session_config().with_tuning(OperationalTuning::new().maximum_command_sockets(1));
    let fake = two_socket_camera();
    let session = Session::open(fake.blocking_wire(), config).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let mut first = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first operation");
    let mut second = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("the second operation is admitted while the one socket is busy");
    first.applied().expect("first operation applied");
    second.applied().expect("queued operation applied");
    assert_eq!(fake.writes(), [ZOOM_STOP.to_vec(), FOCUS_STOP.to_vec()]);
    session.shutdown().expect("owner shutdown");
}

/// A typed operation whose datagram send fails is still admitted; the exact
/// transport error is its outcome (D24). The terminalized entry releases its
/// shared permit, so a one-slot session can immediately admit and complete a
/// later operation.
#[test]
fn a_failed_operation_write_is_its_outcome_and_releases_the_permit() {
    let config =
        session_config().with_admission_capacity(NonZeroUsize::new(1).expect("one permit"));
    let (fake, accepted) = first_write_failure_camera();
    let session = Session::open(fake.blocking_wire(), config).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let error = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission succeeds at admission")
        .applied()
        .expect_err("the failed write is the operation's outcome");
    match &error {
        Error::TransportError(reason) => assert_eq!(reason.as_ref(), FIRST_WRITE_ERROR),
        other => panic!("expected the exact transport error, got {other:?}"),
    }
    assert_eq!(fake.write_count(), 1, "the failed operation tried once");
    assert_eq!(
        accepted_count(&accepted),
        0,
        "the failed send accepted no bytes"
    );

    let metrics = session.metrics().expect("metrics after failed write");
    assert_eq!(metrics.admitted, 1);
    assert_eq!(metrics.terminal, 1);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.pending, 0);
    assert_eq!(metrics.writes, 1, "metrics count the attempted write");
    assert_eq!(metrics.write_failures, 1);

    // With only one admission permit, this proves that terminal removal also
    // released the permit rather than merely dropping the public observer.
    camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("the terminalized request released its admission permit")
        .applied()
        .expect("the later datagram operation must recover normally");
    assert_eq!(fake.write_count(), 2);
    assert_eq!(accepted_count(&accepted), 1);

    let metrics = session.metrics().expect("metrics after recovery");
    assert_eq!(metrics.admitted, 2);
    assert_eq!(metrics.terminal, 2);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.pending, 0);
    assert_eq!(metrics.writes, 2);
    assert_eq!(metrics.write_failures, 1);
    session.shutdown().expect("owner shutdown");
}

/// Profile pacing holds the second write back even though the second socket
/// is free; the transport guard catches an owner that skips that wait.
#[test]
fn typed_operations_respect_profile_pacing() {
    let profile = ProfileSpec::from_compile_time::<SonyFR7>().expect("Sony FR7 profile");
    assert_eq!(
        profile.timing().minimum_command_spacing(),
        Duration::from_millis(35),
        "the pacing precondition must be nonzero"
    );
    let (fake, accepted) = pacing_camera(Duration::from_millis(30));
    let session =
        Session::open(fake.blocking_wire(), SessionConfig::new(profile)).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    let mut first = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first operation");
    let mut second = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("second operation");
    first.applied().expect("first operation applied");
    second.applied().expect("second operation applied");
    assert!(
        fake.writes()
            .iter()
            .all(|write| frames::sony_split(write).is_some()),
        "every Sony write carries its sequence envelope"
    );
    assert_eq!(fake.write_count(), 2, "no write violated pacing");
    assert_eq!(accepted_count(&accepted), 2);
    session.shutdown().expect("owner shutdown");
}
