//! Issue #714: raw lost-ACK recovery has the same bounded-wait and Urgent
//! safety-lane behavior on the async owner as on the blocking owner. Both
//! owners drive the same shell core (#780), so the parity legs give each the
//! same script and the same write barriers.

#![cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
#![allow(clippy::expect_used)]

#[path = "common/fake_camera.rs"]
mod fake_camera;
#[macro_use]
#[path = "common/matrix.rs"]
mod matrix;
#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use grafton_visca::{
    completion::AppliedOnly,
    profile::ProfileSpec,
    raw::{self, RawReplyShape},
    request::builtin::{FocusStop, ZoomDrive},
    transport::AddressingMode,
    AffectedAxes, ControlClass, Error, Executor, RetryClass, Session, SessionConfig, TimeoutClass,
};
#[cfg(feature = "blocking")]
use grafton_visca::{CameraId, OperationalTuning};

#[cfg(feature = "blocking")]
use fake_camera::FOCUS_STOP;
use fake_camera::{frames, FakeCamera, ZOOM_STOP};
use profile_fixtures::NonDefaultCompileTimeProfile;

/// An ACK on socket one from camera two.
#[cfg(feature = "blocking")]
const CAMERA_TWO_ACK_SOCKET_ONE: &[u8] = &[0xa0, 0x41, 0xff];

/// A raw datagram camera whose replies are scripted per write: each write
/// queues its step's replies. Writes past the script draw no reply.
fn scripted_camera(steps: Vec<Vec<Vec<u8>>>) -> FakeCamera {
    let mut steps: VecDeque<_> = steps.into();
    FakeCamera::new(move |_, answer| {
        for reply in steps.pop_front().unwrap_or_default() {
            answer.reply(reply);
        }
    })
}

/// Physical pacing bound for a write the owner may issue at once.
const PROMPT: Duration = Duration::from_millis(50);

/// Bound for a write the owner issues after its command spacing or the
/// predecessor's ACK deadline, but never after a quarantine release.
#[cfg(feature = "blocking")]
const UNHELD: Duration = Duration::from_secs(1);

/// Asserts that write `count` reached the camera within `bound` of `since`.
///
/// The shared wait helpers only guard against a hang; this is the promptness
/// claim. It is measured after the wait returns, so scheduling delay can only
/// lengthen the observed time, never hide a late write.
fn assert_written_within(camera: &FakeCamera, count: usize, since: Instant, bound: Duration) {
    let elapsed = since.elapsed();
    assert!(
        camera.write_count() >= count && elapsed < bound,
        "write {count} took {elapsed:?}, over its {bound:?} bound"
    );
}

fn session_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("two-socket raw profile"),
    )
}

#[cfg(feature = "blocking")]
fn two_camera_session_config(command_spacing: Duration) -> SessionConfig {
    let mut config = session_config();
    config
        .register_target(
            CameraId::CAMERA_2,
            ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
                .expect("two-socket raw profile"),
        )
        .expect("second serial target");
    config.with_tuning(OperationalTuning::new().command_spacing(command_spacing))
}

fn ordinary_operation() -> raw::AppliedOnly {
    let policy = raw::Policy::new(TimeoutClass::Quick, RetryClass::Never, ControlClass::Normal)
        .expect("raw policy")
        .with_reply_shape(RawReplyShape::AckThenCompletion);
    raw::AppliedOnly::with_policy(ZOOM_STOP, AffectedAxes::ZOOM, policy)
        .expect("ordinary raw operation")
}

#[cfg(feature = "blocking")]
fn ordinary_operation_for(target: CameraId) -> raw::AppliedOnly {
    let mut wire = ZOOM_STOP.to_vec();
    wire[0] = target.to_address_byte();
    raw::AppliedOnly::with_policy(
        wire,
        AffectedAxes::ZOOM,
        raw::Policy::new(TimeoutClass::Quick, RetryClass::Never, ControlClass::Normal)
            .expect("raw policy")
            .with_reply_shape(RawReplyShape::AckThenCompletion),
    )
    .expect("targeted ordinary raw operation")
}

/// Ordinary work stays queued until the predecessor's 100 ms ACK deadline
/// plus its one-second ambiguity window, then writes.
async fn ordinary_successor_waits_for_lost_ack_quarantine<E: Executor>(executor: E) {
    let camera = scripted_camera(vec![vec![], vec![frames::ack(1), frames::complete(1)]]);
    let session = Session::open(
        camera.async_wire().with_addressing(AddressingMode::Ip),
        session_config(),
        executor.clone(),
    )
    .await
    .expect("async owner session");
    let view = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");
    let mut predecessor = view
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .await
        .expect("predecessor admission");
    let submitted = Instant::now();
    camera.wait_for_writes_async(&executor, 1).await;
    assert_written_within(&camera, 1, submitted, PROMPT);
    let first_written_at = Instant::now();
    executor.sleep(Duration::from_millis(150)).await;
    let ordinary = ordinary_operation();
    let mut successor = view
        .submit::<AppliedOnly, _>(&ordinary)
        .await
        .expect("ordinary successor admission");
    camera.wait_for_writes_async(&executor, 2).await;
    assert_written_within(&camera, 2, first_written_at, Duration::from_secs(2));
    let release_elapsed = first_written_at.elapsed();
    assert!(
        release_elapsed >= Duration::from_millis(950),
        "ordinary successor wrote before quarantine release: {release_elapsed:?}"
    );
    assert!(
        release_elapsed < Duration::from_secs(2),
        "ordinary successor missed quarantine release: {release_elapsed:?}"
    );
    assert!(matches!(
        predecessor.applied().await,
        Err(Error::UnsequencedCommandUnconfirmed)
    ));
    successor
        .applied()
        .await
        .expect("ordinary successor settles after release");
    session.shutdown().expect("owner shutdown");
}

/// Urgent work crosses the pre-ACK candidate immediately. Every ACK queued
/// by the second send is read while both candidates are open and binds to
/// neither, so both receipts eventually report the unconfirmed outcome.
async fn urgent_work_crosses_the_pre_ack_candidate<E: Executor>(executor: E) {
    let camera = scripted_camera(vec![
        vec![],
        vec![frames::ack(1), frames::ack(2), frames::complete(2)],
    ]);
    let session = Session::open(
        camera.async_wire().with_addressing(AddressingMode::Ip),
        session_config(),
        executor.clone(),
    )
    .await
    .expect("async owner session");
    let view = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");
    let mut predecessor = view
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .await
        .expect("predecessor admission");
    let submitted = Instant::now();
    camera.wait_for_writes_async(&executor, 1).await;
    assert_written_within(&camera, 1, submitted, PROMPT);
    let urgent_started = Instant::now();
    let mut urgent = view
        .submit::<AppliedOnly, _>(&FocusStop)
        .await
        .expect("urgent admission");
    camera.wait_for_writes_async(&executor, 2).await;
    assert_written_within(&camera, 2, urgent_started, PROMPT);
    assert!(matches!(
        urgent.applied().await,
        Err(Error::UnsequencedCommandUnconfirmed)
    ));
    assert!(matches!(
        predecessor.applied().await,
        Err(Error::UnsequencedCommandUnconfirmed)
    ));
    session.shutdown().expect("owner shutdown");
}

runtime_matrix!(
    ordinary_successor_waits_for_lost_ack_quarantine,
    urgent_work_crosses_the_pre_ack_candidate
);

#[cfg(feature = "blocking")]
mod parity {
    use super::*;
    use grafton_visca::{
        blocking::{Session as BlockingSession, SessionConfig as BlockingSessionConfig},
        transport::{AddressingMode, TransportConfig},
    };

    /// How soon the urgent stop must follow its submission while camera one's
    /// ordinary request sits in its lost-ACK `PreAck` hold.
    ///
    /// Crossing the hold, the stop waits only for the 20 ms command spacing.
    /// Held behind it, the stop could not be written before the ordinary
    /// request's ambiguity window closes: one second after that request's
    /// write, less the 100 ms ACK deadline that `ordinary.applied()` already
    /// waited out before the stop was submitted, so at least ~900 ms. 500 ms
    /// sits between the two with more than 400 ms of margin on each side, so
    /// neither scheduler load nor a slow CI host can flip the verdict.
    const STOP_CROSSES_PREACK: Duration = Duration::from_millis(500);

    /// The two-camera serial transcript's camera: the first write draws
    /// camera two's ACK, the fifth draws the urgent stop's ACK and completion.
    /// The blocking and async owners get the same script.
    fn pending_cancel_camera() -> FakeCamera {
        scripted_camera(vec![
            vec![CAMERA_TWO_ACK_SOCKET_ONE.to_vec()],
            vec![],
            vec![],
            vec![],
            vec![frames::ack(1), frames::complete(1)],
        ])
    }

    fn serial_config() -> TransportConfig {
        let mut config = TransportConfig::default();
        config.addressing = AddressingMode::Serial;
        config
    }

    fn blocking_transcript() -> Vec<Vec<u8>> {
        let camera = FakeCamera::silent();
        let session = BlockingSession::open(
            camera.blocking_wire(),
            BlockingSessionConfig::new(
                ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
                    .expect("raw profile"),
            ),
        )
        .expect("blocking session");
        let view = session
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera view");
        let _predecessor = view
            .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
            .expect("blocking predecessor");
        let submitted = Instant::now();
        camera.wait_for_writes(1);
        assert_written_within(&camera, 1, submitted, UNHELD);
        let _urgent = view
            .submit::<AppliedOnly, _>(&FocusStop)
            .expect("blocking urgent");
        let submitted = Instant::now();
        let transcript = camera.wait_for_writes(2);
        assert_written_within(&camera, 2, submitted, UNHELD);
        session.shutdown().expect("blocking shutdown");
        transcript
    }

    /// One five-write public-facade transcript spans both #744 defects:
    /// camera-two's paced cancellation must not reject camera-one's normal
    /// request, and the later urgent stop must bind its ACK through that
    /// normal request's lost-ACK `PreAck` hold.
    fn blocking_pending_cancel_and_preack_transcript() -> Vec<Vec<u8>> {
        const SPACING: Duration = Duration::from_millis(20);

        let camera = pending_cancel_camera();
        let session = BlockingSession::open(
            camera
                .blocking_wire()
                .with_config(serial_config())
                .with_addressing(AddressingMode::Serial),
            two_camera_session_config(SPACING),
        )
        .expect("blocking multi-camera session");
        let camera_one = session
            .camera_for::<NonDefaultCompileTimeProfile>(CameraId::CAMERA_1)
            .expect("camera one view");
        let camera_two = session
            .camera_for::<NonDefaultCompileTimeProfile>(CameraId::CAMERA_2)
            .expect("camera two view");

        let mut predecessor = camera_two
            .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
            .expect("camera two predecessor");
        let submitted = Instant::now();
        camera.wait_for_writes(1);
        assert_written_within(&camera, 1, submitted, UNHELD);
        let _socket_successor = camera_two
            .submit::<AppliedOnly, _>(&ordinary_operation_for(CameraId::CAMERA_2))
            .expect("camera two successor");
        let submitted = Instant::now();
        camera.wait_for_writes(2);
        assert_written_within(&camera, 2, submitted, UNHELD);
        // A zero-length wait records the paced cancellation and returns; the
        // camera never answers it.
        assert!(matches!(
            predecessor.cancel_with_timeout(Duration::ZERO),
            Err(Error::ObservationTimeout { .. })
        ));
        let mut ordinary = camera_one
            .submit::<AppliedOnly, _>(&ordinary_operation())
            .expect("camera one ordinary admission");
        let submitted = Instant::now();
        camera.wait_for_writes(4);
        assert_written_within(&camera, 4, submitted, UNHELD);
        assert!(matches!(
            ordinary.applied(),
            Err(Error::UnsequencedCommandUnconfirmed)
        ));
        let mut stop = camera_one
            .submit::<AppliedOnly, _>(&FocusStop)
            .expect("urgent stop crosses camera-one PreAck hold");
        let stop_submitted = Instant::now();
        camera.wait_for_writes(5);
        assert_written_within(&camera, 5, stop_submitted, STOP_CROSSES_PREACK);
        stop.applied()
            .expect("the urgent stop ACK is attributable and completes");

        let transcript = camera.writes();
        session.shutdown().expect("blocking shutdown");
        transcript
    }

    async fn blocking_and_async_owners_emit_the_same_urgent_transcript<E: Executor>(executor: E) {
        let blocking = blocking_transcript();
        let camera = FakeCamera::silent();
        let session = Session::open(
            camera.async_wire().with_addressing(AddressingMode::Ip),
            session_config(),
            executor.clone(),
        )
        .await
        .expect("async session");
        let view = session
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera view");
        let _predecessor = view
            .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
            .await
            .expect("async predecessor");
        let submitted = Instant::now();
        camera.wait_for_writes_async(&executor, 1).await;
        assert_written_within(&camera, 1, submitted, PROMPT);
        let _urgent = view
            .submit::<AppliedOnly, _>(&FocusStop)
            .await
            .expect("async urgent");
        let submitted = Instant::now();
        let asynchronous = camera.wait_for_writes_async(&executor, 2).await;
        assert_written_within(&camera, 2, submitted, PROMPT);
        session.shutdown().expect("async shutdown");

        assert_eq!(asynchronous, blocking);
    }

    async fn blocking_and_async_owners_match_the_pending_cancel_recovery_transcript<E: Executor>(
        executor: E,
    ) {
        const SPACING: Duration = Duration::from_millis(20);

        let blocking = blocking_pending_cancel_and_preack_transcript();
        let camera = pending_cancel_camera();
        let session = Session::open(
            camera
                .async_wire()
                .with_config(serial_config())
                .with_addressing(AddressingMode::Serial),
            two_camera_session_config(SPACING),
            executor.clone(),
        )
        .await
        .expect("async multi-camera session");
        let camera_one = session
            .camera_for::<NonDefaultCompileTimeProfile>(CameraId::CAMERA_1)
            .expect("camera one view");
        let camera_two = session
            .camera_for::<NonDefaultCompileTimeProfile>(CameraId::CAMERA_2)
            .expect("camera two view");

        let mut predecessor = camera_two
            .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
            .await
            .expect("camera two predecessor");
        let submitted = Instant::now();
        camera.wait_for_writes_async(&executor, 1).await;
        assert_written_within(&camera, 1, submitted, UNHELD);
        let _socket_successor = camera_two
            .submit::<AppliedOnly, _>(&ordinary_operation_for(CameraId::CAMERA_2))
            .await
            .expect("camera two successor");
        let submitted = Instant::now();
        camera.wait_for_writes_async(&executor, 2).await;
        assert_written_within(&camera, 2, submitted, UNHELD);
        // The request is delivered before the zero-length wait expires, so the
        // owner records the paced cancellation exactly as the blocking owner
        // does; the camera never answers it.
        assert!(matches!(
            predecessor.cancel_with_timeout(Duration::ZERO).await,
            Err(Error::ObservationTimeout { .. })
        ));
        let mut ordinary = camera_one
            .submit::<AppliedOnly, _>(&ordinary_operation())
            .await
            .expect("camera one ordinary admission");
        let submitted = Instant::now();
        camera.wait_for_writes_async(&executor, 4).await;
        assert_written_within(&camera, 4, submitted, UNHELD);
        assert!(matches!(
            ordinary.applied().await,
            Err(Error::UnsequencedCommandUnconfirmed)
        ));
        let mut stop = camera_one
            .submit::<AppliedOnly, _>(&FocusStop)
            .await
            .expect("urgent stop crosses camera-one PreAck hold");
        let stop_submitted = Instant::now();
        camera.wait_for_writes_async(&executor, 5).await;
        assert_written_within(&camera, 5, stop_submitted, STOP_CROSSES_PREACK);
        stop.applied()
            .await
            .expect("the urgent stop ACK is attributable and completes");

        let asynchronous = camera.writes();
        session.shutdown().expect("async shutdown");
        assert_eq!(
            asynchronous, blocking,
            "both owners must emit the identical #744 transcript"
        );
        assert_eq!(
            asynchronous,
            vec![
                vec![0x82, 0x01, 0x04, 0x07, 0x02, 0xff],
                vec![0x82, 0x01, 0x04, 0x07, 0x00, 0xff],
                vec![0x82, 0x21, 0xff],
                ZOOM_STOP.to_vec(),
                FOCUS_STOP.to_vec(),
            ],
            "the transcript retains the cancellation, camera-one write, and acknowledged urgent stop"
        );
    }

    runtime_matrix!(
        blocking_and_async_owners_emit_the_same_urgent_transcript,
        blocking_and_async_owners_match_the_pending_cancel_recovery_transcript
    );
}
