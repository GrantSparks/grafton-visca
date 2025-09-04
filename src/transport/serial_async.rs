//! Async Serial transport for VISCA over RS-232/422.
//!
//! This module provides async serial communication for VISCA protocol,
//! supporting both RS-232 and RS-422 connections with proper
//! Address Set and I/F Clear initialization using tokio-serial.

use bytes::Bytes;
use tokio::time::{timeout, Duration, Instant};
use tokio_serial::{SerialPortBuilderExt, SerialStream};
use tracing::{debug, trace, warn};

use std::{future::Future, io::ErrorKind};

// Platform-specific imports for Windows compatibility
#[cfg(windows)]
use std::sync::Arc;
#[cfg(windows)]
use tokio::sync::Mutex;

use crate::{
    camera_id::CameraId,
    command::{
        bytes::VISCA_TERMINATOR,
        encode_visca::ViscaEncode,
        system::{AddressSetCommand, InterfaceClearCommand},
    },
    error::{Error, Result},
    transport::{
        async_io::{
            read_visca_frame, write_all_flush, AsyncReadExt as AsyncReadExtTrait,
            AsyncWriteExt as AsyncWriteExtTrait,
        },
        buffer::{BufferConfig, BufferManager},
        AsyncTransport, RetryConfig,
    },
};

/// Wrapper around tokio-serial's SerialStream to implement our async I/O traits.
///
/// This adapter allows serial ports to use the same unified frame reading logic
/// as TCP/UDP transports, ensuring consistent VISCA frame handling across all
/// transport types.
#[derive(Debug)]
struct TokioSerialAdapter {
    stream: SerialStream,
}

impl TokioSerialAdapter {
    /// Create a new adapter wrapping a SerialStream.
    pub fn new(stream: SerialStream) -> Self {
        Self { stream }
    }

    /// Get a mutable reference to the underlying SerialStream.
    pub fn inner_mut(&mut self) -> &mut SerialStream {
        &mut self.stream
    }
}

impl AsyncReadExtTrait for TokioSerialAdapter {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        use tokio::io::AsyncReadExt;
        Ok(self.stream.read(buf).await?)
    }

    async fn read_until(&mut self, delimiter: u8, buf: &mut Vec<u8>) -> Result<usize, Error> {
        use tokio::io::AsyncReadExt;
        // For serial, we need to read byte-by-byte since BufReader might not work well
        // with serial streams due to their low-level nature and potential timing issues
        let start_len = buf.len();
        loop {
            let mut byte = [0u8; 1];
            match self.stream.read(&mut byte).await? {
                0 => return Ok(buf.len() - start_len), // EOF
                1 => {
                    buf.push(byte[0]);
                    if byte[0] == delimiter {
                        return Ok(buf.len() - start_len);
                    }
                }
                _ => unreachable!(), // read(&mut [u8; 1]) should only return 0 or 1
            }
        }
    }
}

impl AsyncWriteExtTrait for TokioSerialAdapter {
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        use tokio::io::AsyncWriteExt;
        Ok(self.stream.write_all(buf).await?)
    }

    async fn flush(&mut self) -> Result<(), Error> {
        use tokio::io::AsyncWriteExt;
        Ok(self.stream.flush().await?)
    }
}

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

// Platform-specific adapter wrapper.
// On Windows, SerialStream contains a raw pointer that isn't Sync,
// so we need to wrap it in Arc<Mutex>. On other platforms, we use it directly.
#[cfg(not(windows))]
type AdapterWrapper = TokioSerialAdapter;

#[cfg(windows)]
type AdapterWrapper = Arc<Mutex<TokioSerialAdapter>>;

/// Async serial transport implementation.
#[cfg(not(windows))]
#[derive(Debug)]
pub struct AsyncSerialTransport {
    adapter: AdapterWrapper,
    buffer_manager: BufferManager,
    config: AsyncSerialConfig,
}

/// Async serial transport implementation.
#[cfg(windows)]
pub struct AsyncSerialTransport {
    adapter: AdapterWrapper,
    buffer_manager: BufferManager,
    config: AsyncSerialConfig,
}

// Manual Debug implementation for Windows
#[cfg(windows)]
impl std::fmt::Debug for AsyncSerialTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncSerialTransport")
            .field("adapter", &"Arc<Mutex<TokioSerialAdapter>>")
            .field("buffer_manager", &self.buffer_manager)
            .field("config", &self.config)
            .finish()
    }
}

impl AsyncSerialTransport {
    /// Create a new async serial transport with the given configuration.
    pub async fn new(config: AsyncSerialConfig) -> Result<Self> {
        // Open serial port
        #[cfg(unix)]
        let mut port = tokio_serial::new(&config.port, config.baud_rate)
            .timeout(config.read_timeout)
            .open_native_async()
            .map_err(|e| {
                Error::TransportError(format!("Failed to open serial port: {e}").into())
            })?;

        #[cfg(not(unix))]
        let port = tokio_serial::new(&config.port, config.baud_rate)
            .timeout(config.read_timeout)
            .open_native_async()
            .map_err(|e| {
                Error::TransportError(format!("Failed to open serial port: {e}").into())
            })?;

        // Configure port settings
        #[cfg(unix)]
        port.set_exclusive(false).map_err(|e| {
            Error::TransportError(format!("Failed to set exclusive mode: {e}").into())
        })?;

        let if_clear = config.if_clear_on_connect;
        let address_set = config.address_set_on_connect;

        let adapter = TokioSerialAdapter::new(port);
        let buffer_manager = BufferManager::new(BufferConfig::for_serial());

        // Wrap adapter based on platform
        #[cfg(not(windows))]
        let adapter_wrapper = adapter;

        #[cfg(windows)]
        let adapter_wrapper = Arc::new(Mutex::new(adapter));

        let mut transport = Self {
            adapter: adapter_wrapper,
            buffer_manager,
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
            .map_err(|e| Error::TransportError(format!("Failed to encode IF Clear: {e}").into()))?;
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
            debug!("Address Set attempt {attempt}", attempt = attempt + 1);
            let cmd = AddressSetCommand::new();
            let mut buffer = [0u8; 16];
            // AddressSetCommand is const-constructed and guaranteed to encode
            let len = cmd
                .encode_into(CameraId::CAMERA_1, &mut buffer)
                .map_err(|e| {
                    Error::TransportError(format!("Failed to encode Address Set: {e}").into())
                })?;
            self.send_raw(&buffer[..len]).await?;

            // Parse response properly
            match self.recv_address_set_response(Duration::from_secs(2)).await {
                Ok(camera_count) => {
                    debug!("Address Set successful, found {camera_count} cameras");
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
        use bytes::BytesMut;
        use tokio::io::AsyncReadExt;

        let mut camera_count = 0;
        let start = Instant::now();

        // Temporarily increase buffer space for address set responses
        let mut response_buffer = BytesMut::with_capacity(128);

        while start.elapsed() < timeout_duration {
            // Try to read some data with timeout
            let mut temp_buf = vec![0u8; 64];
            #[cfg(not(windows))]
            let read_result = timeout(
                Duration::from_millis(50),
                self.adapter.inner_mut().read(&mut temp_buf),
            )
            .await;

            #[cfg(windows)]
            let read_result = {
                let mut adapter = self.adapter.lock().await;
                timeout(
                    Duration::from_millis(50),
                    adapter.inner_mut().read(&mut temp_buf),
                )
                .await
            };

            match read_result {
                Ok(Ok(n)) if n > 0 => {
                    response_buffer.extend_from_slice(&temp_buf[..n]);
                    trace!("Address Set response: {:02X?}", &temp_buf[..n]);

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
                                debug!("Camera {camera_count} assigned address");
                                i += 4;
                            } else if response_buffer[i + 2] == 0x02
                                && response_buffer[i + 3] == VISCA_TERMINATOR
                            {
                                // End of address setting
                                debug!("Address Set complete, {camera_count} cameras found");
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
                        format!("Error reading Address Set response: {e}").into(),
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

    /// Send raw bytes to the serial port.
    async fn send_raw(&mut self, data: &[u8]) -> Result<()> {
        #[cfg(not(windows))]
        {
            timeout(
                self.config.write_timeout,
                write_all_flush(&mut self.adapter, data),
            )
            .await
            .map_err(|_| Error::Timeout)?
        }

        #[cfg(windows)]
        {
            let mut adapter = self.adapter.lock().await;
            timeout(
                self.config.write_timeout,
                write_all_flush(&mut *adapter, data),
            )
            .await
            .map_err(|_| Error::Timeout)?
        }
    }

    /// Receive a complete VISCA frame from the serial port.
    async fn recv_frame(&mut self) -> Result<Bytes> {
        #[cfg(not(windows))]
        {
            timeout(
                self.config.read_timeout,
                read_visca_frame(&mut self.adapter, &self.buffer_manager),
            )
            .await
            .map_err(|_| Error::Timeout)?
        }

        #[cfg(windows)]
        {
            let mut adapter = self.adapter.lock().await;
            timeout(
                self.config.read_timeout,
                read_visca_frame(&mut *adapter, &self.buffer_manager),
            )
            .await
            .map_err(|_| Error::Timeout)?
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
