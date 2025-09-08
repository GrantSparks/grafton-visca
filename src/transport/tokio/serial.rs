//! Tokio serial transport implementation using the generic async_serial module.

use std::time::Duration;
use tokio::time::Instant;
use tokio_serial::{SerialPortBuilderExt, SerialStream};
use tracing::{debug, trace, warn};

use crate::{
    camera_id::CameraId,
    command::{
        bytes::VISCA_TERMINATOR,
        encode_visca::ViscaEncode,
        system::{AddressSetCommand, InterfaceClearCommand},
    },
    error::{Error, Result},
    transport::{
        async_io::{AsyncReadExt as AsyncReadExtTrait, AsyncWriteExt as AsyncWriteExtTrait},
        buffer::BufferConfig,
        builder::TransportConfig,
        RetryConfig,
    },
};

/// Serial transport for async VISCA communication using tokio.
///
/// This is a type alias for the generic Serial transport specialized for tokio's TokioSerialAdapter.
pub type Serial = crate::transport::async_serial::Serial<TokioSerialAdapter>;

/// Wrapper around tokio-serial's SerialStream to implement our async I/O traits.
///
/// This adapter allows serial ports to use the same unified frame reading logic
/// as TCP/UDP transports, ensuring consistent VISCA frame handling across all
/// transport types.
#[derive(Debug)]
pub struct TokioSerialAdapter {
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

/// Helper methods for creating tokio serial transports.
impl Serial {
    /// Connect to a serial port with the given configuration.
    ///
    /// This method opens the serial port, configures it, and optionally performs
    /// I/F Clear and Address Set initialization.
    pub async fn connect(config: SerialConfig) -> Result<Self> {
        // Open serial port
        #[cfg(unix)]
        let mut port = tokio_serial::new(&config.port, config.baud_rate)
            .timeout(config.read_timeout)
            .open_native_async()
            .map_err(|e| Error::ConnectionFailed {
                addr: config.port.clone().into(),
                source: std::io::Error::other(format!("Failed to open serial port: {e}")),
            })?;

        #[cfg(not(unix))]
        let port = tokio_serial::new(&config.port, config.baud_rate)
            .timeout(config.read_timeout)
            .open_native_async()
            .map_err(|e| Error::ConnectionFailed {
                addr: config.port.clone().into(),
                source: std::io::Error::other(format!("Failed to open serial port: {e}")),
            })?;

        // Configure port settings
        #[cfg(unix)]
        port.set_exclusive(false)
            .map_err(|e| Error::Io(std::io::Error::other(e)))?;

        let mut adapter = TokioSerialAdapter::new(port);

        // Create TransportConfig from SerialConfig
        let transport_config = TransportConfig {
            connect_timeout: Duration::from_secs(5), // Not used for serial
            read_timeout: config.read_timeout,
            write_timeout: config.write_timeout,
            retry_config: config.retry_config,
            buffer_config: BufferConfig::for_raw_ip(), // Serial uses raw VISCA
            tcp_nodelay: None,
            ttl: None,
        };

        // Perform initialization if requested
        if config.if_clear_on_connect {
            send_if_clear(&mut adapter).await?;
        }
        if config.address_set_on_connect {
            send_address_set(&mut adapter).await?;
        }

        Ok(Self::new(adapter, transport_config))
    }

    /// Connect to a serial port with default configuration.
    pub async fn connect_default(port: &str) -> Result<Self> {
        let config = SerialConfig {
            port: port.to_string(),
            ..Default::default()
        };
        Self::connect(config).await
    }
}

/// Send I/F Clear command to reset all devices on the bus.
async fn send_if_clear<S: AsyncWriteExtTrait>(stream: &mut S) -> Result<()> {
    debug!("Sending I/F Clear command");
    let cmd = InterfaceClearCommand::new();
    let mut buffer = [0u8; 16];
    // InterfaceClearCommand is const-constructed and guaranteed to encode
    let len = cmd
        .encode_into(CameraId::CAMERA_1, &mut buffer)
        .map_err(|e| Error::TransportError(format!("Failed to encode IF Clear: {e}").into()))?;

    stream.write_all(&buffer[..len]).await?;
    stream.flush().await?;

    // Wait for I/F Clear to complete
    tokio::time::sleep(Duration::from_millis(100)).await;
    Ok(())
}

/// Send Address Set command to assign addresses to devices.
/// Returns the number of cameras detected.
async fn send_address_set<S: AsyncReadExtTrait + AsyncWriteExtTrait>(stream: &mut S) -> Result<u8> {
    let max_attempts = 3;

    for attempt in 0..max_attempts {
        debug!("Address Set attempt {}", attempt + 1);
        let cmd = AddressSetCommand::new();
        let mut buffer = [0u8; 16];
        // AddressSetCommand is const-constructed and guaranteed to encode
        let len = cmd
            .encode_into(CameraId::CAMERA_1, &mut buffer)
            .map_err(|e| {
                Error::TransportError(format!("Failed to encode Address Set: {e}").into())
            })?;

        stream.write_all(&buffer[..len]).await?;
        stream.flush().await?;

        // Parse response properly
        match recv_address_set_response(stream, Duration::from_secs(2)).await {
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
async fn recv_address_set_response<S: AsyncReadExtTrait>(
    stream: &mut S,
    timeout_duration: Duration,
) -> Result<u8> {
    use bytes::BytesMut;

    let mut camera_count = 0;
    let start = Instant::now();

    // Temporarily increase buffer space for address set responses
    let mut response_buffer = BytesMut::with_capacity(128);

    while start.elapsed() < timeout_duration {
        // Try to read some data with timeout
        let mut temp_buf = vec![0u8; 64];
        let read_result =
            tokio::time::timeout(Duration::from_millis(50), stream.read(&mut temp_buf)).await;

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
            Ok(Err(e)) => {
                // Check if it's an I/O error with TimedOut kind
                if let Error::Io(io_err) = &e {
                    if io_err.kind() == std::io::ErrorKind::TimedOut {
                        // Timeout on this read, but total timeout not reached yet
                        continue;
                    }
                }
                return Err(e);
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
