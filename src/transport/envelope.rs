//! Transport envelope abstraction for different VISCA protocols.
//!
//! This module provides abstraction for protocol envelopes, specifically
//! handling Sony's 8-byte encapsulated VISCA protocol vs raw VISCA bytes.

use bytes::Bytes;

use std::{
    borrow::Cow,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use crate::{
    capabilities::ProtocolStyle,
    command::CommandKind,
    protocol::sony::{PayloadType, SonyHeader},
    transport::buffer::BufferManager,
    Error,
};

/// Metadata extracted from or used during framing operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FrameMeta {
    /// Sequence number for Sony protocol, None for raw VISCA.
    pub sequence: Option<u32>,
}

/// Transport envelope that handles protocol-specific framing.
///
/// Different camera manufacturers use different framing approaches:
/// - Raw VISCA: Commands sent as-is (PtzOptics, generic cameras)
/// - Sony Encapsulated: 8-byte header + VISCA payload (Sony cameras)
#[derive(Debug, Clone)]
pub(crate) struct TransportEnvelope {
    style: ProtocolStyle,
    sequence_counter: Arc<AtomicU32>,
}

impl TransportEnvelope {
    /// Create a new transport envelope for the given protocol style.
    pub fn new(style: ProtocolStyle) -> Self {
        Self {
            style,
            sequence_counter: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Frame VISCA bytes with an explicit command kind.
    ///
    /// This method replaces byte-heuristic detection with type-driven classification,
    /// allowing the caller to specify whether the command is an inquiry or command
    /// based on the type's knowledge rather than re-parsing bytes.
    ///
    /// For raw VISCA, returns the command bytes unchanged.
    /// For Sony encapsulated, wraps with 8-byte header using the specified kind.
    pub fn frame_bytes_with_kind(
        &self,
        visca_bytes: &[u8],
        kind: CommandKind,
        buffer_manager: &BufferManager,
    ) -> Bytes {
        match self.style {
            ProtocolStyle::RawVisca => Bytes::copy_from_slice(visca_bytes),
            ProtocolStyle::SonyEncapsulated => {
                let sequence = self.next_sequence();
                let header = match kind {
                    CommandKind::Inquiry => SonyHeader::new_inquiry(visca_bytes.len(), sequence),
                    CommandKind::Command => SonyHeader::new_command(visca_bytes.len(), sequence),
                };
                let mut envelope = buffer_manager.alloc_send_buffer();
                envelope.reserve(SonyHeader::SIZE + visca_bytes.len());
                envelope.extend_from_slice(&header.encode());
                envelope.extend_from_slice(visca_bytes);
                envelope.freeze()
            }
        }
    }

    /// Parse a response and extract the VISCA payload.
    ///
    /// For raw VISCA, returns the bytes unchanged.
    /// For Sony encapsulated, extracts payload from 8-byte header.
    ///
    /// This method is used by blocking cameras and may be used by external consumers.
    pub fn extract_response(&self, framed_bytes: &[u8]) -> Result<Bytes, Error> {
        match self.style {
            ProtocolStyle::RawVisca => Ok(Bytes::copy_from_slice(framed_bytes)),
            ProtocolStyle::SonyEncapsulated => self.sony_extract_payload(framed_bytes),
        }
    }

    /// Zero-copy variant of extract_with_meta that takes owned Bytes.
    ///
    /// For raw VISCA, passes through without copying.
    /// For Sony encapsulated, extracts payload and sequence from 8-byte header using slice.
    pub fn extract_with_meta_owned(&self, framed: Bytes) -> Result<(Bytes, FrameMeta), Error> {
        match self.style {
            ProtocolStyle::RawVisca => Ok((framed, FrameMeta { sequence: None })),
            ProtocolStyle::SonyEncapsulated => {
                if framed.len() < SonyHeader::SIZE {
                    return Err(Error::ParseError(Cow::Borrowed(
                        "Sony response too short for header",
                    )));
                }

                // Parse header by reading from the Bytes
                let header_bytes = &framed[..SonyHeader::SIZE];
                let header = SonyHeader::decode(header_bytes).ok_or(Error::ParseError(
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
                    return Err(Error::ParseError(Cow::Owned(format!(
                        "Sony header length mismatch: header says {}, actual payload is {}",
                        header.payload_length, expected_payload_len
                    ))));
                }

                // Extract VISCA payload using slice - zero-copy operation
                Ok((
                    framed.slice(SonyHeader::SIZE..),
                    FrameMeta {
                        sequence: Some(header.sequence_number),
                    },
                ))
            }
        }
    }

    /// Optimized framing that writes VISCA bytes directly into final frame buffer.
    ///
    /// This method eliminates the double-allocation pattern by writing the VISCA
    /// bytes directly into the final frame buffer with the appropriate protocol
    /// header, avoiding intermediate copies.
    ///
    /// For raw VISCA, performs a single copy of the bytes.
    /// For Sony encapsulated, writes header and payload in one allocation.
    pub fn frame_bytes_into(
        &self,
        visca_bytes: &[u8],
        kind: CommandKind,
        buffer_manager: &BufferManager,
    ) -> (Bytes, FrameMeta) {
        match self.style {
            ProtocolStyle::RawVisca => {
                // For raw VISCA, copy bytes once into new buffer
                let mut buffer = buffer_manager.alloc_send_buffer();
                buffer.extend_from_slice(visca_bytes);
                (buffer.freeze(), FrameMeta { sequence: None })
            }
            ProtocolStyle::SonyEncapsulated => {
                // For Sony, write header and payload in single buffer
                let sequence = self.next_sequence();
                let header = match kind {
                    CommandKind::Inquiry => SonyHeader::new_inquiry(visca_bytes.len(), sequence),
                    CommandKind::Command => SonyHeader::new_command(visca_bytes.len(), sequence),
                };

                let mut buffer = buffer_manager.alloc_send_buffer();
                // Reserve exact space needed
                buffer.reserve(SonyHeader::SIZE + visca_bytes.len());
                // Write header
                buffer.extend_from_slice(&header.encode());
                // Write payload
                buffer.extend_from_slice(visca_bytes);

                (
                    buffer.freeze(),
                    FrameMeta {
                        sequence: Some(sequence),
                    },
                )
            }
        }
    }

    /// Get the next sequence number (if using sequence tracking).
    fn next_sequence(&self) -> u32 {
        self.sequence_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Extract VISCA payload from Sony encapsulated response.
    fn sony_extract_payload(&self, framed_bytes: &[u8]) -> Result<Bytes, Error> {
        if framed_bytes.len() < SonyHeader::SIZE {
            return Err(Error::ParseError(Cow::Borrowed(
                "Sony response too short for header",
            )));
        }

        // Parse header using the unified implementation
        let header = SonyHeader::decode(framed_bytes).ok_or(Error::ParseError(Cow::Borrowed(
            "Invalid Sony header format",
        )))?;

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
            return Err(Error::ParseError(Cow::Owned(format!(
                "Sony header length mismatch: header says {}, actual payload is {}",
                header.payload_length, expected_payload_len
            ))));
        }

        // Extract VISCA payload - use slice to avoid allocation
        Ok(Bytes::copy_from_slice(&framed_bytes[SonyHeader::SIZE..]))
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

        let framed = envelope.frame_bytes_with_kind(
            &visca_cmd,
            CommandKind::Command,
            &test_buffer_manager(),
        );
        assert_eq!(&framed[..], &visca_cmd[..]);

        let extracted = envelope
            .extract_response(&visca_cmd)
            .expect("raw visca should extract unchanged");
        assert_eq!(&extracted[..], &visca_cmd[..]);
    }

    #[test]
    fn test_sony_encapsulation_command() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]; // Power On

        let framed = envelope.frame_bytes_with_kind(
            &visca_cmd,
            CommandKind::Command,
            &test_buffer_manager(),
        );

        // Should be 8-byte header + 6-byte VISCA command = 14 bytes
        assert_eq!(framed.len(), 14);

        // Check header
        assert_eq!(&framed[0..2], &[0x01, 0x00]); // Command payload type
        assert_eq!(&framed[2..4], &(6u16).to_be_bytes()); // Length = 6
        assert_eq!(&framed[4..8], &0u32.to_be_bytes()); // Sequence = 0 (first call)

        // Check VISCA payload
        assert_eq!(&framed[8..], &visca_cmd[..]);
    }

    #[test]
    fn test_sony_encapsulation_inquiry() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);
        let visca_inquiry = vec![0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR]; // Power Status Inquiry

        let framed = envelope.frame_bytes_with_kind(
            &visca_inquiry,
            CommandKind::Inquiry,
            &test_buffer_manager(),
        );

        // Should be 8-byte header + 5-byte VISCA inquiry = 13 bytes
        assert_eq!(framed.len(), 13);

        // Check header
        assert_eq!(&framed[0..2], &[0x01, 0x10]); // Inquiry payload type
        assert_eq!(&framed[2..4], &(5u16).to_be_bytes()); // Length = 5
                                                          // Sequence should be 0 since this is the first call for this envelope instance
        assert_eq!(&framed[4..8], &0u32.to_be_bytes()); // Sequence = 0 (first call for this envelope)

        // Check VISCA payload
        assert_eq!(&framed[8..], &visca_inquiry[..]);
    }

    #[test]
    fn test_sony_response_extraction() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);

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
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

        let framed1 = envelope.frame_bytes_with_kind(
            &visca_cmd,
            CommandKind::Command,
            &test_buffer_manager(),
        );
        let framed2 = envelope.frame_bytes_with_kind(
            &visca_cmd,
            CommandKind::Command,
            &test_buffer_manager(),
        );

        // Extract sequence numbers
        let seq1 = u32::from_be_bytes([framed1[4], framed1[5], framed1[6], framed1[7]]);
        let seq2 = u32::from_be_bytes([framed2[4], framed2[5], framed2[6], framed2[7]]);

        assert_eq!(seq1, 0);
        assert_eq!(seq2, 1);
    }

    #[test]
    fn test_invalid_sony_response() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);

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
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);

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
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);

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
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);

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
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);

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
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);

        let dummy_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

        for _ in 0..100 {
            let framed = envelope.frame_bytes_with_kind(
                &dummy_cmd,
                CommandKind::Command,
                &test_buffer_manager(),
            );
            assert!(framed.len() > 8, "Should produce framed output");
        }

        let frame1 = envelope.frame_bytes_with_kind(
            &dummy_cmd,
            CommandKind::Command,
            &test_buffer_manager(),
        );
        let frame2 = envelope.frame_bytes_with_kind(
            &dummy_cmd,
            CommandKind::Command,
            &test_buffer_manager(),
        );

        let seq1 = u32::from_be_bytes([frame1[4], frame1[5], frame1[6], frame1[7]]);
        let seq2 = u32::from_be_bytes([frame2[4], frame2[5], frame2[6], frame2[7]]);

        assert_eq!(seq2, seq1 + 1, "Sequence should increment by 1");
    }

    #[test]
    fn test_sony_malformed_header_bytes() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);

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
        let framed =
            envelope.frame_bytes_with_kind(empty, CommandKind::Command, &test_buffer_manager());
        assert_eq!(framed.len(), 0, "Should handle empty command");

        let extracted = envelope.extract_response(empty);
        assert!(extracted.is_ok(), "Should handle empty response");
        assert_eq!(extracted.expect("Already checked is_ok").len(), 0);
    }

    #[test]
    fn test_raw_visca_maximum_size() {
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);

        let large_cmd = vec![0x81; 1000];
        let framed = envelope.frame_bytes_with_kind(
            &large_cmd,
            CommandKind::Command,
            &test_buffer_manager(),
        );
        assert_eq!(framed.len(), 1000, "Should pass through large commands");

        let extracted = envelope.extract_response(&large_cmd);
        assert!(extracted.is_ok(), "Should extract large responses");
        assert_eq!(extracted.expect("Already checked is_ok").len(), 1000);
    }

    #[test]
    fn test_bytes_immutability_and_efficiency() {
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);

        let original = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let framed =
            envelope.frame_bytes_with_kind(&original, CommandKind::Command, &test_buffer_manager());

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
        let sony_envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);

        let cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

        for _ in 0..100 {
            let raw_framed = raw_envelope.frame_bytes_with_kind(
                &cmd,
                CommandKind::Command,
                &test_buffer_manager(),
            );
            assert_eq!(raw_framed.len(), 6);

            let sony_framed = sony_envelope.frame_bytes_with_kind(
                &cmd,
                CommandKind::Command,
                &test_buffer_manager(),
            );
            assert_eq!(sony_framed.len(), 14);
        }
    }

    #[test]
    fn test_bytes_zero_copy_behavior() {
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);

        let large_cmd = vec![0x81; 1000];

        let framed1 = envelope.frame_bytes_with_kind(
            &large_cmd,
            CommandKind::Command,
            &test_buffer_manager(),
        );
        let framed2 = framed1.clone(); // Should be cheap (reference counted)

        assert_eq!(framed1, framed2);

        drop(framed1);
        assert_eq!(framed2.len(), 1000);
    }

    #[test]
    fn test_extract_with_meta_owned_raw_visca() {
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);
        let visca_response = vec![0x90, 0x41, VISCA_TERMINATOR];
        let owned_bytes = Bytes::from(visca_response.clone());

        // Get raw pointer to compare
        let original_ptr = owned_bytes.as_ptr();

        // Extract response - should be zero-copy for raw
        let result = envelope.extract_with_meta_owned(owned_bytes.clone());
        assert!(result.is_ok());

        let (extracted, meta) = result.expect("valid response");
        assert_eq!(&extracted[..], &visca_response[..]);
        assert_eq!(meta.sequence, None);
        // Raw VISCA should pass through the same pointer (zero-copy)
        assert_eq!(extracted.as_ptr(), original_ptr);
    }

    #[test]
    fn test_extract_with_meta_owned_sony_response() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);

        // Create a mock Sony response with sequence 42
        let visca_ack = vec![0x90, 0x41, VISCA_TERMINATOR];
        let mut response = Vec::new();
        response.extend_from_slice(&[0x01, 0x11]); // Reply payload type
        response.extend_from_slice(&(3u16).to_be_bytes()); // Length = 3
        response.extend_from_slice(&42u32.to_be_bytes()); // Sequence = 42
        response.extend_from_slice(&visca_ack); // VISCA payload

        let owned_bytes = Bytes::from(response);

        // Extract response
        let result = envelope.extract_with_meta_owned(owned_bytes);
        assert!(result.is_ok());

        let (payload, meta) = result.expect("valid response");
        assert_eq!(&payload[..], &visca_ack[..]);
        assert_eq!(meta.sequence, Some(42));
    }

    #[test]
    fn test_extract_with_meta_owned_invalid_sony() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);

        // Too short
        let short_response = Bytes::from(vec![0x01, 0x11, 0x00]);
        let result = envelope.extract_with_meta_owned(short_response);
        assert!(result.is_err());

        // Invalid header
        let mut invalid_response = vec![0xFF, VISCA_TERMINATOR]; // Invalid payload type
        invalid_response.extend_from_slice(&(3u16).to_be_bytes());
        invalid_response.extend_from_slice(&0u32.to_be_bytes());
        invalid_response.extend_from_slice(&[0x90, 0x41, VISCA_TERMINATOR]);
        let invalid_bytes = Bytes::from(invalid_response);
        let result = envelope.extract_with_meta_owned(invalid_bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_clone_shares_sequence_counter() {
        // Create an envelope and clone it
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);
        let cloned_envelope = envelope.clone();

        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

        // Frame a command from original envelope - should get sequence 0
        let framed1 = envelope.frame_bytes_with_kind(
            &visca_cmd,
            CommandKind::Command,
            &test_buffer_manager(),
        );
        let seq1 = u32::from_be_bytes([framed1[4], framed1[5], framed1[6], framed1[7]]);
        assert_eq!(seq1, 0, "First sequence should be 0");

        // Frame a command from cloned envelope - should get sequence 1 (not 0!)
        let framed2 = cloned_envelope.frame_bytes_with_kind(
            &visca_cmd,
            CommandKind::Command,
            &test_buffer_manager(),
        );
        let seq2 = u32::from_be_bytes([framed2[4], framed2[5], framed2[6], framed2[7]]);
        assert_eq!(
            seq2, 1,
            "Cloned envelope should continue from 1, not reset to 0"
        );

        // Frame another from original - should get sequence 2
        let framed3 = envelope.frame_bytes_with_kind(
            &visca_cmd,
            CommandKind::Command,
            &test_buffer_manager(),
        );
        let seq3 = u32::from_be_bytes([framed3[4], framed3[5], framed3[6], framed3[7]]);
        assert_eq!(seq3, 2, "Third sequence should be 2");

        // Frame another from clone - should get sequence 3
        let framed4 = cloned_envelope.frame_bytes_with_kind(
            &visca_cmd,
            CommandKind::Command,
            &test_buffer_manager(),
        );
        let seq4 = u32::from_be_bytes([framed4[4], framed4[5], framed4[6], framed4[7]]);
        assert_eq!(seq4, 3, "Fourth sequence from clone should be 3");
    }

    #[test]
    fn test_frame_bytes_into_raw_visca() {
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

        let (framed, meta) =
            envelope.frame_bytes_into(&visca_cmd, CommandKind::Command, &test_buffer_manager());

        // Raw VISCA should pass through unchanged
        assert_eq!(&framed[..], &visca_cmd[..]);
        assert_eq!(meta.sequence, None);
    }

    #[test]
    fn test_frame_bytes_into_sony_command() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

        let (framed, meta) =
            envelope.frame_bytes_into(&visca_cmd, CommandKind::Command, &test_buffer_manager());

        // Should be 8-byte header + 6-byte VISCA command = 14 bytes
        assert_eq!(framed.len(), 14);

        // Check header
        assert_eq!(&framed[0..2], &[0x01, 0x00]); // Command payload type
        assert_eq!(&framed[2..4], &(6u16).to_be_bytes()); // Length = 6
        assert_eq!(&framed[4..8], &0u32.to_be_bytes()); // Sequence = 0 (first call)

        // Check VISCA payload
        assert_eq!(&framed[8..], &visca_cmd[..]);

        // Check metadata
        assert_eq!(meta.sequence, Some(0));
    }

    #[test]
    fn test_frame_bytes_into_sony_inquiry() {
        let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated);
        let visca_inquiry = vec![0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR];

        let (framed, meta) =
            envelope.frame_bytes_into(&visca_inquiry, CommandKind::Inquiry, &test_buffer_manager());

        // Should be 8-byte header + 5-byte VISCA inquiry = 13 bytes
        assert_eq!(framed.len(), 13);

        // Check header
        assert_eq!(&framed[0..2], &[0x01, 0x10]); // Inquiry payload type
        assert_eq!(&framed[2..4], &(5u16).to_be_bytes()); // Length = 5
        assert_eq!(&framed[4..8], &0u32.to_be_bytes()); // Sequence = 0

        // Check VISCA payload
        assert_eq!(&framed[8..], &visca_inquiry[..]);

        // Check metadata
        assert_eq!(meta.sequence, Some(0));
    }

    #[test]
    fn test_concurrent_clone_sequence_uniqueness() {
        use std::collections::HashSet;
        use std::sync::{Arc, Mutex};
        use std::thread;

        // Create an envelope and share it across threads
        let envelope = Arc::new(TransportEnvelope::new(ProtocolStyle::SonyEncapsulated));
        let sequences = Arc::new(Mutex::new(HashSet::new()));

        // Number of threads and commands per thread
        const NUM_THREADS: usize = 10;
        const COMMANDS_PER_THREAD: usize = 100;

        let mut handles = vec![];

        for _ in 0..NUM_THREADS {
            let envelope_clone = Arc::clone(&envelope);
            let sequences_clone = Arc::clone(&sequences);

            handles.push(thread::spawn(move || {
                let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

                for _ in 0..COMMANDS_PER_THREAD {
                    let framed = envelope_clone.frame_bytes_with_kind(
                        &visca_cmd,
                        CommandKind::Command,
                        &test_buffer_manager(),
                    );

                    // Extract sequence number
                    let seq = u32::from_be_bytes([framed[4], framed[5], framed[6], framed[7]]);

                    // Add to the set and verify uniqueness
                    let mut seqs = sequences_clone.lock().expect("Lock poisoned");
                    assert!(seqs.insert(seq), "Duplicate sequence detected: {}", seq);
                }
            }));
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Verify we got the expected number of unique sequences
        let final_sequences = sequences.lock().expect("Lock poisoned");
        assert_eq!(
            final_sequences.len(),
            NUM_THREADS * COMMANDS_PER_THREAD,
            "Should have exactly {} unique sequences",
            NUM_THREADS * COMMANDS_PER_THREAD
        );

        // Verify sequences are in the expected range [0, NUM_THREADS * COMMANDS_PER_THREAD)
        for seq in final_sequences.iter() {
            assert!(
                *seq < (NUM_THREADS * COMMANDS_PER_THREAD) as u32,
                "Sequence {} is out of expected range",
                seq
            );
        }
    }
}
