//! Issue #566: retry coverage through both facades.
//!
//! The engine tests in `src/runtime/engine/tests.rs` pin the state machine.
//! This file pins what a caller actually observes: which camera refusals are
//! replayed and which are surfaced, that a sequence-correlated Sony movement
//! command survives a lost ACK and a silent post-ACK camera, and that an
//! unresolvable Sony cancellation reaches the caller as
//! `Error::CancellationUnconfirmed`. A raw command with the same ambiguity fails
//! only itself as `Error::UnsequencedCommandUnconfirmed` (issue #671) while the
//! session keeps running.
//!
//! The conclusive camera refusals stay on raw VISCA, while the ambiguous
//! ACK/completion and cancellation cases use Sony's sequence envelope: raw
//! traffic cannot safely identify a later reply after a successful write has
//! gone unresolved.
//!
//! Every scenario runs on the blocking facade and on the async facade under
//! each enabled runtime, because a retry policy that only holds on one owner
//! is not a policy.

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
#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;
#[path = "common/retry_requests.rs"]
mod retry_requests;

use std::{collections::VecDeque, time::Duration};

use grafton_visca::{
    completion::AppliedOnly, profile::ProfileSpec, profiles::SonyFR7, raw,
    request::builtin::ZoomStop, types::ZoomPosition, ControlClass, Error, InquiryRoute, RetryClass,
    SessionConfig, TimeoutClass,
};

use fake_camera::{frames, FakeCamera};
use profile_fixtures::NonDefaultCompileTimeProfile;
use retry_requests::{MovementCommand, StandardCommand};

const ZOOM_POSITION_INQUIRY: &[u8] = &[0x81, 0x09, 0x04, 0x47, 0xff];

fn decode_custom_inquiry(payload: &[u8]) -> Result<Vec<u8>, Error> {
    Ok(payload.to_vec())
}

fn custom_zoom_inquiry() -> raw::Inquiry<Vec<u8>> {
    raw::Inquiry::from_fn(
        ZOOM_POSITION_INQUIRY,
        InquiryRoute::RAW,
        decode_custom_inquiry,
        TimeoutClass::Inquiry,
        RetryClass::Inquiry,
        ControlClass::Normal,
    )
    .expect("valid custom inquiry")
}

/// A camera whose answer to the n-th write is `steps[n]`, and whose answer to
/// every write past the script is `trailing`. Replies come back in the
/// framing of the request: raw VISCA, or Sony's envelope echoing the
/// request's sequence number.
fn scripted(steps: Vec<Vec<Vec<u8>>>, trailing: Vec<Vec<u8>>) -> FakeCamera {
    let mut steps = VecDeque::from(steps);
    FakeCamera::visca(move |_, answer| {
        for reply in steps.pop_front().unwrap_or_else(|| trailing.clone()) {
            answer.reply(reply);
        }
    })
}

fn session_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("two-socket runtime profile"),
    )
}

fn sony_session_config() -> SessionConfig {
    SessionConfig::new(ProfileSpec::from_compile_time::<SonyFR7>().expect("Sony FR7 profile"))
}

fn standard_reply() -> Vec<Vec<u8>> {
    vec![frames::ack(1), frames::complete(1)]
}

/// `0x41` — the camera refuses to execute right now.
fn not_executable() -> Vec<u8> {
    frames::not_executable(0)
}

/// `0x05` — the camera has no free command socket.
fn no_socket() -> Vec<u8> {
    frames::no_socket(0)
}

/// The zoom position `0x1234` as inquiry data.
fn zoom_position_reply() -> Vec<u8> {
    frames::inquiry_reply(&[0x01, 0x02, 0x03, 0x04])
}

facade_matrix! {
    /// Issue #566: `0x41` is retried for a movement-class request.
    fn a_refused_movement_command_is_replayed_and_then_succeeds() {
        let camera = scripted(vec![vec![not_executable()], standard_reply()], standard_reply());
        let session = open!(camera, session_config()).expect("owner session");
        let view = session
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera view");

        wait!(view.execute(&MovementCommand))
            .expect("a refused movement command must be replayed, not failed");

        assert_eq!(
            camera.write_count(),
            2,
            "the refusal is transient for a movement request, so the frame is reissued"
        );
        session.shutdown().expect("owner shutdown");
    }

    /// A typed STOP refused with `0x41` reports the camera's conclusive
    /// refusal at once: resending it cannot change the standing condition (a
    /// focus STOP under auto-focus) and would only delay a halt's verdict.
    fn a_refused_typed_stop_surfaces_the_refusal_without_replay() {
        let camera = scripted(vec![vec![not_executable()], standard_reply()], standard_reply());
        let session = open!(camera, session_config()).expect("owner session");
        let view = session
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera view");

        let error = wait!(wait!(view.submit::<AppliedOnly, _>(&ZoomStop))
            .expect("submission")
            .applied_with_timeout(Duration::from_secs(5)))
        .expect_err("a refused STOP surfaces the camera's refusal");
        assert!(
            matches!(error, Error::CommandNotExecutable),
            "expected the camera's own refusal, got {error:?}"
        );
        assert_eq!(camera.write_count(), 1, "a refused STOP is not rewritten");
        session.shutdown().expect("owner shutdown");
    }

    /// Issue #566: the same `0x41` is terminal for a standard-class request,
    /// which is the whole point of the `movement_not_executable` distinction.
    fn a_refused_standard_command_surfaces_the_refusal_without_replay() {
        let camera = scripted(vec![vec![not_executable()]], standard_reply());
        let session = open!(camera, session_config()).expect("owner session");
        let view = session
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera view");

        let error = wait!(view.execute(&StandardCommand))
            .expect_err("a standard request must surface the camera's refusal");
        assert!(
            matches!(error, Error::CommandNotExecutable),
            "expected the camera's own refusal, got {error:?}"
        );
        assert_eq!(
            camera.write_count(),
            1,
            "a standard request must not replay a refusal"
        );

        // The session is unharmed by a refused command.
        wait!(view.execute(&StandardCommand)).expect("the session survives a refused command");
        session.shutdown().expect("owner shutdown");
    }

    /// Issue #566: `0x05` (`NoSocket`) is a capacity answer, so it is replayed
    /// for a standard request too — unlike `0x41`.
    fn a_no_socket_answer_is_replayed_for_a_standard_command() {
        let camera = scripted(vec![vec![no_socket()], standard_reply()], standard_reply());
        let session = open!(camera, session_config()).expect("owner session");
        let view = session
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera view");

        wait!(view.execute(&StandardCommand))
            .expect("a camera with no free socket must be retried, not failed");
        assert_eq!(
            camera.write_count(),
            2,
            "a capacity answer is replayed regardless of retry class"
        );
        session.shutdown().expect("owner shutdown");
    }

    /// Issue #566: generated noun accessors carry the closed built-in inquiry
    /// provenance through generic public lowering. A raw/custom inquiry with
    /// the exact same bytes remains terminal on `0x02`, proving this is
    /// provenance rather than wire-shape policy.
    fn a_builtin_inquiry_syntax_error_is_replayed_but_custom_syntax_is_terminal() {
        let camera = scripted(
            vec![vec![frames::syntax_error()], vec![zoom_position_reply()]],
            Vec::new(),
        );
        let session = open!(camera, session_config()).expect("owner session");
        let view = session
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera view");

        let position = wait!(view.zoom().position())
            .expect("a generated inquiry must retry transient 0x02");
        assert_eq!(position, ZoomPosition::new(0x1234).expect("zoom position"));
        assert_eq!(
            camera.write_count(),
            2,
            "the built-in inquiry is reissued once"
        );
        session.shutdown().expect("owner shutdown");

        let camera = scripted(
            vec![vec![frames::syntax_error()], vec![zoom_position_reply()]],
            Vec::new(),
        );
        let session = open!(camera, session_config()).expect("owner session");
        let view = session
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera view");

        let custom = custom_zoom_inquiry();
        let error = wait!(view.inquire(&custom))
            .expect_err("a custom inquiry must surface the camera's syntax verdict");
        assert!(matches!(error, Error::SyntaxError));
        assert_eq!(
            camera.write_count(),
            1,
            "a custom inquiry must not reach the scripted resend"
        );
        session.shutdown().expect("owner shutdown");
    }

    /// Issue #566: a sequence-correlated Sony movement request survives a lost
    /// ACK (same-sequence retransmission, docs/visca_reference.md §5.3). Raw
    /// VISCA has no request identity after a successful write, so replay there
    /// would be unsafe; a raw command instead fails per-request and quarantines
    /// its slot (issue #671), covered by the engine and #565 tests.
    fn a_sony_movement_command_survives_a_lost_ack() {
        // The first write draws no answer at all; the ACK deadline lapses and
        // the frame is reissued.
        let camera = scripted(vec![Vec::new(), standard_reply()], standard_reply());
        let session = open!(camera, sony_session_config()).expect("owner session");
        let view = session.camera::<SonyFR7>().expect("camera view");

        wait!(wait!(view.submit::<AppliedOnly, _>(&ZoomStop))
            .expect("submission")
            .applied_with_timeout(Duration::from_secs(5)))
        .expect("a lost ACK must be retried for a movement request");
        let writes = camera.writes();
        assert_eq!(writes.len(), 2, "the lost ACK reissues the frame");
        assert_eq!(
            frames::sony_sequence(&writes[0]).expect("Sony write must carry its sequence header"),
            frames::sony_sequence(&writes[1]).expect("Sony write must carry its sequence header"),
            "a Sony retry must preserve the logical request sequence"
        );
        session.shutdown().expect("owner shutdown");
    }

    /// Issue #795: the ACK proved the camera accepted the command, and Sony
    /// same-sequence retransmission (docs/visca_reference.md §5.3) recovers a
    /// lost message, not an accepted one. A camera that ACKs and then goes
    /// silent fails the command unconfirmed with no second write.
    fn a_silent_camera_after_its_ack_is_never_rewritten() {
        let camera = scripted(vec![vec![frames::ack(1)], standard_reply()], standard_reply());
        let session = open!(camera, sony_session_config()).expect("owner session");
        let view = session.camera::<SonyFR7>().expect("camera view");

        let error = wait!(wait!(view.submit::<AppliedOnly, _>(&ZoomStop))
            .expect("submission")
            .applied_with_timeout(Duration::from_secs(30)))
        .expect_err("a post-ACK completion timeout is terminal");
        assert_eq!(
            error.failure_context(),
            Some(grafton_visca::FailureContext::new(
                grafton_visca::FailureStage::Terminal,
                grafton_visca::Certainty::Unconfirmed
            )),
            "{error:?}"
        );
        assert_eq!(
            camera.write_count(),
            1,
            "an acknowledged command is never rewritten"
        );
        session.shutdown().expect("owner shutdown");
    }

    /// Issue #566: `Error::CancellationUnconfirmed` reaches the caller through
    /// a sequence-correlated Sony owner. A raw command with the same ambiguity
    /// fails per-request as `UnsequencedCommandUnconfirmed` (issue #671) while
    /// its session keeps running.
    fn an_unresolvable_cancellation_reaches_the_caller() {
        // The camera never answers anything: neither the command nor the cancel.
        let camera = FakeCamera::silent();
        let session = open!(camera, sony_session_config()).expect("owner session");
        let view = session.camera::<SonyFR7>().expect("camera view");

        let mut operation =
            wait!(view.submit::<AppliedOnly, _>(&ZoomStop)).expect("submission");
        let error = wait!(operation.cancel_with_timeout(Duration::from_secs(5)))
            .expect_err("a camera that never answers cannot confirm a cancellation");
        assert!(
            matches!(error, Error::CancellationUnconfirmed),
            "expected the ambiguity verdict, got {error:?}"
        );
        assert!(
            !camera.writes().is_empty(),
            "the original frame was transmitted"
        );
        session.shutdown().expect("owner shutdown");
    }
}
