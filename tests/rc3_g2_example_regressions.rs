//! Hardware-free regressions for two PTZOptics G2 observations that drove
//! `examples/cancellation.rs --g2-unsupported` and `examples/motion_safety.rs`.
//!
//! The examples are binaries and cannot be linked into a test, so these
//! characterization tests pin the library behaviour the example fixes rely on.

#![cfg(feature = "blocking")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use grafton_visca_test_support::fake_camera;

use std::{thread, time::Duration};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    camera::profiles::PtzOpticsG2,
    profile::ProfileSpec,
    transport::SendSemantics,
    CancellationOutcome, HaltOutcome,
};

use fake_camera::{frames, FakeCamera, FOCUS_STOP, ZOOM_STOP, ZOOM_TELE};

const PAN_TILT_STOP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0c, 0x0a, 0x03, 0x03, 0xff];

/// A G2 on a byte stream: a session over `camera` on the real profile.
fn g2_session(camera: &FakeCamera) -> Session {
    Session::open(
        camera.blocking_wire().with_semantics(SendSemantics::Stream),
        SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile")),
    )
    .expect("session")
}

/// The G2's halt answers: pan/tilt STOP completes on socket one and zoom STOP
/// on socket two; the focus STOP draws `focus`.
fn halt_camera(focus: Vec<Vec<u8>>) -> FakeCamera {
    FakeCamera::new(move |bytes, answer| match bytes {
        b if b == PAN_TILT_STOP => {
            answer.reply(frames::ack(1)).reply(frames::complete(1));
        }
        b if b == ZOOM_STOP => {
            answer.reply(frames::ack(2)).reply(frames::complete(2));
        }
        b if b == FOCUS_STOP => {
            for reply in &focus {
                answer.reply(reply.clone());
            }
        }
        _ => {}
    })
}

fn all_writes() -> Vec<Vec<u8>> {
    vec![
        PAN_TILT_STOP.to_vec(),
        ZOOM_STOP.to_vec(),
        FOCUS_STOP.to_vec(),
    ]
}

/// Hardware observation: a continuous zoom drive (`81 01 04 07 02 FF`) on a
/// real PTZOptics G2 gets ACK and completion together about 50 ms after the
/// write while the zoom keeps moving (0x0EFC to 0x344B over 3 s). `cancel()`
/// then returns `Ok(Completed)` and writes nothing, so the example must send
/// its zoom STOP unconditionally rather than only after a refused cancel.
#[test]
fn g2_cancel_after_immediate_completion_is_completed_and_sends_nothing() {
    let fake = FakeCamera::new(|bytes, answer| {
        if bytes == ZOOM_TELE {
            answer.reply(frames::ack(1)).reply(frames::complete(1));
        }
    });
    let session = g2_session(&fake);
    let view = session.camera::<PtzOpticsG2>().expect("camera");

    let mut operation = view.zoom().tele().expect("tele admitted");
    thread::sleep(Duration::from_millis(100));
    let outcome = operation.cancel();
    assert!(
        matches!(outcome, Ok(CancellationOutcome::Completed)),
        "{outcome:?}"
    );
    assert_eq!(fake.writes(), [ZOOM_TELE.to_vec()]);
    session.shutdown().expect("shutdown");
}

/// Hardware observation (rc.3 motion_safety, PTZOptics G2 cam4 in auto focus,
/// the factory default): focus STOP is rejected with `90 61 41 FF` (command not
/// executable on a free socket) while pan/tilt and zoom STOP succeed. The
/// report is pan/tilt Applied, zoom Applied, focus `Failed(_)` (rc.3 reports
/// `UnsequencedCommandUnconfirmed`; the exact classification is deliberately
/// not pinned, so a later conclusive not-executable mapping does not break
/// this); `into_result()` collapses it to an error, which is why the example
/// must inspect each axis instead.
#[test]
fn g2_auto_focus_halt_report_keeps_pan_tilt_and_zoom_applied() {
    let fake = halt_camera(vec![frames::not_executable(1)]);
    let session = g2_session(&fake);
    let view = session.camera::<PtzOpticsG2>().expect("camera");

    let report = view.motion().stop_all_motion().expect("halt accepted");

    assert!(
        matches!(report.pan_tilt, HaltOutcome::Applied),
        "{report:?}"
    );
    assert!(matches!(report.zoom, HaltOutcome::Applied), "{report:?}");
    assert!(matches!(report.focus, HaltOutcome::Failed(_)), "{report:?}");
    assert!(report.into_result().is_err());
    assert_eq!(fake.writes(), all_writes());
    session.shutdown().expect("shutdown");
}

/// Hardware observation: with manual focus the same PTZOptics G2 accepts the
/// focus STOP (`90 42 FF`, `90 52 FF`), so all three axes are Applied and the
/// example exits 0.
#[test]
fn g2_manual_focus_halt_report_is_all_applied() {
    let fake = halt_camera(vec![frames::ack(2), frames::complete(2)]);
    let session = g2_session(&fake);
    let view = session.camera::<PtzOpticsG2>().expect("camera");

    let report = view.motion().stop_all_motion().expect("halt accepted");

    assert!(
        matches!(report.pan_tilt, HaltOutcome::Applied),
        "{report:?}"
    );
    assert!(matches!(report.zoom, HaltOutcome::Applied), "{report:?}");
    assert!(matches!(report.focus, HaltOutcome::Applied), "{report:?}");
    assert!(report.into_result().is_ok());
    assert_eq!(fake.writes(), all_writes());
    session.shutdown().expect("shutdown");
}
