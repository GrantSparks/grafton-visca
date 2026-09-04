//! Protocol-aware framer for unified handling of VISCA and Sony-encapsulated frames.
//!
//! This module provides a zero-copy, protocol-aware frame decoder that correctly handles:
//! - Raw VISCA: splits frames on 0xFF terminator
//! - Sony-encapsulated: reads header + payload_length bytes (handles 0xFF in header/sequence)
//!
//! This replaces the naive 0xFF scanner to prevent premature splitting of Sony frames.

use bytes::{Bytes, BytesMut};

use crate::{
    command::bytes::VISCA_TERMINATOR, protocol::sony::SonyHeader, CameraId, Error, ViscaSocket,
};

#[cfg(test)]
use crate::protocol::sony::PayloadType;

#[cfg(any(feature = "async", feature = "blocking", test))]
use crate::transport::buffer::BufferConfig;

/// Wire framing selected for one transport connection.
///
/// A production connection's envelope is known before any response bytes
/// arrive, so owners must select [`Self::RawVisca`] or
/// [`Self::SonyEncapsulated`]. Test builds additionally retain a legacy
/// auto-detect mode for compatibility coverage; production code cannot select
/// it from untrusted received bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramingMode {
    /// Raw VISCA frames are delimited solely by the `0xFF` terminator.
    RawVisca,
    /// Sony VISCA-over-IP frames use the eight-byte Sony header and its length.
    SonyEncapsulated,
    /// Legacy compatibility mode that chooses Sony framing from the first two
    /// received bytes and otherwise scans for a raw VISCA terminator.
    #[cfg(test)]
    AutoDetect,
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
        target: CameraId,
        kind: RawIncompletePrefix,
    },
}

/// A zero-copy, protocol-aware VISCA/Sony frame decoder.
///
/// This decoder accumulates bytes and correctly drains complete frames according
/// to its configured [`FramingMode`]:
/// - Raw VISCA splits on the `0xFF` terminator
/// - Sony encapsulation reads the eight-byte header plus its payload length
/// - Enforcing maximum frame and buffer sizes to prevent unbounded growth
#[derive(Debug)]
pub struct ProtocolFramer {
    buf: BytesMut,
    mode: FramingMode,
    max_frame_size: usize,
    max_buffer_size: usize,
}

impl ProtocolFramer {
    /// Create a legacy auto-detecting framer with the specified initial capacity.
    ///
    /// New production code that knows its wire envelope should select an
    /// explicit [`FramingMode`] with [`Self::new_with_limits_and_mode`].
    #[cfg(test)]
    pub fn new(capacity: usize) -> Self {
        Self::new_with_limits_and_mode(capacity, usize::MAX, usize::MAX, FramingMode::AutoDetect)
    }

    /// Create a legacy auto-detecting framer with size limits from `BufferConfig`.
    ///
    /// This compatibility constructor preserves the historical behavior of
    /// detecting a Sony header from received bytes. Owners with a validated
    /// envelope must instead call [`Self::new_with_config_and_mode`].
    #[cfg(test)]
    pub fn new_with_config(config: BufferConfig) -> Self {
        Self::new_with_config_and_mode(config, FramingMode::AutoDetect)
    }

    /// Create a protocol framer with size limits and an explicit wire mode.
    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub fn new_with_config_and_mode(config: BufferConfig, mode: FramingMode) -> Self {
        Self {
            buf: BytesMut::with_capacity(config.recv_buffer_size),
            mode,
            // A single frame should not exceed what we provisioned for one recv.
            // This aligns limits with the per-transport expectation.
            max_frame_size: config.recv_buffer_size,
            max_buffer_size: config.max_buffer_size,
        }
    }

    /// Create a legacy auto-detecting framer with explicit size limits.
    #[cfg(test)]
    pub fn new_with_limits(capacity: usize, max_frame_size: usize, max_buffer_size: usize) -> Self {
        Self::new_with_limits_and_mode(
            capacity,
            max_frame_size,
            max_buffer_size,
            FramingMode::AutoDetect,
        )
    }

    /// Create a framer with explicit size limits and an explicit wire mode.
    #[cfg(test)]
    pub fn new_with_limits_and_mode(
        capacity: usize,
        max_frame_size: usize,
        max_buffer_size: usize,
        mode: FramingMode,
    ) -> Self {
        Self {
            buf: BytesMut::with_capacity(capacity),
            mode,
            max_frame_size,
            max_buffer_size,
        }
    }

    /// Push a chunk of bytes into the framer's buffer.
    /// Returns an error if the buffer would exceed max_buffer_size.
    #[cfg(test)]
    pub fn push(&mut self, chunk: Bytes) -> Result<(), Error> {
        let new_len = self.buf.len() + chunk.len();
        if new_len > self.max_buffer_size {
            return Err(Error::ResponseTooLarge {
                max_size: self.max_buffer_size,
            });
        }
        self.buf.extend_from_slice(&chunk);
        Ok(())
    }

    /// Push a slice of bytes into the framer's buffer.
    /// Returns an error if the buffer would exceed max_buffer_size.
    /// This avoids the need to construct a Bytes object when you already have a slice.
    pub fn push_slice(&mut self, chunk: &[u8]) -> Result<(), Error> {
        let new_len = self.buf.len() + chunk.len();
        if new_len > self.max_buffer_size {
            return Err(Error::ResponseTooLarge {
                max_size: self.max_buffer_size,
            });
        }
        self.buf.extend_from_slice(chunk);
        Ok(())
    }

    /// Drain all complete frames from the buffer.
    ///
    /// Returns an iterator that yields zero-copy `Bytes` for each complete frame,
    /// or an error if a frame exceeds size limits.
    /// Incomplete frames remain in the buffer for future processing.
    ///
    /// Frames are decoded only according to this framer's configured
    /// [`FramingMode`].
    pub fn drain_frames(&mut self) -> impl Iterator<Item = Result<Bytes, Error>> + '_ {
        std::iter::from_fn(move || self.extract_next_frame())
    }

    /// Extract the next complete frame from the buffer if available.
    fn extract_next_frame(&mut self) -> Option<Result<Bytes, Error>> {
        match self.mode {
            // A raw owner must never infer Sony framing from response bytes:
            // malformed/noise prefixes can legally contain the Sony type bytes.
            FramingMode::RawVisca => self.extract_raw_frame(),
            FramingMode::SonyEncapsulated => self.extract_sony_frame(),
            // Preserve the historical lower-level behavior for callers that
            // intentionally accept auto detection. Production owners do not
            // construct framers in this mode.
            #[cfg(test)]
            FramingMode::AutoDetect => {
                if self.buf.len() < 2 {
                    return None;
                }
                if PayloadType::from_bytes([self.buf[0], self.buf[1]]).is_some() {
                    // Match the legacy fallback exactly: a recognized type
                    // waits for a complete header, but a header decoder that
                    // rejects those eight bytes is delimiter-framed as raw.
                    if self.buf.len() < SonyHeader::SIZE {
                        return None;
                    }
                    if SonyHeader::decode(&self.buf[..SonyHeader::SIZE]).is_some() {
                        return self.extract_sony_frame();
                    }
                }
                self.extract_raw_frame()
            }
        }
    }

    /// Extract one raw VISCA frame at its terminator.
    fn extract_raw_frame(&mut self) -> Option<Result<Bytes, Error>> {
        if let Some(pos) = self.buf.iter().position(|&b| b == VISCA_TERMINATOR) {
            let frame_size = pos + 1;
            if frame_size > self.max_frame_size {
                // Frame exceeds maximum allowed size
                // Clear up to and including the terminator to recover
                let _ = self.buf.split_to(frame_size);
                return Some(Err(Error::ResponseTooLarge {
                    max_size: self.max_frame_size,
                }));
            }
            // Split at terminator position + 1 to include the terminator
            Some(Ok(self.buf.split_to(frame_size).freeze()))
        } else {
            None
        }
    }

    /// Extract one Sony frame from its header and declared payload length.
    fn extract_sony_frame(&mut self) -> Option<Result<Bytes, Error>> {
        if self.buf.len() < SonyHeader::SIZE {
            return None;
        }

        // Sony mode deliberately does not fall back to delimiter framing:
        // `0xFF` can occur in the header and sequence number. An invalid
        // header has no reliable payload boundary, so it remains buffered
        // until the configured maximum-buffer handling decides recovery.
        let header = SonyHeader::decode(&self.buf[..SonyHeader::SIZE])?;
        let total_frame_size = SonyHeader::SIZE + header.payload_length as usize;

        if total_frame_size > self.max_frame_size {
            // Discard the known header so a direct framer caller can resume
            // after reporting the oversized Sony frame.
            let _ = self.buf.split_to(SonyHeader::SIZE);
            return Some(Err(Error::ResponseTooLarge {
                max_size: self.max_frame_size,
            }));
        }

        if self.buf.len() >= total_frame_size {
            return Some(Ok(self.buf.split_to(total_frame_size).freeze()));
        }

        None
    }

    /// Try to extract any available frame when the stream has ended.
    /// This handles edge cases where EOF is encountered with partial data.
    #[cfg(all(test, feature = "blocking"))]
    pub fn drain_on_eof(&mut self) -> Option<Result<Bytes, Error>> {
        if self.buf.is_empty() {
            return None;
        }

        match self.mode {
            // Compatibility mode historically recovers a terminator-delimited
            // raw frame at EOF even after a Sony-looking prefix.
            FramingMode::RawVisca | FramingMode::AutoDetect => self.extract_raw_frame(),
            FramingMode::SonyEncapsulated => self.extract_sony_frame(),
        }
    }

    /// Returns the number of bytes currently buffered but not yet parsed into complete frames.
    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub(crate) fn buffered_len(&self) -> usize {
        self.buf.len()
    }

    /// Returns true if the buffer is empty.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Whether bytes remain buffered without yet forming a complete frame.
    /// Transport owners use this to enforce datagram boundaries while keeping
    /// stream partial-frame state across reads.
    pub fn has_buffered_data(&self) -> bool {
        !self.buf.is_empty()
    }

    /// Classify the first retained raw input once for both owner shells.
    ///
    /// `target_for_source` must apply the owner's same strict source-byte
    /// routing rule used for complete responses.  A source it cannot route is
    /// reported as [`RawBufferedInput::Malformed`], never as an error: stream
    /// noise is discardable framing input, not a session-wide framing failure.
    /// An empty buffer and a non-raw framer have no raw-prefix evidence.
    pub(crate) fn buffered_raw_incomplete_prefix(
        &self,
        mut target_for_source: impl FnMut(u8) -> Option<CameraId>,
    ) -> Option<RawBufferedInput> {
        if self.mode != FramingMode::RawVisca || self.buf.is_empty() {
            return None;
        }
        if self.buf.contains(&VISCA_TERMINATOR) {
            return Some(RawBufferedInput::Complete);
        }

        let source = self.buf[0];
        let Some(target) = target_for_source(source) else {
            return Some(RawBufferedInput::Malformed);
        };
        let kind = match self.buf.get(1).copied() {
            None => RawIncompletePrefix::SourceOnly,
            // An ACK socket nibble is a preference for assigning a free
            // socket, never evidence of who owns a named socket already.
            Some(0x40..=0x4f) => RawIncompletePrefix::Ack,
            Some(0x50) => RawIncompletePrefix::SocketlessCompletion,
            Some(0x60) => RawIncompletePrefix::SocketlessError,
            Some(0x51 | 0x61) => RawIncompletePrefix::NamedCompletionOrError(ViscaSocket::S1),
            Some(0x52 | 0x62) => RawIncompletePrefix::NamedCompletionOrError(ViscaSocket::S2),
            Some(_) => RawIncompletePrefix::Noncorrelating,
        };
        Some(RawBufferedInput::Incomplete { target, kind })
    }

    /// Discard exactly the first retained raw frame, or the sole incomplete
    /// raw fragment when no terminator has arrived yet.
    ///
    /// Bytes after the first terminator are preserved. That distinction is
    /// load-bearing for multi-camera serial: an expiring target must not erase
    /// a later complete response already buffered for another target.
    pub(crate) fn discard_first_raw_input(&mut self) -> Result<(), Error> {
        if self.mode != FramingMode::RawVisca {
            return Err(Error::InvalidState(
                "target-aware discard requires a raw VISCA framer".into(),
            ));
        }
        if let Some(pos) = self.buf.iter().position(|&byte| byte == VISCA_TERMINATOR) {
            let _ = self.buf.split_to(pos + 1);
        } else {
            self.buf.clear();
        }
        Ok(())
    }

    /// Clear the internal buffer, discarding any incomplete frames.
    pub fn clear(&mut self) {
        self.buf.clear();
    }

    /// Returns the maximum buffer size limit.
    #[cfg(test)]
    pub fn max_buffer_size(&self) -> usize {
        self.max_buffer_size
    }

    /// Push a slice of bytes with automatic resync on max-buffer overflow.
    ///
    /// This method handles the case where the internal buffer has grown to its
    /// limit without finding a complete frame boundary. When this occurs:
    ///
    /// 1. The buffer is cleared to discard the un-framable accumulated bytes
    /// 2. The incoming chunk is retried (it may contain the start of a valid frame)
    /// 3. If the retry also fails, the original error is returned
    ///
    /// This prevents permanent runtime stalls by ensuring the framer can always
    /// recover from buffer overflow conditions and resume processing valid frames.
    ///
    /// # Arguments
    ///
    /// * `chunk` - The bytes to push into the framer buffer
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Push succeeded normally
    /// * `Ok(false)` - Push required resync (buffer was cleared and chunk re-pushed)
    /// * `Err(Error::ResponseTooLarge)` - Resync failed (chunk alone exceeds max_buffer_size)
    #[cfg(any(feature = "transport-serial", feature = "transport-serial-tokio", test))]
    pub fn push_slice_with_resync(&mut self, chunk: &[u8]) -> Result<bool, Error> {
        match self.push_slice(chunk) {
            Ok(()) => Ok(true),
            Err(Error::ResponseTooLarge { max_size }) => {
                // Buffer overflow detected - attempt resync
                let buffered_before = self.buf.len();
                tracing::warn!(
                    buffered_len = buffered_before,
                    max_buffer_size = max_size,
                    chunk_len = chunk.len(),
                    "Framer buffer overflow detected, resyncing"
                );

                // Clear the buffer to recover
                self.clear();

                // Retry pushing the chunk - it may contain the start of a valid frame
                match self.push_slice(chunk) {
                    Ok(()) => {
                        tracing::debug!(
                            chunk_len = chunk.len(),
                            "Resync successful, chunk pushed after clearing buffer"
                        );
                        Ok(false)
                    }
                    Err(e) => {
                        // Chunk alone exceeds max_buffer_size - this is a configuration
                        // issue (e.g., recv_buffer_size > max_buffer_size)
                        tracing::error!(
                            chunk_len = chunk.len(),
                            max_buffer_size = max_size,
                            "Resync failed: chunk alone exceeds max_buffer_size"
                        );
                        Err(e)
                    }
                }
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_visca_single_frame() {
        let mut framer = ProtocolFramer::new(256);
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
        let mut framer = ProtocolFramer::new(256);
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
        let mut framer = ProtocolFramer::new(256);
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
        let framer = ProtocolFramer::new_with_limits_and_mode(32, 32, 32, FramingMode::RawVisca);

        assert_eq!(framer.buffered_raw_incomplete_prefix(|_| None), None);
    }

    #[test]
    fn raw_prefix_classifier_reports_one_byte_fragment() {
        let mut framer =
            ProtocolFramer::new_with_limits_and_mode(32, 32, 32, FramingMode::RawVisca);
        framer.push_slice(&[0x90]).unwrap();

        assert_eq!(
            framer.buffered_raw_incomplete_prefix(|source| {
                (source == 0x90).then_some(CameraId::CAMERA_1)
            }),
            Some(RawBufferedInput::Incomplete {
                target: CameraId::CAMERA_1,
                kind: RawIncompletePrefix::SourceOnly,
            })
        );
    }

    #[test]
    fn raw_prefix_classifier_reports_two_byte_fragment() {
        let mut framer =
            ProtocolFramer::new_with_limits_and_mode(32, 32, 32, FramingMode::RawVisca);
        framer.push_slice(&[0x90, 0x51]).unwrap();

        assert_eq!(
            framer.buffered_raw_incomplete_prefix(|source| {
                (source == 0x90).then_some(CameraId::CAMERA_1)
            }),
            Some(RawBufferedInput::Incomplete {
                target: CameraId::CAMERA_1,
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
            let mut framer =
                ProtocolFramer::new_with_limits_and_mode(32, 32, 32, FramingMode::RawVisca);
            framer.push_slice(bytes).unwrap();
            assert_eq!(
                framer.buffered_raw_incomplete_prefix(|source| {
                    (source == 0x90).then_some(CameraId::CAMERA_1)
                }),
                Some(RawBufferedInput::Incomplete {
                    target: CameraId::CAMERA_1,
                    kind: expected,
                }),
                "bytes {bytes:02x?}",
            );
        }
    }

    #[test]
    fn raw_prefix_classifier_preserves_complete_input_for_normal_decode() {
        let mut framer =
            ProtocolFramer::new_with_limits_and_mode(32, 32, 32, FramingMode::RawVisca);
        framer
            .push_slice(&[0x90, 0x51, VISCA_TERMINATOR, 0x90, 0x41, VISCA_TERMINATOR])
            .unwrap();

        assert_eq!(
            framer.buffered_raw_incomplete_prefix(|source| {
                (source == 0x90).then_some(CameraId::CAMERA_1)
            }),
            Some(RawBufferedInput::Complete)
        );
    }

    #[test]
    fn raw_prefix_classifier_treats_one_byte_terminated_input_as_complete() {
        let mut framer =
            ProtocolFramer::new_with_limits_and_mode(32, 32, 32, FramingMode::RawVisca);
        framer
            .push_slice(&[0x90, VISCA_TERMINATOR, 0x51, VISCA_TERMINATOR])
            .unwrap();

        assert_eq!(
            framer.buffered_raw_incomplete_prefix(|source| {
                (source == 0x90).then_some(CameraId::CAMERA_1)
            }),
            Some(RawBufferedInput::Complete)
        );
    }

    #[test]
    fn raw_prefix_classifier_ignores_non_raw_framing() {
        let framer =
            ProtocolFramer::new_with_limits_and_mode(32, 32, 32, FramingMode::SonyEncapsulated);

        assert_eq!(framer.buffered_raw_incomplete_prefix(|_| None), None);
    }

    /// Issue #745: malformed incomplete prefixes are classified at the raw
    /// framer boundary, where they can be discarded like their complete twins
    /// instead of escaping as fallible owner routing input.
    #[test]
    fn raw_prefix_classifier_marks_invalid_response_sources_malformed() {
        for bytes in [&[0x80, 0x50, 0xdd][..], &[0x88, 0x30, 0x02], &[0x00]] {
            let mut framer =
                ProtocolFramer::new_with_limits_and_mode(32, 32, 32, FramingMode::RawVisca);
            framer.push_slice(bytes).unwrap();
            assert_eq!(
                framer.buffered_raw_incomplete_prefix(|source| {
                    (source == 0x90).then_some(CameraId::CAMERA_1)
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
            let mut framer =
                ProtocolFramer::new_with_limits_and_mode(32, 32, 32, FramingMode::RawVisca);
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
        let mut framer = ProtocolFramer::new(256);

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
        let mut framer =
            ProtocolFramer::new_with_limits_and_mode(32, 32, 32, FramingMode::SonyEncapsulated);
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
    fn sony_mode_rejects_oversized_declared_payload_before_it_arrives() {
        let mut framer =
            ProtocolFramer::new_with_limits_and_mode(32, 20, 32, FramingMode::SonyEncapsulated);
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
            Some(Err(Error::ResponseTooLarge { max_size: 20 }))
        ));
        assert!(framer.is_empty());
    }

    #[test]
    fn test_sony_frame_with_ff_in_sequence() {
        let mut framer = ProtocolFramer::new(256);

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
        let mut framer = ProtocolFramer::new(256);

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
        let mut framer = ProtocolFramer::new(256);

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

    #[test]
    fn test_mixed_sony_and_raw_frames() {
        let mut framer = ProtocolFramer::new(256);

        // First: Sony frame
        let sony_header = SonyHeader {
            payload_type: PayloadType::ViscaReply,
            payload_length: 3,
            sequence_number: 0x11111111,
        };
        let mut data = Vec::from(sony_header.encode());
        data.extend_from_slice(&[0x90, 0x50, VISCA_TERMINATOR]);

        // Then: Raw VISCA frame
        data.extend_from_slice(&[0x81, 0x01, 0x04, 0x07, VISCA_TERMINATOR]);

        // Then: Another Sony frame with 0xFF in sequence
        let sony_header2 = SonyHeader {
            payload_type: PayloadType::ViscaReply,
            payload_length: 2,
            sequence_number: 0xFFFFFFFF,
        };
        data.extend_from_slice(&sony_header2.encode());
        data.extend_from_slice(&[0x90, VISCA_TERMINATOR]);

        framer.push(Bytes::from(data)).unwrap();
        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].len(), SonyHeader::SIZE + 3, "First Sony frame");
        assert_eq!(
            frames[1],
            Bytes::from(vec![0x81, 0x01, 0x04, 0x07, VISCA_TERMINATOR]),
            "Raw VISCA frame"
        );
        assert_eq!(frames[2].len(), SonyHeader::SIZE + 2, "Second Sony frame");
        assert!(framer.is_empty());
    }

    #[test]
    fn test_trailing_incomplete_frame() {
        let mut framer = ProtocolFramer::new(256);
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
    fn test_invalid_sony_header_falls_back_to_raw() {
        let mut framer = ProtocolFramer::new(256);

        // Start with 0x01 but invalid payload type byte
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
        let mut framer = ProtocolFramer::new(256);
        framer.push(Bytes::new()).unwrap();

        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames.len(), 0);
        assert!(framer.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut framer = ProtocolFramer::new(256);
        framer.push(Bytes::from(vec![0x81, 0x01, 0x04])).unwrap();

        assert_eq!(framer.buffered_len(), 3);
        framer.clear();
        assert_eq!(framer.buffered_len(), 0);
        assert!(framer.is_empty());
    }

    #[test]
    fn test_back_to_back_sony_frames() {
        let mut framer = ProtocolFramer::new(256);

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
        let mut framer = ProtocolFramer::new(256);

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
        let mut framer = ProtocolFramer::new_with_limits(256, 20, 1024);

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
            Err(Error::ResponseTooLarge { max_size }) => {
                assert_eq!(*max_size, 20);
            }
            _ => panic!("Expected ResponseTooLarge error"),
        }
    }

    #[test]
    fn test_raw_visca_frame_exceeds_max_size() {
        // Create framer with small max frame size
        let mut framer = ProtocolFramer::new_with_limits(256, 10, 1024);

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
            Err(Error::ResponseTooLarge { max_size }) => {
                assert_eq!(*max_size, 10);
            }
            _ => panic!("Expected ResponseTooLarge error"),
        }

        // Buffer should be cleared after error
        assert!(framer.is_empty());
    }

    #[test]
    fn test_buffer_exceeds_max_size() {
        // Create framer with small max buffer size
        let mut framer = ProtocolFramer::new_with_limits(256, 100, 50);

        // Try to push data that would exceed buffer limit
        let data = vec![0x81; 60]; // 60 bytes > 50 max buffer

        let result = framer.push(Bytes::from(data));
        match result {
            Err(Error::ResponseTooLarge { max_size }) => {
                assert_eq!(max_size, 50);
            }
            _ => panic!("Expected ResponseTooLarge error"),
        }
    }

    #[test]
    fn test_invalid_sony_header_with_large_payload_falls_back() {
        // Create framer with reasonable limits
        let mut framer = ProtocolFramer::new_with_limits(256, 100, 1024);

        // Start with 0x01 but invalid payload type, then valid raw VISCA
        // This tests that invalid Sony headers still fall back to raw VISCA
        let data = vec![
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
        ];

        framer.push(Bytes::from(data.clone())).unwrap();

        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames.len(), 1);
        // Should treat as raw VISCA and stop at first 0xFF
        assert_eq!(frames[0].len(), 11);
    }

    #[test]
    fn test_multiple_frames_with_limits() {
        // Create framer with reasonable limits
        let mut framer = ProtocolFramer::new_with_limits(256, 50, 200);

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
        let config = BufferConfig::for_sony_ip();
        let mut framer = ProtocolFramer::new_with_config(config);

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
        let config = BufferConfig::for_sony_ip();
        let mut framer = ProtocolFramer::new_with_config(config);

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
            Err(Error::ResponseTooLarge { max_size }) => {
                assert_eq!(*max_size, config.recv_buffer_size);
            }
            _ => panic!("Expected ResponseTooLarge error with max_size = recv_buffer_size"),
        }
    }

    #[test]
    fn test_framer_accepts_frame_eq_recv_size() {
        // Create framer with Sony IP config (recv_buffer_size = 512)
        let config = BufferConfig::for_sony_ip();
        let mut framer = ProtocolFramer::new_with_config(config);

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
        let config = BufferConfig::for_raw_ip();
        let mut framer = ProtocolFramer::new_with_config(config);

        // Create a raw VISCA frame that exceeds recv_buffer_size
        let mut large_frame = vec![0x81; 257]; // 257 > 256
        large_frame[256] = VISCA_TERMINATOR; // Terminate at position 256

        framer.push(Bytes::from(large_frame)).unwrap();

        // Should get an error when trying to extract
        let results: Vec<_> = framer.drain_frames().collect();
        assert_eq!(results.len(), 1);
        match &results[0] {
            Err(Error::ResponseTooLarge { max_size }) => {
                assert_eq!(*max_size, config.recv_buffer_size);
            }
            _ => panic!("Expected ResponseTooLarge error"),
        }
    }

    // Tests for push_slice_with_resync (Issue #465)

    #[test]
    fn test_push_slice_with_resync_normal_operation() {
        // Normal push should return Ok(true)
        let mut framer = ProtocolFramer::new_with_limits(256, 100, 100);

        let chunk = vec![0x81, 0x01, 0x04, 0x07, VISCA_TERMINATOR];
        let result = framer.push_slice_with_resync(&chunk);

        assert!(
            matches!(result, Ok(true)),
            "Expected Ok(true) for normal push"
        );
        assert_eq!(framer.buffered_len(), 5);

        // Should be able to drain the frame
        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames.len(), 1);
        assert!(framer.is_empty());
    }

    #[test]
    fn test_push_slice_with_resync_clears_buffer_on_overflow() {
        // Create framer with small max buffer size
        let mut framer = ProtocolFramer::new_with_limits(256, 100, 50);

        // Fill buffer close to limit with data that has no frame terminator
        let garbage = vec![0x81; 45]; // 45 bytes, no terminator
        framer.push_slice(&garbage).unwrap();
        assert_eq!(framer.buffered_len(), 45);

        // This chunk would cause overflow (45 + 10 = 55 > 50)
        // push_slice_with_resync should clear and retry
        let new_chunk = vec![
            0x90,
            0x50,
            VISCA_TERMINATOR,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
        ];
        let result = framer.push_slice_with_resync(&new_chunk);

        // Should succeed with resync (Ok(false) means resync occurred)
        assert!(
            matches!(result, Ok(false)),
            "Expected Ok(false) indicating resync, got {:?}",
            result
        );

        // Buffer should now contain only the new chunk
        assert_eq!(framer.buffered_len(), 10);

        // Should be able to extract the valid frame from the new chunk
        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Bytes::from(vec![0x90, 0x50, VISCA_TERMINATOR]));

        // Remaining bytes should be in buffer
        assert_eq!(framer.buffered_len(), 7);
    }

    #[test]
    fn test_push_slice_with_resync_fails_when_chunk_exceeds_max() {
        // Create framer with very small max buffer size
        let mut framer = ProtocolFramer::new_with_limits(256, 100, 20);

        // Chunk alone exceeds max_buffer_size - resync cannot help
        let large_chunk = vec![0x81; 30]; // 30 > 20
        let result = framer.push_slice_with_resync(&large_chunk);

        // Should return error even after resync attempt
        assert!(
            matches!(result, Err(Error::ResponseTooLarge { max_size: 20 })),
            "Expected ResponseTooLarge error, got {:?}",
            result
        );

        // Buffer should be empty after failed resync
        assert!(framer.is_empty());
    }

    #[test]
    fn test_push_slice_with_resync_recovery_then_valid_frame() {
        // Test the full recovery scenario from Issue #465:
        // 1. Buffer fills with garbage (no terminator)
        // 2. Overflow occurs
        // 3. Resync clears buffer
        // 4. Subsequent valid frames are processed correctly

        let mut framer = ProtocolFramer::new_with_limits(256, 100, 50);

        // Step 1: Fill with garbage that has no terminator
        let garbage = vec![0x81; 48];
        framer.push_slice(&garbage).unwrap();
        assert_eq!(framer.buffered_len(), 48);

        // Step 2 & 3: Push chunk that triggers overflow and resync
        let recovery_chunk = vec![0x90, 0x50, VISCA_TERMINATOR];
        let result = framer.push_slice_with_resync(&recovery_chunk);
        assert!(matches!(result, Ok(false)), "Expected resync to occur");

        // Step 4: Verify valid frame can be extracted after recovery
        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Bytes::from(vec![0x90, 0x50, VISCA_TERMINATOR]));
        assert!(framer.is_empty());

        // Additional verification: framer continues to work normally
        let next_frame = vec![0x81, 0x01, 0x04, 0x07, VISCA_TERMINATOR];
        let result = framer.push_slice_with_resync(&next_frame);
        assert!(
            matches!(result, Ok(true)),
            "Normal push should return Ok(true)"
        );

        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Bytes::from(next_frame));
    }

    #[test]
    fn test_push_slice_with_resync_multiple_overflows() {
        // Test that multiple overflow/resync cycles work correctly
        let mut framer = ProtocolFramer::new_with_limits(256, 100, 30);

        for i in 0..3 {
            // Fill close to limit
            let garbage = vec![0x81; 28];
            framer.push_slice(&garbage).unwrap();
            assert_eq!(
                framer.buffered_len(),
                28,
                "Iteration {i}: buffer should have 28 bytes"
            );

            // Trigger overflow and resync
            let chunk = vec![0x90, 0x50, VISCA_TERMINATOR];
            let result = framer.push_slice_with_resync(&chunk);
            assert!(
                matches!(result, Ok(false)),
                "Iteration {i}: Expected resync"
            );

            // Drain the valid frame
            let frames: Vec<_> = framer
                .drain_frames()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert_eq!(frames.len(), 1, "Iteration {i}: Should have one frame");
            assert!(framer.is_empty(), "Iteration {i}: Buffer should be empty");
        }
    }

    #[test]
    fn test_push_slice_with_resync_preserves_valid_data() {
        // When resync occurs, the new chunk may contain multiple frames
        // or partial frame data - verify this is preserved correctly
        let mut framer = ProtocolFramer::new_with_limits(256, 100, 50);

        // Fill buffer with garbage
        let garbage = vec![0x81; 48];
        framer.push_slice(&garbage).unwrap();

        // New chunk contains two complete frames
        let two_frames = vec![
            0x90,
            0x50,
            VISCA_TERMINATOR, // Frame 1
            0x90,
            0x41,
            VISCA_TERMINATOR, // Frame 2
        ];
        let result = framer.push_slice_with_resync(&two_frames);
        assert!(matches!(result, Ok(false)));

        // Both frames should be extractable
        let frames: Vec<_> = framer
            .drain_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], Bytes::from(vec![0x90, 0x50, VISCA_TERMINATOR]));
        assert_eq!(frames[1], Bytes::from(vec![0x90, 0x41, VISCA_TERMINATOR]));
        assert!(framer.is_empty());
    }

    #[test]
    fn test_max_buffer_size_accessor() {
        let framer = ProtocolFramer::new_with_limits(256, 100, 42);
        assert_eq!(framer.max_buffer_size(), 42);

        let config = BufferConfig::for_sony_ip();
        let framer = ProtocolFramer::new_with_config(config);
        assert_eq!(framer.max_buffer_size(), config.max_buffer_size);
    }
}
