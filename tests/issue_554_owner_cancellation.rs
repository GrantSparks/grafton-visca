//! Owner-backed cancellation acceptance for the built-in PTZOptics G2 profile.
//!
//! The G2 profile deliberately has no VISCA socket-cancel support. These tests
//! exercise the root session on both facades and keep the wire transcript
//! observable at the transport boundary. Admission returns before the owner
//! necessarily writes the command, so each test confirms the write before it
//! cancels; terminal progression is observed through the same operation handle
//! while the owner drives all protocol progress.
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

#[path = "common/fake_camera.rs"]
mod fake_camera;
#[macro_use]
#[path = "common/matrix.rs"]
mod matrix;

use std::time::Duration;

use grafton_visca::{
    completion::AppliedOnly,
    profile::{OperationalTuning, ProfileSpec},
    profiles::PtzOpticsG2,
    request::builtin::FocusStop,
    CancellationOutcome, Error, SessionConfig,
};

use fake_camera::{frames, FakeCamera, FOCUS_STOP, ZOOM_STOP};

fn g2_config(maximum_command_sockets: Option<u8>) -> SessionConfig {
    let profile = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("built-in G2 profile");
    let config = SessionConfig::new(profile);
    match maximum_command_sockets {
        Some(maximum) => {
            config.with_tuning(OperationalTuning::new().maximum_command_sockets(maximum))
        }
        None => config,
    }
}

/// A camera that leaves its first write to the test and answers every later
/// write with ACK and completion on socket 1 in one read.
fn completing_after_first() -> FakeCamera {
    let mut writes = 0usize;
    FakeCamera::new(move |_, answer| {
        writes += 1;
        if writes > 1 {
            answer.reply(fake_camera::ack_and_complete(1));
        }
    })
}

/// A transmitted cancellation is refused by profile policy, and the refused
/// handle still observes the original operation to its terminal state.
///
/// With `ack_before_cancel` the original is executing on its socket when it
/// is cancelled; without it the original is written but not yet acknowledged.
macro_rules! sent_cancel_is_not_supported {
    ($ack_before_cancel:expr) => {{
        let ack_before_cancel: bool = $ack_before_cancel;
        let fake = completing_after_first();
        let session = open!(fake, g2_config(Some(1))).expect("owner session");
        let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

        let mut original = wait!(camera.zoom().stop()).expect("transmitted operation");
        wait_for_writes!(fake, 1);
        assert_eq!(fake.writes(), vec![ZOOM_STOP.to_vec()]);

        if ack_before_cancel {
            // The response is delivered as a separate owner input turn. Once
            // the read is complete, a short pause lets the ACK transition the
            // exact original operation into its executing socket phase.
            fake.push(frames::ack(1));
            wait_for_reads!(fake, 1);
            pause!(Duration::from_millis(5));
        }

        let refused = wait!(original.cancel())
            .expect_err("G2 sent cancellation must be rejected by profile policy");
        assert!(matches!(refused, Error::NotSupported));
        // A refusal leaves the handle observing the original operation (#777).
        assert_eq!(fake.writes(), vec![ZOOM_STOP.to_vec()]);

        if ack_before_cancel {
            fake.push(frames::complete(1));
        } else {
            fake.push(fake_camera::ack_and_complete(1));
        }

        within!(Duration::from_secs(2), original.applied())
            .expect("the handle still observes the original operation");

        // A one-socket tuning makes this next operation wait for the original
        // terminal frame. Its successful applied wait therefore proves the
        // rejected cancellation did not remove or terminalize the original
        // entry, and that no cancellation frame was transmitted.
        let mut next =
            wait!(camera.submit::<AppliedOnly, _>(&FocusStop)).expect("next operation admission");
        within!(Duration::from_secs(2), next.applied())
            .expect("next operation applied after original terminal");
        assert_eq!(fake.writes(), vec![ZOOM_STOP.to_vec(), FOCUS_STOP.to_vec()]);

        session.shutdown().expect("owner shutdown");
    }};
}

facade_matrix! {
    /// A still-queued operation cancels locally, without touching the wire.
    fn queued_cancel_is_local() {
        let fake = FakeCamera::silent();
        let session = open!(fake, g2_config(None)).expect("owner session");
        let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

        // G2 has two command sockets. Keep both occupied before admitting the
        // third typed operation so its cancellation is unambiguously pre-wire.
        let first = wait!(camera.zoom().stop()).expect("first operation");
        wait_for_writes!(fake, 1);
        fake.push(frames::ack(1));
        wait_for_reads!(fake, 1);
        let second = wait!(camera.submit::<AppliedOnly, _>(&FocusStop)).expect("second operation");
        wait_for_writes!(fake, 2);
        fake.push(frames::ack(2));
        wait_for_reads!(fake, 2);

        let mut queued = wait!(camera.pan_tilt().home()).expect("queued operation");
        assert!(matches!(
            wait!(queued.cancel_with_timeout(Duration::from_secs(1))),
            Ok(CancellationOutcome::Cancelled)
        ));
        assert_eq!(
            fake.writes(),
            vec![ZOOM_STOP.to_vec(), FOCUS_STOP.to_vec()]
        );

        first.detach();
        second.detach();
        session.shutdown().expect("owner shutdown");
    }

    /// Cancelling a written but unacknowledged operation is refused.
    fn sent_cancel_is_not_supported_before_ack() {
        sent_cancel_is_not_supported!(false);
    }

    /// Cancelling an acknowledged, executing operation is refused.
    fn sent_cancel_is_not_supported_after_ack() {
        sent_cancel_is_not_supported!(true);
    }

    /// A completion the owner drained before the cancellation boundary wins.
    fn terminal_race_reports_completed() {
        let fake = FakeCamera::silent();
        let session = open!(fake, g2_config(Some(1))).expect("owner session");
        let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

        let mut original = wait!(camera.zoom().stop()).expect("transmitted operation");
        wait_for_writes!(fake, 1);
        // One receive turn carries both frames. The owner drains both before
        // it can service the later cancellation boundary, making completion
        // the race winner even though the profile does not support socket
        // cancellation.
        fake.push(fake_camera::ack_and_complete(1));
        wait_for_reads!(fake, 1);
        for _ in 0..5 {
            pause!(Duration::from_millis(1));
        }

        assert!(matches!(
            wait!(original.cancel_with_timeout(Duration::from_secs(1))),
            Ok(CancellationOutcome::Completed)
        ));
        assert_eq!(fake.writes(), vec![ZOOM_STOP.to_vec()]);
        session.shutdown().expect("owner shutdown");
    }
}
