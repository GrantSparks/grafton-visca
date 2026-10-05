//! Testing utilities for the grafton-visca library
//!
//! This module provides testing infrastructure, including a simulator for Standard
//! VISCA framing in integration tests. Its pan/tilt position inquiries always use
//! the standard signed 4+4-nibble reply form; it is not a profile- or
//! codec-selectable simulator and does not model Sony BRC-300's profile-owned
//! 5+4 position codec.

// The reply frames shared by the testkit helpers, the simulator, and (through
// `#[path]`) the integration suite's fake camera. Each of those uses a
// different subset — the request-framing helpers serve only the fake camera —
// so no single crate build references every item.
#[cfg(any(feature = "test-utils", all(test, feature = "runtime-tokio")))]
#[allow(dead_code)]
mod frames;
#[cfg(any(feature = "test-utils", all(test, feature = "runtime-tokio")))]
use crate::command::VISCA_TERMINATOR;

#[cfg(all(feature = "runtime-tokio", any(test, feature = "test-utils")))]
pub mod camera_simulator;

#[cfg(all(feature = "runtime-tokio", any(test, feature = "test-utils")))]
pub use camera_simulator::ViscaCameraSimulator;

/// Testing toolkit for deterministic and scriptable transport testing.
///
/// This module provides utilities for writing deterministic tests that don't rely
/// on real wall-clock time or unpredictable timing behavior.
#[cfg(any(test, feature = "test-utils"))]
pub mod testkit;

/// Internal protocol fuzz entry points.
#[doc(hidden)]
#[cfg(all(feature = "test-utils", feature = "blocking"))]
pub mod fuzz;

#[cfg(all(test, feature = "test-utils"))]
#[allow(clippy::expect_used, clippy::panic)]
mod frame_tests {
    //! Pins the reply frames against the crate's own reply decoder, and the
    //! Sony envelope against the crate's header encoder.

    use super::frames;
    use crate::{
        command::response::{InquiryKind, Response},
        protocol::sony::SonyHeader,
        Error,
    };

    fn decode(frame: &[u8]) -> Response {
        Response::parse_with_type(frame, &InquiryKind::Power).expect("a well-formed reply frame")
    }

    #[test]
    fn buffer_full_has_one_socketless_frame_shape() {
        assert_eq!(frames::buffer_full(), [0x90, 0x60, 0x03, 0xFF]);
        assert!(matches!(
            decode(&frames::buffer_full()),
            Response::Error(Error::CommandBufferFull)
        ));
    }

    #[test]
    fn reply_frames_decode_as_the_reply_they_name() {
        assert!(matches!(decode(&frames::ack(1)), Response::CmdAck { .. }));
        assert!(matches!(
            decode(&frames::complete(2)),
            Response::Completion { .. }
        ));
        assert!(matches!(
            decode(&frames::syntax_error()),
            Response::Error(Error::SyntaxError)
        ));
        assert!(matches!(
            decode(&frames::not_executable(1)),
            Response::Error(Error::CommandNotExecutable)
        ));
        assert!(matches!(
            decode(&frames::canceled(1)),
            Response::Error(Error::CommandCanceled)
        ));
        assert_eq!(frames::ack(9), [0x90, 0x49, 0xFF], "socket is masked");
        assert_eq!(frames::inquiry_reply(&[0x02]), [0x90, 0x50, 0x02, 0xFF]);
    }

    #[test]
    fn sony_reply_header_matches_the_crate_encoder() {
        let payload = frames::complete(1);
        let frame = frames::sony_reply(0x0102_0304, &payload);
        assert_eq!(
            frame[..frames::SONY_HEADER_LEN],
            SonyHeader::new_reply(payload.len(), 0x0102_0304).encode()
        );
        assert_eq!(frames::sony_sequence(&frame), Some(0x0102_0304));
        assert_eq!(frames::visca_payload(&frame), payload.as_slice());
        assert_eq!(frames::reply_like(&frame, &payload), frame);

        let raw = [0x81, 0x09, 0x04, 0x00, 0xFF];
        assert_eq!(frames::sony_split(&raw), None, "raw VISCA has no envelope");
        assert_eq!(frames::visca_payload(&raw), raw);
        assert_eq!(frames::reply_like(&raw, &payload), payload);

        for payload_type in frames::SONY_PAYLOAD_TYPES {
            let mut frame = frames::sony_reply(7, &payload);
            frame[..2].copy_from_slice(&payload_type);
            assert_eq!(frames::sony_split(&frame), Some((7, payload.as_slice())));
        }
    }

    #[test]
    fn sony_payload_types_match_the_crate_envelope() {
        use crate::protocol::sony::PayloadType;

        for payload_type in frames::SONY_PAYLOAD_TYPES {
            assert!(
                PayloadType::from_bytes(payload_type).is_some(),
                "{payload_type:02X?} is not a crate payload type"
            );
        }
        for first in [0x01, 0x02] {
            for second in 0..=u8::MAX {
                let bytes = [first, second];
                assert_eq!(
                    PayloadType::from_bytes(bytes).is_some(),
                    frames::SONY_PAYLOAD_TYPES.contains(&bytes),
                    "{bytes:02X?}"
                );
            }
        }
    }

    #[test]
    #[should_panic(expected = "unknown Sony payload type")]
    fn sony_split_rejects_an_unknown_payload_type() {
        let mut frame = frames::sony_reply(1, &frames::ack(1));
        frame[1] = 0x30;
        let _ = frames::sony_split(&frame);
    }

    #[test]
    #[should_panic(expected = "Sony length field disagrees with the payload")]
    fn sony_split_rejects_a_length_mismatch() {
        let mut frame = frames::sony_reply(1, &frames::ack(1));
        frame.push(0xFF);
        let _ = frames::visca_payload(&frame);
    }

    #[test]
    #[should_panic(expected = "truncated Sony envelope header")]
    fn sony_split_rejects_a_truncated_header() {
        let frame = frames::sony_reply(1, &frames::ack(1));
        let _ = frames::sony_sequence(&frame[..5]);
    }
}
