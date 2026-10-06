//! Issue #629: a blocking pump that ends the session must report the session's
//! boundary error, never the raw transport cause.
//!
//! The escape sites were the observation paths that pump without a receipt to
//! consult — settlement polling above all. A raw `Error::Io` classifies as
//! survivable (`requires_new_session() == false`), so an auto-reconnect loop
//! keyed on that predicate kept using a session the owner had already closed.

#![cfg(feature = "blocking")]

#[path = "common/fake_camera.rs"]
mod fake_camera;
#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{collections::VecDeque, sync::Arc, time::Duration};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    completion::{AppliedOnly, Targeted},
    profile::ProfileSpec,
    request::builtin::{ZoomStop, ZoomTarget},
    transport::{AddressingMode, SendSemantics, TransportConfig},
    types::ZoomPosition,
    CameraId, Certainty, Error, FailureStage, OperationalTuning,
};

use fake_camera::{frames, BlockingWire, FakeCamera, Read};
use profile_fixtures::DirectZoomOnlyTypedSupport;

/// `90 50 0p 0q 0r 0s FF`: the zoom position the settlement baseline reads.
const ZOOM_POSITION_REPLY: &[u8] = &[0x90, 0x50, 0x00, 0x01, 0x00, 0x00, 0xff];

/// A stream camera whose reads are scripted per write: `script[n]` is what it
/// queues for reading after the n-th write. A write past the script queues
/// nothing. `fail_on_send` fails that numbered write (counting from one) once;
/// the failed write queues no reads and does not consume a script entry.
fn probe(script: Vec<Vec<Read>>, fail_on_send: Option<(usize, Error)>) -> FakeCamera {
    let mut script: VecDeque<Vec<Read>> = script.into();
    let mut fail_on_send = fail_on_send;
    let mut sends = 0usize;
    FakeCamera::new(move |_, answer| {
        sends += 1;
        if fail_on_send
            .as_ref()
            .is_some_and(|(send, _)| *send == sends)
        {
            answer.fail_send(
                fail_on_send
                    .take()
                    .expect("matching configured send failure")
                    .1,
            );
            return;
        }
        for read in script.pop_front().unwrap_or_default() {
            match read {
                Ok(frame) => answer.reply(frame),
                Err(error) => answer.fault(error),
            };
        }
    })
}

/// The camera's stream wire, reporting the default addressing mode.
fn stream_wire(camera: &FakeCamera) -> BlockingWire {
    camera
        .blocking_wire()
        .with_semantics(SendSemantics::Stream)
        .with_addressing(AddressingMode::default())
}

/// The camera's stream wire on a serial bus.
fn serial_stream_wire(camera: &FakeCamera) -> BlockingWire {
    let mut config = TransportConfig::default();
    config.addressing = AddressingMode::Serial;
    camera
        .blocking_wire()
        .with_semantics(SendSemantics::Stream)
        .with_config(config)
        .with_addressing(AddressingMode::Serial)
}

/// The review's probe: the operation applies, the settlement baseline is read,
/// and the connection then dies inside the interval pump between two position
/// samples. The caller must be told the session is gone.
#[test]
fn connection_reset_during_settlement_polling_classifies_as_session_death() {
    let fake = probe(
        vec![
            // The zoom target is acknowledged and applied.
            vec![Ok(frames::ack(1)), Ok(frames::complete(1))],
            // The settlement baseline inquiry is answered, and the peer then
            // resets the connection before the next sample can be taken.
            vec![
                Ok(ZOOM_POSITION_REPLY.to_vec()),
                Err(Error::Io(Arc::new(std::io::Error::from(
                    std::io::ErrorKind::ConnectionReset,
                )))),
            ],
        ],
        None,
    );
    let config = SessionConfig::new(
        ProfileSpec::from_compile_time::<DirectZoomOnlyTypedSupport>()
            .expect("settlement-polling runtime profile"),
    );
    let session = Session::open(stream_wire(&fake), config).expect("owner session");
    let camera = session
        .camera::<DirectZoomOnlyTypedSupport>()
        .expect("camera view");

    let error = camera
        .submit::<Targeted, _>(&ZoomTarget::new(
            ZoomPosition::new(0x0100).expect("zoom position"),
        ))
        .expect("submission")
        .settled()
        .expect_err("a reset connection cannot settle the operation");

    let Error::SettlementObservationFailed { source, .. } = &error else {
        panic!("settlement must preserve its inquiry cause, got {error:?}");
    };
    let Error::ConnectionClosed { reason, .. } = source.as_ref() else {
        panic!("settlement must report the session close, got {error:?}");
    };
    assert!(!error.is_retryable());
    let context = error
        .failure_context()
        .expect("applied move observation context");
    assert_eq!(context.stage, FailureStage::Observation);
    assert_eq!(context.certainty, Certainty::Unconfirmed);
    let reason = reason.as_ref().expect("the read fault names the close");
    assert!(
        reason.contains("connection reset"),
        "the transport cause must survive in the close reason: {reason}"
    );
    assert!(
        error.requires_new_session(),
        "a settlement wait interrupted by a dead transport must ask for a new session"
    );
    assert_eq!(
        fake.write_count(),
        2,
        "one operation write and one settlement baseline inquiry"
    );
}

/// A targeted operation can have already applied when another target retries.
/// Its retry write is still a stream boundary: the public settlement wait must
/// surface `StreamPoisoned`, not consume the remainder of its own timeout.
#[test]
fn stream_retry_write_failure_during_settlement_is_not_reported_as_timeout() {
    let profile = ProfileSpec::from_compile_time::<DirectZoomOnlyTypedSupport>()
        .expect("settlement-polling runtime profile");
    let config = SessionConfig::new(profile.clone())
        .with_target(CameraId::CAMERA_2, profile)
        .expect("second serial target")
        .with_tuning(OperationalTuning::new().retry_timing(
            Duration::from_millis(50),
            Duration::from_millis(500),
            Duration::from_secs(10),
        ));
    let fake = probe(
        vec![
            vec![Ok(frames::ack(1)), Ok(frames::complete(1))],
            vec![Ok(vec![0xa0, 0x60, 0x03, 0xff])],
            vec![Ok(ZOOM_POSITION_REPLY.to_vec())],
        ],
        Some((
            4,
            Error::TransportError("public settlement retry write failed".into()),
        )),
    );
    let session = Session::open(serial_stream_wire(&fake), config).expect("owner session");
    let first_camera = session
        .camera_for::<DirectZoomOnlyTypedSupport>(CameraId::CAMERA_1)
        .expect("camera one view");
    let second_camera = session
        .camera_for::<DirectZoomOnlyTypedSupport>(CameraId::CAMERA_2)
        .expect("camera two view");

    let mut current = first_camera
        .submit::<Targeted, _>(&ZoomTarget::new(
            ZoomPosition::new(0x0100).expect("zoom position"),
        ))
        .expect("targeted operation write");
    let mut peer = second_camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("peer operation write");

    let error = current
        .settled()
        .expect_err("a stream retry write must end the settlement");
    let Error::SettlementObservationFailed { source, .. } = &error else {
        panic!("settlement must preserve its inquiry cause, got {error:?}");
    };
    let Error::StreamPoisoned { reason, .. } = source.as_ref() else {
        panic!("settlement must report stream poison, got {error:?}");
    };
    assert!(reason.contains("public settlement retry write failed"));
    assert!(error.requires_new_session());
    assert!(!error.is_retryable());
    let context = error
        .failure_context()
        .expect("applied move observation context");
    assert_eq!(context.stage, FailureStage::Observation);
    assert_eq!(context.certainty, Certainty::Unconfirmed);
    assert!(matches!(peer.applied(), Err(Error::StreamPoisoned { .. })));
}
