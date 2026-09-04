//! Unified VISCA response decoding and frame splitting.
//!
//! This module provides zero-cost, runtime-agnostic utilities for parsing VISCA
//! protocol responses and splitting frame buffers. It eliminates duplication between
//! async and sync paths while preserving socket awareness and type safety.

use crate::{
    camera_id::CameraId,
    command::{bytes::VISCA_TERMINATOR, response::Payload},
    ViscaSocket,
};

/// Basic VISCA response kind.
///
/// This enum represents the fundamental response types in the VISCA protocol,
/// providing a minimal, zero-cost abstraction over raw frame bytes.
///
/// The `z0` in each frame shape below is the reply address byte: `z = 8 + n`
/// for the camera at address *n*, so camera 1 answers `90 ...` and camera 7
/// answers `F0 ...`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasicKind {
    /// Acknowledgment - command accepted (z0 4y FF).
    Ack,
    /// Command completed successfully (z0 5y FF).
    Completion,
    /// Data reply from inquiry (z0 50 ... FF).
    DataReply,
    /// Error response with code (z0 6y zz FF).
    Error(u8),
    /// Network change message (z0 38 FF).
    NetworkChange,
    /// Unknown response type.
    Unknown,
}

/// Basic response structure using borrowed slices.
///
/// This struct provides a zero-allocation view of a VISCA response frame,
/// extracting the source address, socket information, and payload data without
/// heap allocations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicResponse<'a> {
    /// The camera that sent this reply, decoded from the reply address byte.
    ///
    /// On a point-to-point transport this is always
    /// [`CameraId::CAMERA_1`](crate::CameraId::CAMERA_1) (`0x90`); on a serial
    /// daisy chain it identifies which of the up to seven addressed devices
    /// answered.
    pub source: CameraId,
    /// The type of response.
    pub kind: BasicKind,
    /// Socket number if present (for ACK/Completion/Error).
    pub socket: Option<ViscaSocket>,
    /// Payload data between header and terminator (for DataReply), wrapped in a type-safe view.
    pub payload: Payload<'a>,
}

/// Decode a single VISCA response frame.
///
/// Takes a complete frame (including terminator) and returns a zero-copy view
/// of the response data. This is the single source of truth for VISCA response
/// parsing across the entire codebase.
///
/// # Reply addressing
///
/// The first byte of a reply is `(8 + n) << 4` for the camera at address *n*,
/// so a daisy chain produces `0x90` (camera 1) through `0xF0` (camera 7). The
/// whole range is accepted and the source address is reported in
/// [`BasicResponse::source`]. Lead bytes below `0x90` are not reply addresses
/// (`0x80`-`0x8F` is the controller/broadcast range, including the `0x88`
/// address-set traffic the serial handshake parses separately) and are
/// rejected as malformed.
///
/// # Arguments
/// * `frame` - Complete VISCA frame including the 0xFF terminator
///
/// # Returns
/// * `Some(BasicResponse)` if the frame is valid
/// * `None` if the frame is malformed
pub fn decode_basic(frame: &[u8]) -> Option<BasicResponse<'_>> {
    let source = CameraId::from_reply_address(*frame.first()?)?;
    decode_basic_for_source(frame, source)
}

/// Decode a response after the transport boundary has strictly established
/// its source and routing target.
///
/// Keeping source validation outside this helper lets the owner classify a
/// serial frame once, without rewriting it into a camera-1 scratch buffer.
pub(crate) fn decode_basic_for_source(frame: &[u8], source: CameraId) -> Option<BasicResponse<'_>> {
    // Minimum valid response is 3 bytes (e.g., 90 38 FF)
    if frame.len() < 3 {
        return None;
    }

    // Check for terminator
    if frame[frame.len() - 1] != VISCA_TERMINATOR {
        return None;
    }

    // Parse based on second byte
    match frame[1] {
        // ACK (z0 4y FF)
        byte if (byte & 0xF0) == 0x40 => {
            // ACKs are fixed-format replies. Do not let trailing bytes turn a
            // malformed ACK into a valid one (or into an Unknown response).
            if frame.len() != 3 {
                return None;
            }
            let socket_num = byte & 0x0F;
            let socket = decode_fixed_socket(socket_num)?;
            Some(BasicResponse {
                source,
                kind: BasicKind::Ack,
                socket,
                payload: Payload::new(&[]),
            })
        }

        // Completion (z0 5y FF) or Data Reply (z0 50 ... FF with more than 3 bytes)
        byte if (byte & 0xF0) == 0x50 => {
            let socket_num = byte & 0x0F;

            // Special case: z0 50 with >3 bytes is data reply
            if socket_num == 0 && frame.len() > 3 {
                // Data reply: z0 50 <payload> FF
                let payload = Payload::new(&frame[2..frame.len() - 1]);
                Some(BasicResponse {
                    source,
                    kind: BasicKind::DataReply,
                    socket: None,
                    payload,
                })
            } else if frame.len() == 3 {
                // Completion: z0 5y FF (including z0 50 FF for socket 0)
                let socket = decode_fixed_socket(socket_num)?;
                Some(BasicResponse {
                    source,
                    kind: BasicKind::Completion,
                    socket,
                    payload: Payload::new(&[]),
                })
            } else {
                // Nonzero-socket completions are fixed-format replies. A
                // malformed frame must not fall through to Unknown.
                None
            }
        }

        // Error (z0 6y zz FF)
        byte if (byte & 0xF0) == 0x60 => {
            if frame.len() != 4 {
                return None;
            }
            let socket_num = byte & 0x0F;
            let socket = decode_fixed_socket(socket_num)?;
            let error_code = frame[2];
            Some(BasicResponse {
                source,
                kind: BasicKind::Error(error_code),
                socket,
                payload: Payload::new(&[]),
            })
        }

        // Network change (z0 38 FF)
        0x38 => {
            if frame.len() != 3 {
                return None;
            }
            Some(BasicResponse {
                source,
                kind: BasicKind::NetworkChange,
                socket: None,
                payload: Payload::new(&[]),
            })
        }

        // Unknown
        _ => Some(BasicResponse {
            source,
            kind: BasicKind::Unknown,
            socket: None,
            payload: Payload::new(&frame[2..frame.len() - 1]),
        }),
    }
}

/// Decode the socket nibble used by a fixed-format response.
///
/// VISCA uses nibble `0` on the wire for a socketless compatibility response,
/// while the only numbered command sockets are `1` and `2`.  The public
/// [`ViscaSocket::from_protocol_byte`] helper intentionally returns `None` for
/// both socketless and invalid values, so the outer `Option` here preserves
/// that distinction for strict fixed-frame parsing: `Some(None)` is valid
/// socketless evidence and `None` is a malformed socket nibble.
fn decode_fixed_socket(nibble: u8) -> Option<Option<ViscaSocket>> {
    match nibble {
        0 => Some(None),
        1 | 2 => ViscaSocket::from_protocol_byte(nibble).map(Some),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    use crate::command::{
        response::{lift_inquiry, InquiryKind, Response},
        InquiryData,
    };

    /// Reply address byte for the camera at chain address `id`.
    fn reply_address(id: u8) -> u8 {
        (8 + id) << 4
    }

    #[test]
    fn test_decode_ack() {
        let frame = vec![0x90, 0x41, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode ACK");
        assert_eq!(response.kind, BasicKind::Ack);
        assert_eq!(response.socket, Some(ViscaSocket::S1));
        assert_eq!(response.payload, Payload::new(&[]));
        assert_eq!(response.source, CameraId::CAMERA_1);

        let frame = vec![0x90, 0x42, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode ACK");
        assert_eq!(response.kind, BasicKind::Ack);
        assert_eq!(response.socket, Some(ViscaSocket::S2));
    }

    #[test]
    fn test_decode_every_chain_source_address() {
        for id in 1..=7u8 {
            let camera = CameraId::new(id).expect("1-7 are valid camera IDs");
            let z0 = reply_address(id);

            let frame = [z0, 0x41, VISCA_TERMINATOR];
            let ack = decode_basic(&frame).unwrap_or_else(|| panic!("camera {id} ACK must decode"));
            assert_eq!(ack.kind, BasicKind::Ack);
            assert_eq!(ack.socket, Some(ViscaSocket::S1));
            assert_eq!(ack.source, camera);

            let frame = [z0, 0x52, VISCA_TERMINATOR];
            let completion = decode_basic(&frame)
                .unwrap_or_else(|| panic!("camera {id} completion must decode"));
            assert_eq!(completion.kind, BasicKind::Completion);
            assert_eq!(completion.socket, Some(ViscaSocket::S2));
            assert_eq!(completion.source, camera);

            let frame = [z0, 0x50, 0x02, VISCA_TERMINATOR];
            let data = decode_basic(&frame)
                .unwrap_or_else(|| panic!("camera {id} data reply must decode"));
            assert_eq!(data.kind, BasicKind::DataReply);
            assert_eq!(data.payload, Payload::new(&[0x02]));
            assert_eq!(data.source, camera);

            let frame = [z0, 0x61, 0x41, VISCA_TERMINATOR];
            let error =
                decode_basic(&frame).unwrap_or_else(|| panic!("camera {id} error must decode"));
            assert_eq!(error.kind, BasicKind::Error(0x41));
            assert_eq!(error.socket, Some(ViscaSocket::S1));
            assert_eq!(error.source, camera);

            let frame = [z0, 0x38, VISCA_TERMINATOR];
            let network = decode_basic(&frame)
                .unwrap_or_else(|| panic!("camera {id} network change must decode"));
            assert_eq!(network.kind, BasicKind::NetworkChange);
            assert_eq!(network.source, camera);
        }
    }

    #[test]
    fn test_decode_rejects_non_reply_addresses() {
        // 0x88 address-set replies are parsed by the serial handshake, never
        // here; 0x81-0x87 are command addresses and 0x80 is the controller.
        for byte in [0x00, 0x7F, 0x80, 0x81, 0x87, 0x88, 0x8F] {
            assert!(
                decode_basic(&[byte, 0x41, VISCA_TERMINATOR]).is_none(),
                "0x{byte:02X} is not a reply address"
            );
        }
    }

    #[test]
    fn test_decode_completion() {
        let frame = vec![0x90, 0x51, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode Completion");
        assert_eq!(response.kind, BasicKind::Completion);
        assert_eq!(response.socket, Some(ViscaSocket::S1));
        assert_eq!(response.payload, Payload::new(&[]));

        let frame = vec![0x90, 0x52, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode Completion");
        assert_eq!(response.kind, BasicKind::Completion);
        assert_eq!(response.socket, Some(ViscaSocket::S2));
    }

    #[test]
    fn test_decode_data_reply() {
        let frame = vec![0x90, 0x50, 0x02, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode DataReply");
        assert_eq!(response.kind, BasicKind::DataReply);
        assert_eq!(response.socket, None);
        assert_eq!(response.payload, Payload::new(&[0x02]));

        let frame = vec![0x90, 0x50, 0x00, 0x01, 0x02, 0x03, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode DataReply");
        assert_eq!(response.kind, BasicKind::DataReply);
        assert_eq!(response.payload, Payload::new(&[0x00, 0x01, 0x02, 0x03]));
    }

    #[test]
    fn test_decode_error() {
        let frame = vec![0x90, 0x60, 0x02, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode Error");
        assert_eq!(response.kind, BasicKind::Error(0x02));
        assert_eq!(response.socket, None);

        let frame = vec![0x90, 0x61, 0x03, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode Error");
        assert_eq!(response.kind, BasicKind::Error(0x03));
        assert_eq!(response.socket, Some(ViscaSocket::S1));
    }

    #[test]
    fn test_decode_network_change() {
        let frame = vec![0x90, 0x38, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode NetworkChange");
        assert_eq!(response.kind, BasicKind::NetworkChange);
        assert_eq!(response.socket, None);
        assert_eq!(response.payload, Payload::new(&[]));
    }

    #[test]
    fn test_decode_invalid() {
        // Too short
        assert!(decode_basic(&[0x90, 0x41]).is_none());

        // Missing terminator
        assert!(decode_basic(&[0x90, 0x41, 0x00]).is_none());

        // Wrong header
        assert!(decode_basic(&[0x80, 0x41, VISCA_TERMINATOR]).is_none());

        // Socket 0 ACK is valid but socket is None
        let response = decode_basic(&[0x90, 0x40, VISCA_TERMINATOR]).expect("Should decode");
        assert_eq!(response.kind, BasicKind::Ack);
        assert_eq!(response.socket, None);

        // Socket 0 Completion is valid but socket is None (when len == 3)
        let response = decode_basic(&[0x90, 0x50, VISCA_TERMINATOR]).expect("Should decode");
        assert_eq!(response.kind, BasicKind::Completion);
        assert_eq!(response.socket, None);
    }

    #[test]
    fn test_decode_rejects_trailing_bytes_on_fixed_replies() {
        // Fixed-format replies must not accept a payload-looking suffix or be
        // reclassified as Unknown when their known prefix is malformed.
        for frame in [
            // ACK: z0 4y FF
            vec![0x90, 0x41, 0x00, VISCA_TERMINATOR],
            // Nonzero-socket completion: z0 5y FF
            vec![0x90, 0x51, 0x00, VISCA_TERMINATOR],
            // Error: z0 6y zz FF
            vec![0x90, 0x61, 0x41, 0x00, VISCA_TERMINATOR],
            // Network change: z0 38 FF
            vec![0x90, 0x38, 0x00, VISCA_TERMINATOR],
        ] {
            assert!(
                decode_basic(&frame).is_none(),
                "malformed fixed frame must be rejected: {frame:02X?}"
            );
        }
    }

    #[test]
    fn test_decode_fixed_socket_nibbles_are_strict() {
        // Nibble 0 is the documented socketless compatibility form; 1 and 2
        // are the two numbered command sockets.  All other nibbles are
        // malformed for fixed ACK, completion, and error frames.
        for (nibble, expected) in [
            (0, None),
            (1, Some(ViscaSocket::S1)),
            (2, Some(ViscaSocket::S2)),
        ] {
            let ack_frame = [0x90, 0x40 | nibble, VISCA_TERMINATOR];
            let ack = decode_basic(&ack_frame).expect("valid ACK socket nibble must decode");
            assert_eq!(ack.kind, BasicKind::Ack);
            assert_eq!(ack.socket, expected);

            let completion_frame = [0x90, 0x50 | nibble, VISCA_TERMINATOR];
            let completion = decode_basic(&completion_frame)
                .expect("valid completion socket nibble must decode");
            assert_eq!(completion.kind, BasicKind::Completion);
            assert_eq!(completion.socket, expected);

            let error_frame = [0x90, 0x60 | nibble, 0x02, VISCA_TERMINATOR];
            let error = decode_basic(&error_frame).expect("valid error socket nibble must decode");
            assert_eq!(error.kind, BasicKind::Error(0x02));
            assert_eq!(error.socket, expected);
        }

        for nibble in 3..=15 {
            assert!(
                decode_basic(&[0x90, 0x40 | nibble, VISCA_TERMINATOR]).is_none(),
                "invalid ACK socket nibble {nibble} must be rejected"
            );
            assert!(
                decode_basic(&[0x90, 0x50 | nibble, VISCA_TERMINATOR]).is_none(),
                "invalid completion socket nibble {nibble} must be rejected"
            );
            assert!(
                decode_basic(&[0x90, 0x60 | nibble, 0x02, VISCA_TERMINATOR]).is_none(),
                "invalid error socket nibble {nibble} must be rejected"
            );
        }
    }

    #[test]
    fn test_decode_socket_zero_data_reply_remains_variable() {
        let response = decode_basic(&[0x90, 0x50, 0x02, 0x03, VISCA_TERMINATOR])
            .expect("socket-zero data reply must decode");
        assert_eq!(response.kind, BasicKind::DataReply);
        assert_eq!(response.socket, None);
        assert_eq!(response.payload, Payload::new(&[0x02, 0x03]));
    }

    #[test]
    fn test_lift_inquiry() {
        // Test ACK lifting
        let basic = BasicResponse {
            source: CameraId::CAMERA_1,
            kind: BasicKind::Ack,
            socket: Some(ViscaSocket::S1),
            payload: Payload::new(&[]),
        };
        let lifted = lift_inquiry(&basic, None).expect("Failed to lift ACK");
        assert!(matches!(lifted, Response::CmdAck { socket } if socket == Some(ViscaSocket::S1)));

        // Test Completion lifting
        let basic = BasicResponse {
            source: CameraId::CAMERA_1,
            kind: BasicKind::Completion,
            socket: Some(ViscaSocket::S2),
            payload: Payload::new(&[]),
        };
        let lifted = lift_inquiry(&basic, None).expect("Failed to lift Completion");
        assert!(
            matches!(lifted, Response::Completion { socket } if socket == Some(ViscaSocket::S2))
        );

        // Test Error lifting
        let basic = BasicResponse {
            source: CameraId::CAMERA_1,
            kind: BasicKind::Error(0x02),
            socket: None,
            payload: Payload::new(&[]),
        };
        let lifted = lift_inquiry(&basic, None).expect("Failed to lift Error");
        assert!(matches!(lifted, Response::Error(_)));

        // Test DataReply without expected type
        let basic = BasicResponse {
            source: CameraId::CAMERA_1,
            kind: BasicKind::DataReply,
            socket: None,
            payload: Payload::new(&[0x02]),
        };
        let lifted = lift_inquiry(&basic, None).expect("Failed to lift DataReply");
        assert!(matches!(
            lifted,
            Response::Unknown { data, .. } if data == vec![0x02]
        ));

        // Test Power inquiry
        let basic = BasicResponse {
            source: CameraId::CAMERA_1,
            kind: BasicKind::DataReply,
            socket: None,
            payload: Payload::new(&[0x02]),
        };
        let lifted =
            lift_inquiry(&basic, Some(&InquiryKind::Power)).expect("Failed to lift Power inquiry");
        match lifted {
            Response::Inquiry(InquiryData::Power { on }) => assert!(on),
            _ => panic!("Expected Power inquiry response"),
        }
    }
}
