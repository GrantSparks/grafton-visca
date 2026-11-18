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
    observability::SessionStatus,
    operation::AppliedOnly,
    profiles::{GenericVisca, ProfileSpec, SonyFr7},
    transport::{IoBounds, SendSemantics},
    types::{IndividualCameraId, PanTiltSpeedMode},
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
}

impl Stream {
    fn new(camera: FakeCamera) -> Self {
        Self { camera }
    }

    fn config() -> IoBounds {
        IoBounds::default()
            .with_buffer_config(
                grafton_visca::transport::BufferConfig::default()
                    .with_recv_buffer_size(64)
                    .with_max_buffer_size(128),
            )
            .expect("buffer bounds")
    }

    #[cfg(feature = "blocking")]
    fn blocking_wire(&self) -> fake_camera::BlockingWire {
        self.camera
            .blocking_wire()
            .with_config(Self::config())
            .with_semantics(SendSemantics::Stream)
    }

    #[cfg(all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    ))]
    fn async_wire(&self) -> fake_camera::AsyncWire {
        self.camera
            .async_wire()
            .with_config(Self::config())
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
            .with_config(Stream::config())
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
            .with_config(Stream::config())
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

}
