//! Test that blocking Serial transport correctly handles back-to-back VISCA frames.
//!
//! This test addresses issue #357 where the blocking transports were dropping
//! frames when multiple VISCA frames arrived in a single serial read.

#![cfg(all(not(feature = "mode-async"), feature = "transport-serial"))]

use bytes::Bytes;
use grafton_visca::{command::CommandKind, transport::BlockingTransport, Error};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Mock serial port that returns pre-configured data
struct MockSerialPort {
    read_data: Arc<Mutex<Vec<u8>>>,
    write_data: Arc<Mutex<Vec<u8>>>,
    read_pos: Arc<Mutex<usize>>,
    timeout: Duration,
}

impl MockSerialPort {
    fn new(data: Vec<u8>) -> Self {
        Self {
            read_data: Arc::new(Mutex::new(data)),
            write_data: Arc::new(Mutex::new(Vec::new())),
            read_pos: Arc::new(Mutex::new(0)),
            timeout: Duration::from_millis(100),
        }
    }
}

impl Read for MockSerialPort {
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

impl Write for MockSerialPort {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut data = self.write_data.lock().unwrap();
        data.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl serialport::SerialPort for MockSerialPort {
    fn name(&self) -> Option<String> {
        Some("mock".to_string())
    }

    fn baud_rate(&self) -> serialport::Result<u32> {
        Ok(9600)
    }

    fn data_bits(&self) -> serialport::Result<serialport::DataBits> {
        Ok(serialport::DataBits::Eight)
    }

    fn flow_control(&self) -> serialport::Result<serialport::FlowControl> {
        Ok(serialport::FlowControl::None)
    }

    fn parity(&self) -> serialport::Result<serialport::Parity> {
        Ok(serialport::Parity::None)
    }

    fn stop_bits(&self) -> serialport::Result<serialport::StopBits> {
        Ok(serialport::StopBits::One)
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }

    fn set_baud_rate(&mut self, _baud_rate: u32) -> serialport::Result<()> {
        Ok(())
    }

    fn set_data_bits(&mut self, _data_bits: serialport::DataBits) -> serialport::Result<()> {
        Ok(())
    }

    fn set_flow_control(
        &mut self,
        _flow_control: serialport::FlowControl,
    ) -> serialport::Result<()> {
        Ok(())
    }

    fn set_parity(&mut self, _parity: serialport::Parity) -> serialport::Result<()> {
        Ok(())
    }

    fn set_stop_bits(&mut self, _stop_bits: serialport::StopBits) -> serialport::Result<()> {
        Ok(())
    }

    fn set_timeout(&mut self, timeout: Duration) -> serialport::Result<()> {
        self.timeout = timeout;
        Ok(())
    }

    fn write_request_to_send(&mut self, _level: bool) -> serialport::Result<()> {
        Ok(())
    }

    fn write_data_terminal_ready(&mut self, _level: bool) -> serialport::Result<()> {
        Ok(())
    }

    fn read_clear_to_send(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }

    fn read_data_set_ready(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }

    fn read_ring_indicator(&mut self) -> serialport::Result<bool> {
        Ok(false)
    }

    fn read_carrier_detect(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }

    fn bytes_to_read(&self) -> serialport::Result<u32> {
        let data = self.read_data.lock().unwrap();
        let pos = self.read_pos.lock().unwrap();
        Ok((data.len() - *pos) as u32)
    }

    fn bytes_to_write(&self) -> serialport::Result<u32> {
        let data = self.write_data.lock().unwrap();
        Ok(data.len() as u32)
    }

    fn clear(&self, _buffer_to_clear: serialport::ClearBuffer) -> serialport::Result<()> {
        Ok(())
    }

    fn try_clone(&self) -> serialport::Result<Box<dyn serialport::SerialPort>> {
        Ok(Box::new(MockSerialPort {
            read_data: Arc::clone(&self.read_data),
            write_data: Arc::clone(&self.write_data),
            read_pos: Arc::clone(&self.read_pos),
            timeout: self.timeout,
        }))
    }

    fn set_break(&self) -> serialport::Result<()> {
        Ok(())
    }

    fn clear_break(&self) -> serialport::Result<()> {
        Ok(())
    }
}

/// Custom Serial transport that uses our mock port
struct MockSerial {
    port: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    framer: grafton_visca::protocol::framer::ProtocolFramer,
}

impl MockSerial {
    fn new(port: Box<dyn serialport::SerialPort>) -> Self {
        Self {
            port: Arc::new(Mutex::new(port)),
            framer: grafton_visca::protocol::framer::ProtocolFramer::new_with_config(
                grafton_visca::transport::buffer::BufferConfig::default(),
            ),
        }
    }
}

impl BlockingTransport for MockSerial {
    fn send_with_kind(&mut self, data: &[u8], _kind: CommandKind) -> Result<(), Error> {
        let mut port = self
            .port
            .lock()
            .map_err(|_| Error::LockPoisoned("serial port mutex"))?;
        port.write_all(data)
            .map_err(|e| Error::TransportError(format!("Serial write error: {e}").into()))?;
        port.flush()
            .map_err(|e| Error::TransportError(format!("Serial flush error: {e}").into()))?;
        Ok(())
    }

    fn recv(&mut self) -> Result<Bytes, Error> {
        // First check if we have a buffered frame from a previous read
        if let Some(frame_result) = self.framer.drain_frames().next() {
            return frame_result;
        }

        let mut temp_buf = [0u8; 256];

        // Read more data until we get a complete frame
        loop {
            let mut port = self
                .port
                .lock()
                .map_err(|_| Error::LockPoisoned("serial port mutex"))?;

            let n = port.read(&mut temp_buf).map_err(Error::Io)?;
            drop(port); // Release lock immediately after reading

            if n == 0 {
                // Connection closed - try to extract any terminated frame
                if let Some(result) = self.framer.drain_on_eof() {
                    return result;
                }

                // No valid frame could be extracted
                if self.framer.is_empty() {
                    return Err(Error::ConnectionClosed {
                        reason: Some("serial port closed".into()),
                    });
                } else {
                    return Err(Error::ConnectionClosed {
                        reason: Some("serial port closed with partial frame".into()),
                    });
                }
            }

            // Push data to framer
            self.framer.push_slice(&temp_buf[..n])?;

            // Try to extract a complete frame
            if let Some(frame_result) = self.framer.drain_frames().next() {
                return frame_result;
            }
        }
    }

    fn recv_with_timeout(&mut self, _duration: Duration) -> Result<Bytes, Error> {
        // For testing, just use regular recv
        self.recv()
    }
}

#[test]
fn test_serial_back_to_back_visca_frames() {
    // Test data: ACK followed immediately by Completion
    // This simulates what happens when the camera sends both frames in one serial read
    let ack = [0x90, 0x41, 0xFF]; // ACK frame
    let completion = [0x90, 0x51, 0xFF]; // Completion frame

    let mut data = Vec::new();
    data.extend_from_slice(&ack);
    data.extend_from_slice(&completion);

    let port = Box::new(MockSerialPort::new(data));
    let mut transport = MockSerial::new(port);

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
fn test_serial_multiple_frames() {
    // Test with 3 frames arriving in one read
    let frame1 = [0x90, 0x41, 0xFF]; // ACK
    let frame2 = [0x90, 0x51, 0xFF]; // Completion
    let frame3 = [0x90, 0x60, 0xFF]; // Another response

    let mut data = Vec::new();
    data.extend_from_slice(&frame1);
    data.extend_from_slice(&frame2);
    data.extend_from_slice(&frame3);

    let port = Box::new(MockSerialPort::new(data));
    let mut transport = MockSerial::new(port);

    // All three frames should be available sequentially
    let recv1 = transport.recv().expect("Should receive frame 1");
    assert_eq!(recv1.as_ref(), &frame1[..]);

    let recv2 = transport.recv().expect("Should receive frame 2");
    assert_eq!(recv2.as_ref(), &frame2[..]);

    let recv3 = transport.recv().expect("Should receive frame 3");
    assert_eq!(recv3.as_ref(), &frame3[..]);
}

#[test]
fn test_serial_address_set_response() {
    // Test Address Set response handling with multiple device responses
    // Device responses: 0x88 0x30 <device_num> 0xFF
    let device1 = [0x88, 0x30, 0x01, 0xFF]; // Device 1
    let device2 = [0x88, 0x30, 0x02, 0xFF]; // Device 2 (end marker)

    let mut data = Vec::new();
    data.extend_from_slice(&device1);
    data.extend_from_slice(&device2);

    let port = Box::new(MockSerialPort::new(data));
    let mut transport = MockSerial::new(port);

    // Should receive both frames correctly
    let frame1 = transport.recv().expect("Should receive device 1 response");
    assert_eq!(frame1.as_ref(), &device1[..]);

    let frame2 = transport.recv().expect("Should receive device 2 response");
    assert_eq!(frame2.as_ref(), &device2[..]);
}

#[test]
fn test_serial_eof_handling() {
    // Test that EOF is handled correctly
    let frame = [0x90, 0x41, 0xFF];
    let port = Box::new(MockSerialPort::new(frame.to_vec()));
    let mut transport = MockSerial::new(port);

    // Should receive the frame successfully
    let recv = transport.recv().expect("Should receive frame before EOF");
    assert_eq!(recv.as_ref(), &frame[..]);

    // Next recv should get EOF
    let result = transport.recv();
    assert!(matches!(result, Err(Error::ConnectionClosed { .. })));
}
