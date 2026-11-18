//! Issue #719: silence is distinct from session death and valid replies expose
//! a positive, facade-level liveness signal.

#![cfg(feature = "blocking")]

use grafton_visca::tuning::RetryLimit;
use grafton_visca_test_support::fake_camera;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use grafton_visca::{
    blocking::Session,
    config::{OperationalTuning, SessionConfig},
    error::{Certainty, FailureStage},
    profiles::{ProfileSpec, PtzOpticsG2},
    transport::SendSemantics,
    Error,
};

use fake_camera::{frames, BlockingWire, FakeCamera};

/// A stream wire onto `camera`.
fn stream_wire(camera: &FakeCamera) -> BlockingWire {
    camera.blocking_wire().with_semantics(SendSemantics::Stream)
}

/// A peer that answers every power inquiry with "on".
fn answering_camera() -> FakeCamera {
    FakeCamera::new(|_, answer| {
        answer.reply(frames::inquiry_reply(&[0x02]));
    })
}

/// A peer whose connection fails its next read with the OS keepalive timeout,
/// as a TCP socket does when the keepalive probes go unanswered.
fn keepalive_expired_camera() -> FakeCamera {
    FakeCamera::new(|_, answer| {
        answer.fault(Error::Io {
            source: Arc::new(std::io::Error::from(std::io::ErrorKind::TimedOut)),
        });
    })
}

fn config() -> SessionConfig {
    SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("built-in profile"))
        .with_tuning(OperationalTuning::new().with_retry_limit(RetryLimit::MIN))
}

/// An admitted inquiry's conclusive failure keeps the precise liveness cause.
fn inquiry_cause(error: &Error) -> &Error {
    let Error::InquiryFailed { source, .. } = error else {
        panic!("an admitted inquiry must report its terminal failure, got {error:?}");
    };
    let context = error.failure_context().expect("terminal inquiry context");
    assert_eq!(context.stage, FailureStage::Terminal);
    assert_eq!(context.certainty, Certainty::FailedConclusively);
    assert_eq!(
        std::error::Error::source(error)
            .expect("the inquiry exposes its exact cause in the source chain")
            .to_string(),
        source.to_string(),
    );
    source.as_ref()
}

#[test]
fn response_counter_distinguishes_silence_from_positive_liveness() {
    let silent = Session::open(stream_wire(&FakeCamera::silent()), &config()).expect("session");
    let before = silent.metrics().expect("metrics before probe");
    let started = Instant::now();
    let error = silent
        .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
        .expect("camera")
        .power()
        .status()
        .expect_err("silent peer must exhaust the bounded probe");
    let cause = inquiry_cause(&error);
    assert!(matches!(cause, Error::Timeout { .. }));
    assert_eq!(cause.failure_context(), error.failure_context());
    assert!(!cause.requires_new_session());
    assert!(matches!(
        error.recovery(),
        grafton_visca::error::Recovery::Resubmit { .. }
    ));
    assert!(!error.requires_new_session());
    // The raw stream writes one inquiry and waits for its reply deadline.
    assert!(started.elapsed() < Duration::from_secs(10));
    let after = silent.metrics().expect("metrics after probe");
    assert_eq!(before.received_frames, 0);
    assert_eq!(after.received_frames, before.received_frames);
    silent
        .close()
        .expect("a silent session is still locally live");

    let answering = Session::open(stream_wire(&answering_camera()), &config()).expect("session");
    let before = answering.metrics().expect("metrics before probe");
    assert!(answering
        .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
        .expect("camera")
        .power()
        .status()
        .expect("power reply")
        .is_on());
    let after = answering.metrics().expect("metrics after probe");
    assert_eq!(after.received_frames, before.received_frames + 1);
    answering.close().expect("close answering session");
}

#[test]
fn os_keepalive_timeout_is_session_death_not_idle_no_data() {
    let session =
        Session::open(stream_wire(&keepalive_expired_camera()), &config()).expect("session");
    let error = session
        .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
        .expect("camera")
        .power()
        .status()
        .expect_err("OS timeout must terminate the connection");
    let cause = inquiry_cause(&error);
    let Error::ConnectionClosed { source, .. } = cause else {
        panic!("the OS keepalive cause must remain a session close, got {error:?}");
    };
    assert!(matches!(source.as_deref(), Some(Error::Io { source: io })
        if io.kind() == std::io::ErrorKind::TimedOut));
    assert!(cause.requires_new_session());
    assert!(error.requires_new_session());
    assert_eq!(
        error.recovery(),
        grafton_visca::error::Recovery::ReopenSession
    );
    assert!(matches!(
        session.close(),
        Err(Error::ConnectionClosed { .. })
    ));
}

#[test]
fn os_keepalive_timeout_on_an_idle_connection_is_session_death() {
    let camera = FakeCamera::silent();
    let session = Session::open(stream_wire(&camera), &config()).expect("session");
    camera.push_fault(Error::Io {
        source: Arc::new(std::io::Error::from(std::io::ErrorKind::TimedOut)),
    });
    camera.wait_for_reads(1);
    let error = session
        .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
        .expect("camera")
        .power()
        .status()
        .expect_err("an idle keepalive expiry must terminate the connection");
    assert!(matches!(error, Error::ConnectionClosed { .. }));
    assert!(error.requires_new_session());
    assert!(matches!(
        session.close(),
        Err(Error::ConnectionClosed { .. })
    ));
}
