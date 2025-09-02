//! Async Serial transport for VISCA over RS-232/422.
//!
//! This module provides async serial communication for VISCA protocol,
//! supporting both RS-232 and RS-422 connections with proper
//! Address Set and I/F Clear initialization using tokio-serial.

use bytes::{Bytes, BytesMut};
use tokio::time::{timeout, Duration, Instant};
use tokio_serial::{SerialPortBuilderExt, SerialStream};
use tracing::{debug, trace, warn};

use std::future::Future;
use std::io::ErrorKind;

use crate::camera_id::CameraId;
use crate::command::bytes::VISCA_TERMINATOR;
use crate::command::encode_visca::ViscaEncode;
use crate::command::system::{AddressSetCommand, InterfaceClearCommand};
use crate::error::{Error, Result};
use crate::transport::{AsyncTransport, RetryConfig};

/// Async serial port configuration for VISCA communication.
#[derive(Debug, Clone)]
pub struct AsyncSerialConfig {
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

impl Default for AsyncSerialConfig {
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

/// Async serial transport implementation.
#[derive(Debug)]
pub struct AsyncSerialTransport {
    port: SerialStream,
    read_buffer: BytesMut,
    config: AsyncSerialConfig,
}

impl AsyncSerialTransport {
    /// Create a new async serial transport with the given configuration.
    pub async fn new(config: AsyncSerialConfig) -> Result<Self> {
        // Open serial port
        let mut port = tokio_serial::new(&config.port, config.baud_rate)
            .timeout(config.read_timeout)
            .open_native_async()
            .map_err(|e| {
                Error::TransportError(format!("Failed to open serial port: {}", e).into())
            })?;

        // Configure port settings
        #[cfg(unix)]
        port.set_exclusive(false).map_err(|e| {
            Error::TransportError(format!("Failed to set exclusive mode: {}", e).into())
        })?;

        let if_clear = config.if_clear_on_connect;
        let address_set = config.address_set_on_connect;

        let read_buffer = BytesMut::with_capacity(256);
        let mut transport = Self {
            port,
            read_buffer,
            config,
        };

        // Perform initialization if requested
        if if_clear {
            transport.send_if_clear().await?;
        }
        if address_set {
            transport.send_address_set().await?;
        }

        Ok(transport)
    }

    /// Send I/F Clear command to reset all devices on the bus.
    pub async fn send_if_clear(&mut self) -> Result<()> {
        debug!("Sending I/F Clear command");
        let cmd = InterfaceClearCommand::new();
        let mut buffer = [0u8; 16];
        // InterfaceClearCommand is const-constructed and guaranteed to encode
        let len = cmd
            .encode_into(CameraId::CAMERA_1, &mut buffer)
            .map_err(|e| {
                Error::TransportError(format!("Failed to encode IF Clear: {}", e).into())
            })?;
        self.send_raw(&buffer[..len]).await?;

        // Wait for I/F Clear to complete
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }

    /// Send Address Set command to assign addresses to devices.
    /// Returns the number of cameras detected.
    pub async fn send_address_set(&mut self) -> Result<u8> {
        let max_attempts = 3;

        for attempt in 0..max_attempts {
            debug!("Address Set attempt {}", attempt + 1);
            let cmd = AddressSetCommand::new();
            let mut buffer = [0u8; 16];
            // AddressSetCommand is const-constructed and guaranteed to encode
            let len = cmd
                .encode_into(CameraId::CAMERA_1, &mut buffer)
                .map_err(|e| {
                    Error::TransportError(format!("Failed to encode Address Set: {}", e).into())
                })?;
            self.send_raw(&buffer[..len]).await?;

            // Parse response properly
            match self.recv_address_set_response(Duration::from_secs(2)).await {
                Ok(camera_count) => {
                    debug!("Address Set successful, found {} cameras", camera_count);
                    return Ok(camera_count);
                }
                Err(Error::Timeout) if attempt < max_attempts - 1 => {
                    warn!("Address Set timeout, retrying...");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(Error::MaxRetriesExceeded)
    }

    /// Receive and parse Address Set response.
    async fn recv_address_set_response(&mut self, timeout_duration: Duration) -> Result<u8> {
        let mut camera_count = 0;
        let start = Instant::now();

        // Temporarily increase buffer space for address set responses
        let mut response_buffer = BytesMut::with_capacity(128);

        while start.elapsed() < timeout_duration {
            // Try to read some data with timeout
            match timeout(
                Duration::from_millis(50),
                self.read_into_buffer(&mut response_buffer),
            )
            .await
            {
                Ok(Ok(n)) if n > 0 => {
                    trace!(
                        "Address Set response: {:02X?}",
                        &response_buffer[..n.min(response_buffer.len())]
                    );

                    // Parse response bytes
                    let mut i = 0;
                    while i < response_buffer.len() {
                        // Look for address setting response: 88 30 0p FF where p is camera number
                        if i + 3 < response_buffer.len()
                            && response_buffer[i] == 0x88
                            && response_buffer[i + 1] == 0x30
                        {
                            if response_buffer[i + 2] >= 0x01
                                && response_buffer[i + 2] <= 0x07
                                && response_buffer[i + 3] == VISCA_TERMINATOR
                            {
                                // Camera address assignment
                                camera_count = response_buffer[i + 2];
                                debug!("Camera {} assigned address", camera_count);
                                i += 4;
                            } else if response_buffer[i + 2] == 0x02
                                && response_buffer[i + 3] == VISCA_TERMINATOR
                            {
                                // End of address setting
                                debug!("Address Set complete, {} cameras found", camera_count);
                                return Ok(camera_count);
                            } else {
                                i += 1;
                            }
                        } else {
                            i += 1;
                        }
                    }
                }
                Ok(Ok(_)) => {
                    // No data read, continue waiting
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Ok(Err(e)) if e.kind() == ErrorKind::TimedOut => {
                    // Timeout on this read, but total timeout not reached yet
                    continue;
                }
                Ok(Err(e)) => {
                    return Err(Error::TransportError(
                        format!("Error reading Address Set response: {}", e).into(),
                    ));
                }
                Err(_) => {
                    // Individual read timeout, continue if total timeout not reached
                    continue;
                }
            }
        }

        // Total timeout reached
        if camera_count > 0 {
            debug!(
                "Address Set timeout reached, but {} cameras were found",
                camera_count
            );
            Ok(camera_count)
        } else {
            debug!("Address Set timeout - no cameras found");
            Err(Error::Timeout)
        }
    }

    /// Helper method to read data into a buffer
    async fn read_into_buffer(&mut self, buffer: &mut BytesMut) -> std::io::Result<usize> {
        use tokio::io::AsyncReadExt;

        let mut temp_buf = vec![0u8; 64];
        match self.port.read(&mut temp_buf).await {
            Ok(n) => {
                buffer.extend_from_slice(&temp_buf[..n]);
                Ok(n)
            }
            Err(e) => Err(e),
        }
    }

    /// Send raw bytes to the serial port.
    async fn send_raw(&mut self, data: &[u8]) -> Result<()> {
        use tokio::io::AsyncWriteExt;

        timeout(self.config.write_timeout, self.port.write_all(data))
            .await
            .map_err(|_| Error::Timeout)?
            .map_err(|e| Error::TransportError(format!("Serial write error: {}", e).into()))?;

        self.port
            .flush()
            .await
            .map_err(|e| Error::TransportError(format!("Serial flush error: {}", e).into()))?;

        Ok(())
    }

    /// Receive a complete VISCA frame from the serial port.
    async fn recv_frame(&mut self) -> Result<Bytes> {
        use tokio::io::AsyncReadExt;

        let start_time = Instant::now();
        self.read_buffer.clear();

        loop {
            // Check for timeout
            if start_time.elapsed() > self.config.read_timeout {
                return Err(Error::Timeout);
            }

            // Try to read some bytes
            let mut temp_buf = [0u8; 64];
            match timeout(Duration::from_millis(50), self.port.read(&mut temp_buf)).await {
                Ok(Ok(n)) if n > 0 => {
                    self.read_buffer.extend_from_slice(&temp_buf[..n]);
                    trace!("Serial read {} bytes: {:02X?}", n, &temp_buf[..n]);

                    // Look for complete VISCA frame (ends with 0xFF)
                    if let Some(terminator_pos) =
                        self.read_buffer.iter().position(|&b| b == VISCA_TERMINATOR)
                    {
                        // Found complete frame
                        let frame_len = terminator_pos + 1;
                        let frame = self.read_buffer.split_to(frame_len).freeze();
                        debug!("Received complete VISCA frame: {:02X?}", frame);
                        return Ok(frame);
                    }

                    // Check for buffer overflow
                    if self.read_buffer.len() > 256 {
                        warn!("Serial receive buffer overflow, clearing buffer");
                        self.read_buffer.clear();
                    }
                }
                Ok(Ok(_)) => {
                    // No data read, continue waiting
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                Ok(Err(e)) if e.kind() == ErrorKind::TimedOut => {
                    // Read timeout, but total timeout not reached
                    continue;
                }
                Ok(Err(e)) => {
                    return Err(Error::TransportError(
                        format!("Serial read error: {}", e).into(),
                    ));
                }
                Err(_) => {
                    // Timeout on individual read, continue if total timeout not reached
                    continue;
                }
            }
        }
    }
}

impl AsyncTransport for AsyncSerialTransport {
    #[allow(clippy::manual_async_fn)]
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
        async move {
            // Pass through the bytes as-is (no address rewrite)
            debug!("Sending VISCA command to serial port: {:02X?}", bytes);
            self.send_raw(bytes).await
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn recv(&mut self) -> impl Future<Output = Result<Bytes, Error>> + Send {
        async move { self.recv_frame().await }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_async_serial_config_default() {
        let config = AsyncSerialConfig::default();
        assert_eq!(config.baud_rate, 9600);
        assert_eq!(config.camera_address, 1);
        assert!(config.if_clear_on_connect);
        assert!(!config.address_set_on_connect);
    }

    #[test]
    fn test_async_serial_config_custom() {
        let config = AsyncSerialConfig {
            port: "/dev/ttyUSB1".to_string(),
            baud_rate: 38400,
            camera_address: 3,
            if_clear_on_connect: false,
            address_set_on_connect: true,
            read_timeout: Duration::from_millis(200),
            write_timeout: Duration::from_millis(200),
            retry_config: RetryConfig::default(),
        };

        assert_eq!(config.port, "/dev/ttyUSB1");
        assert_eq!(config.baud_rate, 38400);
        assert_eq!(config.camera_address, 3);
        assert!(!config.if_clear_on_connect);
        assert!(config.address_set_on_connect);
    }
}
