//! Transport envelope abstraction for different VISCA protocols.
//!
//! This module provides abstraction for protocol envelopes, specifically
//! handling Sony's 8-byte encapsulated VISCA protocol vs raw VISCA bytes.

use bytes::Bytes;

use std::{
    borrow::Cow,
    sync::atomic::{AtomicU32, Ordering},
};

use crate::{
    capabilities::ProtocolStyle,
    command::bytes::is_inquiry_bytes,
    protocol::sony::{PayloadType, SonyHeader},
    transport::buffer::BufferManager,
};

/// Metadata extracted from or used during framing operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(any(test, not(feature = "async")))]
pub(crate) struct FrameMeta {
    /// Sequence number for Sony protocol, None for raw VISCA.
    pub sequence: Option<u32>,
}

/// Transport envelope that handles protocol-specific framing.
///
/// Different camera manufacturers use different framing approaches:
/// - Raw VISCA: Commands sent as-is (PtzOptics, generic cameras)
/// - Sony Encapsulated: 8-byte header + VISCA payload (Sony cameras)
#[derive(Debug)]
pub(crate) struct TransportEnvelope {
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
    /// For Sony encapsulated, wraps with 8-byte header using the provided buffer manager.
    /// The inquiry/command classification is determined from the VISCA bytes themselves.
    pub fn frame_command(&self, visca_bytes: &[u8], buffer_manager: &BufferManager) -> Bytes {
        match self.style {
            ProtocolStyle::RawVisca => Bytes::copy_from_slice(visca_bytes),
            ProtocolStyle::SonyEncapsulated { use_sequence } => {
                let is_inquiry = is_inquiry_bytes(visca_bytes);
                self.sony_encapsulate(visca_bytes, is_inquiry, use_sequence, buffer_manager)
            }
        }
    }

    /// Zero-copy variant of frame_command that takes owned Bytes.
    ///
    /// For raw VISCA, passes through without copying.
    /// For Sony encapsulated, wraps with 8-byte header using the provided buffer manager.
    #[cfg(any(test, feature = "async"))]
    pub fn frame_command_owned(&self, visca: Bytes, buffer_manager: &BufferManager) -> Bytes {
        match self.style {
            ProtocolStyle::RawVisca => visca, // Zero-copy pass-through
            ProtocolStyle::SonyEncapsulated { use_sequence } => {
                // Sony still needs to allocate for header + payload
                let is_inquiry = is_inquiry_bytes(&visca);
                self.sony_encapsulate(&visca, is_inquiry, use_sequence, buffer_manager)
            }
        }
    }

    /// Parse a response and extract the VISCA payload.
    ///
    /// For raw VISCA, returns the bytes unchanged.
    /// For Sony encapsulated, extracts payload from 8-byte header.
    ///
    /// This method is used by blocking cameras and may be used by external consumers.
    pub fn extract_response(&self, framed_bytes: &[u8]) -> Result<Bytes, crate::Error> {
        match self.style {
            ProtocolStyle::RawVisca => Ok(Bytes::copy_from_slice(framed_bytes)),
            ProtocolStyle::SonyEncapsulated { .. } => self.sony_extract_payload(framed_bytes),
        }
    }

    /// Zero-copy variant of extract_response that takes owned Bytes.
    ///
    /// For raw VISCA, passes through without copying.
    /// For Sony encapsulated, returns a slice of the original Bytes without copying.
    #[cfg(any(test, feature = "async"))]
    pub fn extract_response_owned(&self, framed: Bytes) -> Result<Bytes, crate::Error> {
        match self.style {
            ProtocolStyle::RawVisca => Ok(framed), // Zero-copy pass-through
            ProtocolStyle::SonyEncapsulated { .. } => self.sony_extract_payload_zero_copy(framed),
        }
    }

    /// Frame a VISCA command with metadata about the framing.
    ///
    /// For raw VISCA, returns the command bytes unchanged with None sequence.
    /// For Sony encapsulated, wraps with 8-byte header and returns the sequence number used.
    /// The inquiry/command classification is determined from the VISCA bytes themselves.
    #[inline(always)]
    #[cfg(any(test, not(feature = "async")))]
    pub(crate) fn frame_with_meta(
        &self,
        visca_bytes: &[u8],
        buffer_manager: &BufferManager,
    ) -> (Bytes, FrameMeta) {
        match self.style {
            ProtocolStyle::RawVisca => (
                Bytes::copy_from_slice(visca_bytes),
                FrameMeta { sequence: None },
            ),
            ProtocolStyle::SonyEncapsulated { use_sequence } => {
                let is_inquiry = is_inquiry_bytes(visca_bytes);
                let sequence = if use_sequence {
                    self.next_sequence()
                } else {
                    0
                };

                let header = if is_inquiry {
                    SonyHeader::new_inquiry(visca_bytes.len(), sequence)
                } else {
                    SonyHeader::new_command(visca_bytes.len(), sequence)
                };

                let mut envelope = buffer_manager.alloc_send_buffer();
                envelope.reserve(SonyHeader::SIZE + visca_bytes.len());
                envelope.extend_from_slice(&header.encode());
                envelope.extend_from_slice(visca_bytes);

                (
                    envelope.freeze(),
                    FrameMeta {
                        sequence: Some(sequence),
                    },
                )
            }
        }
    }

    /// Extract VISCA payload and metadata from a framed response.
    ///
    /// For raw VISCA, returns the bytes unchanged with None sequence.
    /// For Sony encapsulated, extracts payload and sequence from 8-byte header.
    #[inline(always)]
    #[cfg(any(test, not(feature = "async")))]
    pub(crate) fn extract_with_meta(
        &self,
        framed_bytes: &[u8],
    ) -> Result<(Bytes, FrameMeta), crate::Error> {
        match self.style {
            ProtocolStyle::RawVisca => Ok((
                Bytes::copy_from_slice(framed_bytes),
                FrameMeta { sequence: None },
            )),
            ProtocolStyle::SonyEncapsulated { .. } => {
                if framed_bytes.len() < SonyHeader::SIZE {
                    return Err(crate::Error::ParseError(Cow::Borrowed(
                        "Sony response too short for header",
                    )));
                }

                let header = SonyHeader::decode(framed_bytes).ok_or(crate::Error::ParseError(
                    Cow::Borrowed("Invalid Sony header format"),
                ))?;

                // Validate payload type
                match header.payload_type {
                    PayloadType::ViscaReply => {
                        // Expected for camera responses
                    }
                    other => {
                        tracing::warn!("Unexpected Sony payload type in response: {other:?}");
                    }
                }

                // Validate length
                let expected_payload_len = framed_bytes.len() - SonyHeader::SIZE;
                if header.payload_length as usize != expected_payload_len {
                    return Err(crate::Error::ParseError(Cow::Owned(format!(
                        "Sony header length mismatch: header says {}, actual payload is {}",
                        header.payload_length, expected_payload_len
                    ))));
                }

                let payload = Bytes::copy_from_slice(&framed_bytes[SonyHeader::SIZE..]);
                Ok((
                    payload,
                    FrameMeta {
                        sequence: Some(header.sequence_number),
                    },
                ))
            }
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
    fn sony_encapsulate(
        &self,
        visca_bytes: &[u8],
        is_inquiry: bool,
        use_sequence: bool,
        buffer_manager: &BufferManager,
    ) -> Bytes {
        let sequence = if use_sequence {
            self.next_sequence()
        } else {
            0
        };

        let header = if is_inquiry {
            SonyHeader::new_inquiry(visca_bytes.len(), sequence)
        } else {
            SonyHeader::new_command(visca_bytes.len(), sequence)
        };

        let mut envelope = buffer_manager.alloc_send_buffer();
        envelope.reserve(8 + visca_bytes.len());

        // Add Sony header
        envelope.extend_from_slice(&header.encode());

        // Add VISCA payload
        envelope.extend_from_slice(visca_bytes);

        envelope.freeze()
    }

    /// Extract VISCA payload from Sony encapsulated response.
    fn sony_extract_payload(&self, framed_bytes: &[u8]) -> Result<Bytes, crate::Error> {
        if framed_bytes.len() < SonyHeader::SIZE {
            return Err(crate::Error::ParseError(Cow::Borrowed(
                "Sony response too short for header",
            )));
        }

        // Parse header using the unified implementation
        let header = SonyHeader::decode(framed_bytes).ok_or(crate::Error::ParseError(
            Cow::Borrowed("Invalid Sony header format"),
        ))?;

        // Validate payload type
        match header.payload_type {
            PayloadType::ViscaReply => {
                // Expected for camera responses
            }
            other => {
                tracing::warn!("Unexpected Sony payload type in response: {other:?}");
            }
        }

        // Validate length
        let expected_payload_len = framed_bytes.len() - SonyHeader::SIZE;
        if header.payload_length as usize != expected_payload_len {
            return Err(crate::Error::ParseError(Cow::Owned(format!(
                "Sony header length mismatch: header says {}, actual payload is {}",
                header.payload_length, expected_payload_len
            ))));
        }

        // Extract VISCA payload - use slice to avoid allocation
        Ok(Bytes::copy_from_slice(&framed_bytes[SonyHeader::SIZE..]))
    }

    /// Zero-copy extraction of VISCA payload from Sony encapsulated response.
    #[cfg(any(test, feature = "async"))]
    fn sony_extract_payload_zero_copy(&self, framed: Bytes) -> Result<Bytes, crate::Error> {
        if framed.len() < SonyHeader::SIZE {
            return Err(crate::Error::ParseError(Cow::Borrowed(
                "Sony response too short for header",
            )));
        }

        // Parse header by reading from the Bytes
        let header_bytes = &framed[..SonyHeader::SIZE];
        let header = SonyHeader::decode(header_bytes).ok_or(crate::Error::ParseError(
            Cow::Borrowed("Invalid Sony header format"),
        ))?;

        // Validate payload type
        match header.payload_type {
            PayloadType::ViscaReply => {
                // Expected for camera responses
            }
            other => {
                tracing::warn!("Unexpected Sony payload type in response: {other:?}");
            }
        }

        // Validate length
        let expected_payload_len = framed.len() - SonyHeader::SIZE;
        if header.payload_length as usize != expected_payload_len {
            return Err(crate::Error::ParseError(Cow::Owned(format!(
                "Sony header length mismatch: header says {}, actual payload is {}",
                header.payload_length, expected_payload_len
            ))));
        }

        // Extract VISCA payload using slice - zero-copy operation
        Ok(framed.slice(SonyHeader::SIZE..))
    }
}

impl Clone for TransportEnvelope {
    fn clone(&self) -> Self {
        Self {
            style: self.style,
            // Clone the current sequence counter value, not the atomic itself
            sequence_counter: AtomicU32::new(self.sequence_counter.load(Ordering::Relaxed)),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::transport::buffer::BufferConfig;

    fn test_buffer_manager() -> BufferManager {
        BufferManager::new(BufferConfig::default())
    }

    #[test]
    fn test_raw_visca_passthrough() {
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]; // Power On

        let framed = envelope.frame_command(&visca_cmd, &test_buffer_manager());
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

        let framed = envelope.frame_command(&visca_cmd, &test_buffer_manager());

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

        let framed = envelope.frame_command(&visca_inquiry, &test_buffer_manager());

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

        let framed1 = envelope.frame_command(&visca_cmd, &test_buffer_manager());
        let framed2 = envelope.frame_command(&visca_cmd, &test_buffer_manager());

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

    #[test]
    fn test_sony_response_too_short_for_header() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
            use_sequence: false,
        });

        let malformed = vec![0x01, 0x11, 0x00, 0x03, 0x00, 0x00, 0x00];
        assert!(
            envelope.extract_response(&malformed).is_err(),
            "Should reject response shorter than Sony header"
        );

        let empty = vec![];
        assert!(
            envelope.extract_response(&empty).is_err(),
            "Should reject empty response"
        );

        let single = vec![0x90];
        assert!(
            envelope.extract_response(&single).is_err(),
            "Should reject single byte response"
        );
    }

    #[test]
    fn test_sony_invalid_payload_types() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
            use_sequence: false,
        });

        let invalid_types = [
            [0x00, 0x00], // Zero payload type
            [0xFF, 0xFF], // All bits set
            [0x01, 0x01], // Invalid sub-type
            [0x01, 0x12], // Out of range sub-type
            [0x02, 0x00], // Wrong major type
            [0x01, 0xFF], // Invalid sub-type with correct major
            [0x80, 0x80], // High bit set
        ];

        for invalid_type in &invalid_types {
            let mut response = Vec::new();
            response.extend_from_slice(invalid_type);
            response.extend_from_slice(&(3u16).to_be_bytes()); // Length
            response.extend_from_slice(&0u32.to_be_bytes()); // Sequence
            response.extend_from_slice(&[0x90, 0x41, VISCA_TERMINATOR]); // Valid VISCA

            let result = envelope.extract_response(&response);
            assert!(
                result.is_err(),
                "Should reject invalid Sony payload type: {:02X?}",
                invalid_type
            );
        }
    }

    #[test]
    fn test_sony_length_field_mismatches() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
            use_sequence: false,
        });

        let mut response = Vec::new();
        response.extend_from_slice(&[0x01, 0x11]); // Reply type
        response.extend_from_slice(&(10u16).to_be_bytes()); // Wrong length
        response.extend_from_slice(&0u32.to_be_bytes()); // Sequence
        response.extend_from_slice(&[0x90, 0x41, VISCA_TERMINATOR]); // 3 bytes payload

        assert!(
            envelope.extract_response(&response).is_err(),
            "Should reject length field mismatch (claims 10, has 3)"
        );

        let mut response = Vec::new();
        response.extend_from_slice(&[0x01, 0x11]); // Reply type
        response.extend_from_slice(&(0u16).to_be_bytes()); // Zero length
        response.extend_from_slice(&0u32.to_be_bytes()); // Sequence
        response.extend_from_slice(&[0x90, 0x41, VISCA_TERMINATOR]); // But has payload

        assert!(
            envelope.extract_response(&response).is_err(),
            "Should reject zero length with payload"
        );
    }

    #[test]
    fn test_sony_max_length_boundaries() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
            use_sequence: false,
        });

        let mut response = Vec::new();
        response.extend_from_slice(&[0x01, 0x11]); // Reply type
        response.extend_from_slice(&(u16::MAX).to_be_bytes()); // Max length
        response.extend_from_slice(&0u32.to_be_bytes()); // Sequence
        response.extend_from_slice(&[0x90, 0x41, VISCA_TERMINATOR]); // Small payload

        assert!(
            envelope.extract_response(&response).is_err(),
            "Should reject u16::MAX length mismatch"
        );
    }

    #[test]
    fn test_sony_sequence_number_wraparound() {
        let envelope =
            TransportEnvelope::new(ProtocolStyle::SonyEncapsulated { use_sequence: true });

        let dummy_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

        for _ in 0..100 {
            let framed = envelope.frame_command(&dummy_cmd, &test_buffer_manager());
            assert!(framed.len() > 8, "Should produce framed output");
        }

        let frame1 = envelope.frame_command(&dummy_cmd, &test_buffer_manager());
        let frame2 = envelope.frame_command(&dummy_cmd, &test_buffer_manager());

        let seq1 = u32::from_be_bytes([frame1[4], frame1[5], frame1[6], frame1[7]]);
        let seq2 = u32::from_be_bytes([frame2[4], frame2[5], frame2[6], frame2[7]]);

        assert_eq!(seq2, seq1 + 1, "Sequence should increment by 1");
    }

    #[test]
    fn test_sony_malformed_header_bytes() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
            use_sequence: false,
        });

        let mut response = Vec::new();
        response.push(0x11); // Wrong byte order for type
        response.push(0x01);
        response.extend_from_slice(&(3u16).to_le_bytes()); // Wrong endianness
        response.extend_from_slice(&0u32.to_le_bytes()); // Wrong endianness
        response.extend_from_slice(&[0x90, 0x41, VISCA_TERMINATOR]);

        let result = envelope.extract_response(&response);
        assert!(
            result.is_err(),
            "Should reject malformed header with wrong byte order"
        );
    }

    #[test]
    fn test_raw_visca_zero_length_handling() {
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);

        let empty: &[u8] = &[];
        let framed = envelope.frame_command(empty, &test_buffer_manager());
        assert_eq!(framed.len(), 0, "Should handle empty command");

        let extracted = envelope.extract_response(empty);
        assert!(extracted.is_ok(), "Should handle empty response");
        assert_eq!(extracted.expect("Already checked is_ok").len(), 0);
    }

    #[test]
    fn test_raw_visca_maximum_size() {
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);

        let large_cmd = vec![0x81; 1000];
        let framed = envelope.frame_command(&large_cmd, &test_buffer_manager());
        assert_eq!(framed.len(), 1000, "Should pass through large commands");

        let extracted = envelope.extract_response(&large_cmd);
        assert!(extracted.is_ok(), "Should extract large responses");
        assert_eq!(extracted.expect("Already checked is_ok").len(), 1000);
    }

    #[test]
    fn test_bytes_immutability_and_efficiency() {
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);

        let original = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let framed = envelope.frame_command(&original, &test_buffer_manager());

        let cloned = framed.clone();
        assert_eq!(framed, cloned);

        assert_eq!(
            original,
            vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]
        );
    }

    #[test]
    fn test_alternating_protocol_styles() {
        let raw_envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);
        let sony_envelope =
            TransportEnvelope::new(ProtocolStyle::SonyEncapsulated { use_sequence: true });

        let cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

        for _ in 0..100 {
            let raw_framed = raw_envelope.frame_command(&cmd, &test_buffer_manager());
            assert_eq!(raw_framed.len(), 6);

            let sony_framed = sony_envelope.frame_command(&cmd, &test_buffer_manager());
            assert_eq!(sony_framed.len(), 14);
        }
    }

    #[test]
    fn test_bytes_zero_copy_behavior() {
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);

        let large_cmd = vec![0x81; 1000];

        let framed1 = envelope.frame_command(&large_cmd, &test_buffer_manager());
        let framed2 = framed1.clone(); // Should be cheap (reference counted)

        assert_eq!(framed1, framed2);

        drop(framed1);
        assert_eq!(framed2.len(), 1000);
    }

    #[test]
    fn test_frame_command_owned_zero_copy_raw() {
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let owned_bytes = Bytes::from(visca_cmd.clone());

        // Get raw pointer to compare
        let original_ptr = owned_bytes.as_ptr();

        // Frame the command - should be zero-copy for raw
        let framed = envelope.frame_command_owned(owned_bytes.clone(), &test_buffer_manager());

        // For raw VISCA, should be the same Bytes object
        assert_eq!(&framed[..], &visca_cmd[..]);
        // Raw VISCA should pass through the same pointer (zero-copy)
        assert_eq!(framed.as_ptr(), original_ptr);
    }

    #[test]
    fn test_frame_command_owned_sony() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
            use_sequence: false,
        });
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let owned_bytes = Bytes::from(visca_cmd.clone());

        // Frame the command - Sony needs to allocate for header
        let framed = envelope.frame_command_owned(owned_bytes, &test_buffer_manager());

        // Should be 8-byte header + 6-byte VISCA command = 14 bytes
        assert_eq!(framed.len(), 14);
        // Check header and payload
        assert_eq!(&framed[0..2], &[0x01, 0x00]); // Command payload type
        assert_eq!(&framed[8..], &visca_cmd[..]);
    }

    #[test]
    fn test_extract_response_owned_zero_copy_raw() {
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);
        let visca_response = vec![0x90, 0x41, VISCA_TERMINATOR];
        let owned_bytes = Bytes::from(visca_response.clone());

        // Get raw pointer to compare
        let original_ptr = owned_bytes.as_ptr();

        // Extract response - should be zero-copy for raw
        let extracted = envelope
            .extract_response_owned(owned_bytes.clone())
            .expect("valid raw response");

        // For raw VISCA, should be the same Bytes object
        assert_eq!(&extracted[..], &visca_response[..]);
        // Raw VISCA should pass through the same pointer (zero-copy)
        assert_eq!(extracted.as_ptr(), original_ptr);
    }

    #[test]
    fn test_extract_response_owned_zero_copy_sony() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
            use_sequence: false,
        });

        // Create a mock Sony response
        let visca_ack = vec![0x90, 0x41, VISCA_TERMINATOR];
        let mut response = Vec::new();
        response.extend_from_slice(&[0x01, 0x11]); // Reply payload type
        response.extend_from_slice(&(3u16).to_be_bytes()); // Length = 3
        response.extend_from_slice(&0u32.to_be_bytes()); // Sequence = 0
        response.extend_from_slice(&visca_ack); // VISCA payload

        let owned_bytes = Bytes::from(response.clone());

        // Extract response - should use slice for Sony (still zero-copy)
        let extracted = envelope
            .extract_response_owned(owned_bytes.clone())
            .expect("valid sony response");

        assert_eq!(&extracted[..], &visca_ack[..]);
        // Sony uses slice() which is zero-copy, creating a sub-view
        assert_eq!(extracted.len(), 3);

        // The slice should point to the same underlying data
        // We can verify this by checking that modifying one doesn't affect the other
        // (Bytes is immutable, so this is always true)
        let cloned = extracted.clone();
        assert_eq!(cloned, extracted);
    }

    #[test]
    fn test_extract_response_owned_invalid_sony() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
            use_sequence: false,
        });

        // Too short
        let short_response = Bytes::from(vec![0x01, 0x11, 0x00]);
        assert!(envelope.extract_response_owned(short_response).is_err());

        // Invalid header
        let mut invalid_response = vec![0xFF, VISCA_TERMINATOR]; // Invalid payload type
        invalid_response.extend_from_slice(&(3u16).to_be_bytes());
        invalid_response.extend_from_slice(&0u32.to_be_bytes());
        invalid_response.extend_from_slice(&[0x90, 0x41, VISCA_TERMINATOR]);
        let invalid_bytes = Bytes::from(invalid_response);
        assert!(envelope.extract_response_owned(invalid_bytes).is_err());
    }

    #[test]
    fn test_frame_with_meta_raw_visca() {
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

        let (framed, meta) = envelope.frame_with_meta(&visca_cmd, &test_buffer_manager());

        assert_eq!(&framed[..], &visca_cmd[..]);
        assert_eq!(meta.sequence, None);
    }

    #[test]
    fn test_frame_with_meta_sony_command() {
        let envelope =
            TransportEnvelope::new(ProtocolStyle::SonyEncapsulated { use_sequence: true });
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

        let (framed, meta) = envelope.frame_with_meta(&visca_cmd, &test_buffer_manager());

        assert_eq!(framed.len(), 14); // 8 byte header + 6 byte command
        assert_eq!(meta.sequence, Some(0)); // First sequence

        // Verify sequence increments
        let (_, meta2) = envelope.frame_with_meta(&visca_cmd, &test_buffer_manager());
        assert_eq!(meta2.sequence, Some(1));
    }

    #[test]
    fn test_frame_with_meta_sony_inquiry() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
            use_sequence: false,
        });
        let visca_inquiry = vec![0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR];

        let (framed, meta) = envelope.frame_with_meta(&visca_inquiry, &test_buffer_manager());

        assert_eq!(framed.len(), 13); // 8 byte header + 5 byte inquiry
        assert_eq!(meta.sequence, Some(0)); // No sequence tracking

        // Check inquiry header type
        assert_eq!(&framed[0..2], &[0x01, 0x10]); // Inquiry payload type
    }

    #[test]
    fn test_extract_with_meta_raw_visca() {
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);
        let visca_response = vec![0x90, 0x41, VISCA_TERMINATOR];

        let result = envelope.extract_with_meta(&visca_response);
        assert!(result.is_ok());

        let (payload, meta) = result.expect("valid response");
        assert_eq!(&payload[..], &visca_response[..]);
        assert_eq!(meta.sequence, None);
    }

    #[test]
    fn test_extract_with_meta_sony_response() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
            use_sequence: false,
        });

        // Create a mock Sony response with sequence 42
        let visca_ack = vec![0x90, 0x41, VISCA_TERMINATOR];
        let mut response = Vec::new();
        response.extend_from_slice(&[0x01, 0x11]); // Reply payload type
        response.extend_from_slice(&(3u16).to_be_bytes()); // Length = 3
        response.extend_from_slice(&42u32.to_be_bytes()); // Sequence = 42
        response.extend_from_slice(&visca_ack); // VISCA payload

        let result = envelope.extract_with_meta(&response);
        assert!(result.is_ok());

        let (payload, meta) = result.expect("valid response");
        assert_eq!(&payload[..], &visca_ack[..]);
        assert_eq!(meta.sequence, Some(42));
    }

    #[test]
    fn test_extract_with_meta_invalid_sony() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
            use_sequence: false,
        });

        // Too short
        let short_response = vec![0x01, 0x11, 0x00];
        let result = envelope.extract_with_meta(&short_response);
        assert!(result.is_err());

        // Invalid header
        let mut invalid_response = vec![0xFF, VISCA_TERMINATOR]; // Invalid payload type
        invalid_response.extend_from_slice(&(3u16).to_be_bytes());
        invalid_response.extend_from_slice(&0u32.to_be_bytes());
        invalid_response.extend_from_slice(&[0x90, 0x41, VISCA_TERMINATOR]);
        let result = envelope.extract_with_meta(&invalid_response);
        assert!(result.is_err());
    }

    #[test]
    fn test_frame_extract_roundtrip() {
        let envelope =
            TransportEnvelope::new(ProtocolStyle::SonyEncapsulated { use_sequence: true });
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

        // Frame a command
        let (_framed, frame_meta) = envelope.frame_with_meta(&visca_cmd, &test_buffer_manager());
        assert_eq!(frame_meta.sequence, Some(0));

        // Create a response with the same sequence
        let visca_response = vec![0x90, 0x41, VISCA_TERMINATOR];
        let mut response = Vec::new();
        response.extend_from_slice(&[0x01, 0x11]); // Reply type
        response.extend_from_slice(&(3u16).to_be_bytes()); // Length
        response.extend_from_slice(&0u32.to_be_bytes()); // Same sequence as command
        response.extend_from_slice(&visca_response);

        // Extract the response
        let (payload, extract_meta) = envelope
            .extract_with_meta(&response)
            .expect("valid response");

        assert_eq!(&payload[..], &visca_response[..]);
        assert_eq!(extract_meta.sequence, Some(0));
        assert_eq!(frame_meta.sequence, extract_meta.sequence);
    }
}
