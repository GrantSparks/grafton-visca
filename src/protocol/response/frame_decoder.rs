use bytes::{Bytes, BytesMut};

use crate::command::bytes::VISCA_TERMINATOR;

/// A zero-copy VISCA frame decoder that accumulates bytes and drains complete frames.
///
/// This decoder maintains an internal `BytesMut` buffer and provides methods to:
/// - Push incoming byte chunks
/// - Drain complete 0xFF-terminated frames without copying
///
/// The decoder preserves VISCA invariants by ensuring all emitted frames end with 0xFF.
#[derive(Debug)]
pub struct FrameDecoder {
    buf: BytesMut,
}

impl FrameDecoder {
    /// Create a new frame decoder with the specified initial capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: BytesMut::with_capacity(capacity),
        }
    }

    /// Push a chunk of bytes into the decoder's buffer.
    pub fn push(&mut self, chunk: Bytes) {
        self.buf.extend_from_slice(&chunk);
    }

    /// Drain all complete 0xFF-terminated frames from the buffer.
    ///
    /// Returns an iterator that yields zero-copy `Bytes` for each complete frame.
    /// Incomplete frames remain in the buffer for future processing.
    pub fn drain_frames(&mut self) -> impl Iterator<Item = Bytes> + '_ {
        std::iter::from_fn(move || {
            if let Some(pos) = self.buf.iter().position(|&b| b == VISCA_TERMINATOR) {
                // Split the buffer at the terminator position + 1 to include the terminator
                Some(self.buf.split_to(pos + 1).freeze())
            } else {
                None
            }
        })
    }

    /// Returns the number of bytes currently buffered but not yet parsed into complete frames.
    pub fn buffered_len(&self) -> usize {
        self.buf.len()
    }

    /// Returns true if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Clear the internal buffer, discarding any incomplete frames.
    pub fn clear(&mut self) {
        self.buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_frame_single_chunk() {
        let mut decoder = FrameDecoder::new(256);
        let frame = Bytes::from(vec![0x81, 0x01, 0x04, 0x07, VISCA_TERMINATOR]);

        decoder.push(frame.clone());
        let frames: Vec<_> = decoder.drain_frames().collect();

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], frame);
        assert!(decoder.is_empty());
    }

    #[test]
    fn test_multiple_frames_single_chunk() {
        let mut decoder = FrameDecoder::new(256);
        let frame1 = vec![0x81, 0x01, 0x04, 0x07, VISCA_TERMINATOR];
        let frame2 = vec![0x90, 0x50, VISCA_TERMINATOR];
        let mut combined = frame1.clone();
        combined.extend(&frame2);

        decoder.push(Bytes::from(combined));
        let frames: Vec<_> = decoder.drain_frames().collect();

        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], Bytes::from(frame1));
        assert_eq!(frames[1], Bytes::from(frame2));
        assert!(decoder.is_empty());
    }

    #[test]
    fn test_frame_split_across_chunks() {
        let mut decoder = FrameDecoder::new(256);
        let chunk1 = Bytes::from(vec![0x81, 0x01, 0x04]);
        let chunk2 = Bytes::from(vec![0x07, VISCA_TERMINATOR]);

        decoder.push(chunk1.clone());
        let frames1: Vec<_> = decoder.drain_frames().collect();
        assert_eq!(frames1.len(), 0);
        assert_eq!(decoder.buffered_len(), 3);

        decoder.push(chunk2);
        let frames2: Vec<_> = decoder.drain_frames().collect();
        assert_eq!(frames2.len(), 1);
        assert_eq!(
            frames2[0],
            Bytes::from(vec![0x81, 0x01, 0x04, 0x07, VISCA_TERMINATOR])
        );
        assert!(decoder.is_empty());
    }

    #[test]
    fn test_trailing_incomplete_frame() {
        let mut decoder = FrameDecoder::new(256);
        let data = Bytes::from(vec![0x81, 0x01, 0x04, 0x07, VISCA_TERMINATOR, 0x90, 0x50]);

        decoder.push(data);
        let frames: Vec<_> = decoder.drain_frames().collect();

        assert_eq!(frames.len(), 1);
        assert_eq!(
            frames[0],
            Bytes::from(vec![0x81, 0x01, 0x04, 0x07, VISCA_TERMINATOR])
        );
        assert_eq!(decoder.buffered_len(), 2);
        assert!(!decoder.is_empty());
    }

    #[test]
    fn test_empty_input() {
        let mut decoder = FrameDecoder::new(256);
        decoder.push(Bytes::new());

        let frames: Vec<_> = decoder.drain_frames().collect();
        assert_eq!(frames.len(), 0);
        assert!(decoder.is_empty());
    }

    #[test]
    fn test_all_frames_end_with_terminator() {
        let mut decoder = FrameDecoder::new(256);
        let data = Bytes::from(vec![
            0x81,
            VISCA_TERMINATOR, // Short frame
            0x90,
            0x50,
            VISCA_TERMINATOR, // Medium frame
            0x81,
            0x01,
            0x04,
            0x07,
            0x02,
            0x03,
            VISCA_TERMINATOR, // Long frame
        ]);

        decoder.push(data);
        let frames: Vec<_> = decoder.drain_frames().collect();

        assert_eq!(frames.len(), 3);
        for frame in frames {
            assert_eq!(frame[frame.len() - 1], VISCA_TERMINATOR);
        }
    }

    #[test]
    fn test_clear() {
        let mut decoder = FrameDecoder::new(256);
        decoder.push(Bytes::from(vec![0x81, 0x01, 0x04]));

        assert_eq!(decoder.buffered_len(), 3);
        decoder.clear();
        assert_eq!(decoder.buffered_len(), 0);
        assert!(decoder.is_empty());
    }
}
