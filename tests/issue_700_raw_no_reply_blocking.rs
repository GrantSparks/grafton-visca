//! A no-reply raw plain command succeeds exactly when its blocking transport
//! write succeeds. It deliberately does not create an operation handle, wait
//! for a reply, or claim protocol application.

#![cfg(feature = "blocking")]

#[path = "common/fake_camera.rs"]
mod fake_camera;

use fake_camera::FakeCamera;
use grafton_visca::{
    blocking::{Session, SessionConfig},
    raw::{self, RawReplyShape},
    ControlClass, DiagnosticEvent, DiagnosticOutcome, ProfileSpec, RetryClass, TimeoutClass,
};

#[test]
fn no_reply_plain_command_returns_after_one_write() {
    let fake = FakeCamera::silent();
    let profile = ProfileSpec::from_compile_time::<grafton_visca::profiles::PtzOpticsG2>()
        .expect("runtime profile");
    let session =
        Session::open(fake.blocking_wire(), SessionConfig::new(profile)).expect("blocking session");
    let camera = session
        .camera::<grafton_visca::profiles::PtzOpticsG2>()
        .expect("camera view");
    let diagnostics = session.subscribe_diagnostics(16).expect("diagnostics");

    let wire = [0x81, 0x01, 0x04, 0x07, 0xff];
    let policy = raw::Policy::new(TimeoutClass::Quick, RetryClass::Never, ControlClass::Normal)
        .expect("raw policy")
        .with_reply_shape(RawReplyShape::NoReply);
    let command = raw::Plain::with_policy(wire, policy).expect("no-reply raw command");

    camera
        .execute(&command)
        .expect("a successful write returns plain fire-and-forget success");

    assert_eq!(
        fake.writes(),
        vec![wire.to_vec()],
        "the no-reply command is locally written exactly once"
    );
    // The diagnostic stream is not ordered against the command's return;
    // once `close` returns, every event the owner published is queued.
    drop(camera);
    session.close().expect("owner close");
    assert!(
        std::iter::from_fn(|| diagnostics.try_recv()).any(|event| matches!(
            event,
            DiagnosticEvent::Terminal {
                outcome: DiagnosticOutcome::Written,
                ..
            }
        )),
        "the public diagnostic reports a local write, never Applied"
    );
}
