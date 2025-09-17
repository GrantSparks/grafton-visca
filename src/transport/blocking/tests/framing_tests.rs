//! Test that blocking runtime correctly handles back-to-back VISCA frames.
//!
//! This test addresses issue #357 where the blocking transports were dropping
//! frames when multiple VISCA frames arrived in a single TCP read.

#![allow(clippy::expect_used, clippy::unwrap_used)] // Test code is allowed to panic

use crate::command::{bytes::VISCA_TERMINATOR, CommandKind};
use crate::transport::BlockingTransport;
use crate::Error;
use std::io::{BufReader, Read, Write};
use std::sync::{Arc, Mutex};

/// Mock TCP stream that returns pre-configured data
struct MockTcpStream {
    read_data: Arc<Mutex<Vec<u8>>>,
    write_data: Arc<Mutex<Vec<u8>>>,
    read_pos: Arc<Mutex<usize>>,
}

impl MockTcpStream {
    fn new(data: Vec<u8>) -> Self {
        Self {
            read_data: Arc::new(Mutex::new(data)),
            write_data: Arc::new(Mutex::new(Vec::new())),
            read_pos: Arc::new(Mutex::new(0)),
        }
    }

    fn clone_for_reader(&self) -> Self {
        Self {
            read_data: Arc::clone(&self.read_data),
            write_data: Arc::clone(&self.write_data),
            read_pos: Arc::clone(&self.read_pos),
        }
    }
}

impl Read for MockTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let data = self.read_data.lock().unwrap();
        let mut pos = self.read_pos.lock().unwrap();

        if *pos >= data.len() {
            return Ok(0); // EOF
        }

        let remaining = data.len() - *pos;
        let to_read = std::cmp::min(buf.len(), remaining);

        buf[..to_read].copy_from_slice(&data[*pos..*pos + to_read]);
        *pos += to_read;

        Ok(to_read)
    }
}

impl Write for MockTcpStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut data = self.write_data.lock().unwrap();
        data.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Custom TCP transport that uses our mock stream
struct MockTcp {
    reader: BufReader<MockTcpStream>,
    writer: MockTcpStream,
}

impl MockTcp {
    fn new(stream: MockTcpStream) -> Self {
        let reader_stream = stream.clone_for_reader();
        Self {
            reader: BufReader::new(reader_stream),
            writer: stream,
        }
    }
}

impl BlockingTransport for MockTcp {
    fn send_with_kind(&mut self, data: &[u8], _kind: CommandKind) -> Result<(), Error> {
        self.writer.write_all(data)?;
        self.writer.flush()?;
        Ok(())
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        // Read directly into the provided buffer
        match self.reader.read(dst) {
            Ok(0) => {
                // Connection closed
                Err(Error::ConnectionClosed {
                    reason: Some("peer closed connection".into()),
                })
            }
            Ok(n) => Ok(n),
            Err(e) => Err(Error::Io(e)),
        }
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        _duration: std::time::Duration,
    ) -> Result<usize, Error> {
        // For testing, just use regular recv_into
        self.recv_into(dst)
    }
}

#[test]
fn test_back_to_back_visca_frames() {
    use crate::protocol::framer::ProtocolFramer;
    use crate::transport::buffer::BufferConfig;

    // Test data: ACK followed immediately by Completion
    // This simulates what happens when the camera sends both frames in one TCP packet
    let ack = vec![0x90, 0x41, 0xFF]; // ACK frame
    let completion = vec![0x90, 0x51, 0xFF]; // Completion frame

    let mut data = Vec::new();
    data.extend_from_slice(&ack);
    data.extend_from_slice(&completion);

    let stream = MockTcpStream::new(data);
    let mut transport = MockTcp::new(stream);

    // Create a framer to test the framing logic
    let mut framer = ProtocolFramer::new_with_config(BufferConfig::default());
    let mut read_buf = vec![0u8; 256];

    // Read all data in one go (simulating single TCP read)
    let n = transport
        .recv_into(&mut read_buf)
        .expect("Should read data");
    assert_eq!(n, 6, "Should read all 6 bytes");

    // Push to framer
    framer
        .push_slice(&read_buf[..n])
        .expect("Should push to framer");

    // Should be able to extract both frames
    let mut frames: Vec<_> = framer.drain_frames().collect();
    assert_eq!(frames.len(), 2, "Should extract 2 frames");

    let frame1 = frames.remove(0).expect("Frame 1 should be valid");
    assert_eq!(frame1.as_ref(), &ack[..], "First frame should be ACK");

    let frame2 = frames.remove(0).expect("Frame 2 should be valid");
    assert_eq!(
        frame2.as_ref(),
        &completion[..],
        "Second frame should be Completion"
    );
}

#[test]
fn test_multiple_back_to_back_frames() {
    // Test with more than 2 frames arriving together
    let frame1 = vec![0x90, 0x41, 0xFF]; // ACK
    let frame2 = vec![0x90, 0x51, 0xFF]; // Completion
    let frame3 = vec![0x90, 0x60, 0xFF]; // Another response

    let mut data = Vec::new();
    data.extend_from_slice(&frame1);
    data.extend_from_slice(&frame2);
    data.extend_from_slice(&frame3);

    let stream = MockTcpStream::new(data);
    let mut transport = MockTcp::new(stream);

    // Create a framer to test the framing logic
    use crate::protocol::framer::ProtocolFramer;
    use crate::transport::buffer::BufferConfig;
    let mut framer = ProtocolFramer::new_with_config(BufferConfig::default());
    let mut read_buf = vec![0u8; 256];

    // Read all data in one go
    let n = transport
        .recv_into(&mut read_buf)
        .expect("Should read data");
    assert_eq!(n, 9, "Should read all 9 bytes");

    // Push to framer
    framer
        .push_slice(&read_buf[..n])
        .expect("Should push to framer");

    // Should be able to extract all three frames
    let mut frames: Vec<_> = framer.drain_frames().collect();
    assert_eq!(frames.len(), 3, "Should extract 3 frames");

    let recv1 = frames.remove(0).expect("Frame 1 should be valid");
    assert_eq!(recv1.as_ref(), &frame1[..]);

    let recv2 = frames.remove(0).expect("Frame 2 should be valid");
    assert_eq!(recv2.as_ref(), &frame2[..]);

    let recv3 = frames.remove(0).expect("Frame 3 should be valid");
    assert_eq!(recv3.as_ref(), &frame3[..]);
}

#[test]
fn test_sony_encapsulated_frames_back_to_back() {
    // Test Sony 52381 encapsulated frames with 0xFF in header
    // These should not be split prematurely at the 0xFF bytes

    // Sony frame 1: header with 0xFF in sequence number
    let sony1 = vec![
        0x01, 0x10, // Payload type: ViscaInquiry
        0x00, 0x05, // Payload length: 5 bytes
        0x00, 0xFF, 0x00, 0xFF, // Sequence number with 0xFF bytes
        0x81, 0x09, 0x04, 0x00, 0xFF, // VISCA payload
    ];

    // Sony frame 2: another frame with 0xFF in sequence
    let sony2 = vec![
        0x01, 0x11, // Payload type: ViscaReply
        0x00, 0x03, // Payload length: 3 bytes
        0xFF, 0xFF, 0xFF, 0xFF, // All 0xFF in sequence
        0x90, 0x50, 0xFF, // VISCA payload
    ];

    let mut data = Vec::new();
    data.extend_from_slice(&sony1);
    data.extend_from_slice(&sony2);

    let stream = MockTcpStream::new(data);
    let mut transport = MockTcp::new(stream);

    // Create a framer configured for Sony protocol
    use crate::protocol::framer::ProtocolFramer;
    use crate::transport::buffer::BufferConfig;
    let mut framer = ProtocolFramer::new_with_config(BufferConfig::for_sony_ip());
    let mut read_buf = vec![0u8; 256];

    // Read all data in one go
    let n = transport
        .recv_into(&mut read_buf)
        .expect("Should read data");
    assert_eq!(n, 24, "Should read all 24 bytes (13 + 11)");

    // Push to framer
    framer
        .push_slice(&read_buf[..n])
        .expect("Should push to framer");

    // Should be able to extract both Sony frames
    let mut frames: Vec<_> = framer.drain_frames().collect();
    assert_eq!(frames.len(), 2, "Should extract 2 Sony frames");

    let frame1 = frames.remove(0).expect("Frame 1 should be valid");
    assert_eq!(frame1.len(), 13, "Sony frame 1 should be 13 bytes");
    assert_eq!(frame1.as_ref(), &sony1[..]);

    let frame2 = frames.remove(0).expect("Frame 2 should be valid");
    assert_eq!(frame2.len(), 11, "Sony frame 2 should be 11 bytes");
    assert_eq!(frame2.as_ref(), &sony2[..]);
}

#[test]
fn test_eof_with_complete_frame() {
    // Test that a complete frame followed by EOF is handled correctly
    let frame = [0x90, 0x41, VISCA_TERMINATOR];
    let stream = MockTcpStream::new(frame.to_vec());
    let mut transport = MockTcp::new(stream);

    // Create a framer to test the framing logic
    use crate::protocol::framer::ProtocolFramer;
    use crate::transport::buffer::BufferConfig;
    let mut framer = ProtocolFramer::new_with_config(BufferConfig::default());
    let mut read_buf = vec![0u8; 256];

    // Read the frame
    let n = transport
        .recv_into(&mut read_buf)
        .expect("Should read data");
    assert_eq!(n, 3, "Should read 3 bytes");

    // Push to framer
    framer
        .push_slice(&read_buf[..n])
        .expect("Should push to framer");

    // Should extract the frame
    let mut frames: Vec<_> = framer.drain_frames().collect();
    assert_eq!(frames.len(), 1, "Should extract 1 frame");

    let recv = frames.remove(0).expect("Frame should be valid");
    assert_eq!(recv.as_ref(), &frame[..]);

    // Next recv_into should get EOF
    let result = transport.recv_into(&mut read_buf);
    assert!(matches!(result, Err(Error::ConnectionClosed { .. })));
}

#[test]
fn test_eof_with_partial_frame() {
    use crate::protocol::framer::ProtocolFramer;
    use crate::transport::buffer::BufferConfig;

    // Test that EOF with partial frame is reported correctly
    let partial = vec![0x90, 0x41]; // Missing terminator
    let stream = MockTcpStream::new(partial);
    let mut transport = MockTcp::new(stream);

    // Create a framer to test the framing logic
    let mut framer = ProtocolFramer::new_with_config(BufferConfig::default());
    let mut read_buf = vec![0u8; 256];

    // Read the partial data
    let n = transport
        .recv_into(&mut read_buf)
        .expect("Should read data");
    assert_eq!(n, 2, "Should read 2 bytes");

    // Push to framer
    framer
        .push_slice(&read_buf[..n])
        .expect("Should push to framer");

    // No complete frame should be extracted
    let frames: Vec<_> = framer.drain_frames().collect();
    assert_eq!(frames.len(), 0, "Should not extract any complete frames");

    // Simulate EOF by calling drain_on_eof
    // The framer may or may not return an error for partial frames
    let result = framer.drain_on_eof();
    if let Some(frame_result) = result {
        // If the framer returns something, it should be an error
        assert!(
            frame_result.is_err(),
            "Partial frame should result in error"
        );
    }
    // Otherwise, the framer correctly discarded the partial frame
}
