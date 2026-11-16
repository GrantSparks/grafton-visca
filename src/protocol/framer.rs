//! Bounded raw VISCA and Sony stream framers.
//!
//! Envelope identity is fixed by the concrete framer type. Both framers share
//! one strict overflow policy, and production visitors borrow each frame until
//! it has been decoded, avoiding an owned per-frame buffer or sharing record.

#[cfg(test)]
use bytes::Bytes;
use bytes::{Buf, BytesMut};

#[cfg(test)]
use crate::protocol::sony::PayloadType;
use crate::{
    command::bytes::VISCA_TERMINATOR,
    protocol::{response::decode_fixed_socket, sony::SonyHeader},
    target::TargetSlot,
    transport::buffer::BufferConfig,
    Error, ViscaSocket,
};

/// A framing verdict distinguishes a discarded raw frame from lost position.
///
/// Only [`Self::OversizedRawFrame`] preserves the next frame's boundary. An
/// owner may count and discard that frame on a stream; the other variants are
/// terminal framing failures. Public transport errors retain the configured
/// byte limit through the conversion to [`Error`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FramingError {
    /// A raw frame was discarded through its terminating `0xFF`.
    OversizedRawFrame { max_size: usize },
    /// Accumulated input exceeded the retention bound plus one full read.
    BufferOverflow { max_size: usize },
    /// A Sony header declares a frame larger than the configured frame limit.
    OversizedSonyFrame { max_size: usize },
}

impl From<FramingError> for Error {
    fn from(error: FramingError) -> Self {
        let max_size = match error {
            FramingError::OversizedRawFrame { max_size }
            | FramingError::BufferOverflow { max_size }
            | FramingError::OversizedSonyFrame { max_size } => max_size,
        };
        Self::ResponseTooLarge { max_size }
    }
}

/// The protocol identity visible in an incomplete raw response prefix.
///
/// Classification belongs beside the byte-stream framer so every owner gives
/// the engine identical evidence at a correlation-release boundary (#723).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RawIncompletePrefix {
    /// The source byte (`0x9y..=0xFy`) arrived with no response-class byte.
    SourceOnly,
    /// `0x4y`: an ACK's nibble is an assignment preference, not an owner.
    Ack,
    /// `0x50`: socketless completion evidence.
    SocketlessCompletion,
    /// `0x60`: socketless error evidence.
    SocketlessError,
    /// `0x5y`/`0x6y` for one exact numbered socket.
    NamedCompletionOrError(ViscaSocket),
    /// Bytes cannot correlate to a raw request and therefore cannot revive or
    /// bind a successor.
    Noncorrelating,
}

/// The first retained raw input as seen at a correlation-release boundary.
///
/// The framer owns the distinction between a complete input, an incomplete
/// response-shaped prefix, and bytes which cannot start a response at all.
/// That keeps malformed stream noise on the same discard path as a delimited
/// malformed frame instead of handing it to an owner as a fallible routing
/// operation (#745).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RawBufferedInput {
    /// The first input already has its terminator and must use normal decoding.
    Complete,
    /// An incomplete input starts with a byte that cannot be routed as a VISCA
    /// response source for this owner.
    Malformed,
    /// An incomplete response-shaped input with its resolved target and class.
    Incomplete {
        target: TargetSlot,
        kind: RawIncompletePrefix,
    },
}

/// The common bounded storage, independent of frame grammar.
#[derive(Debug)]
pub(crate) struct FrameBuffer {
    buf: BytesMut,
    max_frame_size: usize,
    max_buffer_size: usize,
}

impl FrameBuffer {
    fn new(config: BufferConfig) -> Self {
        Self {
            buf: BytesMut::with_capacity(config.recv_buffer_size),
            max_frame_size: config.recv_buffer_size,
            max_buffer_size: config.framer_capacity(),
        }
    }
}

mod private {
    pub trait Sealed {}
    impl Sealed for super::RawFramer {}
    impl Sealed for super::SonyFramer {}
}

/// One bounded frame grammar with allocation-free borrowed visitation.
pub(crate) trait Framer: private::Sealed {
    fn buffer(&self) -> &FrameBuffer;
    fn buffer_mut(&mut self) -> &mut FrameBuffer;
    fn from_buffer(buffer: FrameBuffer) -> Self;
    fn frame_len(&self, input: &[u8]) -> Option<Result<usize, FramingError>>;
    fn next_frame_len(&mut self) -> Option<Result<usize, FramingError>> {
        let next = self.frame_len(&self.buffer().buf)?;
        match next {
            Err(FramingError::OversizedRawFrame { .. }) => {
                let length = self
                    .buffer()
                    .buf
                    .iter()
                    .position(|byte| *byte == VISCA_TERMINATOR)?
                    + 1;
                self.buffer_mut().buf.advance(length);
            }
            Err(_) => self.buffer_mut().buf.clear(),
            Ok(_) => {}
        }
        Some(next)
    }

    /// Push bytes without changing retained input if the hard bound is exceeded.
    fn push_slice(&mut self, chunk: &[u8]) -> Result<(), FramingError> {
        let buffer = self.buffer_mut();
        if buffer.buf.len().saturating_add(chunk.len()) > buffer.max_buffer_size {
            return Err(FramingError::BufferOverflow {
                max_size: buffer.max_buffer_size,
            });
        }
        buffer.buf.extend_from_slice(chunk);
        Ok(())
    }

    /// Decode a frame while borrowing its bytes, then consume its exact boundary.
    /// The visitor's output cannot borrow the framer's buffer.
    fn visit_next_frame<T>(
        &mut self,
        visit: impl FnOnce(&[u8]) -> T,
    ) -> Option<Result<T, FramingError>> {
        let length = match self.next_frame_len()? {
            Ok(length) => length,
            Err(error) => return Some(Err(error)),
        };
        let value = visit(&self.buffer().buf[..length]);
        self.buffer_mut().buf.advance(length);
        Some(Ok(value))
    }

    #[cfg(any(test, feature = "transport-serial", feature = "transport-serial-tokio"))]
    fn clear(&mut self) {
        self.buffer_mut().buf.clear();
    }
    fn buffered_len(&self) -> usize {
        self.buffer().buf.len()
    }
    fn has_buffered_data(&self) -> bool {
        !self.buffer().buf.is_empty()
    }

    #[cfg(any(test, all(feature = "test-utils", feature = "blocking")))]
    fn max_buffer_size(&self) -> usize {
        self.buffer().max_buffer_size
    }

    #[cfg(test)]
    fn new_with_limits(capacity: usize, max_frame_size: usize, max_buffer_size: usize) -> Self
    where
        Self: Sized,
    {
        Self::from_buffer(FrameBuffer {
            buf: BytesMut::with_capacity(capacity),
            max_frame_size,
            max_buffer_size,
        })
    }
    #[cfg(test)]
    fn push(&mut self, bytes: Bytes) -> Result<(), FramingError> {
        self.push_slice(&bytes)
    }
    #[cfg(test)]
    fn is_empty(&self) -> bool {
        !self.has_buffered_data()
    }
    /// Owned output is only a test convenience; production uses borrowed visits.
    #[cfg(test)]
    fn drain_frames(&mut self) -> impl Iterator<Item = Result<Bytes, FramingError>> + '_ {
        std::iter::from_fn(move || self.visit_next_frame(Bytes::copy_from_slice))
    }
    #[cfg(all(test, feature = "blocking"))]
    fn drain_on_eof(&mut self) -> Option<Result<Bytes, FramingError>> {
        self.visit_next_frame(Bytes::copy_from_slice)
    }
}

/// Raw VISCA frames end at their terminating `FF` byte.
#[derive(Debug)]
pub(crate) struct RawFramer {
    buffer: FrameBuffer,
}

impl RawFramer {
    pub(crate) fn new(config: BufferConfig) -> Self {
        Self::from_buffer(FrameBuffer::new(config))
    }

    /// Classify the first retained raw input using the owner's routing rule.
    pub(crate) fn buffered_raw_incomplete_prefix(
        &self,
        mut target_for_source: impl FnMut(u8) -> Option<TargetSlot>,
    ) -> Option<RawBufferedInput> {
        if self.buffer.buf.is_empty() {
            return None;
        }
        if self.buffer.buf.contains(&VISCA_TERMINATOR) {
            return Some(RawBufferedInput::Complete);
        }

        let source = self.buffer.buf[0];
        let Some(target) = target_for_source(source) else {
            return Some(RawBufferedInput::Malformed);
        };
        let kind = match self.buffer.buf.get(1).copied() {
            None => RawIncompletePrefix::SourceOnly,
            // An ACK prefix is classified by its message nibble alone: its
            // socket nibble is a preference for assigning a free socket,
            // never evidence of who owns a named socket already. The complete
            // ACK frame is still held to the strict socket grammar of
            // `decode_fixed_socket` when it is decoded.
            Some(0x40..=0x4f) => RawIncompletePrefix::Ack,
            // Completion and error prefixes name a socket, so they use the
            // same strict nibble decoder as complete frames.
            Some(byte @ 0x50..=0x6f) => match decode_fixed_socket(byte & 0x0f) {
                Some(None) if byte & 0xf0 == 0x50 => RawIncompletePrefix::SocketlessCompletion,
                Some(None) => RawIncompletePrefix::SocketlessError,
                Some(Some(socket)) => RawIncompletePrefix::NamedCompletionOrError(socket),
                None => RawIncompletePrefix::Noncorrelating,
            },
            Some(_) => RawIncompletePrefix::Noncorrelating,
        };
        Some(RawBufferedInput::Incomplete { target, kind })
    }

    /// Discard one delimited frame, or the entire sole incomplete fragment.
    pub(crate) fn discard_first_input(&mut self) {
        if let Some(position) = self
            .buffer
            .buf
            .iter()
            .position(|byte| *byte == VISCA_TERMINATOR)
        {
            self.buffer.buf.advance(position + 1);
        } else {
            self.buffer.buf.clear();
        }
    }
}

impl Framer for RawFramer {
    fn buffer(&self) -> &FrameBuffer {
        &self.buffer
    }
    fn buffer_mut(&mut self) -> &mut FrameBuffer {
        &mut self.buffer
    }
    fn from_buffer(buffer: FrameBuffer) -> Self {
        Self { buffer }
    }
    fn frame_len(&self, input: &[u8]) -> Option<Result<usize, FramingError>> {
        let length = input.iter().position(|byte| *byte == VISCA_TERMINATOR)? + 1;
        Some(if length > self.buffer.max_frame_size {
            Err(FramingError::OversizedRawFrame {
                max_size: self.buffer.max_frame_size,
            })
        } else {
            Ok(length)
        })
    }
}

/// Sony frames end at the payload length declared by their eight-byte header.
#[derive(Debug)]
pub(crate) struct SonyFramer {
    buffer: FrameBuffer,
}

impl SonyFramer {
    pub(crate) fn new(config: BufferConfig) -> Self {
        Self::from_buffer(FrameBuffer::new(config))
    }
}

impl Framer for SonyFramer {
    fn buffer(&self) -> &FrameBuffer {
        &self.buffer
    }
    fn buffer_mut(&mut self) -> &mut FrameBuffer {
        &mut self.buffer
    }
    fn from_buffer(buffer: FrameBuffer) -> Self {
        Self { buffer }
    }
    fn frame_len(&self, input: &[u8]) -> Option<Result<usize, FramingError>> {
        let header = SonyHeader::decode(input.get(..SonyHeader::SIZE)?)?;
        let length = SonyHeader::SIZE + usize::from(header.payload_length);
        if length > self.buffer.max_frame_size {
            Some(Err(FramingError::OversizedSonyFrame {
                max_size: self.buffer.max_frame_size,
            }))
        } else {
            (input.len() >= length).then_some(Ok(length))
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    /// A framer with no practical size limits and an explicit wire mode.
    fn raw_framer() -> RawFramer {
        RawFramer::new_with_limits(256, usize::MAX, usize::MAX)
    }

    fn sony_framer() -> SonyFramer {
        SonyFramer::new_with_limits(256, usize::MAX, usize::MAX)
    }

    #[test]
    fn test_raw_visca_single_frame() {
        let mut framer = raw_framer();
        let frame = Bytes::from(vec![0x81, 0x01, 0x04, 0x07, VISCA_TERMINATOR]);

        framer.push(frame.clone()).unwrap();
        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], frame);
        assert!(framer.is_empty());
    }

    #[test]
    fn test_raw_visca_multiple_frames() {
        let mut framer = raw_framer();
        let frame1 = vec![0x81, 0x01, 0x04, 0x07, VISCA_TERMINATOR];
        let frame2 = vec![0x90, 0x50, VISCA_TERMINATOR];
        let mut combined = frame1.clone();
        combined.extend(&frame2);

        framer.push(Bytes::from(combined)).unwrap();
        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], Bytes::from(frame1));
        assert_eq!(frames[1], Bytes::from(frame2));
        assert!(framer.is_empty());
    }

    #[test]
    fn test_raw_visca_split_across_chunks() {
        let mut framer = raw_framer();
        let chunk1 = Bytes::from(vec![0x81, 0x01, 0x04]);
        let chunk2 = Bytes::from(vec![0x07, VISCA_TERMINATOR]);

        framer.push(chunk1).unwrap();
        let frames1: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames1.len(), 0);
        assert_eq!(framer.buffered_len(), 3);

        framer.push(chunk2).unwrap();
        let frames2: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames2.len(), 1);
        assert_eq!(
            frames2[0],
            Bytes::from(vec![0x81, 0x01, 0x04, 0x07, VISCA_TERMINATOR])
        );
        assert!(framer.is_empty());
    }

    #[test]
    fn raw_prefix_classifier_handles_empty_buffer() {
        let framer = RawFramer::new_with_limits(32, 32, 32);

        assert_eq!(framer.buffered_raw_incomplete_prefix(|_| None), None);
    }

    #[test]
    fn raw_prefix_classifier_reports_one_byte_fragment() {
        let mut framer = RawFramer::new_with_limits(32, 32, 32);
        framer.push_slice(&[0x90]).unwrap();

        assert_eq!(
            framer.buffered_raw_incomplete_prefix(|source| {
                (source == 0x90).then_some(TargetSlot::ALL[0])
            }),
            Some(RawBufferedInput::Incomplete {
                target: TargetSlot::CAMERA_1,
                kind: RawIncompletePrefix::SourceOnly,
            })
        );
    }

    #[test]
    fn raw_prefix_classifier_reports_two_byte_fragment() {
        let mut framer = RawFramer::new_with_limits(32, 32, 32);
        framer.push_slice(&[0x90, 0x51]).unwrap();

        assert_eq!(
            framer.buffered_raw_incomplete_prefix(|source| {
                (source == 0x90).then_some(TargetSlot::ALL[0])
            }),
            Some(RawBufferedInput::Incomplete {
                target: TargetSlot::CAMERA_1,
                kind: RawIncompletePrefix::NamedCompletionOrError(ViscaSocket::S1),
            })
        );
    }

    #[test]
    fn raw_incomplete_prefix_classifier_is_owner_independent() {
        let cases: &[(&[u8], RawIncompletePrefix)] = &[
            (&[0x90], RawIncompletePrefix::SourceOnly),
            (&[0x90, 0x41], RawIncompletePrefix::Ack),
            (&[0x90, 0x4f], RawIncompletePrefix::Ack),
            (&[0x90, 0x50], RawIncompletePrefix::SocketlessCompletion),
            (&[0x90, 0x60], RawIncompletePrefix::SocketlessError),
            (
                &[0x90, 0x51],
                RawIncompletePrefix::NamedCompletionOrError(ViscaSocket::S1),
            ),
            (
                &[0x90, 0x62],
                RawIncompletePrefix::NamedCompletionOrError(ViscaSocket::S2),
            ),
            (&[0x90, 0x7f], RawIncompletePrefix::Noncorrelating),
        ];

        for &(bytes, expected) in cases {
            let mut framer = RawFramer::new_with_limits(32, 32, 32);
            framer.push_slice(bytes).unwrap();
            assert_eq!(
                framer.buffered_raw_incomplete_prefix(|source| {
                    (source == 0x90).then_some(TargetSlot::ALL[0])
                }),
                Some(RawBufferedInput::Incomplete {
                    target: TargetSlot::CAMERA_1,
                    kind: expected,
                }),
                "bytes {bytes:02x?}",
            );
        }
    }

    #[test]
    fn raw_prefix_classifier_preserves_complete_input_for_normal_decode() {
        let mut framer = RawFramer::new_with_limits(32, 32, 32);
        framer
            .push_slice(&[0x90, 0x51, VISCA_TERMINATOR, 0x90, 0x41, VISCA_TERMINATOR])
            .unwrap();

        assert_eq!(
            framer.buffered_raw_incomplete_prefix(|source| {
                (source == 0x90).then_some(TargetSlot::ALL[0])
            }),
            Some(RawBufferedInput::Complete)
        );
    }

    #[test]
    fn raw_prefix_classifier_treats_one_byte_terminated_input_as_complete() {
        let mut framer = RawFramer::new_with_limits(32, 32, 32);
        framer
            .push_slice(&[0x90, VISCA_TERMINATOR, 0x51, VISCA_TERMINATOR])
            .unwrap();

        assert_eq!(
            framer.buffered_raw_incomplete_prefix(|source| {
                (source == 0x90).then_some(TargetSlot::ALL[0])
            }),
            Some(RawBufferedInput::Complete)
        );
    }

    /// Issue #745: malformed incomplete prefixes are classified at the raw
    /// framer boundary, where they can be discarded like their complete twins
    /// instead of escaping as fallible owner routing input.
    #[test]
    fn raw_prefix_classifier_marks_invalid_response_sources_malformed() {
        for bytes in [&[0x80, 0x50, 0xdd][..], &[0x88, 0x30, 0x02], &[0x00]] {
            let mut framer = RawFramer::new_with_limits(32, 32, 32);
            framer.push_slice(bytes).unwrap();
            assert_eq!(
                framer.buffered_raw_incomplete_prefix(|source| {
                    (source == 0x90).then_some(TargetSlot::ALL[0])
                }),
                Some(RawBufferedInput::Malformed),
                "bytes {bytes:02x?}",
            );
        }
    }

    /// Malformed/noise raw prefixes may happen to match Sony payload types.
    /// Raw framing is still strictly terminator-delimited; this test does not
    /// claim that valid raw VISCA replies begin with either prefix.
    #[test]
    fn raw_mode_delimits_sony_looking_noise_before_following_replies() {
        for payload_type in [[0x01, 0x11], [0x02, 0x00]] {
            let mut framer = RawFramer::new_with_limits(32, 32, 32);
            // This resembles a Sony header declaring five payload bytes, but
            // it is malformed/noise and reaches a raw VISCA boundary first.
            let noise = [
                payload_type[0],
                payload_type[1],
                0x00,
                0x05,
                0x12,
                0x34,
                0x56,
                0x78,
                0x55,
                VISCA_TERMINATOR,
            ];
            let ack = [0x90, 0x41, VISCA_TERMINATOR];
            let completion = [0x90, 0x51, VISCA_TERMINATOR];
            let mut bytes = noise.to_vec();
            bytes.extend_from_slice(&ack);
            bytes.extend_from_slice(&completion);

            framer.push(Bytes::from(bytes)).unwrap();
            let frames: Vec<_> = framer
                .drain_frames()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();

            assert_eq!(
                frames,
                vec![
                    Bytes::from(noise.to_vec()),
                    Bytes::from(ack.to_vec()),
                    Bytes::from(completion.to_vec())
                ]
            );
            assert!(framer.is_empty());
        }
    }

    #[test]
    fn test_sony_frame_basic() {
        let mut framer = sony_framer();

        // Sony header: ViscaReply, payload_length=5, sequence=0x12345678
        let header = SonyHeader {
            payload_type: PayloadType::ViscaReply,
            payload_length: 5,
            sequence_number: 0x12345678,
        };

        let mut frame = Vec::from(header.encode());
        frame.extend_from_slice(&[0x90, 0x50, 0x02, 0x03, VISCA_TERMINATOR]); // 5-byte payload

        framer.push(Bytes::from(frame.clone())).unwrap();
        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Bytes::from(frame));
        assert!(framer.is_empty());
    }

    #[test]
    fn sony_mode_waits_for_fragmented_header_and_payload_length() {
        let mut framer = SonyFramer::new_with_limits(32, 32, 32);
        let header = SonyHeader {
            payload_type: PayloadType::ViscaReply,
            payload_length: 3,
            sequence_number: 0xff00_ff00,
        };
        let mut frame = header.encode().to_vec();
        frame.extend_from_slice(&[0x90, 0x50, VISCA_TERMINATOR]);

        framer.push(Bytes::copy_from_slice(&frame[..6])).unwrap();
        assert!(framer.drain_frames().next().is_none());

        framer.push(Bytes::copy_from_slice(&frame[6..])).unwrap();
        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames, vec![Bytes::from(frame)]);
        assert!(framer.is_empty());
    }

    #[test]
    fn oversized_sony_payload_is_never_reinterpreted_as_another_frame() {
        let mut framer = SonyFramer::new_with_limits(64, 32, 64);
        let oversized = SonyHeader {
            payload_type: PayloadType::ViscaReply,
            payload_length: 64,
            sequence_number: 1,
        };
        // These bytes belong to the rejected payload, even though they look
        // like a complete encapsulated ACK when stripped of its first header.
        let mut bytes = oversized.encode().to_vec();
        bytes.extend_from_slice(
            &SonyHeader {
                payload_type: PayloadType::ViscaReply,
                payload_length: 3,
                sequence_number: 7,
            }
            .encode(),
        );
        bytes.extend_from_slice(&[0x90, 0x41, 0xff]);
        assert!(framer.push_slice(&bytes).is_ok(), "bounded input");
        let verdicts: Vec<_> = framer.drain_frames().collect();
        assert_eq!(
            verdicts,
            [Err(FramingError::OversizedSonyFrame { max_size: 32 })]
        );
        assert!(framer.drain_frames().next().is_none());
        assert!(framer.is_empty());
    }

    #[test]
    fn sony_mode_rejects_oversized_declared_payload_before_it_arrives() {
        let mut framer = SonyFramer::new_with_limits(32, 20, 32);
        let header = SonyHeader {
            payload_type: PayloadType::ViscaReply,
            payload_length: 13,
            sequence_number: 1,
        };

        framer
            .push(Bytes::copy_from_slice(&header.encode()))
            .unwrap();
        assert!(matches!(
            framer.drain_frames().next(),
            Some(Err(FramingError::OversizedSonyFrame { max_size: 20 }))
        ));
        assert!(framer.is_empty());
    }

    #[test]
    fn test_sony_frame_with_ff_in_sequence() {
        let mut framer = sony_framer();

        // Sony header with 0xFF in sequence number
        let header = SonyHeader {
            payload_type: PayloadType::ViscaReply,
            payload_length: 3,
            sequence_number: 0xFF00FF00, // Contains 0xFF bytes
        };

        let mut frame = Vec::from(header.encode());
        frame.extend_from_slice(&[0x90, 0x50, VISCA_TERMINATOR]); // 3-byte payload

        framer.push(Bytes::from(frame.clone())).unwrap();
        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            frames.len(),
            1,
            "Should extract exactly one complete Sony frame"
        );
        assert_eq!(
            frames[0],
            Bytes::from(frame),
            "Frame should include full header and payload"
        );
        assert!(framer.is_empty());
    }

    #[test]
    fn test_sony_frame_with_ff_in_header() {
        let mut framer = sony_framer();

        // Sony header where payload_length bytes contain 0xFF
        let _header = SonyHeader {
            payload_type: PayloadType::ViscaReply,
            payload_length: 0xFF03, // 0xFF in length field
            sequence_number: 0x12345678,
        };
        // Add exactly 0xFF03 (65283) bytes of payload - just use a small representative sample
        // For testing, we'll use a smaller payload and adjust the header
        let header = SonyHeader {
            payload_type: PayloadType::ViscaReply,
            payload_length: 4,
            sequence_number: 0xFFFFFFFF, // All 0xFF in sequence
        };

        let mut frame = Vec::from(header.encode());
        frame.extend_from_slice(&[0x90, 0x50, 0x02, VISCA_TERMINATOR]); // 4-byte payload

        framer.push(Bytes::from(frame.clone())).unwrap();
        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].len(), SonyHeader::SIZE + 4);
        assert!(framer.is_empty());
    }

    #[test]
    fn test_sony_frame_partial_header() {
        let mut framer = sony_framer();

        // Push only partial Sony header (6 bytes instead of 8)
        let partial = vec![0x01, 0x11, 0x00, 0x05, 0x12, 0x34];
        framer.push(Bytes::from(partial)).unwrap();

        let frames1: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            frames1.len(),
            0,
            "Should not extract frame with partial header"
        );
        assert_eq!(framer.buffered_len(), 6);

        // Push rest of header and payload
        let rest = vec![0x56, 0x78, 0x90, 0x50, 0x02, 0x03, VISCA_TERMINATOR];
        framer.push(Bytes::from(rest)).unwrap();

        let frames2: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames2.len(), 1);
        assert_eq!(frames2[0].len(), SonyHeader::SIZE + 5);
        assert!(framer.is_empty());
    }

    /// Builds `[Sony frame][raw frame][Sony frame with 0xFF in the sequence]`.
    fn mixed_sony_and_raw_stream() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let sony1 = {
            let header = SonyHeader {
                payload_type: PayloadType::ViscaReply,
                payload_length: 3,
                sequence_number: 0x11111111,
            };
            let mut frame = Vec::from(header.encode());
            frame.extend_from_slice(&[0x90, 0x50, VISCA_TERMINATOR]);
            frame
        };
        let raw = vec![0x81, 0x01, 0x04, 0x07, VISCA_TERMINATOR];
        let sony2 = {
            let header = SonyHeader {
                payload_type: PayloadType::ViscaReply,
                payload_length: 2,
                sequence_number: 0xFFFFFFFF,
            };
            let mut frame = Vec::from(header.encode());
            frame.extend_from_slice(&[0x90, VISCA_TERMINATOR]);
            frame
        };
        (sony1, raw, sony2)
    }

    #[test]
    fn sony_mode_does_not_delimit_raw_bytes_that_follow_a_sony_frame() {
        let mut framer = sony_framer();
        let (sony1, raw, sony2) = mixed_sony_and_raw_stream();
        let mut data = sony1.clone();
        data.extend_from_slice(&raw);
        data.extend_from_slice(&sony2);

        framer.push(Bytes::from(data)).unwrap();
        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        // The leading Sony frame is length-delimited. The raw bytes after it
        // are not a Sony header, so Sony mode keeps them buffered rather than
        // delimiting them at a terminator.
        assert_eq!(frames, vec![Bytes::from(sony1)]);
        assert_eq!(framer.buffered_len(), raw.len() + sony2.len());
    }

    #[test]
    fn raw_mode_delimits_every_terminator_including_those_inside_sony_headers() {
        let mut framer = raw_framer();
        let (sony1, raw, sony2) = mixed_sony_and_raw_stream();
        let mut data = sony1.clone();
        data.extend_from_slice(&raw);
        data.extend_from_slice(&sony2);

        framer.push(Bytes::from(data)).unwrap();
        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        // The first Sony frame has no 0xFF in its header, so it happens to end
        // at its own terminator. The second carries 0xFF in its sequence
        // number, which raw mode splits on, because raw framing never
        // interprets a Sony header.
        assert_eq!(
            frames,
            vec![
                Bytes::from(sony1),
                Bytes::from(raw),
                Bytes::from(vec![0x01, 0x11, 0x00, 0x02, VISCA_TERMINATOR]),
                Bytes::from(vec![VISCA_TERMINATOR]),
                Bytes::from(vec![VISCA_TERMINATOR]),
                Bytes::from(vec![VISCA_TERMINATOR]),
                Bytes::from(vec![0x90, VISCA_TERMINATOR]),
            ]
        );
        assert!(framer.is_empty());
    }

    #[test]
    fn test_trailing_incomplete_frame() {
        let mut framer = raw_framer();
        let data = Bytes::from(vec![0x81, 0x01, 0x04, 0x07, VISCA_TERMINATOR, 0x90, 0x50]);

        framer.push(data).unwrap();
        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(frames.len(), 1);
        assert_eq!(
            frames[0],
            Bytes::from(vec![0x81, 0x01, 0x04, 0x07, VISCA_TERMINATOR])
        );
        assert_eq!(framer.buffered_len(), 2);
        assert!(!framer.is_empty());
    }

    #[test]
    fn raw_mode_delimits_a_sony_looking_prefix_at_the_terminator() {
        let mut framer = raw_framer();

        // Starts like a Sony header but carries an unknown payload type.
        let data = vec![0x01, 0x99, 0x04, 0x07, VISCA_TERMINATOR];
        framer.push(Bytes::from(data.clone())).unwrap();

        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Bytes::from(data), "Should treat as raw VISCA");
        assert!(framer.is_empty());
    }

    #[test]
    fn test_empty_input() {
        fn check(mut framer: impl Framer) {
            framer.push(Bytes::new()).unwrap();
            assert!(framer.drain_frames().next().is_none());
            assert!(framer.is_empty());
        }
        check(raw_framer());
        check(sony_framer());
    }

    #[test]
    fn test_clear() {
        let mut framer = raw_framer();
        framer.push(Bytes::from(vec![0x81, 0x01, 0x04])).unwrap();

        assert_eq!(framer.buffered_len(), 3);
        framer.clear();
        assert_eq!(framer.buffered_len(), 0);
        assert!(framer.is_empty());
    }

    #[test]
    fn test_back_to_back_sony_frames() {
        let mut framer = sony_framer();

        // Two Sony frames back-to-back
        let header1 = SonyHeader {
            payload_type: PayloadType::ViscaReply,
            payload_length: 3,
            sequence_number: 0x11111111,
        };
        let header2 = SonyHeader {
            payload_type: PayloadType::ViscaReply,
            payload_length: 2,
            sequence_number: 0xFFFFFFFF, // With 0xFF in sequence
        };

        let mut data = Vec::from(header1.encode());
        data.extend_from_slice(&[0x90, 0x50, VISCA_TERMINATOR]); // First payload
        data.extend_from_slice(&header2.encode()); // Second header
        data.extend_from_slice(&[0x90, VISCA_TERMINATOR]); // Second payload

        framer.push(Bytes::from(data)).unwrap();
        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].len(), SonyHeader::SIZE + 3);
        assert_eq!(frames[1].len(), SonyHeader::SIZE + 2);
        assert!(framer.is_empty());
    }

    #[test]
    fn test_sony_control_frames_are_length_delimited() {
        let mut framer = sony_framer();

        let command_header = SonyHeader {
            payload_type: PayloadType::ControlCommand,
            payload_length: 1,
            sequence_number: 0x11223344,
        };
        let reply_header = SonyHeader {
            payload_type: PayloadType::ControlReply,
            payload_length: 2,
            sequence_number: 0x55667788,
        };

        let mut command = Vec::from(command_header.encode());
        command.extend_from_slice(&[0x01]);
        let mut reply = Vec::from(reply_header.encode());
        reply.extend_from_slice(&[0x0F, 0x01]);

        let mut data = command.clone();
        data.extend_from_slice(&reply);
        framer.push(Bytes::from(data)).unwrap();

        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames, vec![Bytes::from(command), Bytes::from(reply)]);
        assert!(framer.is_empty());
    }

    // New tests for bounded framer behavior

    #[test]
    fn test_sony_frame_exceeds_max_size() {
        // Create framer with small max frame size
        let mut framer = SonyFramer::new_with_limits(256, 20, 1024);

        // Sony header with payload that would exceed limit
        let header = SonyHeader {
            payload_type: PayloadType::ViscaReply,
            payload_length: 15, // 8 header + 15 payload = 23 bytes > 20 max
            sequence_number: 0x12345678,
        };

        let mut data = Vec::from(header.encode());
        // Add some payload bytes (doesn't matter what, frame will be rejected)
        data.extend_from_slice(&[0x90, 0x50, 0x01, 0x02, 0x03]);

        framer.push(Bytes::from(data)).unwrap();

        // Should get an error when trying to extract
        let results: Vec<_> = framer.drain_frames().collect();
        assert_eq!(results.len(), 1);
        match &results[0] {
            Err(FramingError::OversizedSonyFrame { max_size }) => {
                assert_eq!(*max_size, 20);
            }
            _ => panic!("Expected ResponseTooLarge error"),
        }
    }

    #[test]
    fn test_raw_visca_frame_exceeds_max_size() {
        // Create framer with small max frame size
        let mut framer = RawFramer::new_with_limits(256, 10, 1024);

        // Raw VISCA frame that exceeds limit
        let data = vec![
            0x81,
            0x01,
            0x04,
            0x07,
            0x00,
            0x01,
            0x02,
            0x03,
            0x04,
            0x05,
            0x06,
            VISCA_TERMINATOR,
        ]; // 12 bytes > 10 max

        framer.push(Bytes::from(data)).unwrap();

        // Should get an error when trying to extract
        let results: Vec<_> = framer.drain_frames().collect();
        assert_eq!(results.len(), 1);
        match &results[0] {
            Err(FramingError::OversizedRawFrame { max_size }) => {
                assert_eq!(*max_size, 10);
            }
            _ => panic!("Expected ResponseTooLarge error"),
        }

        // Buffer should be cleared after error
        assert!(framer.is_empty());
    }

    #[test]
    fn oversized_raw_frame_keeps_the_next_frame_aligned() {
        let mut framer = RawFramer::new_with_limits(32, 3, 32);
        framer
            .push_slice(&[0x55, 0x55, 0x55, 0xFF, 0x90, 0x41, 0xFF])
            .unwrap();

        let results: Vec<_> = framer.drain_frames().collect();
        assert_eq!(
            results,
            [
                Err(FramingError::OversizedRawFrame { max_size: 3 }),
                Ok(Bytes::from_static(&[0x90, 0x41, 0xFF])),
            ]
        );
        assert!(framer.is_empty());
    }

    #[test]
    fn strict_push_overflow_preserves_the_existing_partial_frame() {
        fn check(mut framer: impl Framer, packet: &[u8]) {
            framer.push_slice(&packet[..2]).unwrap();
            assert_eq!(
                framer.push_slice(&[0; 65]),
                Err(FramingError::BufferOverflow { max_size: 64 })
            );
            assert_eq!(framer.buffered_len(), 2);
            framer.push_slice(&packet[2..]).unwrap();
            let frame = framer
                .visit_next_frame(|frame| frame.to_vec())
                .unwrap()
                .unwrap();
            assert_eq!(frame, packet);
            assert!(!framer.has_buffered_data());
        }
        check(RawFramer::new_with_limits(64, 64, 64), &[0x90, 0x41, 0xff]);
        check(
            SonyFramer::new_with_limits(64, 64, 64),
            &[0x01, 0x11, 0x00, 0x03, 0, 0, 0, 1, 0x90, 0x41, 0xff],
        );
    }

    #[test]
    fn cumulative_overflow_reports_the_configured_retention_plus_one_read() {
        let config = BufferConfig {
            recv_buffer_size: 24,
            max_buffer_size: 48,
        };
        let mut framer = RawFramer::new(config);
        framer.push_slice(&[0x55; 72]).unwrap();
        assert_eq!(
            framer.push_slice(&[0x55]),
            Err(FramingError::BufferOverflow { max_size: 72 })
        );
        let public_error = Error::from(FramingError::BufferOverflow { max_size: 72 });
        assert!(matches!(
            public_error,
            Error::ResponseTooLarge { max_size: 72 }
        ));
    }

    #[test]
    fn test_buffer_exceeds_max_size() {
        // Create framer with small max buffer size
        let mut framer = RawFramer::new_with_limits(256, 100, 50);

        // Try to push data that would exceed buffer limit
        let data = vec![0x81; 60]; // 60 bytes > 50 max buffer

        let result = framer.push(Bytes::from(data));
        match result {
            Err(FramingError::BufferOverflow { max_size }) => {
                assert_eq!(max_size, 50);
            }
            _ => panic!("Expected ResponseTooLarge error"),
        }
    }

    /// Starts with an unknown Sony payload type, then looks like raw VISCA.
    fn invalid_sony_header_then_raw_bytes() -> Vec<u8> {
        vec![
            0x01,
            0x99, // Invalid Sony payload type
            0x00,
            0x50, // Would be large payload length if it were Sony
            0x00,
            0x00,
            0x00,
            0x00, // Would be sequence number
            0x81,
            0x01,
            VISCA_TERMINATOR, // But it's actually raw VISCA
        ]
    }

    #[test]
    fn raw_mode_ignores_an_invalid_sony_header_with_large_declared_payload() {
        let mut framer = RawFramer::new_with_limits(256, 100, 1024);

        framer
            .push(Bytes::from(invalid_sony_header_then_raw_bytes()))
            .unwrap();

        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames.len(), 1);
        // Should treat as raw VISCA and stop at first 0xFF
        assert_eq!(frames[0].len(), 11);
        assert!(framer.is_empty());
    }

    #[test]
    fn sony_mode_retains_an_invalid_header_instead_of_delimiting_it() {
        let mut framer = SonyFramer::new_with_limits(256, 100, 1024);

        framer
            .push(Bytes::from(invalid_sony_header_then_raw_bytes()))
            .unwrap();

        // Sony mode never falls back to terminator framing: the eight header
        // bytes do not decode, so nothing is yielded and nothing is discarded.
        assert!(framer.drain_frames().next().is_none());
        assert_eq!(framer.buffered_len(), 11);
    }

    #[test]
    fn test_multiple_frames_with_limits() {
        // Create framer with reasonable limits
        let mut framer = RawFramer::new_with_limits(256, 50, 200);

        // First frame: small valid frame
        let frame1 = vec![0x81, 0x01, 0x04, 0x07, VISCA_TERMINATOR];

        // Second frame: also valid
        let frame2 = vec![0x90, 0x50, VISCA_TERMINATOR];

        let mut combined = frame1.clone();
        combined.extend(&frame2);

        framer.push(Bytes::from(combined)).unwrap();
        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], Bytes::from(frame1));
        assert_eq!(frames[1], Bytes::from(frame2));
    }

    #[test]
    fn test_framer_with_buffer_config() {
        let config = limits_512();
        let mut framer = RawFramer::new(config);

        // Should use the buffer config's recv_buffer_size for max_frame_size
        let frame = vec![0x81, 0x01, 0x04, 0x07, VISCA_TERMINATOR];
        framer.push(Bytes::from(frame.clone())).unwrap();

        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Bytes::from(frame));
    }

    #[test]
    fn test_framer_rejects_frame_gt_recv_size_for_sony() {
        // Create framer with Sony IP config (recv_buffer_size = 512)
        let config = limits_512();
        let mut framer = SonyFramer::new(config);

        // Build a Sony header with payload_length that exceeds recv_buffer_size
        // recv_buffer_size is 512, so a payload of 505 + 8 byte header = 513 bytes total
        let header = SonyHeader {
            payload_type: PayloadType::ViscaReply,
            payload_length: 505, // 505 + 8 = 513 > 512
            sequence_number: 0x12345678,
        };

        let mut data = Vec::from(header.encode());
        // We don't need to add all 505 bytes, just enough for the framer to detect the size
        data.extend_from_slice(&[0x90; 10]); // Add some payload bytes

        framer.push(Bytes::from(data)).unwrap();

        // Should get an error when trying to extract
        let results: Vec<_> = framer.drain_frames().collect();
        assert_eq!(results.len(), 1);
        match &results[0] {
            Err(FramingError::OversizedSonyFrame { max_size }) => {
                assert_eq!(*max_size, config.recv_buffer_size);
            }
            _ => panic!("Expected ResponseTooLarge error with max_size = recv_buffer_size"),
        }
    }

    #[test]
    fn test_framer_accepts_frame_eq_recv_size() {
        // Create framer with Sony IP config (recv_buffer_size = 512)
        let config = limits_512();
        let mut framer = SonyFramer::new(config);

        // Build a Sony header with payload that exactly equals recv_buffer_size
        // recv_buffer_size is 512, so payload of 504 + 8 byte header = 512 bytes exactly
        let header = SonyHeader {
            payload_type: PayloadType::ViscaReply,
            payload_length: 504, // 504 + 8 = 512
            sequence_number: 0x12345678,
        };

        let mut data = Vec::from(header.encode());
        // Add exactly 504 bytes of payload
        data.extend_from_slice(&[0x90; 504]);

        framer.push(Bytes::from(data.clone())).unwrap();

        // Should successfully extract the frame
        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].len(), 512); // Exactly recv_buffer_size
        assert!(framer.is_empty());
    }

    #[test]
    fn test_raw_visca_respects_recv_buffer_size_limit() {
        // Create framer with raw IP config (recv_buffer_size = 256)
        let config = BufferConfig::for_tcp();
        let mut framer = RawFramer::new(config);

        // Create a raw VISCA frame that exceeds recv_buffer_size
        let mut large_frame = vec![0x81; 257]; // 257 > 256
        large_frame[256] = VISCA_TERMINATOR; // Terminate at position 256

        framer.push(Bytes::from(large_frame)).unwrap();

        // Should get an error when trying to extract
        let results: Vec<_> = framer.drain_frames().collect();
        assert_eq!(results.len(), 1);
        match &results[0] {
            Err(FramingError::OversizedRawFrame { max_size }) => {
                assert_eq!(*max_size, config.recv_buffer_size);
            }
            _ => panic!("Expected ResponseTooLarge error"),
        }
    }

    #[test]
    fn test_max_buffer_size_accessor() {
        let framer = RawFramer::new_with_limits(256, 100, 42);
        assert_eq!(framer.max_buffer_size(), 42);

        let config = limits_512();
        let framer = RawFramer::new(config);
        assert_eq!(framer.max_buffer_size(), config.framer_capacity());
    }

    /// 512-byte frame limit with the standard retention bound.
    fn limits_512() -> BufferConfig {
        BufferConfig {
            recv_buffer_size: 512,
            max_buffer_size: 8192,
        }
    }

    /// #828: a configuration whose retention bound equals its frame limit is valid,
    /// so the framer must accept an incomplete reply followed by one full read
    /// of valid replies. Before the framer reserved room for one read on top of
    /// the retention bound, this burst overflowed (`3 + 64 > 64`) and a raw
    /// stream owner treated the healthy stream as desynchronized.
    #[test]
    fn a_partial_reply_plus_a_full_read_of_valid_replies_never_overflows() {
        let config = BufferConfig {
            recv_buffer_size: 64,
            max_buffer_size: 64,
        };
        let mut framer = RawFramer::new(config);

        // An ACK split across reads leaves three bytes retained.
        framer.push_slice(&[0x90, 0x41, 0x00]).unwrap();
        assert!(framer.drain_frames().next().is_none());

        // The next read fills the whole 64-byte receive buffer with the rest
        // of a 4-byte reply followed by twenty complete 3-byte replies.
        let mut read = vec![VISCA_TERMINATOR];
        for _ in 0..21 {
            read.extend_from_slice(&[0x90, 0x51, VISCA_TERMINATOR]);
        }
        assert_eq!(read.len(), config.recv_buffer_size);
        framer.push_slice(&read).unwrap();

        let frames = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames.len(), 22);
        assert!(framer.is_empty());
    }
}
