//! Transport envelope abstraction for different VISCA protocols.
//!
//! This module provides abstraction for protocol envelopes, specifically
//! handling Sony's 8-byte encapsulated VISCA protocol vs raw VISCA bytes.
//!
//! The envelope system now supports compile-time protocol selection through
//! marker types, eliminating runtime branches in the hot path.

use bytes::Bytes;

use std::{
    borrow::Cow,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use crate::{
    command::CommandKind,
    protocol::sony::{PayloadType, SonyHeader},
    transport::{buffer::BufferManager, builder::AddressingMode},
    Error,
};

/// Metadata extracted from or used during framing operations.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameMeta {
    /// Sequence number for Sony protocol, None for raw VISCA.
    pub sequence: Option<u32>,
}

/// Trait for protocol envelope implementations.
///
/// This trait is sealed and can only be implemented by the provided envelope types.
/// Each envelope type provides compile-time protocol selection, eliminating runtime
/// branches in the hot path.
pub trait Envelope: private::Sealed + Send + Sync + 'static {
    /// Create a new envelope instance with the given addressing mode.
    fn new(addressing: AddressingMode) -> Self;

    /// Frame VISCA bytes with protocol-specific encapsulation.
    fn frame_bytes(
        &self,
        visca_bytes: &[u8],
        kind: CommandKind,
        buffer_manager: &BufferManager,
    ) -> Bytes;

    /// Frame VISCA bytes and return metadata about the framing.
    fn frame_bytes_with_meta(
        &self,
        visca_bytes: &[u8],
        kind: CommandKind,
        buffer_manager: &BufferManager,
    ) -> (Bytes, FrameMeta);

    /// Extract VISCA payload from protocol-specific framing.
    fn extract_response(&self, framed_bytes: &[u8]) -> Result<Bytes, Error>;

    /// Extract payload with metadata (sequence number for Sony).
    fn extract_with_meta(&self, framed: Bytes) -> Result<(Bytes, FrameMeta), Error>;
}

// Private module to seal the trait
mod private {
    pub trait Sealed {}
    impl Sealed for super::RawVisca {}
    impl Sealed for super::SonyEncapsulated {}
}

/// Raw VISCA envelope - no encapsulation.
#[derive(Debug, Clone, Copy)]
pub struct RawVisca {
    addressing: AddressingMode,
}

/// Sony encapsulated envelope - 8-byte header with sequence numbers.
#[derive(Debug, Clone)]
pub struct SonyEncapsulated {
    addressing: AddressingMode,
    sequence_counter: Arc<AtomicU32>,
}

impl Envelope for RawVisca {
    fn new(addressing: AddressingMode) -> Self {
        Self { addressing }
    }

    fn frame_bytes(
        &self,
        visca_bytes: &[u8],
        kind: CommandKind,
        _buffer_manager: &BufferManager,
    ) -> Bytes {
        if visca_bytes.is_empty() {
            return Bytes::new();
        }

        // Normalize the address byte if needed
        let normalized_addr = normalize_address(visca_bytes[0], kind, self.addressing);

        if normalized_addr == visca_bytes[0] {
            Bytes::copy_from_slice(visca_bytes)
        } else {
            let mut normalized = Vec::with_capacity(visca_bytes.len());
            normalized.push(normalized_addr);
            normalized.extend_from_slice(&visca_bytes[1..]);
            Bytes::from(normalized)
        }
    }

    fn frame_bytes_with_meta(
        &self,
        visca_bytes: &[u8],
        kind: CommandKind,
        buffer_manager: &BufferManager,
    ) -> (Bytes, FrameMeta) {
        let framed = self.frame_bytes(visca_bytes, kind, buffer_manager);
        (framed, FrameMeta { sequence: None })
    }

    fn extract_response(&self, framed_bytes: &[u8]) -> Result<Bytes, Error> {
        Ok(Bytes::copy_from_slice(framed_bytes))
    }

    fn extract_with_meta(&self, framed: Bytes) -> Result<(Bytes, FrameMeta), Error> {
        Ok((framed, FrameMeta { sequence: None }))
    }
}

impl Envelope for SonyEncapsulated {
    fn new(addressing: AddressingMode) -> Self {
        Self {
            addressing,
            sequence_counter: Arc::new(AtomicU32::new(0)),
        }
    }

    fn frame_bytes(
        &self,
        visca_bytes: &[u8],
        kind: CommandKind,
        buffer_manager: &BufferManager,
    ) -> Bytes {
        // Delegate to frame_bytes_with_meta to avoid duplication/mismatch entirely
        self.frame_bytes_with_meta(visca_bytes, kind, buffer_manager)
            .0
    }

    fn extract_response(&self, framed_bytes: &[u8]) -> Result<Bytes, Error> {
        self.sony_extract_payload(framed_bytes)
    }

    fn extract_with_meta(&self, framed: Bytes) -> Result<(Bytes, FrameMeta), Error> {
        if framed.len() < SonyHeader::SIZE {
            return Err(Error::ParseError(Cow::Borrowed(
                "Sony response too short for header",
            )));
        }

        let header_bytes = &framed[..SonyHeader::SIZE];
        let header = SonyHeader::decode(header_bytes).ok_or(Error::ParseError(Cow::Borrowed(
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

        let expected_payload_len = framed.len() - SonyHeader::SIZE;
        if header.payload_length as usize != expected_payload_len {
            return Err(Error::ParseError(Cow::Owned(format!(
                "Sony header length mismatch: header says {header_length}, actual payload is {expected_payload_len}",
                header_length = header.payload_length
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

    fn frame_bytes_with_meta(
        &self,
        visca_bytes: &[u8],
        kind: CommandKind,
        buffer_manager: &BufferManager,
    ) -> (Bytes, FrameMeta) {
        if visca_bytes.is_empty() {
            return (Bytes::new(), FrameMeta { sequence: None });
        }

        // Allocate exactly once – this is the sequence written to the header *and* returned in meta
        let sequence = self.sequence_counter.fetch_add(1, Ordering::Relaxed);

        let normalized_addr = normalize_address(visca_bytes[0], kind, self.addressing);
        let header = match kind {
            CommandKind::Inquiry => SonyHeader::new_inquiry(visca_bytes.len(), sequence),
            CommandKind::Command => SonyHeader::new_command(visca_bytes.len(), sequence),
        };

        let mut envelope = buffer_manager.alloc_send_buffer();
        envelope.reserve(SonyHeader::SIZE + visca_bytes.len());
        envelope.extend_from_slice(&header.encode());
        envelope.extend_from_slice(&[normalized_addr]);
        envelope.extend_from_slice(&visca_bytes[1..]);
        let framed = envelope.freeze();

        (
            framed,
            FrameMeta {
                sequence: Some(sequence),
            },
        )
    }
}

/// Normalize the device address byte based on addressing mode.
///
/// Per VISCA-over-IP spec, the device address is always 0x81.
/// Exception: broadcast inquiries (0x88) are preserved for compatibility.
fn normalize_address(original_addr: u8, kind: CommandKind, addressing: AddressingMode) -> u8 {
    match addressing {
        AddressingMode::Serial => {
            // Serial mode: preserve the original address
            original_addr
        }
        AddressingMode::Ip => {
            // IP mode: normalize to 0x81, except for broadcast inquiries
            const BROADCAST_ADDR: u8 = 0x88;
            const NORMALIZED_ADDR: u8 = 0x81;

            if original_addr == BROADCAST_ADDR && kind == CommandKind::Inquiry {
                // Allow broadcast inquiries in IP mode
                BROADCAST_ADDR
            } else {
                // All other cases: normalize to 0x81
                NORMALIZED_ADDR
            }
        }
    }
}

impl SonyEncapsulated {
    fn sony_extract_payload(&self, framed_bytes: &[u8]) -> Result<Bytes, Error> {
        if framed_bytes.len() < SonyHeader::SIZE {
            return Err(Error::ParseError(Cow::Borrowed(
                "Sony response too short for header",
            )));
        }

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

        let expected_payload_len = framed_bytes.len() - SonyHeader::SIZE;
        if header.payload_length as usize != expected_payload_len {
            return Err(Error::ParseError(Cow::Owned(format!(
                "Sony header length mismatch: header says {header_length}, actual payload is {expected_payload_len}",
                header_length = header.payload_length
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
    use crate::{command::bytes::VISCA_TERMINATOR, transport::buffer::BufferConfig};

    fn test_buffer_manager() -> BufferManager {
        BufferManager::new(BufferConfig::default())
    }

    #[test]
    fn test_raw_visca_passthrough() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]; // Power On

        let framed = envelope.frame_bytes(&visca_cmd, CommandKind::Command, &test_buffer_manager());
        assert_eq!(&framed[..], &visca_cmd[..]);

        let extracted = envelope
            .extract_response(&visca_cmd)
            .expect("raw visca should extract unchanged");
        assert_eq!(&extracted[..], &visca_cmd[..]);
    }

    #[test]
    fn test_sony_encapsulation_command() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]; // Power On

        let framed = envelope.frame_bytes(&visca_cmd, CommandKind::Command, &test_buffer_manager());

        // Check it has Sony header (8 bytes) + VISCA payload
        assert_eq!(framed.len(), SonyHeader::SIZE + visca_cmd.len());

        // Verify header format (basic checks)
        assert_eq!(framed[0], 0x01); // Command type
        assert_eq!(framed[1], 0x00); // High nibble of command type

        // Payload length
        let payload_len = ((framed[2] as usize) << 8) | (framed[3] as usize);
        assert_eq!(payload_len, visca_cmd.len());

        // Check VISCA payload starts after header
        assert_eq!(&framed[8..], &visca_cmd[..]);
    }

    #[test]
    fn test_sony_encapsulation_inquiry() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_inquiry = vec![0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR]; // Power Status Inquiry

        let framed =
            envelope.frame_bytes(&visca_inquiry, CommandKind::Inquiry, &test_buffer_manager());

        // Check it has Sony header (8 bytes) + VISCA payload
        assert_eq!(framed.len(), SonyHeader::SIZE + visca_inquiry.len());

        // Verify header format for inquiry
        assert_eq!(framed[0], 0x01); // Inquiry type
        assert_eq!(framed[1], 0x10); // High nibble indicates inquiry
    }

    #[test]
    fn test_sony_response_extraction() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);

        // Create a mock Sony response: 8-byte header + VISCA ACK
        let visca_ack = vec![0x90, 0x41, VISCA_TERMINATOR]; // ACK for socket 1
        let header = SonyHeader::new_reply(visca_ack.len(), 42); // sequence 42
        let mut sony_response = Vec::new();
        sony_response.extend_from_slice(&header.encode());
        sony_response.extend_from_slice(&visca_ack);

        let extracted = envelope
            .extract_response(&sony_response)
            .expect("should extract Sony payload");
        assert_eq!(&extracted[..], &visca_ack[..]);
    }

    #[test]
    fn test_sony_sequence_increment() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

        let framed1 =
            envelope.frame_bytes(&visca_cmd, CommandKind::Command, &test_buffer_manager());
        let seq1 = u32::from_be_bytes([framed1[4], framed1[5], framed1[6], framed1[7]]);

        let framed2 =
            envelope.frame_bytes(&visca_cmd, CommandKind::Command, &test_buffer_manager());
        let seq2 = u32::from_be_bytes([framed2[4], framed2[5], framed2[6], framed2[7]]);

        assert_eq!(seq2, seq1 + 1);
    }

    #[test]
    fn test_address_normalization_ip_mode_command() {
        // In IP mode, command addresses should always be normalized to 0x81
        let envelope = RawVisca::new(AddressingMode::Ip);
        let visca_cmd = vec![0x82, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]; // Using camera ID 2 (0x82)

        let framed = envelope.frame_bytes(&visca_cmd, CommandKind::Command, &test_buffer_manager());

        // Address should be normalized to 0x81
        assert_eq!(framed[0], 0x81);
        assert_eq!(&framed[1..], &visca_cmd[1..]);
    }

    #[test]
    fn test_address_normalization_ip_mode_inquiry_broadcast() {
        // In IP mode, broadcast inquiries should preserve 0x88
        let envelope = RawVisca::new(AddressingMode::Ip);
        let visca_inquiry = vec![0x88, 0x09, 0x04, 0x00, VISCA_TERMINATOR]; // Broadcast inquiry

        let framed =
            envelope.frame_bytes(&visca_inquiry, CommandKind::Inquiry, &test_buffer_manager());

        // Broadcast address should be preserved
        assert_eq!(framed[0], 0x88);
        assert_eq!(&framed[..], &visca_inquiry[..]);
    }

    #[test]
    fn test_address_normalization_serial_mode() {
        // In Serial mode, addresses should be preserved as-is
        let envelope = RawVisca::new(AddressingMode::Serial);

        // Test various camera IDs
        for camera_id in [0x81, 0x82, 0x83, 0x84, 0x88] {
            let visca_cmd = vec![camera_id, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

            let framed =
                envelope.frame_bytes(&visca_cmd, CommandKind::Command, &test_buffer_manager());

            // Address should not be changed in serial mode
            assert_eq!(framed[0], camera_id);
            assert_eq!(&framed[..], &visca_cmd[..]);
        }
    }

    #[test]
    fn test_extract_with_meta_raw_visca() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let visca_response = vec![0x90, 0x41, VISCA_TERMINATOR];
        let owned_bytes = Bytes::from(visca_response.clone());

        let (payload, meta) = envelope
            .extract_with_meta(owned_bytes.clone())
            .expect("should extract");

        assert_eq!(payload, owned_bytes);
        assert_eq!(meta.sequence, None);
    }

    #[test]
    fn test_extract_with_meta_sony_response() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);

        // Create a mock Sony response with sequence 42
        let visca_ack = vec![0x90, 0x41, VISCA_TERMINATOR];
        let header = SonyHeader::new_reply(visca_ack.len(), 42);
        let mut sony_response = Vec::new();
        sony_response.extend_from_slice(&header.encode());
        sony_response.extend_from_slice(&visca_ack);

        let owned_bytes = Bytes::from(sony_response);

        let (payload, meta) = envelope
            .extract_with_meta(owned_bytes)
            .expect("should extract Sony response");

        assert_eq!(&payload[..], &visca_ack[..]);
        assert_eq!(meta.sequence, Some(42));
    }

    #[test]
    fn test_clone_shares_sequence_counter() {
        // Create an envelope and clone it
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let cloned_envelope = envelope.clone();

        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

        // Frame with original
        let framed1 =
            envelope.frame_bytes(&visca_cmd, CommandKind::Command, &test_buffer_manager());
        let seq1 = u32::from_be_bytes([framed1[4], framed1[5], framed1[6], framed1[7]]);

        // Frame with clone - should get next sequence
        let framed2 =
            cloned_envelope.frame_bytes(&visca_cmd, CommandKind::Command, &test_buffer_manager());
        let seq2 = u32::from_be_bytes([framed2[4], framed2[5], framed2[6], framed2[7]]);

        // Sequences should be consecutive since clones share the counter
        assert_eq!(seq2, seq1 + 1);
    }

    #[test]
    fn test_meta_header_sequence_invariance() {
        // Test that the sequence in FrameMeta matches the sequence written to the header
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

        // Call frame_bytes_with_meta
        let (framed, meta) = envelope.frame_bytes_with_meta(
            &visca_cmd,
            CommandKind::Command,
            &test_buffer_manager(),
        );

        // Extract sequence from the header
        let header_sequence = u32::from_be_bytes([framed[4], framed[5], framed[6], framed[7]]);

        // Assert that meta sequence matches header sequence
        assert_eq!(meta.sequence, Some(header_sequence));
    }

    #[test]
    fn test_concurrent_sequence_allocation() {
        use std::sync::Arc;
        use std::thread;

        // Create a shared envelope
        let envelope = Arc::new(SonyEncapsulated::new(AddressingMode::Ip));
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

        // Number of concurrent threads
        const NUM_THREADS: usize = 10;
        const OPS_PER_THREAD: usize = 100;

        let mut handles = vec![];

        for _ in 0..NUM_THREADS {
            let envelope_clone = Arc::clone(&envelope);
            let cmd = visca_cmd.clone();

            let handle = thread::spawn(move || {
                let mut sequences = Vec::new();
                for _ in 0..OPS_PER_THREAD {
                    let (framed, meta) = envelope_clone.frame_bytes_with_meta(
                        &cmd,
                        CommandKind::Command,
                        &test_buffer_manager(),
                    );

                    // Extract sequence from header
                    let header_seq =
                        u32::from_be_bytes([framed[4], framed[5], framed[6], framed[7]]);

                    // Verify meta matches header
                    assert_eq!(
                        meta.sequence,
                        Some(header_seq),
                        "Meta/header sequence mismatch"
                    );

                    sequences.push(header_seq);
                }
                sequences
            });
            handles.push(handle);
        }

        // Collect all sequences from all threads
        let mut all_sequences = Vec::new();
        for handle in handles {
            let sequences = handle.join().expect("Thread should not panic");
            all_sequences.extend(sequences);
        }

        // Verify we got the expected number of sequences
        assert_eq!(all_sequences.len(), NUM_THREADS * OPS_PER_THREAD);

        // Sort and verify all sequences are unique (no duplicates)
        all_sequences.sort_unstable();
        for i in 1..all_sequences.len() {
            assert_ne!(
                all_sequences[i],
                all_sequences[i - 1],
                "Found duplicate sequence numbers"
            );
        }
    }
}
