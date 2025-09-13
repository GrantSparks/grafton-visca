//! Serial transport for VISCA over RS-232/422.
//!
//! This module provides serial communication for VISCA protocol,
//! supporting both RS-232 and RS-422 connections with proper
//! Address Set and I/F Clear initialization.

use bytes::Bytes;
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tracing::{debug, trace};

use crate::{
    command::CommandKind,
    error::{Error, Result},
    protocol::framer::ProtocolFramer,
    transport::{
        serial::{
            handshake::blocking_handshake::{address_set_blocking, if_clear_blocking},
            Config as SerialConfig,
        },
        SyncTransport,
    },
};

// SerialConfig is now imported from the unified serial::Config

/// Serial transport implementation for blocking I/O.
#[derive(Debug)]
pub struct SerialTransport {
    port: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    config: SerialConfig,
    framer: ProtocolFramer,
}

impl SerialTransport {
    /// Create a new serial transport with the given configuration.
    pub fn new(config: SerialConfig) -> Result<Self> {
        // Open serial port
        let port = serialport::new(&config.port, config.baud_rate)
            .timeout(config.read_timeout)
            .open()
            .map_err(|e| {
                Error::TransportError(format!("Failed to open serial port: {e}").into())
            })?;

        let if_clear = config.if_clear_on_connect;
        let address_set = config.address_set_on_connect;

        let transport = Self {
            port: Arc::new(Mutex::new(port)),
            framer: ProtocolFramer::new_with_config(config.buffer_config),
            config,
        };

        // Perform initialization if requested
        if if_clear {
            let mut port = transport
                .port
                .lock()
                .map_err(|_| Error::LockPoisoned("serial port mutex"))?;
            if_clear_blocking(&mut **port)?;
        }
        if address_set {
            let mut port = transport
                .port
                .lock()
                .map_err(|_| Error::LockPoisoned("serial port mutex"))?;
            address_set_blocking(&mut **port, Duration::from_secs(2))?;
        }

        Ok(transport)
    }

    /// Send raw bytes to the serial port.
    fn send_raw(&self, bytes: &[u8]) -> Result<()> {
        let mut port = self
            .port
            .lock()
            .map_err(|_| Error::LockPoisoned("serial port mutex"))?;
        port.write_all(bytes)
            .map_err(|e| Error::TransportError(format!("Serial write error: {e}").into()))?;
        port.flush()
            .map_err(|e| Error::TransportError(format!("Serial flush error: {e}").into()))?;
        trace!(
            "Sent {len} bytes: {bytes:02X?}",
            len = bytes.len(),
            bytes = bytes
        );
        Ok(())
    }

    /// Receive a complete VISCA frame from the serial port.
    fn recv_frame(&mut self) -> Result<Bytes> {
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
                trace!("Received frame: {:?}", frame_result);
                return frame_result;
            }
        }
    }
}

// SerialTransport keeps using &self because it has interior mutability
// This is necessary for hardware constraints
impl SyncTransport for SerialTransport {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<()> {
        // Pass through the bytes as-is (no address rewrite or building)
        let cmd = bytes.to_vec();

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

                    debug!("Retrying serial send (attempt {attempts}): {e:?}");
                    std::thread::sleep(delay);
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn recv(&mut self) -> Result<Bytes> {
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

                    debug!("Retrying serial receive (attempt {attempts}): {e:?}");
                    std::thread::sleep(delay);
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn recv_with_timeout(&mut self, timeout: Duration) -> Result<Bytes> {
        // First check if we have a buffered frame from a previous read
        if let Some(frame_result) = self.framer.drain_frames().next() {
            return frame_result;
        }

        // Temporarily set the timeout on the port
        let mut port = self
            .port
            .lock()
            .map_err(|_| Error::LockPoisoned("serial port mutex"))?;
        let original_timeout = port.timeout();
        port.set_timeout(timeout)
            .map_err(|e| Error::TransportError(format!("Failed to set timeout: {e}").into()))?;
        drop(port);

        let mut temp_buf = [0u8; 256];

        // Read more data until we get a complete frame or timeout
        let result = loop {
            let mut port = self
                .port
                .lock()
                .map_err(|_| Error::LockPoisoned("serial port mutex"))?;

            match port.read(&mut temp_buf) {
                Ok(0) => {
                    drop(port);
                    // Connection closed - try to extract any terminated frame
                    if let Some(result) = self.framer.drain_on_eof() {
                        break result;
                    }

                    // No valid frame could be extracted
                    if self.framer.is_empty() {
                        break Err(Error::ConnectionClosed {
                            reason: Some("serial port closed".into()),
                        });
                    } else {
                        break Err(Error::ConnectionClosed {
                            reason: Some("serial port closed with partial frame".into()),
                        });
                    }
                }
                Ok(n) => {
                    drop(port);
                    // Push data to framer
                    if let Err(e) = self.framer.push_slice(&temp_buf[..n]) {
                        break Err(e);
                    }

                    // Try to extract a complete frame
                    if let Some(frame_result) = self.framer.drain_frames().next() {
                        break frame_result;
                    }
                    // Continue looping to read more data
                }
                Err(io_err)
                    if io_err.kind() == std::io::ErrorKind::TimedOut
                        || io_err.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    drop(port);
                    // Timeout occurred - check if we have a buffered frame
                    if let Some(frame_result) = self.framer.drain_frames().next() {
                        break frame_result;
                    }
                    break Err(Error::Timeout);
                }
                Err(io_err) => {
                    drop(port);
                    break Err(Error::Io(io_err));
                }
            }
        };

        // Restore original timeout
        let mut port = self
            .port
            .lock()
            .map_err(|_| Error::LockPoisoned("serial port mutex"))?;
        port.set_timeout(original_timeout)
            .map_err(|e| Error::TransportError(format!("Failed to restore timeout: {e}").into()))?;

        result
    }
}

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
}
