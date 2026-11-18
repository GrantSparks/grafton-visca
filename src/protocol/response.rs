//! Unified VISCA response decoding and frame splitting.
//!
//! This module provides zero-cost, runtime-agnostic utilities for parsing VISCA
//! protocol responses and splitting frame buffers. It eliminates duplication between
//! async and sync paths while preserving socket awareness and type safety.

use crate::{
    camera_id::CameraId,
    command::{bytes::VISCA_TERMINATOR, response::Payload},
    wire::WireValue,
    ViscaSocket,
};

/// Bytes for the reply address, reply header and terminator.
pub(crate) const REPLY_FRAME_OVERHEAD: usize = 3;

/// A decoded VISCA camera error, shared by routing, retries and diagnostics.
///
/// A named `NoSocket` answers a cancellation; without a socket it may reject
/// a request before it starts. Consumers decide socket ownership first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CameraErrorCode {
    MessageLength,
    Syntax,
    BufferFull,
    Cancelled,
    NoSocket,
    NotExecutable,
    Other(u8),
}

impl CameraErrorCode {
    pub(crate) const fn from_byte(byte: u8) -> Self {
        match byte {
            0x01 => Self::MessageLength,
            0x02 => Self::Syntax,
            0x03 => Self::BufferFull,
            0x04 => Self::Cancelled,
            0x05 => Self::NoSocket,
            0x41 => Self::NotExecutable,
            other => Self::Other(other),
        }
    }

    // Keep the public Error::from_code constructor usable in const contexts.
    pub(crate) const fn into_error(self) -> crate::Error {
        match self {
            Self::MessageLength => crate::Error::MessageLengthError,
            Self::Syntax => crate::Error::SyntaxError,
            Self::BufferFull => crate::Error::CommandBufferFull,
            Self::Cancelled => crate::Error::CommandCancelled,
            Self::NoSocket => crate::Error::NoSocket,
            Self::NotExecutable => crate::Error::CommandNotExecutable,
            Self::Other(code) => crate::Error::Unknown { code },
        }
    }
}

#[cfg(any(feature = "blocking", feature = "async"))]
impl CameraErrorCode {
    /// Only a cancellation can own these errors when they name a socket.
    pub(crate) const fn answers_named_cancellation(self) -> bool {
        match self {
            Self::Cancelled | Self::NoSocket => true,
            Self::MessageLength
            | Self::Syntax
            | Self::BufferFull
            | Self::NotExecutable
            | Self::Other(_) => false,
        }
    }

    pub(crate) const fn confirms_cancellation(self) -> bool {
        match self {
            Self::Cancelled => true,
            Self::MessageLength
            | Self::Syntax
            | Self::BufferFull
            | Self::NoSocket
            | Self::NotExecutable
            | Self::Other(_) => false,
        }
    }

    /// Rejection evidence is conclusive only before the request's ACK.
    pub(crate) const fn refuses_before_start(self) -> bool {
        match self {
            Self::MessageLength
            | Self::Syntax
            | Self::BufferFull
            | Self::NoSocket
            | Self::NotExecutable => true,
            Self::Cancelled | Self::Other(_) => false,
        }
    }

    /// Camera backpressure is distinct from a successful cancellation answer.
    pub(crate) const fn is_busy(self) -> bool {
        match self {
            Self::BufferFull | Self::NoSocket | Self::NotExecutable => true,
            Self::MessageLength | Self::Syntax | Self::Cancelled | Self::Other(_) => false,
        }
    }
}

impl From<u8> for CameraErrorCode {
    fn from(byte: u8) -> Self {
        Self::from_byte(byte)
    }
}

impl From<CameraErrorCode> for crate::Error {
    fn from(code: CameraErrorCode) -> Self {
        code.into_error()
    }
}

/// Basic VISCA response kind.
///
/// This enum represents the fundamental response types in the VISCA protocol,
/// providing a minimal, zero-cost abstraction over raw frame bytes. Each
/// variant carries only the fields its wire grammar permits.
///
/// The `z0` in each frame shape below is the reply address byte: `z = 8 + n`
/// for the camera at address *n*, so camera 1 answers `90 ...` and camera 7
/// answers `F0 ...`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasicKind<'a> {
    /// Acknowledgment - command accepted (z0 4y FF).
    Ack { socket: Option<ViscaSocket> },
    /// Command completed successfully (z0 5y FF).
    Completion { socket: Option<ViscaSocket> },
    /// Data reply from inquiry (z0 50 ... FF).
    DataReply(Payload<'a>),
    /// Error response with code (z0 6y zz FF).
    Error {
        socket: Option<ViscaSocket>,
        code: CameraErrorCode,
    },
    /// Network change message (z0 38 FF).
    NetworkChange,
    /// Unknown response type.
    Unknown(Payload<'a>),
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
    /// [`crate::types::CameraId::CAMERA_1`](crate::types::CameraId::CAMERA_1) (`0x90`); on a serial
    /// daisy chain it identifies which of the up to seven addressed devices
    /// answered.
    pub source: CameraId,
    /// The type of response.
    pub kind: BasicKind<'a>,
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
    if frame.len() < REPLY_FRAME_OVERHEAD {
        return None;
    }

    // Check for terminator
    if frame[frame.len() - 1] != VISCA_TERMINATOR {
        return None;
    }

    let kind = match frame[1] {
        // ACKs are fixed-format replies, including socketless compatibility.
        byte if (byte & 0xf0) == 0x40 => {
            if frame.len() != REPLY_FRAME_OVERHEAD {
                return None;
            }
            BasicKind::Ack {
                socket: decode_fixed_socket(byte & 0x0f)?,
            }
        }
        byte if (byte & 0xf0) == 0x50 => {
            let socket = byte & 0x0f;
            if socket == 0 && frame.len() > REPLY_FRAME_OVERHEAD {
                BasicKind::DataReply(Payload::new(&frame[2..frame.len() - 1]))
            } else if frame.len() == REPLY_FRAME_OVERHEAD {
                BasicKind::Completion {
                    socket: decode_fixed_socket(socket)?,
                }
            } else {
                return None;
            }
        }
        byte if (byte & 0xf0) == 0x60 => {
            if frame.len() != REPLY_FRAME_OVERHEAD + 1 {
                return None;
            }
            BasicKind::Error {
                socket: decode_fixed_socket(byte & 0x0f)?,
                code: CameraErrorCode::from(frame[2]),
            }
        }
        0x38 => {
            if frame.len() != REPLY_FRAME_OVERHEAD {
                return None;
            }
            BasicKind::NetworkChange
        }
        _ => BasicKind::Unknown(Payload::new(&frame[2..frame.len() - 1])),
    };
    Some(BasicResponse { source, kind })
}

/// Decode the socket nibble used by a fixed-format response.
///
/// VISCA uses nibble `0` on the wire for a socketless compatibility response,
/// while the only numbered command sockets are `1` and `2`. The checked
/// [`crate::wire::WireValue`] decoder rejects both socketless and invalid
/// values, so the outer `Option` here preserves
/// that distinction for strict fixed-frame parsing: `Some(None)` is valid
/// socketless evidence and `None` is a malformed socket nibble.
///
/// This is the one socket-nibble grammar: the framer's incomplete-prefix
/// classifier uses it for completion and error prefixes, so a prefix and the
/// complete frame it becomes can never disagree about the socket. An ACK is
/// the one deliberate asymmetry: its incomplete prefix is classified by the
/// message nibble alone, while the complete ACK frame is checked here.
pub(crate) fn decode_fixed_socket(nibble: u8) -> Option<Option<ViscaSocket>> {
    match nibble {
        0 => Some(None),
        1 | 2 => ViscaSocket::from_wire(nibble).ok().map(Some),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    use crate::command::{
        pan_tilt_wire::PanTiltFraming,
        response::{lift_inquiry, InquiryKind, Response},
        InquiryData,
    };

    /// Reply address byte for the camera at chain address `id`.
    fn reply_address(id: u8) -> u8 {
        (8 + id) << 4
    }

    #[test]
    fn decoder_carries_the_closed_code_and_public_constructor_stays_const() {
        const NO_SOCKET: crate::Error = crate::Error::from_code(0x05);
        assert!(matches!(NO_SOCKET, crate::Error::NoSocket));
        for (byte, code) in [
            (0x01, CameraErrorCode::MessageLength),
            (0x02, CameraErrorCode::Syntax),
            (0x03, CameraErrorCode::BufferFull),
            (0x04, CameraErrorCode::Cancelled),
            (0x05, CameraErrorCode::NoSocket),
            (0x41, CameraErrorCode::NotExecutable),
            (0x7f, CameraErrorCode::Other(0x7f)),
        ] {
            assert_eq!(
                decode_basic(&[0x90, 0x61, byte, VISCA_TERMINATOR])
                    .unwrap()
                    .kind,
                BasicKind::Error {
                    socket: Some(ViscaSocket::S1),
                    code
                },
            );
            assert_eq!(
                crate::Error::from(code).to_string(),
                crate::Error::from_code(byte).to_string()
            );
        }
    }

    #[test]
    fn every_camera_code_preserves_decoding_and_public_error() {
        for code in 0..=u8::MAX {
            for socket in [None, Some(ViscaSocket::S1), Some(ViscaSocket::S2)] {
                let nibble = socket.map_or(0, |socket| socket.to_wire());
                let frame = [0x90, 0x60 | nibble, code, VISCA_TERMINATOR];
                assert_eq!(
                    decode_basic(&frame).unwrap().kind,
                    BasicKind::Error {
                        socket,
                        code: code.into()
                    }
                );
            }
            let expected = match code {
                1 => crate::Error::MessageLengthError,
                2 => crate::Error::SyntaxError,
                3 => crate::Error::CommandBufferFull,
                4 => crate::Error::CommandCancelled,
                5 => crate::Error::NoSocket,
                0x41 => crate::Error::CommandNotExecutable,
                _ => crate::Error::Unknown { code },
            };
            assert_eq!(
                std::mem::discriminant(&crate::Error::from_code(code)),
                std::mem::discriminant(&expected)
            );
            assert_eq!(
                crate::Error::from_code(code).to_string(),
                expected.to_string()
            );
        }
    }

    #[test]
    fn test_decode_ack() {
        let frame = vec![0x90, 0x41, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode ACK");
        assert_eq!(
            response.kind,
            BasicKind::Ack {
                socket: Some(ViscaSocket::S1)
            }
        );
        assert_eq!(response.source, CameraId::CAMERA_1);

        let frame = vec![0x90, 0x42, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode ACK");
        assert_eq!(
            response.kind,
            BasicKind::Ack {
                socket: Some(ViscaSocket::S2)
            }
        );
    }

    #[test]
    fn test_decode_every_chain_source_address() {
        for id in 1..=7u8 {
            let camera = CameraId::new(id).expect("1-7 are valid camera IDs");
            let z0 = reply_address(id);

            let frame = [z0, 0x41, VISCA_TERMINATOR];
            let ack = decode_basic(&frame).unwrap_or_else(|| panic!("camera {id} ACK must decode"));
            assert_eq!(
                ack.kind,
                BasicKind::Ack {
                    socket: Some(ViscaSocket::S1)
                }
            );
            assert_eq!(ack.source, camera);

            let frame = [z0, 0x52, VISCA_TERMINATOR];
            let completion = decode_basic(&frame)
                .unwrap_or_else(|| panic!("camera {id} completion must decode"));
            assert_eq!(
                completion.kind,
                BasicKind::Completion {
                    socket: Some(ViscaSocket::S2)
                }
            );
            assert_eq!(completion.source, camera);

            let frame = [z0, 0x50, 0x02, VISCA_TERMINATOR];
            let data = decode_basic(&frame)
                .unwrap_or_else(|| panic!("camera {id} data reply must decode"));
            assert_eq!(data.kind, BasicKind::DataReply(Payload::new(&[0x02])));
            assert_eq!(data.source, camera);

            let frame = [z0, 0x61, 0x41, VISCA_TERMINATOR];
            let error =
                decode_basic(&frame).unwrap_or_else(|| panic!("camera {id} error must decode"));
            assert_eq!(
                error.kind,
                BasicKind::Error {
                    socket: Some(ViscaSocket::S1),
                    code: 0x41.into()
                }
            );
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
        assert_eq!(
            response.kind,
            BasicKind::Completion {
                socket: Some(ViscaSocket::S1)
            }
        );

        let frame = vec![0x90, 0x52, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode Completion");
        assert_eq!(
            response.kind,
            BasicKind::Completion {
                socket: Some(ViscaSocket::S2)
            }
        );
    }

    #[test]
    fn test_decode_data_reply() {
        let frame = vec![0x90, 0x50, 0x02, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode DataReply");
        assert_eq!(response.kind, BasicKind::DataReply(Payload::new(&[0x02])));

        let frame = vec![0x90, 0x50, 0x00, 0x01, 0x02, 0x03, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode DataReply");
        assert_eq!(
            response.kind,
            BasicKind::DataReply(Payload::new(&[0x00, 0x01, 0x02, 0x03]))
        );
    }

    #[test]
    fn test_decode_error() {
        let frame = vec![0x90, 0x60, 0x02, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode Error");
        assert_eq!(
            response.kind,
            BasicKind::Error {
                socket: None,
                code: 0x02.into()
            }
        );

        let frame = vec![0x90, 0x61, 0x03, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode Error");
        assert_eq!(
            response.kind,
            BasicKind::Error {
                socket: Some(ViscaSocket::S1),
                code: 0x03.into()
            }
        );
    }

    #[test]
    fn test_decode_network_change() {
        let frame = vec![0x90, 0x38, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode NetworkChange");
        assert_eq!(response.kind, BasicKind::NetworkChange);
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
        assert_eq!(response.kind, BasicKind::Ack { socket: None });

        // Socket 0 Completion is valid but socket is None (when len == 3)
        let response = decode_basic(&[0x90, 0x50, VISCA_TERMINATOR]).expect("Should decode");
        assert_eq!(response.kind, BasicKind::Completion { socket: None });
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
            assert_eq!(ack.kind, BasicKind::Ack { socket: expected });

            let completion_frame = [0x90, 0x50 | nibble, VISCA_TERMINATOR];
            let completion = decode_basic(&completion_frame)
                .expect("valid completion socket nibble must decode");
            assert_eq!(completion.kind, BasicKind::Completion { socket: expected });

            let error_frame = [0x90, 0x60 | nibble, 0x02, VISCA_TERMINATOR];
            let error = decode_basic(&error_frame).expect("valid error socket nibble must decode");
            assert_eq!(
                error.kind,
                BasicKind::Error {
                    socket: expected,
                    code: 0x02.into()
                }
            );
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
        assert_eq!(
            response.kind,
            BasicKind::DataReply(Payload::new(&[0x02, 0x03]))
        );
    }

    #[test]
    fn test_lift_inquiry() {
        // Test ACK lifting
        let basic = BasicResponse {
            source: CameraId::CAMERA_1,
            kind: BasicKind::Ack {
                socket: Some(ViscaSocket::S1),
            },
        };
        let lifted =
            lift_inquiry(&basic, None, PanTiltFraming::STANDARD).expect("Failed to lift ACK");
        assert!(matches!(lifted, Response::CmdAck { socket } if socket == Some(ViscaSocket::S1)));

        // Test Completion lifting
        let basic = BasicResponse {
            source: CameraId::CAMERA_1,
            kind: BasicKind::Completion {
                socket: Some(ViscaSocket::S2),
            },
        };
        let lifted = lift_inquiry(&basic, None, PanTiltFraming::STANDARD)
            .expect("Failed to lift Completion");
        assert!(
            matches!(lifted, Response::Completion { socket } if socket == Some(ViscaSocket::S2))
        );

        // Test Error lifting
        let basic = BasicResponse {
            source: CameraId::CAMERA_1,
            kind: BasicKind::Error {
                socket: None,
                code: 0x02.into(),
            },
        };
        let lifted =
            lift_inquiry(&basic, None, PanTiltFraming::STANDARD).expect("Failed to lift Error");
        assert!(matches!(lifted, Response::Error(_)));

        // Test DataReply without expected type
        let basic = BasicResponse {
            source: CameraId::CAMERA_1,
            kind: BasicKind::DataReply(Payload::new(&[0x02])),
        };
        let lifted =
            lift_inquiry(&basic, None, PanTiltFraming::STANDARD).expect("Failed to lift DataReply");
        assert!(matches!(
            lifted,
            Response::Unknown { data, .. } if data == vec![0x02]
        ));

        // Test Power inquiry
        let basic = BasicResponse {
            source: CameraId::CAMERA_1,
            kind: BasicKind::DataReply(Payload::new(&[0x02])),
        };
        let lifted = lift_inquiry(&basic, Some(&InquiryKind::Power), PanTiltFraming::STANDARD)
            .expect("Failed to lift Power inquiry");
        match lifted {
            Response::Inquiry(InquiryData::Power { status }) => assert!(status.is_on()),
            _ => panic!("Expected Power inquiry response"),
        }
    }
}
