//! Protocol-aware framer for unified handling of VISCA and Sony-encapsulated frames.
//!
//! This module provides a zero-copy, protocol-aware frame decoder that correctly handles:
//! - Raw VISCA: splits frames on 0xFF terminator
//! - Sony-encapsulated: reads header + payload_length bytes (handles 0xFF in header/sequence)
//!
//! This replaces the naive 0xFF scanner to prevent premature splitting of Sony frames.

use bytes::{Bytes, BytesMut};

use crate::{
    command::bytes::VISCA_TERMINATOR,
    protocol::sony::{PayloadType, SonyHeader},
    transport::buffer::BufferConfig,
    Error,
};

/// A zero-copy, protocol-aware VISCA/Sony frame decoder.
///
/// This decoder accumulates bytes and correctly drains complete frames by:
/// - Detecting Sony headers and reading exact payload lengths
/// - Falling back to 0xFF terminator for raw VISCA
/// - Enforcing maximum frame and buffer sizes to prevent unbounded growth
///
/// This ensures Sony frames with 0xFF in headers/sequence are not prematurely split.
#[derive(Debug)]
pub struct ProtocolFramer {
    buf: BytesMut,
    max_frame_size: usize,
    max_buffer_size: usize,
}

impl ProtocolFramer {
    /// Create a new protocol framer with the specified initial capacity.
    #[cfg(test)]
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: BytesMut::with_capacity(capacity),
            max_frame_size: usize::MAX,
            max_buffer_size: usize::MAX,
        }
    }

    /// Create a new protocol framer with size limits from BufferConfig.
    pub fn new_with_config(config: BufferConfig) -> Self {
        Self {
            buf: BytesMut::with_capacity(config.recv_buffer_size),
            // A single frame should not exceed what we provisioned for one recv.
            // This aligns limits with the per-transport expectation.
            max_frame_size: config.recv_buffer_size,
            max_buffer_size: config.max_buffer_size,
        }
    }

    /// Create a new protocol framer with explicit size limits (mainly for testing).
    #[cfg(test)]
    pub fn new_with_limits(capacity: usize, max_frame_size: usize, max_buffer_size: usize) -> Self {
        Self {
            buf: BytesMut::with_capacity(capacity),
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
    /// This method is protocol-aware:
    /// - Sony frames: waits for full header, then reads exactly payload_length bytes
    /// - Raw VISCA: splits on 0xFF terminator
    pub fn drain_frames(&mut self) -> impl Iterator<Item = Result<Bytes, Error>> + '_ {
        std::iter::from_fn(move || self.extract_next_frame())
    }

    /// Extract the next complete frame from the buffer if available.
    fn extract_next_frame(&mut self) -> Option<Result<Bytes, Error>> {
        // Need at least 2 bytes to check for Sony header
        if self.buf.len() < 2 {
            return None;
        }

        // Check if this looks like a Sony header (0x01 followed by valid payload type)
        if self.buf[0] == 0x01 {
            // Try to decode as Sony header
            if let Some(_payload_type) = PayloadType::from_bytes([self.buf[0], self.buf[1]]) {
                // This looks like a Sony frame - wait for full header
                if self.buf.len() < SonyHeader::SIZE {
                    return None; // Need more data for full header
                }

                // Try to decode the Sony header
                if let Some(header) = SonyHeader::decode(&self.buf[..SonyHeader::SIZE]) {
                    // Valid Sony header - check size limits
                    let total_frame_size = SonyHeader::SIZE + header.payload_length as usize;

                    if total_frame_size > self.max_frame_size {
                        // Frame exceeds maximum allowed size
                        // Clear the invalid header to recover
                        let _ = self.buf.split_to(SonyHeader::SIZE);
                        return Some(Err(Error::ResponseTooLarge {
                            max_size: self.max_frame_size,
                        }));
                    }

                    if self.buf.len() >= total_frame_size {
                        // We have the complete Sony frame
                        return Some(Ok(self.buf.split_to(total_frame_size).freeze()));
                    }

                    // Need more data for complete payload
                    return None;
                }
            }
        }

        // Not a Sony header or invalid Sony header - treat as raw VISCA
        // Look for 0xFF terminator
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

    /// Try to extract any available frame when the stream has ended.
    /// This handles edge cases where EOF is encountered with partial data.
    #[cfg(all(test, not(feature = "mode-async")))]
    pub fn drain_on_eof(&mut self) -> Option<Result<Bytes, Error>> {
        if self.buf.is_empty() {
            return None;
        }

        // If we have data that looks like it could be a Sony header but we hit EOF,
        // treat it as raw VISCA and look for any terminator
        if let Some(pos) = self.buf.iter().position(|&b| b == VISCA_TERMINATOR) {
            let frame_size = pos + 1;
            if frame_size > self.max_frame_size {
                let _ = self.buf.split_to(frame_size);
                return Some(Err(Error::ResponseTooLarge {
                    max_size: self.max_frame_size,
                }));
            }
            return Some(Ok(self.buf.split_to(frame_size).freeze()));
        }

        // No terminator found and EOF reached - can't form a valid frame
        None
    }

    /// Returns the number of bytes currently buffered but not yet parsed into complete frames.
    #[cfg(test)]
    pub fn buffered_len(&self) -> usize {
        self.buf.len()
    }

    /// Returns true if the buffer is empty.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
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
