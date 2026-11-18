//! Transport envelope abstraction for different VISCA protocols.
//!
//! This module provides abstraction for protocol envelopes, specifically
//! handling Sony's 8-byte encapsulated VISCA protocol vs raw VISCA bytes.
//!
//! The envelope system now supports compile-time protocol selection through
//! marker types, eliminating runtime branches in the hot path.

use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

use crate::{
    command::{bytes::VISCA_TERMINATOR, CommandKind},
    protocol::sony::{PayloadType, SonyHeader},
    transport::builder::AddressingMode,
    Error, ProfileEnvelope,
};

/// Sony VISCA-over-IP carries a complete VISCA payload in one 8-byte-header
/// envelope. The supported built-in command families need up to seventeen
/// payload bytes (FR7 positions, R7 p. 18). Preparation enforces each profile's
/// supported capacity; this shared framing limit is not a vendor-wide claim.
/// Command, inquiry, and reply payloads end in [`crate::wire::VISCA_TERMINATOR`].
const MIN_SONY_VISCA_PAYLOAD_LENGTH: usize = 1;
const MAX_SONY_VISCA_PAYLOAD_LENGTH: usize = 17;

/// Reply identity decoded once from the Sony header.
///
/// A zero upper half may indicate a camera returning only the low sixteen
/// bits. Preserve that distinction until the engine selects its correlation
/// index; outgoing metadata always carries the allocated full-width number.
#[cfg(any(feature = "blocking", feature = "async"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrameSequence {
    Full32(u32),
    MaybeTruncated(u16),
}

#[cfg(any(feature = "blocking", feature = "async"))]
impl FrameSequence {
    pub(crate) const fn from_reply_header(sequence_number: u32) -> Self {
        if sequence_number >> 16 == 0 {
            Self::MaybeTruncated(sequence_number as u16)
        } else {
            Self::Full32(sequence_number)
        }
    }
}

/// Sequence identity requested for one Sony logical transmission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SonyRequestSequence {
    /// Allocate the identity for a new logical transmission.
    AllocateFresh,
    /// Replay the exact identity of a previously successful transmission.
    #[cfg_attr(
        not(any(feature = "blocking", feature = "async", test)),
        allow(dead_code)
    )]
    Replay(u32),
}

/// Metadata returned when framing an outgoing VISCA message.
///
/// Raw frames have no sequence field. Sony frames carry the complete allocated
/// 32-bit sequence number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FrameMeta {
    /// A raw VISCA frame without envelope correlation metadata.
    Raw,
    /// A Sony frame with its complete sequence number.
    Sony {
        /// The Sony sequence identity carried by this frame.
        sequence: u32,
    },
}

impl FrameMeta {
    /// Returns this frame's Sony correlation identity, if it has one.
    #[must_use]
    pub const fn sequence(self) -> Option<u32> {
        match self {
            Self::Raw => None,
            Self::Sony { sequence } => Some(sequence),
        }
    }
}

/// Trait for protocol envelope implementations.
///
/// This trait is sealed and can only be implemented by the provided envelope types.
/// Each envelope type provides compile-time protocol selection, eliminating runtime
/// branches in the hot path.
pub trait Envelope: private::Sealed + Send + Sync + 'static {
    /// The envelope identity selected by this sealed implementation.
    ///
    /// Raw VISCA uses socket and ordered inquiry evidence. Sony framing adds
    /// an explicit sequence identity to every request and response.
    const KIND: ProfileEnvelope;

    /// Create a new envelope instance with the given addressing mode.
    fn new(addressing: AddressingMode) -> Self;

    /// Frame VISCA bytes into a reusable buffer with zero allocations.
    ///
    /// This method writes the framed bytes directly into the provided buffer,
    /// avoiding per-send allocations. On success, the buffer is cleared before
    /// the new frame is written. On failure, it is left unchanged and no Sony
    /// sequence number is consumed.
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
    /// or a protocol validation error. Empty `visca_bytes` is rejected with
    /// [`Error::InvalidRequest`] by both envelopes. A [`SonyEncapsulated`] VISCA
    /// command or inquiry must additionally be a complete frame ending in `0xFF`.
    fn frame_into(
        &self,
        visca_bytes: &[u8],
        kind: CommandKind,
        out: &mut bytes::BytesMut,
    ) -> Result<FrameMeta, Error>;
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

/// Owner-facing classification of a validated Sony response envelope.
#[cfg(any(feature = "blocking", feature = "async"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SonyResponse<'a> {
    /// A normal VISCA reply with required Sony correlation metadata.
    Visca {
        payload: &'a [u8],
        sequence: FrameSequence,
    },
    /// A Sony transport-control reply, which is diagnostic and never correlates
    /// to a VISCA request.
    Control { code: u16 },
}

#[cfg(any(feature = "blocking", feature = "async"))]
impl RawVisca {
    pub(crate) const fn addressing(&self) -> AddressingMode {
        self.addressing
    }
}

impl Envelope for RawVisca {
    const KIND: ProfileEnvelope = ProfileEnvelope::RawVisca;

    fn new(addressing: AddressingMode) -> Self {
        Self { addressing }
    }

    fn frame_into(
        &self,
        visca_bytes: &[u8],
        _kind: CommandKind,
        out: &mut bytes::BytesMut,
    ) -> Result<FrameMeta, Error> {
        // Reject before touching `out` so a failure leaves it unchanged.
        if visca_bytes.is_empty() {
            return Err(empty_visca_message());
        }

        out.clear();

        // Normalize the address byte if needed
        let normalized_addr = normalize_address(visca_bytes[0], self.addressing);

        if normalized_addr == visca_bytes[0] {
            // No normalization needed - copy as-is
            out.extend_from_slice(visca_bytes);
        } else {
            // Write normalized address followed by rest of bytes
            out.reserve(visca_bytes.len());
            out.extend_from_slice(&[normalized_addr]);
            out.extend_from_slice(&visca_bytes[1..]);
        }

        Ok(FrameMeta::Raw)
    }
}

impl Envelope for SonyEncapsulated {
    const KIND: ProfileEnvelope = ProfileEnvelope::SonyEncapsulated;

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
        self.frame_into_with_sequence(visca_bytes, kind, SonyRequestSequence::AllocateFresh, out)
            .map(|sequence| FrameMeta::Sony { sequence })
    }
}

impl SonyEncapsulated {
    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) const fn addressing(&self) -> AddressingMode {
        self.addressing
    }

    fn next_sequence(&self, request: SonyRequestSequence) -> u32 {
        match request {
            SonyRequestSequence::AllocateFresh => {
                self.sequence_counter.fetch_add(1, Ordering::Relaxed)
            }
            SonyRequestSequence::Replay(sequence) => sequence,
        }
    }

    pub(crate) fn frame_into_with_sequence(
        &self,
        visca_bytes: &[u8],
        kind: CommandKind,
        requested_sequence: SonyRequestSequence,
        out: &mut bytes::BytesMut,
    ) -> Result<u32, Error> {
        // Validate before either advancing the allocator or touching `out` so
        // a rejected raw request cannot consume a sequence or leave a partial
        // Sony frame behind.
        if visca_bytes.is_empty() {
            return Err(empty_visca_message());
        }
        sony_visca_payload(visca_bytes)?;

        // Allocate only for a new logical message. Retries provide their
        // engine-owned sequence explicitly and must not advance this counter.
        let sequence = self.next_sequence(requested_sequence);

        // Normalize the address byte
        let normalized_addr = normalize_address(visca_bytes[0], self.addressing);

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
        // sequence. Only response extraction may produce uncertain metadata.
        Ok(sequence)
    }

    /// Frame Sony's sequence-number RESET control command.
    ///
    /// The RESET header sequence is ignored by the receiver. Resetting the
    /// local allocator here ensures the first subsequently framed VISCA
    /// request uses sequence zero as required by the control operation.
    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub(crate) fn frame_sequence_reset_into(&self, out: &mut bytes::BytesMut) {
        let header = SonyHeader {
            payload_type: PayloadType::ControlCommand,
            payload_length: 1,
            sequence_number: 0,
        };
        out.clear();
        out.reserve(SonyHeader::SIZE + 1);
        out.extend_from_slice(&header.encode());
        out.extend_from_slice(&[0x01]);
        self.sequence_counter.store(0, Ordering::Relaxed);
    }

    /// Parse one Sony response at the owner boundary without allocating.
    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) fn extract_owner_response<'a>(
        &self,
        framed: crate::protocol::framer::SonyFrame<'a>,
    ) -> Result<SonyResponse<'a>, MalformedFrame> {
        let payload = framed.payload();
        match framed.payload_type() {
            PayloadType::ViscaReply => {
                sony_visca_payload(payload)?;
                Ok(SonyResponse::Visca {
                    payload,
                    sequence: FrameSequence::from_reply_header(framed.sequence()),
                })
            }
            PayloadType::ControlReply => Ok(SonyResponse::Control {
                code: decode_sony_control_reply(payload)?,
            }),
            _ => Err(SonyMalformed::PayloadType(framed.payload_type()).into()),
        }
    }
}

/// Copy verdicts for Sony envelope and payload grammar. Formatting happens
/// only for outgoing request errors; malformed owner input stays allocation-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[cfg_attr(not(any(feature = "blocking", feature = "async")), allow(dead_code))]
pub(crate) enum SonyMalformed {
    #[error("Unexpected Sony payload type in response: {0:?}")]
    PayloadType(PayloadType),
    #[error("Sony VISCA payload length must be between {MIN_SONY_VISCA_PAYLOAD_LENGTH} and {MAX_SONY_VISCA_PAYLOAD_LENGTH} bytes, got {actual}")]
    PayloadLength { actual: usize },
    #[error("Sony VISCA payload must end with the 0xFF VISCA terminator")]
    PayloadTerminator,
    #[error("Sony control reply payload must contain one or two bytes")]
    ControlReplyLength,
}

impl From<SonyMalformed> for Error {
    fn from(verdict: SonyMalformed) -> Self {
        Self::InvalidRequest {
            reason: verdict.to_string().into(),
        }
    }
}

/// Bounded classification of malformed owner input; the owner only counts
/// these verdicts and never projects them into heap-allocated errors.
#[cfg(any(feature = "blocking", feature = "async"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MalformedFrame {
    InvalidSource,
    UnclassifiedResponse,
    Sony(SonyMalformed),
    InvalidSonyHeader,
    DatagramFrameLimit { limit: usize },
    DatagramOversized,
    DatagramTrailingBytes,
}

#[cfg(any(feature = "blocking", feature = "async"))]
impl From<SonyMalformed> for MalformedFrame {
    fn from(verdict: SonyMalformed) -> Self {
        Self::Sony(verdict)
    }
}

/// The error for an outgoing empty VISCA message.
///
/// An empty message is not a VISCA frame, so both envelopes reject it rather
/// than treating it as a no-op.
fn empty_visca_message() -> Error {
    Error::InvalidRequest {
        reason: "VISCA message must not be empty".into(),
    }
}

/// Normalize the device address byte based on addressing mode.
///
/// Serial mode preserves the caller's address byte. In Sony VISCA-over-IP the
/// VISCA device address is fixed as camera 1, so every address, broadcast
/// (`0x88`) included, becomes `0x81` (`docs/visca_reference.md` section 5.2,
/// "In Sony VISCA-over-IP, the VISCA device address is fixed as camera 1", and
/// section 9.1, "Camera address fixed to 1 for VISCA-over-IP").
fn normalize_address(original_addr: u8, addressing: AddressingMode) -> u8 {
    const IP_DEVICE_ADDR: u8 = 0x81;

    match addressing {
        AddressingMode::Serial => original_addr,
        AddressingMode::Ip => IP_DEVICE_ADDR,
    }
}

/// Decode the bounded payload grammar used by Sony control replies.
#[cfg(any(feature = "blocking", feature = "async"))]
fn decode_sony_control_reply(payload: &[u8]) -> Result<u16, SonyMalformed> {
    match payload {
        [code] => Ok(u16::from(*code)),
        [class, detail] => Ok(u16::from_be_bytes([*class, *detail])),
        _ => Err(SonyMalformed::ControlReplyLength),
    }
}

/// The one Sony VISCA payload rule used for both sending and receiving.
fn sony_visca_payload(payload: &[u8]) -> Result<(), SonyMalformed> {
    let actual = payload.len();
    if !(MIN_SONY_VISCA_PAYLOAD_LENGTH..=MAX_SONY_VISCA_PAYLOAD_LENGTH).contains(&actual) {
        return Err(SonyMalformed::PayloadLength { actual });
    }
    if payload.last().copied() != Some(VISCA_TERMINATOR) {
        return Err(SonyMalformed::PayloadTerminator);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    #[cfg(any(feature = "blocking", feature = "async"))]
    use crate::{
        protocol::framer::{FrameScan, Framer, SonyFrame, SonyFramer},
        transport::BufferConfig,
    };

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[allow(clippy::panic)]
    fn frame_view(bytes: &[u8]) -> SonyFrame<'_> {
        let FrameScan::Complete(frame) = SonyFramer::new(BufferConfig::for_udp()).scan(bytes)
        else {
            panic!("fixture must contain a complete Sony frame");
        };
        frame
    }

    // Exact envelope identity tests
    //
    // These are compile-time constants, verified by the type system.
    // The constants are tested implicitly through their usage in runtime configuration.

    #[test]
    fn envelope_kinds_identify_the_exact_wire_protocol() {
        // Verify that the two envelope types have distinct wire kinds.
        // This ensures one serializes and one allows concurrency.
        assert_ne!(
            RawVisca::KIND,
            SonyEncapsulated::KIND,
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
        assert_eq!(meta.sequence(), None);
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
        assert_eq!(meta.sequence(), None);
    }

    #[test]
    fn test_raw_visca_frame_into_normalizes_broadcast_inquiry() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let visca_cmd = vec![0x88, 0x09, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        let meta = envelope
            .frame_into(&visca_cmd, CommandKind::Inquiry, &mut out)
            .expect("raw framing is infallible for valid test input");

        // IP mode fixes the device address to camera 1, broadcast included
        assert_eq!(out[0], 0x81);
        assert_eq!(&out[1..], &visca_cmd[1..]);
        assert_eq!(meta.sequence(), None);
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
        assert_eq!(meta.sequence(), None);
    }

    #[test]
    fn raw_frame_into_rejects_empty_input_and_leaves_output_unchanged() {
        for addressing in [AddressingMode::Ip, AddressingMode::Serial] {
            let envelope = RawVisca::new(addressing);
            for kind in [CommandKind::Command, CommandKind::Inquiry] {
                let mut out = bytes::BytesMut::from(&b"old frame"[..]);

                let result = envelope.frame_into(&[], kind, &mut out);

                assert!(
                    matches!(&result, Err(Error::InvalidRequest { reason: message }) if message.contains("must not be empty")),
                    "expected InvalidRequest for empty input, got {result:?}"
                );
                assert_eq!(out.as_ref(), b"old frame");
            }
        }
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
        assert_eq!(meta.sequence(), Some(0));

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
        assert_eq!(meta.sequence(), Some(0));

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
        assert_eq!(meta1.sequence(), Some(0));
        assert_eq!(meta2.sequence(), Some(1));
        assert_eq!(meta3.sequence(), Some(2));
    }

    #[test]
    fn sony_sequence_reset_uses_control_grammar_and_restarts_allocator() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_cmd = [0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        let first = envelope
            .frame_into(&visca_cmd, CommandKind::Inquiry, &mut out)
            .expect("ordinary Sony inquiry frames");
        assert_eq!(first.sequence(), Some(0));
        let second = envelope
            .frame_into(&visca_cmd, CommandKind::Inquiry, &mut out)
            .expect("ordinary Sony inquiry frames");
        assert_eq!(second.sequence(), Some(1));

        envelope.frame_sequence_reset_into(&mut out);
        assert_eq!(out.as_ref(), &[0x02, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0x01]);

        let after_reset = envelope
            .frame_into(&visca_cmd, CommandKind::Inquiry, &mut out)
            .expect("allocator restarts after RESET");
        assert_eq!(after_reset.sequence(), Some(0));
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
        assert_eq!(first.sequence(), Some(0));

        let retry = envelope
            .frame_into_with_sequence(
                &visca_cmd,
                CommandKind::Command,
                SonyRequestSequence::Replay(first.sequence().expect("Sony sequence")),
                &mut out,
            )
            .expect("explicit Sony retry sequence is valid");
        assert_eq!(retry, 0);
        assert_eq!(out.as_ref(), first_wire.as_slice());

        let next = envelope
            .frame_into(&visca_cmd, CommandKind::Command, &mut out)
            .expect("Sony framing accepts this payload");
        assert_eq!(next.sequence(), Some(1));
    }

    #[test]
    fn sony_outbound_requires_a_terminated_visca_payload() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let invalid_payload = [0x85];
        let mut out = bytes::BytesMut::from(&b"prior framed bytes"[..]);
        let original_out = out.clone();

        assert!(matches!(
            envelope.frame_into(&invalid_payload, CommandKind::Command, &mut out),
            Err(Error::InvalidRequest { .. })
        ));
        assert_eq!(out.as_ref(), original_out.as_ref());

        let visca_payload = [0x85, VISCA_TERMINATOR];
        let meta = envelope
            .frame_into_with_sequence(
                &visca_payload,
                CommandKind::Command,
                SonyRequestSequence::AllocateFresh,
                &mut out,
            )
            .expect("terminated Sony VISCA payload is valid");
        assert_eq!(meta, 0);
        let header = SonyHeader::decode(&out[..SonyHeader::SIZE]).expect("valid header");
        assert_eq!(header.payload_length, 2);
        assert_eq!(&out[SonyHeader::SIZE..], &[0x81, VISCA_TERMINATOR]);
    }

    #[test]
    fn sony_outbound_frames_the_longest_supported_payload() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let mut visca_payload = [0xa5; MAX_SONY_VISCA_PAYLOAD_LENGTH];
        visca_payload[0] = 0x85;
        visca_payload[MAX_SONY_VISCA_PAYLOAD_LENGTH - 1] = VISCA_TERMINATOR;
        let mut out = bytes::BytesMut::new();

        let meta = envelope
            .frame_into_with_sequence(
                &visca_payload,
                CommandKind::Command,
                SonyRequestSequence::AllocateFresh,
                &mut out,
            )
            .expect("the longest supported Sony payload fits the frame");

        assert_eq!(meta, 0);
        let header = SonyHeader::decode(&out[..SonyHeader::SIZE]).expect("valid header");
        assert_eq!(
            header.payload_length as usize,
            MAX_SONY_VISCA_PAYLOAD_LENGTH
        );
        assert_eq!(out[SonyHeader::SIZE], 0x81);
        assert_eq!(&out[SonyHeader::SIZE + 1..], &visca_payload[1..]);
    }

    #[test]
    fn sony_outbound_rejects_unsupported_length_before_mutation_or_sequence_allocation() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let invalid_payload = [0x81; MAX_SONY_VISCA_PAYLOAD_LENGTH + 1];
        let mut out = bytes::BytesMut::from(&b"prior framed bytes"[..]);
        let original_out = out.clone();

        for requested_sequence in [
            SonyRequestSequence::AllocateFresh,
            SonyRequestSequence::Replay(0x1234_5678),
        ] {
            assert!(matches!(
                envelope.frame_into_with_sequence(
                    &invalid_payload,
                    CommandKind::Command,
                    requested_sequence,
                    &mut out,
                ),
                Err(Error::InvalidRequest { .. })
            ));
            assert_eq!(out.as_ref(), original_out.as_ref());
        }

        let valid_payload = [0x81, VISCA_TERMINATOR];
        let meta = envelope
            .frame_into_with_sequence(
                &valid_payload,
                CommandKind::Command,
                SonyRequestSequence::AllocateFresh,
                &mut out,
            )
            .expect("a rejected payload must not consume the first sequence");
        assert_eq!(meta, 0);
    }

    #[test]
    fn sony_frame_into_reports_invalid_nonempty_payload_without_mutation() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let invalid_payload = [0x81; MAX_SONY_VISCA_PAYLOAD_LENGTH + 1];
        let mut out = bytes::BytesMut::from(&b"prior framed bytes"[..]);
        let original_out = out.clone();

        let result = envelope.frame_into(&invalid_payload, CommandKind::Command, &mut out);

        assert!(matches!(result, Err(Error::InvalidRequest { .. })));
        assert_eq!(out.as_ref(), original_out.as_ref());
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
    fn test_sony_frame_into_normalizes_broadcast_inquiry() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let visca_cmd = vec![0x88, 0x09, 0x00, 0x02, VISCA_TERMINATOR];
        let mut out = bytes::BytesMut::new();

        envelope
            .frame_into(&visca_cmd, CommandKind::Inquiry, &mut out)
            .expect("Sony framing accepts this payload");

        // IP mode fixes the device address to camera 1, broadcast included
        assert_eq!(out[SonyHeader::SIZE], 0x81);
    }

    #[test]
    fn sony_outbound_empty_input_is_rejected_without_sequence_allocation() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let mut out = bytes::BytesMut::from(&b"old frame"[..]);

        for kind in [CommandKind::Command, CommandKind::Inquiry] {
            for requested_sequence in [
                SonyRequestSequence::AllocateFresh,
                SonyRequestSequence::Replay(7),
            ] {
                let result =
                    envelope.frame_into_with_sequence(&[], kind, requested_sequence, &mut out);
                assert!(
                    matches!(&result, Err(Error::InvalidRequest { reason: message }) if message.contains("must not be empty")),
                    "expected InvalidRequest for empty input, got {result:?}"
                );
                assert_eq!(out.as_ref(), b"old frame");
            }
        }

        let result = envelope.frame_into(&[], CommandKind::Command, &mut out);
        assert!(matches!(result, Err(Error::InvalidRequest { .. })));
        assert_eq!(out.as_ref(), b"old frame");

        // None of the rejections consumed a sequence number.
        let valid_payload = [0x81, VISCA_TERMINATOR];
        let next = envelope
            .frame_into_with_sequence(
                &valid_payload,
                CommandKind::Command,
                SonyRequestSequence::AllocateFresh,
                &mut out,
            )
            .expect("a rejected empty input must not consume a sequence");
        assert_eq!(next, 0);
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
        assert_eq!(meta1.sequence(), Some(0));

        // Reuse buffer for second command
        let meta2 = envelope
            .frame_into(&cmd2, CommandKind::Inquiry, &mut out)
            .expect("Sony framing accepts this payload");
        let len2 = out.len();
        assert_eq!(meta2.sequence(), Some(1));

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
        for addr in [0x81, 0x82, 0x88] {
            assert_eq!(normalize_address(addr, AddressingMode::Serial), addr);
        }
    }

    #[test]
    fn test_normalize_address_ip_mode_normalizes_device_addresses() {
        // IP mode fixes the device address to camera 1 (0x81)
        for addr in [0x81, 0x82, 0x87] {
            assert_eq!(normalize_address(addr, AddressingMode::Ip), 0x81);
        }
    }

    #[test]
    fn test_normalize_address_ip_mode_normalizes_broadcast_address() {
        // The reference documents no broadcast exception for VISCA-over-IP, so a
        // 0x88 address (as used by a broadcast inquiry) is normalized as well.
        assert_eq!(normalize_address(0x88, AddressingMode::Ip), 0x81);
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn owner_response_preserves_sony_sequence_provenance() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let ack = [0x90, 0x41, VISCA_TERMINATOR];
        for (number, sequence) in [
            (0, FrameSequence::MaybeTruncated(0)),
            (0xFFFF, FrameSequence::MaybeTruncated(0xFFFF)),
            (0x10000, FrameSequence::Full32(0x10000)),
            (u32::MAX, FrameSequence::Full32(u32::MAX)),
        ] {
            let mut framed = SonyHeader::new_reply(ack.len(), number).encode().to_vec();
            framed.extend_from_slice(&ack);
            assert_eq!(
                envelope.extract_owner_response(frame_view(&framed)),
                Ok(SonyResponse::Visca {
                    payload: &ack,
                    sequence
                })
            );
        }
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn sony_owner_rejects_unterminated_visca_replies() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let unterminated_reply = [0x90, 0x41, 0x00];
        let mut framed = SonyHeader::new_reply(unterminated_reply.len(), 42)
            .encode()
            .to_vec();
        framed.extend_from_slice(&unterminated_reply);
        assert_eq!(
            envelope.extract_owner_response(frame_view(&framed)),
            Err(MalformedFrame::Sony(SonyMalformed::PayloadTerminator))
        );
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn every_sony_malformed_verdict_has_a_total_error_projection() {
        let frame = |kind, declared, payload: &[u8]| {
            let mut bytes = SonyHeader {
                payload_type: kind,
                payload_length: declared,
                sequence_number: 42,
            }
            .encode()
            .to_vec();
            bytes.extend_from_slice(payload);
            bytes
        };
        let oversized = [VISCA_TERMINATOR; MAX_SONY_VISCA_PAYLOAD_LENGTH + 1];
        let cases = [
            (
                frame(
                    PayloadType::ViscaCommand,
                    3,
                    &[0x90, 0x41, VISCA_TERMINATOR],
                ),
                SonyMalformed::PayloadType(PayloadType::ViscaCommand),
                "Unexpected Sony payload type in response: ViscaCommand",
            ),
            (
                frame(PayloadType::ViscaReply, 18, &oversized),
                SonyMalformed::PayloadLength { actual: 18 },
                "Sony VISCA payload length must be between 1 and 17 bytes, got 18",
            ),
            (
                frame(PayloadType::ViscaReply, 3, &[0x90, 0x41, 0]),
                SonyMalformed::PayloadTerminator,
                "Sony VISCA payload must end with the 0xFF VISCA terminator",
            ),
            (
                frame(PayloadType::ControlReply, 0, &[]),
                SonyMalformed::ControlReplyLength,
                "Sony control reply payload must contain one or two bytes",
            ),
        ];
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        for (framed, verdict, message) in cases {
            assert_eq!(
                envelope.extract_owner_response(frame_view(&framed)),
                Err(MalformedFrame::Sony(verdict))
            );
            assert_eq!(verdict.to_string(), message);
            assert!(
                matches!(Error::from(verdict), Error::InvalidRequest { reason } if reason == message)
            );
        }
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn sending_and_receiving_share_the_sony_visca_payload_rule() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        for payload in [
            vec![0x81, 0x41, 0x00],
            vec![VISCA_TERMINATOR; MAX_SONY_VISCA_PAYLOAD_LENGTH + 1],
        ] {
            let verdict = sony_visca_payload(&payload).expect_err("malformed payload");
            let mut reply = SonyHeader::new_reply(payload.len(), 42).encode().to_vec();
            reply.extend_from_slice(&payload);
            assert_eq!(
                envelope.extract_owner_response(frame_view(&reply)),
                Err(MalformedFrame::Sony(verdict))
            );
            let mut out = bytes::BytesMut::from(&b"unchanged"[..]);
            let error = envelope
                .frame_into(&payload, CommandKind::Command, &mut out)
                .expect_err("same malformed outgoing payload");
            assert!(
                matches!(error, Error::InvalidRequest { reason } if reason == verdict.to_string())
            );
            assert_eq!(out.as_ref(), b"unchanged");
        }
        for length in [MIN_SONY_VISCA_PAYLOAD_LENGTH, MAX_SONY_VISCA_PAYLOAD_LENGTH] {
            let payload = vec![VISCA_TERMINATOR; length];
            assert_eq!(sony_visca_payload(&payload), Ok(()));
        }
        assert_eq!(
            sony_visca_payload(&[]),
            Err(SonyMalformed::PayloadLength { actual: 0 })
        );
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn sony_control_replies_are_classified_for_owner_diagnostics() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let frame = |payload: &[u8]| {
            let header = SonyHeader {
                payload_type: PayloadType::ControlReply,
                payload_length: payload.len() as u16,
                sequence_number: 42,
            };
            let mut framed = header.encode().to_vec();
            framed.extend_from_slice(payload);
            framed
        };

        let ack = frame(&[0x01]);
        assert!(matches!(
            envelope.extract_owner_response(frame_view(&ack)),
            Ok(SonyResponse::Control { code: 0x0001 })
        ));
        let sequence_error = frame(&[0x0F, 0x01]);
        assert!(matches!(
            envelope.extract_owner_response(frame_view(&sequence_error)),
            Ok(SonyResponse::Control { code: 0x0F01 })
        ));
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn malformed_sony_control_reply_lengths_are_rejected() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        for payload in [&[][..], &[0x0F, 0x01, 0x02][..]] {
            let header = SonyHeader {
                payload_type: PayloadType::ControlReply,
                payload_length: payload.len() as u16,
                sequence_number: 0,
            };
            let mut framed = header.encode().to_vec();
            framed.extend_from_slice(payload);
            assert!(matches!(
                envelope.extract_owner_response(frame_view(&framed)),
                Err(MalformedFrame::Sony(SonyMalformed::ControlReplyLength))
            ));
        }
    }
}
