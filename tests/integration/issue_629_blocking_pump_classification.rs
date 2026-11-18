//! Issue #629: a blocking pump that ends the session must report the session's
//! boundary error, never the raw transport cause.
//!
//! The escape sites were the observation paths that pump without a receipt to
//! consult — settlement polling above all. A raw `Error::Io` classifies as
//! survivable (`requires_new_session() == false`), so an auto-reconnect loop
//! keyed on that predicate kept using a session the owner had already closed.

#![cfg(feature = "blocking")]

use grafton_visca_test_support::{fake_camera, profile_fixtures};

use std::{collections::VecDeque, sync::Arc, time::Duration};

use grafton_visca::{
    blocking::Session,
    config::{OperationalTuning, SessionConfig},
    error::{Certainty, FailureStage},
    operation::{AppliedOnly, Targeted},
    profiles::ProfileSpec,
    transport::{AddressingMode, IoBounds, SendSemantics},
    types::ZoomPosition,
    Error,
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
    let config = IoBounds::default();

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
                Err(Error::Io {
                    source: Arc::new(std::io::Error::from(std::io::ErrorKind::ConnectionReset)),
                }),
            ],
        ],
        None,
    );
    let config = SessionConfig::new(
        ProfileSpec::from_compile_time::<DirectZoomOnlyTypedSupport>()
            .expect("settlement-polling runtime profile"),
    );
    let session = Session::open(stream_wire(&fake), &config).expect("owner session");
    let camera = session
        .camera::<DirectZoomOnlyTypedSupport>(grafton_visca::types::IndividualCameraId::CAMERA_1)
        .expect("camera view");

    let error = camera
        .submit::<Targeted, _>(&grafton_visca::command::zoom::SetPosition::new(
            ZoomPosition::new(0x0100).expect("zoom position"),
        ))
        .expect("submission")
        .settled()
        .expect_err("a reset connection cannot settle the operation");

    let Error::SettlementObservationFailed { source, .. } = &error else {
        panic!("settlement must preserve its inquiry cause, got {error:?}");
    };
    let Error::ConnectionClosed { source, .. } = source.as_ref() else {
        panic!("settlement must report the session close, got {error:?}");
    };
    assert!(!matches!(
        error.recovery(),
        grafton_visca::error::Recovery::Resubmit { .. }
    ));
    let context = error
        .failure_context()
        .expect("applied move observation context");
    assert_eq!(context.stage, FailureStage::Observation);
    assert_eq!(context.certainty, Certainty::Unconfirmed);
    let cause = source.as_ref().expect("the read fault preserves its cause");
    assert!(matches!(cause.as_ref(), Error::Io { source: io }
        if io.kind() == std::io::ErrorKind::ConnectionReset));
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

/// Whether polling had an admitted engine inquiry when the stream failed.
#[derive(Clone, Copy)]
enum PollAdmission {
    Rejected,
    Admitted,
}

/// Both admission boundaries preserve the exact poison and OS cause. Only an
/// admitted inquiry has the additional conclusively-failed inquiry context.
fn assert_settlement_stream_failure(
    error: &Error,
    admission: PollAdmission,
    expected_io: &Arc<std::io::Error>,
) {
    let Error::SettlementObservationFailed { source, .. } = error else {
        panic!("settlement must preserve its polling failure, got {error:?}");
    };
    let mut public_cause =
        std::error::Error::source(error).expect("settlement exposes its exact polling failure");
    assert_eq!(public_cause.to_string(), source.to_string());
    let poison = match admission {
        PollAdmission::Rejected => source.as_ref(),
        PollAdmission::Admitted => {
            let inquiry = source.as_ref();
            let Error::InquiryFailed { source, .. } = inquiry else {
                panic!(
                    "an admitted polling inquiry must retain its terminal context, got {error:?}"
                );
            };
            let context = inquiry.failure_context().expect("terminal inquiry context");
            assert_eq!(context.stage, FailureStage::Terminal);
            assert_eq!(context.certainty, Certainty::FailedConclusively);
            assert!(inquiry.requires_new_session());
            public_cause = public_cause
                .source()
                .expect("the inquiry exposes its terminal stream cause");
            source.as_ref()
        }
    };
    let Error::StreamPoisoned { source, .. } = poison else {
        panic!("settlement must report stream poison, got {error:?}");
    };
    assert_eq!(public_cause.to_string(), poison.to_string());
    let Error::TransportError {
        source: io_error, ..
    } = source.as_ref()
    else {
        panic!("poison must preserve its typed transport cause, got {error:?}");
    };
    let Error::Io { source: actual_io } = io_error.as_ref() else {
        panic!("transport failure must preserve its OS cause, got {error:?}");
    };
    assert!(Arc::ptr_eq(actual_io, expected_io));
    assert_eq!(actual_io.kind(), std::io::ErrorKind::Other);
    assert_eq!(actual_io.to_string(), "public settlement write failed");

    let public_transport = public_cause
        .source()
        .expect("poison exposes its typed write cause");
    assert!(public_transport.downcast_ref::<Arc<Error>>().is_some());
    let public_io = public_transport
        .source()
        .expect("transport exposes I/O cause")
        .source()
        .expect("I/O wrapper exposes the original OS error")
        .downcast_ref::<Arc<std::io::Error>>()
        .expect("the source chain retains the concrete OS cause");
    assert!(Arc::ptr_eq(public_io, expected_io));
    assert!(error.requires_new_session());
    assert_eq!(
        error.recovery(),
        grafton_visca::error::Recovery::ReopenSession
    );
    let context = error
        .failure_context()
        .expect("applied move observation context");
    assert_eq!(context.stage, FailureStage::Observation);
    assert_eq!(context.certainty, Certainty::Unconfirmed);
}

const ZOOM_TARGET: &[u8] = &[0x81, 0x01, 0x04, 0x47, 0x00, 0x01, 0x00, 0x00, 0xff];
const PEER_STOP: &[u8] = &[0x82, 0x01, 0x04, 0x07, 0x00, 0xff];
const ZOOM_INQUIRY: &[u8] = &[0x81, 0x09, 0x04, 0x47, 0xff];

/// A targeted operation can already have applied when another target retries.
/// Wait for that retry's actual write failure and teardown before polling, so
/// the next inquiry is certainly rejected before admission, on every OS.
#[test]
fn stream_retry_write_failure_before_settlement_inquiry_is_session_death() {
    let profile = ProfileSpec::from_compile_time::<DirectZoomOnlyTypedSupport>()
        .expect("settlement-polling runtime profile");
    let config = SessionConfig::new(profile.clone())
        .with_target(grafton_visca::types::IndividualCameraId::CAMERA_2, profile)
        .expect("second serial target")
        .with_tuning(
            OperationalTuning::new()
                .with_initial_retry_backoff(Duration::from_millis(50))
                .with_maximum_retry_backoff(Duration::from_millis(500))
                .with_retry_budget(Duration::from_secs(10)),
        );
    let io = Arc::new(std::io::Error::other("public settlement write failed"));
    let failure = Error::transport_error(Error::Io {
        source: Arc::clone(&io),
    });
    let mut peer_writes = 0;
    let fake = FakeCamera::new(move |frame, answer| match frame {
        ZOOM_TARGET => {
            answer.reply(frames::ack(1)).reply(frames::complete(1));
        }
        PEER_STOP => {
            peer_writes += 1;
            match peer_writes {
                1 => {
                    answer.reply([0xa0, 0x60, 0x03, 0xff]);
                }
                2 => {
                    answer.fail_send(failure.clone());
                }
                _ => panic!("the failed peer retry must never be replayed"),
            }
        }
        _ => panic!("unexpected write before settlement: {frame:02x?}"),
    });
    let session = Session::open(serial_stream_wire(&fake), &config).expect("owner session");
    let first_camera = session
        .camera::<DirectZoomOnlyTypedSupport>(grafton_visca::types::IndividualCameraId::CAMERA_1)
        .expect("camera one view");
    let second_camera = session
        .camera::<DirectZoomOnlyTypedSupport>(grafton_visca::types::IndividualCameraId::CAMERA_2)
        .expect("camera two view");
    let mut current = first_camera
        .submit::<Targeted, _>(&grafton_visca::command::zoom::SetPosition::new(
            ZoomPosition::new(0x0100).expect("zoom position"),
        ))
        .expect("targeted operation admission");
    current
        .applied()
        .expect("cache application before the peer retry");
    let mut peer = second_camera
        .submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)
        .expect("peer operation admission");
    assert!(matches!(peer.applied(), Err(Error::StreamPoisoned { .. })));
    assert!(matches!(
        session.clone().close(),
        Err(Error::StreamPoisoned { .. })
    ));

    let error = current
        .settled()
        .expect_err("a poisoned session cannot admit settlement queries");
    assert_settlement_stream_failure(&error, PollAdmission::Rejected, &io);
    current
        .applied()
        .expect("settlement failure cannot retract application");
    assert_eq!(fake.writes(), [ZOOM_TARGET, PEER_STOP, PEER_STOP]);
}

/// Failing the position inquiry's own write proves it was already admitted.
/// Its terminal must retain InquiryFailed as well as the session poison;
/// teardown racing the caller's wake cannot erase that delivered receipt.
#[test]
fn stream_inquiry_write_failure_during_settlement_retains_its_terminal_context() {
    let profile = ProfileSpec::from_compile_time::<DirectZoomOnlyTypedSupport>()
        .expect("settlement-polling runtime profile");
    let io = Arc::new(std::io::Error::other("public settlement write failed"));
    let failure = Error::transport_error(Error::Io {
        source: Arc::clone(&io),
    });
    let fake = FakeCamera::new(move |frame, answer| match frame {
        ZOOM_TARGET => {
            answer.reply(frames::ack(1)).reply(frames::complete(1));
        }
        ZOOM_INQUIRY => {
            answer.fail_send(failure.clone());
        }
        _ => panic!("unexpected settlement write: {frame:02x?}"),
    });
    let session =
        Session::open(stream_wire(&fake), &SessionConfig::new(profile)).expect("owner session");
    let camera = session
        .camera::<DirectZoomOnlyTypedSupport>(grafton_visca::types::IndividualCameraId::CAMERA_1)
        .expect("camera view");
    let mut current = camera
        .submit::<Targeted, _>(&grafton_visca::command::zoom::SetPosition::new(
            ZoomPosition::new(0x0100).expect("zoom position"),
        ))
        .expect("targeted operation admission");

    let error = current
        .settled()
        .expect_err("the admitted settlement inquiry write fails");
    assert_settlement_stream_failure(&error, PollAdmission::Admitted, &io);
    current
        .applied()
        .expect("settlement failure cannot retract application");
    assert!(matches!(session.close(), Err(Error::StreamPoisoned { .. })));
    assert_eq!(fake.writes(), [ZOOM_TARGET, ZOOM_INQUIRY]);
}
