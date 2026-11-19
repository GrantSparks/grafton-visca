//! Stream framing preserves delimited noise and terminates on position loss.

#![cfg(any(
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]

use grafton_visca::{
    config::SessionConfig,
    observability::{DiagnosticCapacity, DiagnosticEvent, DiagnosticResponse, SessionStatus},
    operation::AppliedOnly,
    profiles::{GenericVisca, ProfileSpec, SonyFr7},
    transport::{IoBounds, SendSemantics},
    types::{IndividualCameraId, PanTiltSpeedMode, PowerStatus},
    Error,
};
use grafton_visca_test_support::{facade_matrix, fake_camera};

use fake_camera::{frames, FakeCamera, ZOOM_TELE};

// A declared payload of 57 bytes exceeds the 64-byte frame limit with its header.
const OVERSIZED_SONY_HEADER: [u8; 8] = [0x01, 0x11, 0x00, 0x39, 0, 0, 0, 1];

fn assert_oversized_stream_poison(error: &Error) {
    assert!(
        matches!(error, Error::StreamPoisoned { source, .. }
        if matches!(source.as_ref(), Error::ResponseTooLarge { max_size: 64, .. })),
        "{error:?}"
    );
}

struct Stream {
    camera: FakeCamera,
    recv_buffer_size: usize,
}

impl Stream {
    fn new(camera: FakeCamera) -> Self {
        Self::with_recv_buffer_size(camera, 64)
    }

    fn with_recv_buffer_size(camera: FakeCamera, recv_buffer_size: usize) -> Self {
        Self {
            camera,
            recv_buffer_size,
        }
    }

    fn config(recv_buffer_size: usize) -> IoBounds {
        IoBounds::default()
            .with_buffer_config(
                grafton_visca::transport::BufferConfig::default()
                    .with_recv_buffer_size(recv_buffer_size)
                    .with_max_buffer_size(recv_buffer_size.max(128)),
            )
            .expect("buffer bounds")
    }

    #[cfg(feature = "blocking")]
    fn blocking_wire(&self) -> fake_camera::BlockingWire {
        self.camera
            .blocking_wire()
            .with_config(Self::config(self.recv_buffer_size))
            .with_semantics(SendSemantics::Stream)
    }

    #[cfg(all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    ))]
    fn async_wire(&self) -> fake_camera::AsyncWire {
        self.camera
            .async_wire()
            .with_config(Self::config(self.recv_buffer_size))
            .with_semantics(SendSemantics::Stream)
    }
}

// The same malformed header is recoverable when its datagram boundary is known.
struct Datagram(Stream);
impl Datagram {
    fn new(camera: FakeCamera) -> Self {
        Self(Stream::new(camera))
    }
    #[cfg(feature = "blocking")]
    fn blocking_wire(&self) -> fake_camera::BlockingWire {
        self.0
            .camera
            .blocking_wire()
            .with_config(Stream::config(self.0.recv_buffer_size))
            .with_semantics(SendSemantics::Datagram)
    }
    #[cfg(all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    ))]
    fn async_wire(&self) -> fake_camera::AsyncWire {
        self.0
            .camera
            .async_wire()
            .with_config(Stream::config(self.0.recv_buffer_size))
            .with_semantics(SendSemantics::Datagram)
    }
}

facade_matrix! {
    fn oversized_delimited_raw_noise_is_counted_and_the_following_ack_still_applies() {
        let camera = FakeCamera::new(|_, answer| {
            // A 106-byte delimited frame split over reads, beyond the 64-byte
            // frame limit but within the 192-byte retention-plus-read bound.
            answer.reply(vec![0x55; 64]);
            let mut tail = vec![0x55; 41];
            tail.extend_from_slice(&[0xFF, 0x90, 0x41, 0xFF, 0x90, 0x51, 0xFF]);
            answer.reply(tail);
        });
        let config = SessionConfig::new(ProfileSpec::from_compile_time::<GenericVisca>().unwrap());
        let session = open!(Stream::new(camera), config).expect("stream owner");
        let view = session.camera::<GenericVisca>(grafton_visca::types::IndividualCameraId::CAMERA_1).unwrap();
        wait!(wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)).unwrap().applied())
            .expect("only the following valid ACK and completion apply the command");

        let metrics = wait!(session.metrics()).expect("the owner remains usable");
        assert_eq!(metrics.session, SessionStatus::Running);
        assert_eq!(metrics.ignored_malformed_frames, 1);
        session.shutdown().unwrap();
    }

    fn cumulative_raw_overflow_still_terminates_the_session() {
        let camera = FakeCamera::new(|_, answer| {
            for _ in 0..3 {
                answer.reply(vec![0x55; 64]);
            }
            answer.reply(vec![0x55]);
        });
        let config = SessionConfig::new(ProfileSpec::from_compile_time::<GenericVisca>().unwrap());
        let session = open!(Stream::new(camera), config).expect("stream owner");
        let view = session.camera::<GenericVisca>(grafton_visca::types::IndividualCameraId::CAMERA_1).unwrap();
        let error = wait!(wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)).unwrap().applied())
            .expect_err("193 boundary-free bytes exceed the configured 192-byte bound");
        assert!(error.requires_new_session(), "{error:?}");
    }

    fn an_oversized_sony_header_still_terminates_the_session() {
        let camera = FakeCamera::new(|write, answer| {
            if write.starts_with(&[0x02, 0x00]) {
                return; // sequence-reset control write
            }
            answer.reply(OVERSIZED_SONY_HEADER);
        });
        let config = SessionConfig::new(ProfileSpec::from_compile_time::<SonyFr7>().unwrap());
        let session = open!(Stream::new(camera), config).expect("Sony stream owner");
        let view = session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1).unwrap();
        let error = wait!(wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)).unwrap().applied())
            .expect_err("the declared Sony frame cannot be honoured");
        assert!(error.requires_new_session(), "{error:?}");
    }
    fn a_sony_completion_before_a_framing_failure_commits_in_every_read_layout() {
        // ACK and completion are each 11 bytes. Exercise complete frames per
        // read, fragmented frames, and all replies plus the failure in one read.
        for chunk_size in [11, 4, 64] {
            let camera = FakeCamera::new(move |write, answer| {
                if write.starts_with(&[0x02, 0x00]) { return; }
                let (sequence, payload) = frames::sony_split(write).unwrap();
                if payload == ZOOM_TELE {
                    answer.reply(frames::sony_reply(sequence, &frames::ack(1)));
                    return;
                }
                let bytes = [
                    frames::sony_reply(sequence, &frames::ack(2)),
                    // Delimited ACK with an invalid VISCA response source.
                    frames::sony_reply(sequence, &[0x80, 0x42, 0xff]),
                    frames::sony_reply(sequence, &frames::complete(2)),
                    OVERSIZED_SONY_HEADER.to_vec(),
                ].concat();
                for chunk in bytes.chunks(chunk_size) {
                    answer.reply(chunk);
                }
            });
            let config = SessionConfig::from_compile_time::<SonyFr7>().unwrap();
            let session = open!(Stream::new(camera), config).unwrap();
            let view = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).unwrap();
            let mut held = wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Tele)).unwrap();
            let mode = PanTiltSpeedMode::Fine50;
            wait!(view.pan_tilt().set_speed_mode(mode)).expect("the setter completed before position loss");
            assert_eq!(view.state_cache().pan_tilt().speed_mode(), Some(mode));
            assert_oversized_stream_poison(&wait!(held.applied()).unwrap_err());
            let metrics = wait!(session.metrics()).unwrap();
            assert_eq!(metrics.session, SessionStatus::Poisoned);
            assert_eq!(metrics.ignored_malformed_frames, 1);
            assert_eq!(metrics.cache_updates, 1);
            assert_oversized_stream_poison(&wait!(session.close()).unwrap_err());
        }
    }

    fn an_undecodable_sony_header_terminates_without_waiting_for_more_input() {
        let camera = FakeCamera::new(|write, answer| {
            if write.starts_with(&[0x02, 0x00]) { return; }
            answer.reply(vec![0x90]);
            answer.reply(vec![0x41]);
        });
        let config = SessionConfig::new(ProfileSpec::from_compile_time::<SonyFr7>().unwrap());
        let session = open!(Stream::new(camera), config).expect("Sony stream owner");
        let view = session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1).unwrap();
        let error = wait!(wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)).unwrap().applied()).expect_err("invalid payload type loses the stream boundary");
        assert!(error.requires_new_session(), "{error:?}");
        assert!(format!("{error:?}").contains("Sony header"), "{error:?}");
    }

    fn an_undecodable_sony_datagram_is_counted_and_the_next_reply_applies() {
        let camera = FakeCamera::new(|write, answer| {
            if write.starts_with(&[0x02, 0x00]) { return; }
            // A valid camera rejection inside this malformed datagram must
            // be discarded with its trailing bytes.
            let sequence = frames::sony_sequence(write).unwrap();
            let mut invalid = frames::sony_reply(sequence, &frames::not_executable(0));
            invalid.extend_from_slice(&[0x90, 0x41]);
            answer.reply(invalid);
            for class in [0x41, 0x51] {
                let mut reply = vec![1, 0x11, 0, 3];
                reply.extend_from_slice(&write[4..8]);
                reply.extend_from_slice(&[0x90, class, 0xff]);
                answer.reply(reply);
            }
        });
        let config = SessionConfig::new(ProfileSpec::from_compile_time::<SonyFr7>().unwrap());
        let session = open!(Datagram::new(camera), config).expect("Sony datagram owner");
        let view = session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1).unwrap();
        wait!(wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)).unwrap().applied()).expect("the next Sony ACK and completion still apply");
        let metrics = wait!(session.metrics()).unwrap();
        assert_eq!(metrics.session, SessionStatus::Running);
        assert_eq!(metrics.ignored_malformed_frames, 1);
        session.shutdown().unwrap();
    }

    fn sony_control_errors_are_observable_without_resolving_a_live_inquiry() {
        for stream in [false, true] {
            let fake = FakeCamera::new(|write, answer| {
                let (sequence, payload) = frames::sony_split(write).unwrap();
                assert_eq!(payload, [0x81, 0x09, 0x04, 0x00, 0xff]);
                for detail in [0x01, 0x02] {
                    let mut control = vec![0x02, 0x00, 0x00, 0x02];
                    control.extend_from_slice(&sequence.to_be_bytes());
                    control.extend_from_slice(&[0x0f, detail]);
                    answer.reply(control);
                }
                answer.reply(frames::sony_reply(sequence, &[0x90, 0x50, 0x02, 0xff]));
            });
            let config = SessionConfig::from_compile_time::<SonyFr7>().unwrap();
            let session = if stream {
                open!(Stream::new(fake.clone()), config)
            } else {
                open!(Datagram::new(fake.clone()), config)
            }.expect("Sony session");
            let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).unwrap();
            let diagnostics = wait!(session.subscribe_diagnostics(DiagnosticCapacity::MAX)).unwrap();

            let reply = wait!(camera.power().status())
                .expect("the sequenced VISCA reply completes the inquiry");
            assert_eq!(reply, PowerStatus::On);
            let responses: Vec<_> = std::iter::from_fn(|| diagnostics.try_recv())
                .filter_map(|event| match event {
                    DiagnosticEvent::FrameReceived { target, response, .. } => {
                        assert_eq!(target, IndividualCameraId::CAMERA_1);
                        Some(response)
                    }
                    _ => None,
                })
                .collect();
            assert!(matches!(responses.as_slice(), [
                DiagnosticResponse::SonyControl { code: 0x0F01, .. },
                DiagnosticResponse::SonyControl { code: 0x0F02, .. },
                DiagnosticResponse::InquiryReply,
            ]), "{responses:?}");
            let metrics = wait!(session.metrics()).unwrap();
            assert_eq!(metrics.terminal, 1);
            assert_eq!(metrics.retries_scheduled, 0);
            assert_eq!(metrics.protocol_errors, 0);
            assert_eq!(metrics.ignored_malformed_frames, 0);
            assert_eq!(fake.write_count(), 1);
            wait!(session.close()).unwrap();
        }
    }
}

#[cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
mod cancellation {
    use super::*;
    use fake_camera::ZOOM_STOP;
    use grafton_visca::operation::CancellationOutcome;
    use std::{io, time::Duration};

    const CANCEL_SOCKET_1: &[u8] = &[0x81, 0x21, 0xff];
    const INITIAL_WAIT: Duration = Duration::from_millis(50);

    fn fail_cancel_write() -> FakeCamera {
        FakeCamera::visca(|payload, answer| {
            if payload == CANCEL_SOCKET_1 {
                answer.fail_send(io::Error::from(io::ErrorKind::BrokenPipe).into());
            }
        })
    }

    macro_rules! record_cancel_before_ack {
        ($session:expr, $op:expr) => {{
            assert!(matches!(
                wait!($op.cancel_with_timeout(INITIAL_WAIT)),
                Err(Error::ObservationTimeout { .. })
            ));
            assert_eq!(wait!($session.metrics()).unwrap().cancellations, 1);
        }};
    }

    facade_matrix! {
        paused:

        fn a_raw_completion_in_the_same_read_suppresses_a_pending_socket_cancel() {
            // With 63 notices the completion occupies slot 65 and must still
            // beat a cancellation whose ACK was decoded in slot 1.
            for notices in [0, 63] {
                let fake = fail_cancel_write();
                let stream = Stream::with_recv_buffer_size(fake.clone(), 1024);
                let config = SessionConfig::from_compile_time::<GenericVisca>().unwrap();
                let session = open!(stream, config).unwrap();
                let camera = session.camera::<GenericVisca>(IndividualCameraId::CAMERA_1).unwrap();
                let mut operation = wait!(camera.zoom().stop()).unwrap();
                wait_for_writes!(fake, 1);
                record_cancel_before_ack!(session, operation);

                let mut burst = frames::ack(1);
                for _ in 0..notices {
                    burst.extend_from_slice(&[0x90, 0x38, 0xff]);
                }
                burst.extend(frames::complete(1));
                fake.push(burst);

                assert_eq!(wait!(operation.cancel()).unwrap(), CancellationOutcome::Completed);
                wait!(operation.applied()).unwrap();
                let metrics = wait!(session.metrics()).unwrap();
                assert_eq!(metrics.session, SessionStatus::Running);
                assert_eq!(metrics.received_frames, notices + 2);
                assert_eq!(fake.read_count(), 1);
                assert_eq!(fake.writes(), vec![ZOOM_STOP.to_vec()]);
                wait!(session.close()).unwrap();
            }
        }

        fn an_ack_without_another_complete_reply_sends_the_pending_socket_cancel_once() {
            for split_confirmation in [false, true] {
                let confirmation = frames::canceled(1);
                let split = if split_confirmation { 2 } else { 0 };
                let tail = confirmation[split..].to_vec();
                let fake = FakeCamera::new(move |payload, answer| {
                    if payload == CANCEL_SOCKET_1 {
                        answer.reply(tail.clone());
                    }
                });
                let config = SessionConfig::from_compile_time::<GenericVisca>().unwrap();
                let session = open!(Stream::new(fake.clone()), config).unwrap();
                let camera = session.camera::<GenericVisca>(IndividualCameraId::CAMERA_1).unwrap();
                let mut operation = wait!(camera.zoom().stop()).unwrap();
                wait_for_writes!(fake, 1);
                record_cancel_before_ack!(session, operation);

                // The partial reply can finish only after the write. Its
                // prefix must allow the pending cancellation to proceed.
                let mut burst = frames::ack(1);
                burst.extend_from_slice(&confirmation[..split]);
                fake.push(burst);

                assert_eq!(wait!(operation.cancel()).unwrap(), CancellationOutcome::Cancelled);
                assert!(matches!(wait!(operation.applied()), Err(Error::CommandCancelled)));
                assert_eq!(fake.writes(), vec![ZOOM_STOP.to_vec(), CANCEL_SOCKET_1.to_vec()]);
                wait!(session.close()).unwrap();
            }
        }

        fn a_pending_cancel_keeps_its_completion_before_fatal_sony_framing_loss() {
            let fake = fail_cancel_write();
            let config = SessionConfig::from_compile_time::<SonyFr7>().unwrap();
            let session = open!(Stream::new(fake.clone()), config).unwrap();
            let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).unwrap();
            let startup = fake.write_count();
            let mut operation = wait!(camera.zoom().stop()).unwrap();
            wait_for_writes!(fake, startup + 1);
            record_cancel_before_ack!(session, operation);

            let writes = fake.writes();
            let (sequence, payload) = frames::sony_split(&writes[startup]).unwrap();
            assert_eq!(payload, ZOOM_STOP);
            fake.push([
                frames::sony_reply(sequence, &frames::ack(1)),
                frames::sony_reply(sequence, &frames::complete(1)),
                OVERSIZED_SONY_HEADER.to_vec(),
            ].concat());

            assert_eq!(wait!(operation.cancel()).unwrap(), CancellationOutcome::Completed);
            wait!(operation.applied()).unwrap();
            assert_oversized_stream_poison(&wait!(session.close()).unwrap_err());
            assert_eq!(fake.writes(), writes);
        }

        fn a_sony_socket_reassignment_in_the_same_datagram_suppresses_a_pending_cancel() {
            for first_completes_in_burst in [true, false] {
                let fake = FakeCamera::silent();
                let config = SessionConfig::from_compile_time::<SonyFr7>().unwrap();
                let session = open!(Datagram::new(fake.clone()), config).unwrap();
                let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).unwrap();
                let startup = fake.write_count();
                let mut first = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Tele)).unwrap();
                wait_for_writes!(fake, startup + 1);
                let mut next = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Wide)).unwrap();
                wait_for_writes!(fake, startup + 2);
                // The observation wait outlasts the FR7's 35 ms write pacing.
                record_cancel_before_ack!(session, first);

                let writes = fake.writes();
                let (first_seq, first_payload) = frames::sony_split(&writes[startup]).unwrap();
                let (next_seq, next_payload) = frames::sony_split(&writes[startup + 1]).unwrap();
                assert_eq!(first_payload, ZOOM_TELE);
                assert_eq!(next_payload, [0x81, 0x01, 0x04, 0x07, 0x03, 0xff]);
                let mut burst = frames::sony_reply(first_seq, &frames::ack(1));
                if first_completes_in_burst {
                    burst.extend(frames::sony_reply(first_seq, &frames::complete(1)));
                }
                burst.extend(frames::sony_reply(next_seq, &frames::ack(1)));
                fake.push(burst);

                if first_completes_in_burst {
                    assert_eq!(wait!(first.cancel()).unwrap(), CancellationOutcome::Completed);
                } else {
                    assert!(matches!(wait!(first.cancel()), Err(Error::CancellationUnconfirmed)));
                    fake.push(frames::sony_reply(first_seq, &frames::complete(1)));
                }
                wait!(first.applied()).unwrap();
                fake.push(frames::sony_reply(next_seq, &frames::complete(1)));
                wait!(next.applied()).unwrap();
                assert_eq!(wait!(session.metrics()).unwrap().session, SessionStatus::Running);
                assert_eq!(fake.writes(), writes);
                wait!(session.close()).unwrap();
            }
        }
    }
}
