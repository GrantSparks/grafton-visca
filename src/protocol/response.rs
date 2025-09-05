//! Unified VISCA response decoding and frame splitting.
//!
//! This module provides zero-cost, runtime-agnostic utilities for parsing VISCA
//! protocol responses and splitting frame buffers. It eliminates duplication between
//! async and sync paths while preserving socket awareness and type safety.

use crate::{command::bytes::VISCA_TERMINATOR, command::response::payload::Payload, ViscaSocket};

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

/// Zero-copy frame iterator for VISCA frame splitting.
///
/// This iterator finds complete frames (terminated by 0xFF) in a buffer
/// without allocating memory for each frame.
#[derive(Debug)]
pub struct FrameIter<'a> {
    buf: &'a [u8],
    cursor: usize,
}

impl<'a> FrameIter<'a> {
    /// Create a new frame iterator over a buffer.
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, cursor: 0 }
    }

    /// Get the remaining unparsed buffer.
    ///
    /// This returns any incomplete frame data that hasn't been consumed yet.
    pub fn remainder(&self) -> &'a [u8] {
        &self.buf[self.cursor..]
    }
}

impl<'a> Iterator for FrameIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.buf.len() {
            return None;
        }

        // Find the next terminator
        if let Some(pos) = self.buf[self.cursor..]
            .iter()
            .position(|&b| b == VISCA_TERMINATOR)
        {
            let start = self.cursor;
            let end = self.cursor + pos + 1; // Include the terminator
            self.cursor = end;
            Some(&self.buf[start..end])
        } else {
            None
        }
    }
}

/// Parse multiple frames from a buffer (compatibility function).
///
/// This function provides backward compatibility with the existing API
/// while using the new zero-copy FrameIter internally.
///
/// # Arguments
/// * `buffer` - Buffer potentially containing multiple VISCA frames
///
/// # Returns
/// * Tuple of (complete_frames, remaining_bytes)
pub fn parse_frames(buffer: &[u8]) -> (Vec<Vec<u8>>, Vec<u8>) {
    let iter = FrameIter::new(buffer);
    let mut frames = Vec::new();

    for frame in iter {
        frames.push(frame.to_vec());
    }

    let remainder = if !frames.is_empty() {
        let last_end = frames.iter().map(|f| f.len()).sum::<usize>();
        buffer[last_end..].to_vec()
    } else {
        buffer.to_vec()
    };

    (frames, remainder)
}

/// Find the next complete frame in a buffer (compatibility function).
///
/// This function provides backward compatibility with the existing API.
///
/// # Arguments
/// * `buffer` - Buffer to search for a complete frame
///
/// # Returns
/// * `Some((frame, remaining))` if a complete frame is found
/// * `None` if no complete frame is found
pub fn find_next_frame(buffer: &[u8]) -> Option<(Vec<u8>, &[u8])> {
    if let Some(pos) = buffer.iter().position(|&b| b == VISCA_TERMINATOR) {
        let frame = buffer[..=pos].to_vec();
        let remaining = &buffer[pos + 1..];
        Some((frame, remaining))
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    use crate::command::response::{lift_inquiry, ViscaResponse, ViscaResponseType};
    use crate::command::InquiryResponse;

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
    fn test_frame_iter() {
        let buffer = vec![
            0x90,
            0x41,
            VISCA_TERMINATOR,
            0x90,
            0x51,
            VISCA_TERMINATOR,
            0x90,
            0x50,
            0x02,
            VISCA_TERMINATOR,
            0x90,
            0x60, // Incomplete frame
        ];

        let mut iter = FrameIter::new(&buffer);

        let frame1 = iter.next().expect("Expected first frame");
        assert_eq!(frame1, &[0x90, 0x41, VISCA_TERMINATOR]);

        let frame2 = iter.next().expect("Expected second frame");
        assert_eq!(frame2, &[0x90, 0x51, VISCA_TERMINATOR]);

        let frame3 = iter.next().expect("Expected third frame");
        assert_eq!(frame3, &[0x90, 0x50, 0x02, VISCA_TERMINATOR]);

        assert!(iter.next().is_none());
        assert_eq!(iter.remainder(), &[0x90, 0x60]);
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
        assert!(
            matches!(lifted, ViscaResponse::CmdAck { socket } if socket == Some(ViscaSocket::S1))
        );

        // Test Completion lifting
        let basic = BasicResponse {
            kind: BasicKind::Completion,
            socket: Some(ViscaSocket::S2),
            payload: Payload::new(&[]),
        };
        let lifted = lift_inquiry(&basic, None).expect("Failed to lift Completion");
        assert!(
            matches!(lifted, ViscaResponse::Completion { socket } if socket == Some(ViscaSocket::S2))
        );

        // Test Error lifting
        let basic = BasicResponse {
            kind: BasicKind::Error(0x02),
            socket: None,
            payload: Payload::new(&[]),
        };
        let lifted = lift_inquiry(&basic, None).expect("Failed to lift Error");
        assert!(matches!(lifted, ViscaResponse::Error(_)));

        // Test DataReply without expected type
        let basic = BasicResponse {
            kind: BasicKind::DataReply,
            socket: None,
            payload: Payload::new(&[0x02]),
        };
        let lifted = lift_inquiry(&basic, None).expect("Failed to lift DataReply");
        assert!(matches!(
            lifted,
            ViscaResponse::Unknown { data, .. } if data == vec![0x02]
        ));

        // Test Power inquiry
        let basic = BasicResponse {
            kind: BasicKind::DataReply,
            socket: None,
            payload: Payload::new(&[0x02]),
        };
        let lifted = lift_inquiry(&basic, Some(&ViscaResponseType::Power))
            .expect("Failed to lift Power inquiry");
        match lifted {
            ViscaResponse::Inquiry(InquiryResponse::Power { on }) => assert!(on),
            _ => panic!("Expected Power inquiry response"),
        }
    }

    #[test]
    fn test_parse_frames_compat() {
        let buffer = vec![
            0x90,
            0x41,
            VISCA_TERMINATOR,
            0x90,
            0x51,
            VISCA_TERMINATOR,
            0x90,
            0x50,
        ];
        let (frames, remaining) = parse_frames(&buffer);

        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], vec![0x90, 0x41, VISCA_TERMINATOR]);
        assert_eq!(frames[1], vec![0x90, 0x51, VISCA_TERMINATOR]);
        assert_eq!(remaining, vec![0x90, 0x50]);
    }
}
