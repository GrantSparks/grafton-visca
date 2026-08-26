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
    transport::builder::AddressingMode,
    Error,
};

/// Metadata extracted from or used during VISCA framing operations.
///
/// Raw VISCA frames do not carry request sequence numbers, so
/// [`FrameMeta::sequence`] is `None` for [`RawVisca`]. Sony encapsulated frames
/// carry a sequence number that the runtime uses to correlate replies.
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
    /// Whether this envelope supports sequence-based request/response correlation.
    ///
    /// When `true`, the runtime can send multiple concurrent inquiries because
    /// each response carries a sequence number that identifies the original request.
    ///
    /// When `false`, the runtime must serialize inquiries because responses cannot
    /// be reliably correlated to requests without sequence numbers.
    ///
    /// # Protocol Implications
    ///
    /// - **Raw VISCA** (`RawVisca`): Returns `false` - no sequence numbers, responses
    ///   must be matched by content or FIFO ordering, which is unreliable for
    ///   concurrent requests.
    ///
    /// - **Sony Encapsulated** (`SonyEncapsulated`): Returns `true` - 8-byte header
    ///   includes sequence numbers for reliable concurrent operation.
    const SUPPORTS_SEQUENCE_CORRELATION: bool;

    /// Create a new envelope instance with the given addressing mode.
    fn new(addressing: AddressingMode) -> Self;

    /// Frame VISCA bytes into a reusable buffer with zero allocations.
    ///
    /// This method writes the framed bytes directly into the provided buffer,
    /// avoiding per-send allocations. The buffer is cleared before use.
    ///
    /// # Arguments
    ///
    /// * `visca_bytes` - The VISCA command bytes to frame
    /// * `kind` - The command kind (Command or Inquiry)
    /// * `out` - The output buffer to write into (will be cleared first)
    ///
    /// # Returns
    ///
    /// Metadata about the framing operation (sequence number for Sony, None for Raw)
    fn frame_into(
        &self,
        visca_bytes: &[u8],
        kind: CommandKind,
        out: &mut bytes::BytesMut,
    ) -> FrameMeta;

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

impl RawVisca {
    /// Returns the immutable addressing mode selected for this envelope.
    pub(crate) const fn addressing(self) -> AddressingMode {
        self.addressing
    }
}

impl SonyEncapsulated {
    /// Returns the immutable addressing mode selected for this envelope.
    pub(crate) const fn addressing(&self) -> AddressingMode {
        self.addressing
    }
}

impl Envelope for RawVisca {
    /// Raw VISCA does not support sequence correlation - responses cannot be
    /// reliably matched to requests when multiple inquiries are in flight.
    const SUPPORTS_SEQUENCE_CORRELATION: bool = false;

    fn new(addressing: AddressingMode) -> Self {
        Self { addressing }
    }

    fn frame_into(
        &self,
        visca_bytes: &[u8],
        kind: CommandKind,
        out: &mut bytes::BytesMut,
    ) -> FrameMeta {
        out.clear();

        if visca_bytes.is_empty() {
            return FrameMeta { sequence: None };
        }

        // Normalize the address byte if needed
        let normalized_addr = normalize_address(visca_bytes[0], kind, self.addressing);

        if normalized_addr == visca_bytes[0] {
            // No normalization needed - copy as-is
            out.extend_from_slice(visca_bytes);
        } else {
            // Write normalized address followed by rest of bytes
            out.reserve(visca_bytes.len());
            out.extend_from_slice(&[normalized_addr]);
            out.extend_from_slice(&visca_bytes[1..]);
        }

        FrameMeta { sequence: None }
    }

    fn extract_response(&self, framed_bytes: &[u8]) -> Result<Bytes, Error> {
        Ok(Bytes::copy_from_slice(framed_bytes))
    }

    fn extract_with_meta(&self, framed: Bytes) -> Result<(Bytes, FrameMeta), Error> {
        Ok((framed, FrameMeta { sequence: None }))
    }
}

impl Envelope for SonyEncapsulated {
    /// Sony encapsulated protocol supports sequence correlation via 8-byte header,
    /// enabling reliable concurrent inquiry execution.
    const SUPPORTS_SEQUENCE_CORRELATION: bool = true;

    fn new(addressing: AddressingMode) -> Self {
        Self {
            addressing,
            sequence_counter: Arc::new(AtomicU32::new(0)),
        }
    }

    fn frame_into(
        &self,
        visca_bytes: &[u8],
        kind: CommandKind,
        out: &mut bytes::BytesMut,
    ) -> FrameMeta {
        out.clear();

        if visca_bytes.is_empty() {
            return FrameMeta { sequence: None };
        }

        // Allocate sequence number atomically
        let sequence = self.sequence_counter.fetch_add(1, Ordering::Relaxed);

        // Normalize the address byte
        let normalized_addr = normalize_address(visca_bytes[0], kind, self.addressing);

        // Build Sony header
        let header = match kind {
            CommandKind::Inquiry => SonyHeader::new_inquiry(visca_bytes.len(), sequence),
            CommandKind::Command => SonyHeader::new_command(visca_bytes.len(), sequence),
        };

        // Write header + normalized address + remaining bytes directly into out
        out.reserve(SonyHeader::SIZE + visca_bytes.len());
        out.extend_from_slice(&header.encode());
        out.extend_from_slice(&[normalized_addr]);
        out.extend_from_slice(&visca_bytes[1..]);

        FrameMeta {
            sequence: Some(sequence),
        }
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
    use crate::command::bytes::VISCA_TERMINATOR;

    // Sequence correlation capability tests
    //
    // These are compile-time constants, verified by the type system.
    // The constants are tested implicitly through their usage in runtime configuration.

    #[test]
    fn test_envelope_sequence_correlation_constants_are_opposite() {
        // Verify that the two envelope types have opposite sequence correlation support.
        // This ensures one serializes and one allows concurrency.
        assert_ne!(
            RawVisca::SUPPORTS_SEQUENCE_CORRELATION,
            SonyEncapsulated::SUPPORTS_SEQUENCE_CORRELATION,
            "RawVisca and SonyEncapsulated must have different sequence correlation support"
        );
    }

    // RawVisca frame_into tests

    #[test]
    fn test_raw_visca_frame_into_passthrough_serial_mode() {
        let envelope = RawVisca::new(AddressingMode::Serial);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        let meta = envelope.frame_into(&visca_cmd, CommandKind::Command, &mut out);

        // Serial mode: should preserve address byte (no normalization)
        assert_eq!(&out[..], &visca_cmd[..]);
        assert_eq!(meta.sequence, None);
    }

    #[test]
    fn test_raw_visca_frame_into_normalizes_address_ip_mode() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let visca_cmd = vec![0x82, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        let meta = envelope.frame_into(&visca_cmd, CommandKind::Command, &mut out);

        // IP mode: should normalize 0x82 to 0x81
        assert_eq!(out[0], 0x81);
        assert_eq!(&out[1..], &visca_cmd[1..]);
        assert_eq!(meta.sequence, None);
    }

    #[test]
    fn test_raw_visca_frame_into_preserves_broadcast_inquiry() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let visca_cmd = vec![0x88, 0x09, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        let meta = envelope.frame_into(&visca_cmd, CommandKind::Inquiry, &mut out);

        // Broadcast inquiry (0x88) should be preserved even in IP mode
        assert_eq!(out[0], 0x88);
        assert_eq!(&out[1..], &visca_cmd[1..]);
        assert_eq!(meta.sequence, None);
    }

    #[test]
    fn test_raw_visca_frame_into_normalizes_broadcast_command() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let visca_cmd = vec![0x88, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        let meta = envelope.frame_into(&visca_cmd, CommandKind::Command, &mut out);

        // Broadcast address on Command (not Inquiry) should be normalized to 0x81
        assert_eq!(out[0], 0x81);
        assert_eq!(&out[1..], &visca_cmd[1..]);
        assert_eq!(meta.sequence, None);
    }

    #[test]
    fn test_raw_visca_frame_into_empty_input() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let mut out = bytes::BytesMut::new();

        let meta = envelope.frame_into(&[], CommandKind::Command, &mut out);

        assert_eq!(out.len(), 0);
        assert_eq!(meta.sequence, None);
    }

    #[test]
    fn test_raw_visca_frame_into_clears_buffer() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::from(&b"old data"[..]);

        envelope.frame_into(&visca_cmd, CommandKind::Command, &mut out);

        // Should clear old data and contain only new framed command
        assert_eq!(&out[..], &visca_cmd[..]);
    }

    // SonyEncapsulated frame_into tests

    #[test]
    fn test_sony_frame_into_command_with_sequence() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        let meta = envelope.frame_into(&visca_cmd, CommandKind::Command, &mut out);

        // Should have Sony header (8 bytes) + VISCA payload
        assert_eq!(out.len(), SonyHeader::SIZE + visca_cmd.len());

        // Check sequence number was assigned
        assert_eq!(meta.sequence, Some(0));

        // Decode header to verify
        let header = SonyHeader::decode(&out[..SonyHeader::SIZE]).expect("valid header");
        assert_eq!(header.payload_length as usize, visca_cmd.len());
        assert_eq!(header.sequence_number, 0);
        assert_eq!(header.payload_type, PayloadType::ViscaCommand);

        // Check VISCA payload (should have normalized address 0x81)
        assert_eq!(&out[SonyHeader::SIZE..], &visca_cmd[..]);
    }

    #[test]
    fn test_sony_frame_into_inquiry_with_sequence() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_cmd = vec![0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        let meta = envelope.frame_into(&visca_cmd, CommandKind::Inquiry, &mut out);

        // Check sequence number
        assert_eq!(meta.sequence, Some(0));

        // Decode header to verify inquiry type
        let header = SonyHeader::decode(&out[..SonyHeader::SIZE]).expect("valid header");
        assert_eq!(header.payload_type, PayloadType::ViscaInquiry);
        assert_eq!(header.payload_length as usize, visca_cmd.len());
    }

    #[test]
    fn test_sony_frame_into_increments_sequence() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        // Frame multiple commands
        let meta1 = envelope.frame_into(&visca_cmd, CommandKind::Command, &mut out);
        let meta2 = envelope.frame_into(&visca_cmd, CommandKind::Command, &mut out);
        let meta3 = envelope.frame_into(&visca_cmd, CommandKind::Command, &mut out);

        // Sequence numbers should increment
        assert_eq!(meta1.sequence, Some(0));
        assert_eq!(meta2.sequence, Some(1));
        assert_eq!(meta3.sequence, Some(2));
    }

    #[test]
    fn test_sony_frame_into_normalizes_address_ip_mode() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_cmd = vec![0x85, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        envelope.frame_into(&visca_cmd, CommandKind::Command, &mut out);

        // Address byte should be normalized to 0x81
        assert_eq!(out[SonyHeader::SIZE], 0x81);
        assert_eq!(&out[SonyHeader::SIZE + 1..], &visca_cmd[1..]);
    }

    #[test]
    fn test_sony_frame_into_preserves_broadcast_inquiry() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_cmd = vec![0x88, 0x09, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        envelope.frame_into(&visca_cmd, CommandKind::Inquiry, &mut out);

        // Broadcast inquiry (0x88) should be preserved
        assert_eq!(out[SonyHeader::SIZE], 0x88);
    }

    #[test]
    fn test_sony_frame_into_empty_input() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let mut out = bytes::BytesMut::new();

        let meta = envelope.frame_into(&[], CommandKind::Command, &mut out);

        assert_eq!(out.len(), 0);
        assert_eq!(meta.sequence, None);
    }

    #[test]
    fn test_sony_frame_into_clears_buffer() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::from(&b"old data that should be cleared"[..]);

        envelope.frame_into(&visca_cmd, CommandKind::Command, &mut out);

        // Should clear old data and contain only Sony header + VISCA payload
        assert_eq!(out.len(), SonyHeader::SIZE + visca_cmd.len());
    }

    #[test]
    fn test_sony_frame_into_reusable_buffer() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let cmd1 = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let cmd2 = vec![0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        // Frame first command
        let meta1 = envelope.frame_into(&cmd1, CommandKind::Command, &mut out);
        assert_eq!(meta1.sequence, Some(0));

        // Reuse buffer for second command
        let meta2 = envelope.frame_into(&cmd2, CommandKind::Inquiry, &mut out);
        let len2 = out.len();
        assert_eq!(meta2.sequence, Some(1));

        // Buffer should be properly sized for second command
        assert_eq!(len2, SonyHeader::SIZE + cmd2.len());

        // Verify second command is correct
        let header = SonyHeader::decode(&out[..SonyHeader::SIZE]).expect("valid header");
        assert_eq!(header.sequence_number, 1);
        assert_eq!(header.payload_type, PayloadType::ViscaInquiry);
    }

    // Address normalization tests

    #[test]
    fn test_normalize_address_serial_mode_preserves_all() {
        // Serial mode should preserve any address
        assert_eq!(
            normalize_address(0x81, CommandKind::Command, AddressingMode::Serial),
            0x81
        );
        assert_eq!(
            normalize_address(0x82, CommandKind::Command, AddressingMode::Serial),
            0x82
        );
        assert_eq!(
            normalize_address(0x88, CommandKind::Inquiry, AddressingMode::Serial),
            0x88
        );
    }

    #[test]
    fn test_normalize_address_ip_mode_normalizes_commands() {
        // IP mode commands should normalize to 0x81
        assert_eq!(
            normalize_address(0x81, CommandKind::Command, AddressingMode::Ip),
            0x81
        );
        assert_eq!(
            normalize_address(0x82, CommandKind::Command, AddressingMode::Ip),
            0x81
        );
        assert_eq!(
            normalize_address(0x87, CommandKind::Command, AddressingMode::Ip),
            0x81
        );
    }

    #[test]
    fn test_normalize_address_ip_mode_normalizes_inquiries() {
        // IP mode non-broadcast inquiries should normalize to 0x81
        assert_eq!(
            normalize_address(0x81, CommandKind::Inquiry, AddressingMode::Ip),
            0x81
        );
        assert_eq!(
            normalize_address(0x82, CommandKind::Inquiry, AddressingMode::Ip),
            0x81
        );
    }

    #[test]
    fn test_normalize_address_ip_mode_preserves_broadcast_inquiry() {
        // IP mode broadcast inquiry (0x88) should be preserved
        assert_eq!(
            normalize_address(0x88, CommandKind::Inquiry, AddressingMode::Ip),
            0x88
        );
    }

    #[test]
    fn test_normalize_address_ip_mode_normalizes_broadcast_command() {
        // IP mode broadcast command (0x88) should be normalized
        assert_eq!(
            normalize_address(0x88, CommandKind::Command, AddressingMode::Ip),
            0x81
        );
    }

    // Extraction tests (existing)

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
}
