//! VISCA frame parsing utilities.
//!
//! This module handles VISCA message boundary detection and framing,
//! separating protocol parsing from raw IO operations.

use bytes::BytesMut;
use std::borrow::Cow;

use crate::command::const_encoding::VISCA_TERMINATOR;
use crate::Error;

/// VISCA frame parser that handles message boundary detection.
///
/// This parser accumulates bytes and extracts complete VISCA frames
/// (messages terminated with 0xFF).
#[derive(Debug, Default)]
pub struct FrameParser {
    /// Internal buffer for accumulating bytes
    buffer: BytesMut,
}

impl FrameParser {
    /// Create a new frame parser with default capacity.
    pub fn new() -> Self {
        Self::with_capacity(256)
    }

    /// Create a new frame parser with specified initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: BytesMut::with_capacity(capacity),
        }
    }

    /// Feed bytes into the parser.
    ///
    /// This appends the provided bytes to the internal buffer.
    pub fn feed(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Try to extract a complete VISCA frame from the buffer.
    ///
    /// Returns `Some(frame)` if a complete frame is available,
    /// or `None` if more data is needed.
    pub fn next_frame(&mut self) -> Option<BytesMut> {
        // Look for VISCA terminator (0xFF)
        if let Some(pos) = self.buffer.iter().position(|&b| b == VISCA_TERMINATOR) {
            // Include the terminator in the frame
            let frame_len = pos + 1;

            // Split off the frame from the buffer
            let frame = self.buffer.split_to(frame_len);

            Some(frame)
        } else {
            None
        }
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Get the current buffer length.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Clear the internal buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Reserve additional capacity in the buffer.
    pub fn reserve(&mut self, additional: usize) {
        self.buffer.reserve(additional);
    }
}

/// Validate that a byte slice is a valid VISCA frame.
///
/// A valid VISCA frame must:
/// - Be non-empty
/// - End with the VISCA terminator (0xFF)
/// - Not exceed maximum frame size (typically 16 bytes for commands)
pub fn validate_frame(data: &[u8]) -> Result<(), Error> {
    if data.is_empty() {
        return Err(Error::ParseError(Cow::Borrowed("Empty VISCA frame")));
    }

    if data[data.len() - 1] != VISCA_TERMINATOR {
        return Err(Error::ParseError(Cow::Borrowed(
            "VISCA frame missing terminator",
        )));
    }

    // VISCA frames are typically at most 16 bytes
    if data.len() > 32 {
        return Err(Error::ParseError(Cow::Owned(format!(
            "VISCA frame too large: {} bytes",
            data.len()
        ))));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn test_frame_extraction() {
        let mut parser = FrameParser::new();

        // Feed a complete frame
        parser.feed(&[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);

        let frame = parser.next_frame().expect("should extract frame");
        assert_eq!(&frame[..], &[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);

        // Buffer should be empty after extraction
        assert!(parser.is_empty());
    }

    #[test]
    fn test_partial_frame() {
        let mut parser = FrameParser::new();

        // Feed partial frame
        parser.feed(&[0x81, 0x01, 0x04]);
        assert!(parser.next_frame().is_none());

        // Complete the frame
        parser.feed(&[0x00, 0x02, 0xFF]);
        let frame = parser.next_frame().expect("should extract frame");
        assert_eq!(&frame[..], &[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);
    }

    #[test]
    fn test_multiple_frames() {
        let mut parser = FrameParser::new();

        // Feed two frames at once
        parser.feed(&[
            0x81, 0x01, 0x04, 0x00, 0x02, 0xFF, // Frame 1
            0x90, 0x41, 0xFF, // Frame 2
        ]);

        let frame1 = parser.next_frame().expect("should extract first frame");
        assert_eq!(&frame1[..], &[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);

        let frame2 = parser.next_frame().expect("should extract second frame");
        assert_eq!(&frame2[..], &[0x90, 0x41, 0xFF]);

        assert!(parser.is_empty());
    }

    #[test]
    fn test_frame_validation() {
        // Valid frame
        assert!(validate_frame(&[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]).is_ok());

        // Empty frame
        assert!(validate_frame(&[]).is_err());

        // Missing terminator
        assert!(validate_frame(&[0x81, 0x01, 0x04, 0x00, 0x02]).is_err());

        // Too large
        let mut large = vec![0x81; 40];
        large.push(0xFF);
        assert!(validate_frame(&large).is_err());
    }
}
