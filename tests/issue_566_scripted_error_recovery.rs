//! Issue #566: facade-level error recovery driven by the shipped scripted
//! transport.
//!
//! `src/testing/testkit/scripted_transport.rs` ships a public error-recovery
//! script vocabulary — a camera answering BUFFER FULL, NOT EXECUTABLE, NO
//! SOCKET or a transient inquiry SYNTAX ERROR and then succeeding — that no
//! test in this repository had ever run. Every one of those helpers was dead
//! code sitting in the published `test-utils` API. This file drives them
//! through the real blocking facade, which is what they were built for.
//!
//! `test-utils` is not part of the default feature set and enables no facade
//! on its own, so this file needs a leg that unions it with one: the
//! `test-utils + blocking + Tokio` entry in `.github/workflows/ci.yml` and its
//! twin in `.github/scripts/test-all-features.sh` (#628). The bare
//! `test-utils` leg compiles this file away, and the all-features job is a
//! `cargo check`, so neither one runs it. The retry behavior itself is
//! additionally pinned without `test-utils` by
//! `tests/issue_566_retry_recovery_blocking.rs` and its async twin, so no
//! behavior depends on this file alone.

#![cfg(all(feature = "test-utils", feature = "blocking"))]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::time::Duration;

use grafton_visca::{
    blocking::{Session, SessionConfig},
    command::ZoomPositionInquiry,
    completion::AppliedOnly,
    profile::ProfileSpec,
    request::{self, builtin::ZoomStop},
    testing::testkit::{
        helpers::{self, errors},
        ScriptedBlockingTransport, Step,
    },
    types::ZoomPosition,
    CameraId, ControlClass, Error, Request, RetryClass, TimeoutClass,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

/// The exact bytes a zoom-position inquiry puts on the wire for camera 1.
const ZOOM_POSITION_INQUIRY: [u8; 5] = [0x81, 0x09, 0x04, 0x47, 0xff];

/// A plain command in the standard retry class, so the class under test is
/// stated rather than inherited from a built-in.
#[derive(Debug)]
struct StandardCommand;

impl Request for StandardCommand {
    type Class = request::Plain;
    const MAX_SIZE: usize = 3;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
    const RETRY_CLASS: RetryClass = RetryClass::Standard;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn write_into(&self, target: CameraId, out: &mut [u8]) -> Result<usize, Error> {
        out[..3].copy_from_slice(&[target.to_address_byte(), 0x01, 0xff]);
        Ok(3)
    }
}

fn session_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("two-socket runtime profile"),
    )
}

fn open(steps: Vec<Step>) -> (Session, ScriptedBlockingTransport) {
    let transport = ScriptedBlockingTransport::new(steps);
    let probe = transport.clone();
    let session = Session::open(transport, session_config()).expect("owner session");
    (session, probe)
}

/// `helpers::buffer_full_then_success`: a busy camera is replayed once.
#[test]
fn a_busy_camera_is_replayed_once_and_then_succeeds() {
    let (session, probe) = open(helpers::buffer_full_then_success(1));
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .execute(&StandardCommand)
        .expect("a full command buffer must be replayed, not failed");
    assert_eq!(probe.sent().len(), 2, "the frame is reissued once");
    session.shutdown().expect("owner shutdown");
}

/// `helpers::buffer_full_sequence_then_success`: several consecutive BUFFER
/// FULL answers stay inside the standard budget and still succeed.
#[test]
fn a_repeatedly_busy_camera_stays_inside_its_retry_budget() {
    let (session, probe) = open(helpers::buffer_full_sequence_then_success(1, 3));
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .execute(&StandardCommand)
        .expect("three busy answers stay inside the quick budget of five");
    assert_eq!(probe.sent().len(), 4, "three replays and the winning frame");
    session.shutdown().expect("owner shutdown");
}

/// `helpers::not_executable_then_success`: `0x41` is transient for a movement
/// request, which is exactly the `movement_not_executable` distinction.
#[test]
fn a_refused_movement_command_is_replayed_once_and_then_succeeds() {
    let (session, probe) = open(helpers::not_executable_then_success(0));
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission")
        .applied_with_timeout(Duration::from_secs(5))
        .expect("a refused movement command must be replayed");
    assert_eq!(probe.sent().len(), 2);
    session.shutdown().expect("owner shutdown");
}

/// `helpers::not_executable_sequence_then_success`: the movement budget is
/// three, so three refusals still resolve.
#[test]
fn a_repeatedly_refused_movement_command_stays_inside_its_budget() {
    let (session, probe) = open(helpers::not_executable_sequence_then_success(0, 3));
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission")
        .applied_with_timeout(Duration::from_secs(5))
        .expect("three refusals stay inside the movement budget of three");
    assert_eq!(probe.sent().len(), 4);
    session.shutdown().expect("owner shutdown");
}

/// The same `0x41` is terminal for a standard-class request. The helper builds
/// a script that would have succeeded on replay; the point is that no replay
/// happens, so the second step is never consumed.
#[test]
fn a_refused_standard_command_is_terminal() {
    let (session, probe) = open(helpers::not_executable_sequence_then_success(0, 1));
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let error = camera
        .execute(&StandardCommand)
        .expect_err("a standard request must surface the camera's refusal");
    assert!(
        matches!(error, Error::CommandNotExecutable),
        "expected the camera's own refusal, got {error:?}"
    );
    assert_eq!(probe.sent().len(), 1, "no replay was attempted");
    session.shutdown().expect("owner shutdown");
}

/// `errors::no_socket` and `helpers::auto_respond_step`: `0x05` is a capacity
/// answer and is replayed even for a standard request.
#[test]
fn a_camera_with_no_free_socket_is_replayed() {
    let (session, probe) = open(vec![errors::no_socket(), helpers::auto_respond_step()]);
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .execute(&StandardCommand)
        .expect("a camera with no free socket must be replayed, not failed");
    assert_eq!(probe.sent().len(), 2);
    session.shutdown().expect("owner shutdown");
}

/// `helpers::syntax_error_then_inquiry_success` scripts the #536 recovery: a
/// camera answers an inquiry with a transient `0x02` and then answers it
/// properly on the resend.
///
/// **That recovery is deliberately not reachable from the public facade**, and
/// this test is what says so. `builtin_inquiry_syntax` is set only by
/// `prepare_builtin_inquiry`, which the crate uses for its own settlement
/// polling; `Camera::inquire` lowers through `prepare_inquiry`, where a caller
/// may have supplied a syntax the camera genuinely rejects, and replaying that
/// would be a guess. So the caller sees `0x02` and the scripted resend is never
/// reached. The unit test that pins the policy itself is
/// `prepared::tests::inquiry_syntax_retry_is_reserved_for_builtin_inquiries`.
#[test]
fn a_facade_inquiry_syntax_error_is_reported_rather_than_replayed() {
    let (session, probe) = open(helpers::syntax_error_then_inquiry_success(
        ZOOM_POSITION_INQUIRY.to_vec(),
        vec![0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xff],
    ));
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let error = camera
        .inquire(&ZoomPositionInquiry)
        .expect_err("a caller-supplied inquiry syntax is not replayed on 0x02");
    assert!(
        matches!(error, Error::SyntaxError),
        "expected the camera's own rejection, got {error:?}"
    );
    assert_eq!(
        probe.sent().len(),
        1,
        "the scripted resend must never be reached"
    );
    session.shutdown().expect("owner shutdown");
}

/// `helpers::inquiry_response`: the plain, non-failing inquiry script, so the
/// replay above is measured against a known-good baseline of one write.
#[test]
fn a_healthy_inquiry_is_answered_on_the_first_write() {
    let (session, probe) = open(vec![helpers::inquiry_response(
        ZOOM_POSITION_INQUIRY.to_vec(),
        1,
        vec![0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xff],
    )]);
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let position = camera.inquire(&ZoomPositionInquiry).expect("inquiry reply");
    assert_eq!(position, ZoomPosition::new(0x1234).expect("zoom position"));
    assert_eq!(probe.sent().len(), 1);
    session.shutdown().expect("owner shutdown");
}
