//! Unified VISCA response decoding and frame splitting.
//!
//! This module provides zero-cost, runtime-agnostic utilities for parsing VISCA
//! protocol responses and splitting frame buffers. It eliminates duplication between
//! async and sync paths while preserving socket awareness and type safety.

use crate::{
    command::{bytes::VISCA_TERMINATOR, response::payload::Payload},
    ViscaSocket,
};

/// Basic VISCA response kind.
///
/// This enum represents the fundamental response types in the VISCA protocol,
/// providing a minimal, zero-cost abstraction over raw frame bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasicKind {
    /// Acknowledgment - command accepted (90 4y FF).
    Ack,
    /// Command completed successfully (90 5y FF).
    Completion,
    /// Data reply from inquiry (90 50 ... FF).
    DataReply,
    /// Error response with code (90 6y zz FF).
    Error(u8),
    /// Network change message (90 38 FF).
    NetworkChange,
    /// Unknown response type.
    Unknown,
}

/// Basic response structure using borrowed slices.
///
/// This struct provides a zero-allocation view of a VISCA response frame,
/// extracting socket information and payload data without heap allocations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicResponse<'a> {
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
/// # Arguments
/// * `frame` - Complete VISCA frame including the 0xFF terminator
///
/// # Returns
/// * `Some(BasicResponse)` if the frame is valid
/// * `None` if the frame is malformed
pub fn decode_basic(frame: &[u8]) -> Option<BasicResponse<'_>> {
    // Minimum valid response is 3 bytes (e.g., 90 38 FF)
    if frame.len() < 3 {
        return None;
    }

    // Check for terminator
    if frame[frame.len() - 1] != VISCA_TERMINATOR {
        return None;
    }

    // Check first byte (should be 9x for replies)
    if (frame[0] & 0xF0) != 0x90 {
        return None;
    }

    // Parse based on second byte
    match frame[1] {
        // ACK (90 4y FF)
        byte if (byte & 0xF0) == 0x40 => {
            let socket_num = byte & 0x0F;
            let socket = ViscaSocket::from_protocol_byte(socket_num);
            Some(BasicResponse {
                kind: BasicKind::Ack,
                socket,
                payload: Payload::new(&[]),
            })
        }

        // Completion (90 5y FF) or Data Reply (90 50 ... FF with more than 3 bytes)
        byte if (byte & 0xF0) == 0x50 => {
            let socket_num = byte & 0x0F;

            // Special case: 90 50 with >3 bytes is data reply
            if socket_num == 0 && frame.len() > 3 {
                // Data reply: 90 50 <payload> FF
                let payload = Payload::new(&frame[2..frame.len() - 1]);
                Some(BasicResponse {
                    kind: BasicKind::DataReply,
                    socket: None,
                    payload,
                })
            } else {
                // Completion: 90 5y FF (including 90 50 FF for socket 0)
                let socket = ViscaSocket::from_protocol_byte(socket_num);
                Some(BasicResponse {
                    kind: BasicKind::Completion,
                    socket,
                    payload: Payload::new(&[]),
                })
            }
        }

        // Error (90 6y zz FF)
        byte if (byte & 0xF0) == 0x60 => {
            if frame.len() >= 4 {
                let socket_num = byte & 0x0F;
                let socket = ViscaSocket::from_protocol_byte(socket_num);
                let error_code = frame[2];
                Some(BasicResponse {
                    kind: BasicKind::Error(error_code),
                    socket,
                    payload: Payload::new(&[]),
                })
            } else {
                None
            }
        }

        // Network change (90 38 FF)
        0x38 => Some(BasicResponse {
            kind: BasicKind::NetworkChange,
            socket: None,
            payload: Payload::new(&[]),
        }),

        // Unknown
        _ => Some(BasicResponse {
            kind: BasicKind::Unknown,
            socket: None,
            payload: Payload::new(&frame[2..frame.len() - 1]),
        }),
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

    #[test]
    fn test_decode_ack() {
        let frame = vec![0x90, 0x41, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode ACK");
        assert_eq!(response.kind, BasicKind::Ack);
        assert_eq!(response.socket, Some(ViscaSocket::S1));
        assert_eq!(response.payload, Payload::new(&[]));

        let frame = vec![0x90, 0x42, VISCA_TERMINATOR];
        let response = decode_basic(&frame).expect("Failed to decode ACK");
        assert_eq!(response.kind, BasicKind::Ack);
        assert_eq!(response.socket, Some(ViscaSocket::S2));
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
    fn test_lift_inquiry() {
        // Test ACK lifting
        let basic = BasicResponse {
            kind: BasicKind::Ack,
            socket: Some(ViscaSocket::S1),
            payload: Payload::new(&[]),
        };
        let lifted = lift_inquiry(&basic, None).expect("Failed to lift ACK");
        assert!(matches!(lifted, Response::CmdAck { socket } if socket == Some(ViscaSocket::S1)));

        // Test Completion lifting
        let basic = BasicResponse {
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
            kind: BasicKind::Error(0x02),
            socket: None,
            payload: Payload::new(&[]),
        };
        let lifted = lift_inquiry(&basic, None).expect("Failed to lift Error");
        assert!(matches!(lifted, Response::Error(_)));

        // Test DataReply without expected type
        let basic = BasicResponse {
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
