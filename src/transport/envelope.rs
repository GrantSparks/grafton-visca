//! Transport envelope abstraction for different VISCA protocols.
//!
//! This module provides abstraction for protocol envelopes, specifically
//! handling Sony's 8-byte encapsulated VISCA protocol vs raw VISCA bytes.

use bytes::{Bytes, BytesMut};

use std::{
    borrow::Cow,
    sync::atomic::{AtomicU32, Ordering},
};

use crate::capabilities::ProtocolStyle;

/// Transport envelope that handles protocol-specific framing.
///
/// Different camera manufacturers use different framing approaches:
/// - Raw VISCA: Commands sent as-is (PtzOptics, generic cameras)  
/// - Sony Encapsulated: 8-byte header + VISCA payload (Sony cameras)
#[derive(Debug)]
pub struct TransportEnvelope {
    style: ProtocolStyle,
    sequence_counter: AtomicU32,
}

impl TransportEnvelope {
    /// Create a new transport envelope for the given protocol style.
    pub fn new(style: ProtocolStyle) -> Self {
        Self {
            style,
            sequence_counter: AtomicU32::new(0),
        }
    }

    /// Frame a VISCA command according to the protocol style.
    ///
    /// For raw VISCA, returns the command bytes unchanged.
    /// For Sony encapsulated, wraps with 8-byte header.
    pub fn frame_command(&self, visca_bytes: &[u8], is_inquiry: bool) -> Bytes {
        match self.style {
            ProtocolStyle::RawVisca => Bytes::copy_from_slice(visca_bytes),
            ProtocolStyle::SonyEncapsulated { use_sequence } => {
                self.sony_encapsulate(visca_bytes, is_inquiry, use_sequence)
            }
        }
    }

    /// Parse a response and extract the VISCA payload.
    ///
    /// For raw VISCA, returns the bytes unchanged.
    /// For Sony encapsulated, extracts payload from 8-byte header.
    pub fn extract_response(&self, framed_bytes: &[u8]) -> Result<Bytes, crate::Error> {
        match self.style {
            ProtocolStyle::RawVisca => Ok(Bytes::copy_from_slice(framed_bytes)),
            ProtocolStyle::SonyEncapsulated { .. } => self.sony_extract_payload(framed_bytes),
        }
    }

    /// Get the next sequence number (if using sequence tracking).
    fn next_sequence(&self) -> u32 {
        self.sequence_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Encapsulate VISCA command with Sony 8-byte header.
    ///
    /// Header format:
    /// ```text
    /// ┌─────────────── 8 bytes ───────────────┐┌──── VISCA frame ─┐  
    /// │ Payload-Type │ Length │ Sequence-No. ││ 8x … payload … FF │  
    /// └──────────────────────────────────────┘└─────────────────────┘
    /// ```
    fn sony_encapsulate(&self, visca_bytes: &[u8], is_inquiry: bool, use_sequence: bool) -> Bytes {
        let payload_type = if is_inquiry {
            SonyPayloadType::Inquiry
        } else {
            SonyPayloadType::Command
        };

        let length = visca_bytes.len() as u16;
        let sequence = if use_sequence {
            self.next_sequence()
        } else {
            0
        };

        let mut envelope = BytesMut::with_capacity(8 + visca_bytes.len());

        // Payload Type (2 bytes)
        envelope.extend_from_slice(&payload_type.to_bytes());

        // Length (2 bytes, big-endian)
        envelope.extend_from_slice(&length.to_be_bytes());

        // Sequence Number (4 bytes, big-endian)
        envelope.extend_from_slice(&sequence.to_be_bytes());

        // VISCA payload
        envelope.extend_from_slice(visca_bytes);

        envelope.freeze()
    }

    /// Extract VISCA payload from Sony encapsulated response.
    fn sony_extract_payload(&self, framed_bytes: &[u8]) -> Result<Bytes, crate::Error> {
        if framed_bytes.len() < 8 {
            return Err(crate::Error::ParseError(Cow::Borrowed(
                "Sony response too short for header",
            )));
        }

        // Parse header
        let payload_type_bytes = [framed_bytes[0], framed_bytes[1]];
        let length = u16::from_be_bytes([framed_bytes[2], framed_bytes[3]]);
        let _sequence = u32::from_be_bytes([
            framed_bytes[4],
            framed_bytes[5],
            framed_bytes[6],
            framed_bytes[7],
        ]);

        // Validate payload type
        match SonyPayloadType::from_bytes(payload_type_bytes) {
            Some(SonyPayloadType::Reply) => {
                // Expected for camera responses
            }
            Some(other) => {
                log::warn!("Unexpected Sony payload type in response: {other:?}");
            }
            None => {
                return Err(crate::Error::ParseError(Cow::Owned(format!(
                    "Invalid Sony payload type: {payload_type_bytes:02X?}"
                ))));
            }
        }

        // Validate length
        let expected_payload_len = framed_bytes.len() - 8;
        if length as usize != expected_payload_len {
            return Err(crate::Error::ParseError(Cow::Owned(format!(
                "Sony header length mismatch: header says {length}, actual payload is {expected_payload_len}"
            ))));
        }

        // Extract VISCA payload - use slice to avoid allocation
        Ok(Bytes::copy_from_slice(&framed_bytes[8..]))
    }
}

/// Sony payload types for the 8-byte header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SonyPayloadType {
    /// Command packet (0x01 0x00)
    Command,
    /// Inquiry packet (0x01 0x10)  
    Inquiry,
    /// Reply packet (0x01 0x11)
    Reply,
}

impl SonyPayloadType {
    /// Convert payload type to byte representation.
    fn to_bytes(self) -> [u8; 2] {
        match self {
            SonyPayloadType::Command => [0x01, 0x00],
            SonyPayloadType::Inquiry => [0x01, 0x10],
            SonyPayloadType::Reply => [0x01, 0x11],
        }
    }

    /// Parse payload type from bytes.
    fn from_bytes(bytes: [u8; 2]) -> Option<Self> {
        match bytes {
            [0x01, 0x00] => Some(SonyPayloadType::Command),
            [0x01, 0x10] => Some(SonyPayloadType::Inquiry),
            [0x01, 0x11] => Some(SonyPayloadType::Reply),
            _ => None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::command::const_encoding::VISCA_TERMINATOR;

    #[test]
    fn test_raw_visca_passthrough() {
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]; // Power On

        let framed = envelope.frame_command(&visca_cmd, false);
        assert_eq!(&framed[..], &visca_cmd[..]);

        let extracted = envelope
            .extract_response(&visca_cmd)
            .expect("raw visca should extract unchanged");
        assert_eq!(&extracted[..], &visca_cmd[..]);
    }

    #[test]
    fn test_sony_encapsulation_command() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
            use_sequence: false,
        });
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]; // Power On

        let framed = envelope.frame_command(&visca_cmd, false);

        // Should be 8-byte header + 6-byte VISCA command = 14 bytes
        assert_eq!(framed.len(), 14);

        // Check header
        assert_eq!(&framed[0..2], &[0x01, 0x00]); // Command payload type
        assert_eq!(&framed[2..4], &(6u16).to_be_bytes()); // Length = 6
        assert_eq!(&framed[4..8], &0u32.to_be_bytes()); // Sequence = 0

        // Check VISCA payload
        assert_eq!(&framed[8..], &visca_cmd[..]);
    }

    #[test]
    fn test_sony_encapsulation_inquiry() {
        let envelope =
            TransportEnvelope::new(ProtocolStyle::SonyEncapsulated { use_sequence: true });
        let visca_inquiry = vec![0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR]; // Power Status Inquiry

        let framed = envelope.frame_command(&visca_inquiry, true);

        // Should be 8-byte header + 5-byte VISCA inquiry = 13 bytes
        assert_eq!(framed.len(), 13);

        // Check header
        assert_eq!(&framed[0..2], &[0x01, 0x10]); // Inquiry payload type
        assert_eq!(&framed[2..4], &(5u16).to_be_bytes()); // Length = 5
        assert_eq!(&framed[4..8], &0u32.to_be_bytes()); // Sequence = 0 (first call)

        // Check VISCA payload
        assert_eq!(&framed[8..], &visca_inquiry[..]);
    }

    #[test]
    fn test_sony_response_extraction() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
            use_sequence: false,
        });

        // Create a mock Sony response: 8-byte header + VISCA ACK
        let visca_ack = vec![0x90, 0x41, VISCA_TERMINATOR]; // ACK for socket 1
        let mut response = Vec::new();
        response.extend_from_slice(&[0x01, 0x11]); // Reply payload type
        response.extend_from_slice(&(3u16).to_be_bytes()); // Length = 3
        response.extend_from_slice(&42u32.to_be_bytes()); // Sequence = 42
        response.extend_from_slice(&visca_ack); // VISCA payload

        let extracted = envelope
            .extract_response(&response)
            .expect("valid sony response should extract");
        assert_eq!(&extracted[..], &visca_ack[..]);
    }

    #[test]
    fn test_sony_sequence_increment() {
        let envelope =
            TransportEnvelope::new(ProtocolStyle::SonyEncapsulated { use_sequence: true });
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

        let framed1 = envelope.frame_command(&visca_cmd, false);
        let framed2 = envelope.frame_command(&visca_cmd, false);

        // Extract sequence numbers
        let seq1 = u32::from_be_bytes([framed1[4], framed1[5], framed1[6], framed1[7]]);
        let seq2 = u32::from_be_bytes([framed2[4], framed2[5], framed2[6], framed2[7]]);

        assert_eq!(seq1, 0);
        assert_eq!(seq2, 1);
    }

    #[test]
    fn test_invalid_sony_response() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
            use_sequence: false,
        });

        // ViscaResponse too short
        let short_response = vec![0x01, 0x11, 0x00];
        assert!(envelope.extract_response(&short_response).is_err());

        // Invalid payload type
        let mut invalid_response = vec![0xFF, VISCA_TERMINATOR]; // Invalid payload type
        invalid_response.extend_from_slice(&(3u16).to_be_bytes());
        invalid_response.extend_from_slice(&0u32.to_be_bytes());
        invalid_response.extend_from_slice(&[0x90, 0x41, VISCA_TERMINATOR]);
        assert!(envelope.extract_response(&invalid_response).is_err());
    }
}
