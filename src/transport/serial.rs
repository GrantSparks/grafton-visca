//! Serial transport for VISCA over RS-232/422.
//!
//! This module provides serial communication for VISCA protocol,
//! supporting both RS-232 and RS-422 connections with proper
//! Address Set and I/F Clear initialization.

use bytes::{Bytes, BytesMut};
use log::{debug, trace, warn};

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::error::{Error, Result};
use crate::protocol::encode::{FrameBuilder, VISCA_TERMINATOR};
use crate::transport::{BlockingTransport, RetryConfig};

/// Serial port configuration for VISCA communication.
#[derive(Debug, Clone)]
pub struct SerialConfig {
    /// Serial port path (e.g., "/dev/ttyUSB0" on Unix, "COM1" on Windows).
    pub port: String,
    /// Baud rate (typically 9600 or 38400 for VISCA).
    pub baud_rate: u32,
    /// Camera address (1-7 for RS-232, 1-112 for RS-422).
    pub camera_address: u8,
    /// Whether to perform I/F Clear on connect.
    pub if_clear_on_connect: bool,
    /// Whether to perform Address Set on connect.
    pub address_set_on_connect: bool,
    /// Read timeout for serial operations.
    pub read_timeout: Duration,
    /// Write timeout for serial operations.
    pub write_timeout: Duration,
    /// Retry configuration for serial operations.
    pub retry_config: RetryConfig,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            port: "/dev/ttyUSB0".to_string(),
            baud_rate: 9600,
            camera_address: 1,
            if_clear_on_connect: true,
            address_set_on_connect: false,
            read_timeout: Duration::from_millis(100),
            write_timeout: Duration::from_millis(100),
            retry_config: RetryConfig::default(),
        }
    }
}

/// Serial transport implementation for blocking I/O.
#[derive(Debug)]
pub struct SerialTransport {
    port: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    camera_address: u8,
    read_buffer: Arc<Mutex<BytesMut>>,
    config: SerialConfig,
}

impl SerialTransport {
    /// Create a new serial transport with the given configuration.
    pub fn new(config: SerialConfig) -> Result<Self> {
        // Open serial port
        let port = serialport::new(&config.port, config.baud_rate)
            .timeout(config.read_timeout)
            .open()
            .map_err(|e| {
                Error::TransportError(format!("Failed to open serial port: {}", e).into())
            })?;

        let camera_address = config.camera_address;
        let if_clear = config.if_clear_on_connect;
        let address_set = config.address_set_on_connect;

        let transport = Self {
            port: Arc::new(Mutex::new(port)),
            camera_address,
            read_buffer: Arc::new(Mutex::new(BytesMut::with_capacity(256))),
            config,
        };

        // Perform initialization if requested
        if if_clear {
            transport.send_if_clear()?;
        }
        if address_set {
            transport.send_address_set()?;
        }

        Ok(transport)
    }

    /// Send I/F Clear command to reset all devices on the bus.
    pub fn send_if_clear(&self) -> Result<()> {
        debug!("Sending I/F Clear command");
        let cmd = crate::protocol::encode::encode_if_clear();
        self.send_raw(&cmd)?;
        // Wait for I/F Clear to complete
        std::thread::sleep(Duration::from_millis(100));
        Ok(())
    }

    /// Send Address Set command to assign addresses to devices.
    /// Returns the number of cameras detected.
    pub fn send_address_set(&self) -> Result<u8> {
        let max_attempts = 3;

        for attempt in 0..max_attempts {
            debug!("Address Set attempt {}", attempt + 1);
            let cmd = crate::protocol::encode::encode_address_set();
            self.send_raw(&cmd)?;

            // Parse response properly
            match self.recv_address_set_response(Duration::from_secs(2)) {
                Ok(camera_count) => {
                    debug!("Address Set successful, found {} cameras", camera_count);
                    return Ok(camera_count);
                }
                Err(Error::Timeout) if attempt < max_attempts - 1 => {
                    warn!("Address Set timeout, retrying...");
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(Error::MaxRetriesExceeded)
    }

    /// Receive and parse Address Set response.
    fn recv_address_set_response(&self, timeout: Duration) -> Result<u8> {
        let mut buffer = [0u8; 64];
        let mut port = self
            .port
            .lock()
            .map_err(|_| Error::LockPoisoned("serial port mutex"))?;

        // Temporarily set timeout
        let original_timeout = port.timeout();
        port.set_timeout(timeout)
            .map_err(|e| Error::TransportError(format!("Failed to set timeout: {}", e).into()))?;

        let mut camera_count = 0;
        let start = Instant::now();

        // Read all device responses
        while start.elapsed() < timeout {
            match port.read(&mut buffer) {
                Ok(n) if n > 0 => {
                    trace!("Address Set response: {:02X?}", &buffer[..n]);

                    // Parse response bytes
                    let mut i = 0;
                    while i < n {
                        // Device response format: 0x88 0x30 <device_num> 0xFF
                        if i + 3 < n && buffer[i] == 0x88 && buffer[i + 1] == 0x30 {
                            if buffer[i + 2] == 0x02 && buffer[i + 3] == VISCA_TERMINATOR {
                                // End of address setting
                                debug!("Address Set complete, {} cameras found", camera_count);

                                // Restore timeout
                                port.set_timeout(original_timeout).ok();
                                return Ok(camera_count);
                            } else if buffer[i + 3] == VISCA_TERMINATOR {
                                // Device response
                                camera_count += 1;
                                trace!("Camera {} responded", camera_count);
                            }
                            i += 4;
                        } else {
                            i += 1;
                        }
                    }
                }
                Ok(_) => {
                    // No data, continue waiting
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    // Timeout - no more devices
                    debug!("Address Set timeout - {} cameras found", camera_count);

                    // Restore timeout
                    port.set_timeout(original_timeout).ok();

                    if camera_count > 0 {
                        return Ok(camera_count);
                    } else {
                        return Err(Error::Timeout);
                    }
                }
                Err(e) => {
                    // Restore timeout
                    port.set_timeout(original_timeout).ok();
                    return Err(Error::TransportError(
                        format!("Error reading Address Set response: {}", e).into(),
                    ));
                }
            }
        }

        // Restore timeout
        port.set_timeout(original_timeout).ok();

        if camera_count > 0 {
            Ok(camera_count)
        } else {
            Err(Error::Timeout)
        }
    }

    /// Send raw bytes to the serial port.
    fn send_raw(&self, bytes: &[u8]) -> Result<()> {
        let mut port = self
            .port
            .lock()
            .map_err(|_| Error::LockPoisoned("serial port mutex"))?;
        port.write_all(bytes)
            .map_err(|e| Error::TransportError(format!("Serial write error: {}", e).into()))?;
        port.flush()
            .map_err(|e| Error::TransportError(format!("Serial flush error: {}", e).into()))?;
        trace!("Sent {} bytes: {:02X?}", bytes.len(), bytes);
        Ok(())
    }

    /// Receive a complete VISCA frame from the serial port.
    fn recv_frame(&self) -> Result<Bytes> {
        let mut buffer = self
            .read_buffer
            .lock()
            .map_err(|_| Error::LockPoisoned("serial port mutex"))?;
        let mut port = self
            .port
            .lock()
            .map_err(|_| Error::LockPoisoned("serial port mutex"))?;
        let mut temp_buf = [0u8; 256];

        loop {
            // Check if we have a complete frame in the buffer
            if let Some(pos) = buffer.iter().position(|&b| b == VISCA_TERMINATOR) {
                let frame = buffer.split_to(pos + 1);
                trace!("Received frame: {:02X?}", frame);
                return Ok(frame.freeze());
            }

            // Read more data
            match port.read(&mut temp_buf) {
                Ok(n) if n > 0 => {
                    buffer.extend_from_slice(&temp_buf[..n]);
                    trace!("Read {} bytes from serial", n);
                }
                Ok(_) => {
                    return Err(Error::Timeout);
                }
                Err(e) => {
                    return Err(Error::TransportError(
                        format!("Serial read error: {}", e).into(),
                    ));
                }
            }
        }
    }

    /// Create a command builder with the camera address.
    fn build_command(&self, bytes: &[u8]) -> Vec<u8> {
        FrameBuilder::new()
            .device(self.camera_address)
            .bytes(bytes)
            .build()
    }
}

// SerialTransport keeps using &self because it has interior mutability
// This is necessary for hardware constraints
impl BlockingTransport for SerialTransport {
    fn send_blocking(&mut self, bytes: &[u8]) -> Result<()> {
        // Add camera address and terminator if not already present
        let cmd = if bytes[0] & 0xF0 == 0x80 {
            // Already has address
            bytes.to_vec()
        } else {
            // Build command with address
            self.build_command(bytes)
        };

        // Send with retry logic
        let mut attempts = 0;
        let start_time = Instant::now();

        loop {
            match self.send_raw(&cmd) {
                Ok(()) => return Ok(()),
                Err(e)
                    if e.is_retryable()
                        && self.config.retry_config.should_retry(attempts, start_time) =>
                {
                    attempts += 1;
                    let delay = self
                        .config
                        .retry_config
                        .calculate_delay(attempts, e.suggested_retry_delay());

                    if start_time.elapsed() + delay > self.config.retry_config.max_retry_duration {
                        return Err(Error::MaxRetriesExceeded);
                    }

                    debug!("Retrying serial send (attempt {}): {:?}", attempts, e);
                    std::thread::sleep(delay);
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn recv_blocking(&mut self) -> Result<Bytes> {
        let mut attempts = 0;
        let start_time = Instant::now();

        loop {
            match self.recv_frame() {
                Ok(frame) => return Ok(frame),
                Err(e)
                    if e.is_retryable()
                        && self.config.retry_config.should_retry(attempts, start_time) =>
                {
                    attempts += 1;
                    let delay = self
                        .config
                        .retry_config
                        .calculate_delay(attempts, e.suggested_retry_delay());

                    if start_time.elapsed() + delay > self.config.retry_config.max_retry_duration {
                        return Err(Error::MaxRetriesExceeded);
                    }

                    debug!("Retrying serial receive (attempt {}): {:?}", attempts, e);
                    std::thread::sleep(delay);
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn recv_blocking_with_timeout(&mut self, timeout: Duration) -> Result<Bytes> {
        // Temporarily set the timeout on the port
        let mut port = self
            .port
            .lock()
            .map_err(|_| Error::LockPoisoned("serial port mutex"))?;
        let original_timeout = port.timeout();
        port.set_timeout(timeout)
            .map_err(|e| Error::TransportError(format!("Failed to set timeout: {}", e).into()))?;
        drop(port);

        let result = self.recv_frame();

        // Restore original timeout
        let mut port = self
            .port
            .lock()
            .map_err(|_| Error::LockPoisoned("serial port mutex"))?;
        port.set_timeout(original_timeout).map_err(|e| {
            Error::TransportError(format!("Failed to restore timeout: {}", e).into())
        })?;

        result
    }
}

// Note: Async serial transport implementation removed.
// If async serial support is needed in the future, consider using tokio-serial
// or async-std-serial crates with proper native async implementations.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serial_config_default() {
        let config = SerialConfig::default();
        assert_eq!(config.baud_rate, 9600);
        assert_eq!(config.camera_address, 1);
        assert!(config.if_clear_on_connect);
        assert!(!config.address_set_on_connect);
    }

    #[test]
    fn test_command_builder_integration() {
        let transport = SerialTransport {
            port: Arc::new(Mutex::new(Box::new(MockSerialPort::new()))),
            camera_address: 1,
            read_buffer: Arc::new(Mutex::new(BytesMut::new())),
            config: SerialConfig::default(),
        };

        let cmd = transport.build_command(&[0x01, 0x04, 0x00, 0x02]);
        assert_eq!(cmd, vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]);
    }

    /// Mock serial port for testing.
    struct MockSerialPort {
        read_data: Vec<u8>,
        write_data: Vec<u8>,
    }

    impl MockSerialPort {
        fn new() -> Self {
            Self {
                read_data: vec![],
                write_data: vec![],
            }
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
            Duration::from_millis(100)
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

        fn set_timeout(&mut self, _timeout: Duration) -> serialport::Result<()> {
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
            Ok(self.read_data.len() as u32)
        }

        fn bytes_to_write(&self) -> serialport::Result<u32> {
            Ok(self.write_data.len() as u32)
        }

        fn clear(&self, _buffer_to_clear: serialport::ClearBuffer) -> serialport::Result<()> {
            Ok(())
        }

        fn try_clone(&self) -> serialport::Result<Box<dyn serialport::SerialPort>> {
            Ok(Box::new(MockSerialPort::new()))
        }

        fn set_break(&self) -> serialport::Result<()> {
            Ok(())
        }

        fn clear_break(&self) -> serialport::Result<()> {
            Ok(())
        }
    }

    impl Read for MockSerialPort {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let len = std::cmp::min(buf.len(), self.read_data.len());
            buf[..len].copy_from_slice(&self.read_data[..len]);
            self.read_data.drain(..len);
            Ok(len)
        }
    }

    impl Write for MockSerialPort {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.write_data.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}
