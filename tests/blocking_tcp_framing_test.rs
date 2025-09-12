//! Test that blocking TCP transport correctly handles back-to-back VISCA frames.
//!
//! This test addresses issue #357 where the blocking transports were dropping
//! frames when multiple VISCA frames arrived in a single TCP read.

#![cfg(not(feature = "async"))]

use bytes::Bytes;
use grafton_visca::{command::CommandKind, transport::SyncTransport, Error};
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
    framer: grafton_visca::protocol::framer::ProtocolFramer,
    temp_buf: [u8; 256],
}

impl MockTcp {
    fn new(stream: MockTcpStream) -> Self {
        let reader_stream = stream.clone_for_reader();
        Self {
            reader: BufReader::new(reader_stream),
            writer: stream,
            framer: grafton_visca::protocol::framer::ProtocolFramer::new_with_config(
                grafton_visca::transport::buffer::BufferConfig::default(),
            ),
            temp_buf: [0u8; 256],
        }
    }
}

impl SyncTransport for MockTcp {
    fn send_with_kind(&mut self, data: &[u8], _kind: CommandKind) -> Result<(), Error> {
        self.writer.write_all(data)?;
        self.writer.flush()?;
        Ok(())
    }

    fn recv(&mut self) -> Result<Bytes, Error> {
        // First check if we have a buffered frame from a previous read
        if let Some(frame_result) = self.framer.drain_frames().next() {
            return frame_result;
        }

        // Read more data until we get a complete frame
        loop {
            let n = self.reader.read(&mut self.temp_buf).map_err(Error::Io)?;

            if n == 0 {
                // Connection closed - try to extract any terminated frame
                if let Some(result) = self.framer.drain_on_eof() {
                    return result;
                }

                // No valid frame could be extracted
                if self.framer.is_empty() {
                    return Err(Error::ConnectionClosed {
                        reason: Some("peer closed connection".into()),
                    });
                } else {
                    return Err(Error::ConnectionClosed {
                        reason: Some("connection closed with partial frame".into()),
                    });
                }
            }

            // Push data to framer
            self.framer.push_slice(&self.temp_buf[..n])?;

            // Try to extract a complete frame
            if let Some(frame_result) = self.framer.drain_frames().next() {
                return frame_result;
            }
        }
    }

    fn recv_with_timeout(&mut self, _duration: std::time::Duration) -> Result<Bytes, Error> {
        // For testing, just use regular recv
        self.recv()
    }
}

#[test]
fn test_back_to_back_visca_frames() {
    // Test data: ACK followed immediately by Completion
    // This simulates what happens when the camera sends both frames in one TCP packet
    let ack = vec![0x90, 0x41, 0xFF]; // ACK frame
    let completion = vec![0x90, 0x51, 0xFF]; // Completion frame

    let mut data = Vec::new();
    data.extend_from_slice(&ack);
    data.extend_from_slice(&completion);

    let stream = MockTcpStream::new(data);
    let mut transport = MockTcp::new(stream);

    // First recv should return the ACK
    let frame1 = transport.recv().expect("Should receive ACK frame");
    assert_eq!(frame1.as_ref(), &ack[..], "First frame should be ACK");

    // Second recv should return the Completion (from the buffer, no new read)
    let frame2 = transport.recv().expect("Should receive Completion frame");
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

    // All three frames should be available sequentially
    let recv1 = transport.recv().expect("Should receive frame 1");
    assert_eq!(recv1.as_ref(), &frame1[..]);

    let recv2 = transport.recv().expect("Should receive frame 2");
    assert_eq!(recv2.as_ref(), &frame2[..]);

    let recv3 = transport.recv().expect("Should receive frame 3");
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

    // First recv should return complete Sony frame 1 (13 bytes)
    let frame1 = transport.recv().expect("Should receive Sony frame 1");
    assert_eq!(frame1.len(), 13, "Sony frame 1 should be 13 bytes");
    assert_eq!(frame1.as_ref(), &sony1[..]);

    // Second recv should return complete Sony frame 2 (11 bytes)
    let frame2 = transport.recv().expect("Should receive Sony frame 2");
    assert_eq!(frame2.len(), 11, "Sony frame 2 should be 11 bytes");
    assert_eq!(frame2.as_ref(), &sony2[..]);
}

#[test]
fn test_eof_with_complete_frame() {
    // Test that a complete frame followed by EOF is handled correctly
    let frame = [0x90, 0x41, 0xFF];
    let stream = MockTcpStream::new(frame.to_vec());
    let mut transport = MockTcp::new(stream);

    // Should receive the frame successfully
    let recv = transport.recv().expect("Should receive frame before EOF");
    assert_eq!(recv.as_ref(), &frame[..]);

    // Next recv should get EOF
    let result = transport.recv();
    assert!(matches!(result, Err(Error::ConnectionClosed { .. })));
}

#[test]
fn test_eof_with_partial_frame() {
    // Test that EOF with partial frame is reported correctly
    let partial = vec![0x90, 0x41]; // Missing terminator
    let stream = MockTcpStream::new(partial);
    let mut transport = MockTcp::new(stream);

    // Should get connection closed with partial frame error
    let result = transport.recv();
    match result {
        Err(Error::ConnectionClosed { reason }) => {
            assert!(reason.is_some());
            let reason_str = reason.unwrap();
            assert!(reason_str.contains("partial") || reason_str.contains("closed"));
        }
        _ => panic!("Expected ConnectionClosed error with partial frame"),
    }
}
