//! Issue #719: silence is distinct from session death and valid replies expose
//! a positive, facade-level liveness signal.

#![cfg(feature = "blocking")]

use grafton_visca_test_support::fake_camera;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use grafton_visca::{
    blocking::Session, camera::profiles::PtzOpticsG2, profile::ProfileSpec,
    transport::SendSemantics, Error, OperationalTuning, SessionConfig,
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
        answer.fault(Error::Io(Arc::new(std::io::Error::from(
            std::io::ErrorKind::TimedOut,
        ))));
    })
}

fn config() -> SessionConfig {
    SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("built-in profile"))
        .with_tuning(OperationalTuning::new().retry_limit(0))
}

#[test]
fn response_counter_distinguishes_silence_from_positive_liveness() {
    let silent = Session::open(stream_wire(&FakeCamera::silent()), config()).expect("session");
    let before = silent.metrics().expect("metrics before probe");
    let started = Instant::now();
    let error = silent
        .camera::<PtzOpticsG2>()
        .expect("camera")
        .power()
        .state()
        .expect_err("silent peer must exhaust the bounded probe");
    assert!(matches!(error, Error::Timeout { .. }));
    assert!(error.is_retryable());
    assert!(!error.requires_new_session());
    // The probe is bounded by the profile's own retry budget: ten seconds
    // for a 1 s inquiry deadline. Tuning may lengthen that bound but never
    // shorten it (#828 M3), so a faster liveness verdict is a caller-side
    // deadline, not a tuning value.
    assert!(started.elapsed() < Duration::from_secs(10));
    let after = silent.metrics().expect("metrics after probe");
    assert_eq!(before.received_frames, 0);
    assert_eq!(after.received_frames, before.received_frames);
    silent
        .close()
        .expect("a silent session is still locally live");

    let answering = Session::open(stream_wire(&answering_camera()), config()).expect("session");
    let before = answering.metrics().expect("metrics before probe");
    assert!(answering
        .camera::<PtzOpticsG2>()
        .expect("camera")
        .power()
        .state()
        .expect("power reply"));
    let after = answering.metrics().expect("metrics after probe");
    assert_eq!(after.received_frames, before.received_frames + 1);
    answering.close().expect("close answering session");
}

#[test]
fn os_keepalive_timeout_is_session_death_not_idle_no_data() {
    let session =
        Session::open(stream_wire(&keepalive_expired_camera()), config()).expect("session");
    let error = session
        .camera::<PtzOpticsG2>()
        .expect("camera")
        .power()
        .state()
        .expect_err("OS timeout must terminate the connection");
    assert!(matches!(error, Error::ConnectionClosed { .. }));
    assert!(error.requires_new_session());
    assert!(matches!(
        session.close(),
        Err(Error::ConnectionClosed { .. })
    ));
}

#[test]
fn os_keepalive_timeout_on_an_idle_connection_is_session_death() {
    let camera = FakeCamera::silent();
    let session = Session::open(stream_wire(&camera), config()).expect("session");
    camera.push_fault(Error::Io(Arc::new(std::io::Error::from(
        std::io::ErrorKind::TimedOut,
    ))));
    camera.wait_for_reads(1);
    let error = session
        .camera::<PtzOpticsG2>()
        .expect("camera")
        .power()
        .state()
        .expect_err("an idle keepalive expiry must terminate the connection");
    assert!(matches!(error, Error::ConnectionClosed { .. }));
    assert!(error.requires_new_session());
    assert!(matches!(
        session.close(),
        Err(Error::ConnectionClosed { .. })
    ));
}
