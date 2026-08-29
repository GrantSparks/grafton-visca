//! Tokio serial transport implementation using the generic async_serial module.

use tokio_serial::{SerialPortBuilderExt, SerialStream};

use std::{sync::Arc, time::Duration};

use crate::{
    error::{Error, Result},
    executor::TokioExecutor,
    transport::{
        async_io::{AsyncReadExt as AsyncReadExtTrait, AsyncWriteExt as AsyncWriteExtTrait},
        builder::TransportConfig,
        serial::{
            handshake::async_handshake::{address_set_async, if_clear_async},
            Config as SerialConfig,
        },
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
                source: Arc::new(std::io::Error::other(format!(
                    "Failed to open serial port: {e}"
                ))),
            })?;

        #[cfg(not(unix))]
        let port = tokio_serial::new(&config.port, config.baud_rate)
            .timeout(config.read_timeout)
            .open_native_async()
            .map_err(|e| Error::ConnectionFailed {
                addr: config.port.clone().into(),
                source: Arc::new(std::io::Error::other(format!(
                    "Failed to open serial port: {e}"
                ))),
            })?;

        // Configure port settings
        #[cfg(unix)]
        port.set_exclusive(false)
            .map_err(|e| Error::Io(Arc::new(std::io::Error::other(e))))?;

        let mut adapter = TokioSerialAdapter::new(port);

        // Create TransportConfig from SerialConfig
        let transport_config = TransportConfig {
            connect_timeout: Duration::from_secs(5), // Not used for serial
            read_timeout: config.read_timeout,
            write_timeout: config.write_timeout,
            buffer_config: config.buffer_config,
            addressing: crate::transport::builder::AddressingMode::Serial, // Serial uses Serial addressing
            tcp_nodelay: None,
            ttl: None,
            tcp_keepalive: None,
        };

        // Create executor for handshake operations
        let executor = TokioExecutor::from_current()?;

        // Perform initialization if requested
        if config.if_clear_on_connect {
            if_clear_async(&executor, &mut adapter).await?;
        }
        if config.address_set_on_connect {
            address_set_async(&executor, &mut adapter, Duration::from_secs(2)).await?;
        }

        Ok(Self::new(adapter, transport_config))
    }

    /// Connect to a serial port with default configuration.
    pub async fn connect_default(port: &str) -> Result<Self> {
        let config = SerialConfig::new(port);
        Self::connect(config).await
    }
}
