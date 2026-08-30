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

/// Sony VISCA-over-IP carries a complete VISCA payload in one 8-byte-header
/// envelope. The protocol permits one through sixteen payload bytes.
const MIN_SONY_VISCA_PAYLOAD_LENGTH: usize = 1;
const MAX_SONY_VISCA_PAYLOAD_LENGTH: usize = 16;

/// The sequence provenance carried by a framed VISCA message.
///
/// Sony's header has space for a 32-bit sequence, but some cameras return only
/// the low 16 bits in a reply while leaving the rest of the header in place.
/// A zero upper half is therefore not proof that the camera sent a genuine
/// small 32-bit sequence: it is represented as [`Self::Lower16`] so the engine
/// can apply its collision-safe fallback. Outgoing Sony frames are always
/// represented as [`Self::Full32`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameSequence {
    /// A sequence value whose complete 32-bit identity is present.
    Full32(u32),
    /// A Sony response carrying only a potentially truncated low 16-bit value.
    Lower16(u16),
}

impl FrameSequence {
    /// Return the numeric sequence value represented by this metadata.
    ///
    /// This is used at the transport/engine boundary for outgoing metadata;
    /// receive-side callers should preserve the variant so provenance is not
    /// lost before correlation.
    #[inline]
    pub const fn value(self) -> u32 {
        match self {
            Self::Full32(value) => value,
            Self::Lower16(value) => value as u32,
        }
    }
}

/// Metadata extracted from or used during VISCA framing operations.
///
/// Raw VISCA frames do not carry request sequence numbers, so
/// [`FrameMeta::sequence`] is `None` for [`RawVisca`]. Sony encapsulated frames
/// carry typed sequence provenance that the runtime uses to correlate replies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameMeta {
    /// Sequence provenance for Sony protocol, `None` for raw VISCA.
    pub sequence: Option<FrameSequence>,
}

/// Trait for protocol envelope implementations.
///
/// This trait is sealed and can only be implemented by the provided envelope types.
/// Each envelope type provides compile-time protocol selection, eliminating runtime
/// branches in the hot path.
pub trait Envelope: private::Sealed + Send + Sync + 'static {
    /// Whether this envelope supports sequence-based request/response correlation.
    ///
    /// When `true`, the runtime can correlate concurrent requests exactly
    /// because each response carries a sequence number that identifies the
    /// original request.
    ///
    /// When `false`, correlation must come from VISCA evidence and bounded
    /// owner policy rather than an envelope identity. The raw owner gates
    /// unacknowledged commands and applies its separately documented inquiry
    /// route/FIFO policy.
    ///
    /// # Protocol Implications
    ///
    /// - **Raw VISCA** (`RawVisca`): Returns `false` - no sequence numbers. The
    ///   owner permits only one unacknowledged command per target, then uses
    ///   the socket assigned by its ACK; it never guesses command ownership by
    ///   FIFO order. Inquiry replies use content/route evidence and a
    ///   per-target FIFO fallback, so concurrent raw inquiries remain bounded
    ///   by that weaker correlation model.
    ///
    /// - **Sony Encapsulated** (`SonyEncapsulated`): Returns `true` - 8-byte header
    ///   includes sequence numbers for reliable concurrent operation.
    const SUPPORTS_SEQUENCE_CORRELATION: bool;

    /// Create a new envelope instance with the given addressing mode.
    fn new(addressing: AddressingMode) -> Self;

    /// Frame VISCA bytes into a reusable buffer with zero allocations.
    ///
    /// This method writes the framed bytes directly into the provided buffer,
    /// avoiding per-send allocations. On success, the buffer is cleared before
    /// the new frame is written. On failure, it is left unchanged.
    ///
    /// # Arguments
    ///
    /// * `visca_bytes` - The VISCA command bytes to frame
    /// * `kind` - The command kind (Command or Inquiry)
    /// * `out` - The output buffer to write into
    ///
    /// # Returns
    ///
    /// Metadata about the framing operation (sequence number for Sony, None for Raw),
    /// or a protocol validation error.
    fn frame_into(
        &self,
        visca_bytes: &[u8],
        kind: CommandKind,
        out: &mut bytes::BytesMut,
    ) -> Result<FrameMeta, Error>;

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
    ) -> Result<FrameMeta, Error> {
        out.clear();

        if visca_bytes.is_empty() {
            return Ok(FrameMeta { sequence: None });
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

        Ok(FrameMeta { sequence: None })
    }

    fn extract_response(&self, framed_bytes: &[u8]) -> Result<Bytes, Error> {
        Ok(Bytes::copy_from_slice(framed_bytes))
    }

    fn extract_with_meta(&self, framed: Bytes) -> Result<(Bytes, FrameMeta), Error> {
        Ok((framed, FrameMeta { sequence: None }))
    }
}

impl RawVisca {
    pub(crate) fn frame_into_with_sequence(
        &self,
        visca_bytes: &[u8],
        kind: CommandKind,
        requested_sequence: Option<u32>,
        out: &mut bytes::BytesMut,
    ) -> Result<FrameMeta, Error> {
        if requested_sequence.is_some() {
            return Err(Error::InvalidRequest(
                "raw VISCA framing cannot accept an explicit Sony sequence".into(),
            ));
        }
        self.frame_into(visca_bytes, kind, out)
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
    ) -> Result<FrameMeta, Error> {
        self.frame_into_with_sequence(visca_bytes, kind, None, out)
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

        validate_sony_response_header(header)?;

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
                sequence: Some(if header.sequence_number >> 16 == 0 {
                    // A zero upper half is ambiguous on the wire: it may be a
                    // genuine small full-width sequence or a camera-truncated
                    // reply. Preserve that uncertainty for engine correlation.
                    FrameSequence::Lower16(header.sequence_number as u16)
                } else {
                    FrameSequence::Full32(header.sequence_number)
                }),
            },
        ))
    }
}

impl SonyEncapsulated {
    /// Seed the sequence allocator for production-path correlation tests.
    #[cfg(all(test, feature = "blocking"))]
    pub(crate) fn set_sequence_for_test(&self, sequence: u32) {
        self.sequence_counter.store(sequence, Ordering::Relaxed);
    }

    pub(crate) fn frame_into_with_sequence(
        &self,
        visca_bytes: &[u8],
        kind: CommandKind,
        requested_sequence: Option<u32>,
        out: &mut bytes::BytesMut,
    ) -> Result<FrameMeta, Error> {
        if visca_bytes.is_empty() {
            if requested_sequence.is_some() {
                return Err(Error::InvalidRequest(
                    "an explicit Sony sequence requires a non-empty VISCA message".into(),
                ));
            }
            // Empty input is the legacy no-op sentinel, not a Sony wire
            // payload. Preserve its existing buffer-clearing behavior.
            out.clear();
            return Ok(FrameMeta { sequence: None });
        }

        // Validate before either advancing the allocator or touching `out` so
        // a rejected raw request cannot consume a sequence or leave a partial
        // Sony frame behind.
        validate_sony_request_payload_length(visca_bytes.len())?;

        // Allocate only for a new logical message. Retries provide their
        // engine-owned sequence explicitly and must not advance this counter.
        let sequence = requested_sequence
            .unwrap_or_else(|| self.sequence_counter.fetch_add(1, Ordering::Relaxed));

        // Normalize the address byte
        let normalized_addr = normalize_address(visca_bytes[0], kind, self.addressing);

        // Build Sony header
        let header = match kind {
            CommandKind::Inquiry => SonyHeader::new_inquiry(visca_bytes.len(), sequence),
            CommandKind::Command => SonyHeader::new_command(visca_bytes.len(), sequence),
        };

        // Write header + normalized address + remaining bytes directly into out
        out.clear();
        out.reserve(SonyHeader::SIZE + visca_bytes.len());
        out.extend_from_slice(&header.encode());
        out.extend_from_slice(&[normalized_addr]);
        out.extend_from_slice(&visca_bytes[1..]);

        // Sony request framing always emits and reports the complete 32-bit
        // sequence. Only response extraction may produce Lower16 metadata.
        Ok(FrameMeta {
            sequence: Some(FrameSequence::Full32(sequence)),
        })
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

        validate_sony_response_header(header)?;

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

/// Validate the Sony header fields that are meaningful at the response
/// envelope boundary.  Commands, inquiries, and control messages are valid
/// Sony *wire* payload types, but they are not replies and must never enter the
/// response correlation path as if they were one.
fn validate_sony_response_header(header: SonyHeader) -> Result<(), Error> {
    if header.payload_type != PayloadType::ViscaReply {
        return Err(Error::ParseError(Cow::Owned(format!(
            "Unexpected Sony payload type in response: {:?}",
            header.payload_type
        ))));
    }

    let payload_length = usize::from(header.payload_length);
    if !(MIN_SONY_VISCA_PAYLOAD_LENGTH..=MAX_SONY_VISCA_PAYLOAD_LENGTH).contains(&payload_length) {
        return Err(Error::ParseError(Cow::Owned(format!(
            "Sony payload length must be between {MIN_SONY_VISCA_PAYLOAD_LENGTH} and {MAX_SONY_VISCA_PAYLOAD_LENGTH} bytes, got {payload_length}"
        ))));
    }

    Ok(())
}

/// Validate an outgoing non-empty Sony VISCA payload before framing it.
fn validate_sony_request_payload_length(payload_length: usize) -> Result<(), Error> {
    if !(MIN_SONY_VISCA_PAYLOAD_LENGTH..=MAX_SONY_VISCA_PAYLOAD_LENGTH).contains(&payload_length) {
        return Err(Error::InvalidRequest(Cow::Owned(format!(
            "Sony VISCA-over-IP payload length must be between {MIN_SONY_VISCA_PAYLOAD_LENGTH} and {MAX_SONY_VISCA_PAYLOAD_LENGTH} bytes, got {payload_length}"
        ))));
    }

    Ok(())
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

        let meta = envelope
            .frame_into(&visca_cmd, CommandKind::Command, &mut out)
            .expect("raw framing is infallible for valid test input");

        // Serial mode: should preserve address byte (no normalization)
        assert_eq!(&out[..], &visca_cmd[..]);
        assert_eq!(meta.sequence, None);
    }

    #[test]
    fn test_raw_visca_frame_into_normalizes_address_ip_mode() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let visca_cmd = vec![0x82, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        let meta = envelope
            .frame_into(&visca_cmd, CommandKind::Command, &mut out)
            .expect("raw framing is infallible for valid test input");

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

        let meta = envelope
            .frame_into(&visca_cmd, CommandKind::Inquiry, &mut out)
            .expect("raw framing is infallible for valid test input");

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

        let meta = envelope
            .frame_into(&visca_cmd, CommandKind::Command, &mut out)
            .expect("raw framing is infallible for valid test input");

        // Broadcast address on Command (not Inquiry) should be normalized to 0x81
        assert_eq!(out[0], 0x81);
        assert_eq!(&out[1..], &visca_cmd[1..]);
        assert_eq!(meta.sequence, None);
    }

    #[test]
    fn test_raw_visca_frame_into_empty_input() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let mut out = bytes::BytesMut::new();

        let meta = envelope
            .frame_into(&[], CommandKind::Command, &mut out)
            .expect("raw framing is infallible for empty test input");

        assert_eq!(out.len(), 0);
        assert_eq!(meta.sequence, None);
    }

    #[test]
    fn test_raw_visca_frame_into_clears_buffer() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::from(&b"old data"[..]);

        envelope
            .frame_into(&visca_cmd, CommandKind::Command, &mut out)
            .expect("raw framing is infallible for valid test input");

        // Should clear old data and contain only new framed command
        assert_eq!(&out[..], &visca_cmd[..]);
    }

    // SonyEncapsulated frame_into tests

    #[test]
    fn test_sony_frame_into_command_with_sequence() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        let meta = envelope
            .frame_into(&visca_cmd, CommandKind::Command, &mut out)
            .expect("Sony framing accepts this payload");

        // Should have Sony header (8 bytes) + VISCA payload
        assert_eq!(out.len(), SonyHeader::SIZE + visca_cmd.len());

        // Check sequence number was assigned
        assert_eq!(meta.sequence, Some(FrameSequence::Full32(0)));

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

        let meta = envelope
            .frame_into(&visca_cmd, CommandKind::Inquiry, &mut out)
            .expect("Sony framing accepts this payload");

        // Check sequence number
        assert_eq!(meta.sequence, Some(FrameSequence::Full32(0)));

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
        let meta1 = envelope
            .frame_into(&visca_cmd, CommandKind::Command, &mut out)
            .expect("Sony framing accepts this payload");
        let meta2 = envelope
            .frame_into(&visca_cmd, CommandKind::Command, &mut out)
            .expect("Sony framing accepts this payload");
        let meta3 = envelope
            .frame_into(&visca_cmd, CommandKind::Command, &mut out)
            .expect("Sony framing accepts this payload");

        // Sequence numbers should increment
        assert_eq!(meta1.sequence, Some(FrameSequence::Full32(0)));
        assert_eq!(meta2.sequence, Some(FrameSequence::Full32(1)));
        assert_eq!(meta3.sequence, Some(FrameSequence::Full32(2)));
    }

    #[test]
    fn explicit_sony_retransmission_reuses_sequence_without_advancing_counter() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        let first = envelope
            .frame_into(&visca_cmd, CommandKind::Command, &mut out)
            .expect("Sony framing accepts this payload");
        let first_wire = out.to_vec();
        assert_eq!(first.sequence, Some(FrameSequence::Full32(0)));

        let retry = envelope
            .frame_into_with_sequence(
                &visca_cmd,
                CommandKind::Command,
                first.sequence.map(FrameSequence::value),
                &mut out,
            )
            .expect("explicit Sony retry sequence is valid");
        assert_eq!(retry.sequence, Some(FrameSequence::Full32(0)));
        assert_eq!(out.as_ref(), first_wire.as_slice());

        let next = envelope
            .frame_into(&visca_cmd, CommandKind::Command, &mut out)
            .expect("Sony framing accepts this payload");
        assert_eq!(next.sequence, Some(FrameSequence::Full32(1)));
    }

    #[test]
    fn sony_outbound_frames_one_byte_payload() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_payload = [0x85];
        let mut out = bytes::BytesMut::new();

        let meta = envelope
            .frame_into_with_sequence(&visca_payload, CommandKind::Command, None, &mut out)
            .expect("one-byte Sony payload is within the wire limit");

        assert_eq!(meta.sequence, Some(FrameSequence::Full32(0)));
        let header = SonyHeader::decode(&out[..SonyHeader::SIZE]).expect("valid header");
        assert_eq!(header.payload_length, 1);
        assert_eq!(&out[SonyHeader::SIZE..], &[0x81]);
    }

    #[test]
    fn sony_outbound_frames_sixteen_byte_payload() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let mut visca_payload = [0xa5; MAX_SONY_VISCA_PAYLOAD_LENGTH];
        visca_payload[0] = 0x85;
        let mut out = bytes::BytesMut::new();

        let meta = envelope
            .frame_into_with_sequence(&visca_payload, CommandKind::Command, None, &mut out)
            .expect("sixteen-byte Sony payload is within the wire limit");

        assert_eq!(meta.sequence, Some(FrameSequence::Full32(0)));
        let header = SonyHeader::decode(&out[..SonyHeader::SIZE]).expect("valid header");
        assert_eq!(
            header.payload_length as usize,
            MAX_SONY_VISCA_PAYLOAD_LENGTH
        );
        assert_eq!(out[SonyHeader::SIZE], 0x81);
        assert_eq!(&out[SonyHeader::SIZE + 1..], &visca_payload[1..]);
    }

    #[test]
    fn sony_outbound_rejects_seventeen_byte_payload_before_mutation_or_sequence_allocation() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let invalid_payload = [0x81; MAX_SONY_VISCA_PAYLOAD_LENGTH + 1];
        let mut out = bytes::BytesMut::from(&b"prior framed bytes"[..]);
        let original_out = out.clone();

        for requested_sequence in [None, Some(0x1234_5678)] {
            assert!(matches!(
                envelope.frame_into_with_sequence(
                    &invalid_payload,
                    CommandKind::Command,
                    requested_sequence,
                    &mut out,
                ),
                Err(Error::InvalidRequest(_))
            ));
            assert_eq!(out.as_ref(), original_out.as_ref());
        }

        let valid_payload = [0x81];
        let meta = envelope
            .frame_into_with_sequence(&valid_payload, CommandKind::Command, None, &mut out)
            .expect("a rejected payload must not consume the first sequence");
        assert_eq!(meta.sequence, Some(FrameSequence::Full32(0)));
    }

    #[test]
    fn sony_frame_into_reports_invalid_nonempty_payload_without_mutation() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let invalid_payload = [0x81; MAX_SONY_VISCA_PAYLOAD_LENGTH + 1];
        let mut out = bytes::BytesMut::from(&b"prior framed bytes"[..]);
        let original_out = out.clone();

        let result = envelope.frame_into(&invalid_payload, CommandKind::Command, &mut out);

        assert!(matches!(result, Err(Error::InvalidRequest(_))));
        assert_eq!(out.as_ref(), original_out.as_ref());
    }

    #[test]
    fn raw_framing_rejects_an_explicit_sony_sequence() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        assert!(matches!(
            envelope.frame_into_with_sequence(&visca_cmd, CommandKind::Command, Some(0), &mut out,),
            Err(Error::InvalidRequest(_))
        ));
    }

    #[test]
    fn test_sony_frame_into_normalizes_address_ip_mode() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_cmd = vec![0x85, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        envelope
            .frame_into(&visca_cmd, CommandKind::Command, &mut out)
            .expect("Sony framing accepts this payload");

        // Address byte should be normalized to 0x81
        assert_eq!(out[SonyHeader::SIZE], 0x81);
        assert_eq!(&out[SonyHeader::SIZE + 1..], &visca_cmd[1..]);
    }

    #[test]
    fn test_sony_frame_into_preserves_broadcast_inquiry() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_cmd = vec![0x88, 0x09, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        envelope
            .frame_into(&visca_cmd, CommandKind::Inquiry, &mut out)
            .expect("Sony framing accepts this payload");

        // Broadcast inquiry (0x88) should be preserved
        assert_eq!(out[SonyHeader::SIZE], 0x88);
    }

    #[test]
    fn sony_outbound_empty_input_is_legacy_noop_without_sequence_allocation() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let mut out = bytes::BytesMut::from(&b"old frame"[..]);

        let meta = envelope
            .frame_into(&[], CommandKind::Command, &mut out)
            .expect("empty input remains an explicit no-op");

        assert_eq!(out.len(), 0);
        assert_eq!(meta.sequence, None);

        let valid_payload = [0x81];
        let next = envelope
            .frame_into_with_sequence(&valid_payload, CommandKind::Command, None, &mut out)
            .expect("empty no-op must not consume a sequence");
        assert_eq!(next.sequence, Some(FrameSequence::Full32(0)));

        let original_out = out.clone();
        assert!(matches!(
            envelope.frame_into_with_sequence(&[], CommandKind::Command, Some(7), &mut out),
            Err(Error::InvalidRequest(_))
        ));
        assert_eq!(out.as_ref(), original_out.as_ref());
    }

    #[test]
    fn test_sony_frame_into_clears_buffer() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::from(&b"old data that should be cleared"[..]);

        envelope
            .frame_into(&visca_cmd, CommandKind::Command, &mut out)
            .expect("Sony framing accepts this payload");

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
        let meta1 = envelope
            .frame_into(&cmd1, CommandKind::Command, &mut out)
            .expect("Sony framing accepts this payload");
        assert_eq!(meta1.sequence, Some(FrameSequence::Full32(0)));

        // Reuse buffer for second command
        let meta2 = envelope
            .frame_into(&cmd2, CommandKind::Inquiry, &mut out)
            .expect("Sony framing accepts this payload");
        let len2 = out.len();
        assert_eq!(meta2.sequence, Some(FrameSequence::Full32(1)));

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
    fn test_extract_with_meta_sony_zero_upper_sequence_is_lower16() {
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
        assert_eq!(meta.sequence, Some(FrameSequence::Lower16(42)));
    }

    #[test]
    fn test_extract_with_meta_sony_nonzero_upper_sequence_is_full32() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_ack = vec![0x90, 0x41, VISCA_TERMINATOR];
        let header = SonyHeader::new_reply(visca_ack.len(), 0x1234_002a);
        let mut sony_response = header.encode().to_vec();
        sony_response.extend_from_slice(&visca_ack);

        let (payload, meta) = envelope
            .extract_with_meta(Bytes::from(sony_response))
            .expect("should extract Sony response");

        assert_eq!(&payload[..], &visca_ack[..]);
        assert_eq!(meta.sequence, Some(FrameSequence::Full32(0x1234_002a)));
    }
}
