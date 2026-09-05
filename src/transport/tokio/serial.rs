//! Tokio serial transport implementation using the generic async_serial module.

use tokio_serial::{SerialPortBuilderExt, SerialStream};

use std::{future::Future, sync::Arc, time::Duration};

use crate::{
    error::{Error, Result},
    executor::{TokioBoundFuture, TokioExecutor},
    transport::{
        async_io::{AsyncReadExt as AsyncReadExtTrait, AsyncWriteExt as AsyncWriteExtTrait},
        builder::TransportConfig,
        serial::{
            device_timeout,
            handshake::async_handshake::{address_set_async, if_clear_async},
            startup_plan, Config as SerialConfig, StartupOperation,
        },
    },
};

#[cfg(test)]
use crate::executor::Executor;

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
    async fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> Result<usize, Error> {
        use tokio::io::AsyncReadExt;
        Ok(self.stream.read(buf).await?)
    }
}

impl AsyncWriteExtTrait for TokioSerialAdapter {
    async fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> Result<(), Error> {
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
    #[allow(clippy::manual_async_fn)]
    pub fn connect(config: SerialConfig) -> impl Future<Output = Result<Self>> + Send {
        async move {
            // Construct and validate before opening the descriptor. Canonical
            // CameraConfig already preflights this, but direct serial connectors
            // must provide the same no-I/O guarantee.
            let transport_config = TransportConfig {
                connect_timeout: Duration::from_secs(5), // Not used for serial
                read_timeout: config.read_timeout,
                write_timeout: config.write_timeout,
                buffer_config: config.buffer_config,
                addressing: crate::transport::builder::AddressingMode::Serial,
                tcp_nodelay: None,
                ttl: None,
                tcp_keepalive: None,
            };
            transport_config.validate()?;
            let startup_operations = startup_plan(&config);
            let write_timeout = config.write_timeout;
            let buffer_config = config.buffer_config;

            // Open serial port
            #[cfg(unix)]
            let mut port = tokio_serial::new(&config.port, config.baud_rate)
                .timeout(device_timeout(config.read_timeout))
                .open_native_async()
                .map_err(|e| Error::ConnectionFailed {
                    addr: config.port.clone().into(),
                    source: Arc::new(std::io::Error::other(format!(
                        "Failed to open serial port: {e}"
                    ))),
                })?;

            #[cfg(not(unix))]
            let port = tokio_serial::new(&config.port, config.baud_rate)
                .timeout(device_timeout(config.read_timeout))
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

            // Create executor for handshake operations
            let executor = TokioExecutor::from_current()?;

            perform_startup_handshakes(
                executor,
                &mut adapter,
                startup_operations,
                write_timeout,
                buffer_config,
            )
            .await?;

            Ok(Self::new(adapter, transport_config))
        }
    }

    /// Connect on an explicitly selected Tokio runtime.
    ///
    /// `TokioRuntime::from_handle` uses this path so opening the async serial
    /// descriptor and its optional timer-driven handshake both bind to the
    /// same runtime that will own the session actor.
    pub(crate) fn connect_on(
        handle: tokio::runtime::Handle,
        config: SerialConfig,
    ) -> impl Future<Output = Result<Self>> + Send {
        // Polling through this wrapper retains the caller-owned future while
        // still installing the selected handle before opening the async
        // descriptor and at every handshake poll.
        TokioBoundFuture::new(handle, Self::connect(config))
    }

    /// Connect to a serial port with default configuration.
    pub async fn connect_default(port: &str) -> Result<Self> {
        let config = SerialConfig::new(port);
        Self::connect(config).await
    }
}

/// Perform the requested serial bus startup operations in protocol order.
#[allow(clippy::manual_async_fn)]
fn perform_startup_handshakes<'a>(
    executor: TokioExecutor,
    io: &'a mut TokioSerialAdapter,
    startup_operations: [Option<StartupOperation>; 2],
    write_timeout: Duration,
    buffer_config: crate::transport::BufferConfig,
) -> impl Future<Output = Result<()>> + Send + 'a {
    async move {
        for operation in startup_operations.into_iter().flatten() {
            match operation {
                StartupOperation::AddressSet => {
                    address_set_async(
                        &executor,
                        io,
                        Duration::from_secs(2),
                        write_timeout,
                        buffer_config,
                    )
                    .await?;
                }
                StartupOperation::InterfaceClear => {
                    if_clear_async(&executor, io, write_timeout).await?;
                }
            }
        }

        Ok(())
    }
}

/// Generic transcript seam for startup sequencing tests.
///
/// Production startup uses the concrete Tokio adapter above so its returned
/// constructor future can promise `Send`; tests retain this runtime-agnostic
/// seam to exercise the protocol order and deadline behavior without a device.
#[cfg(test)]
async fn perform_startup_handshakes_for_test<E, S>(
    executor: &E,
    io: &mut S,
    config: &SerialConfig,
) -> Result<()>
where
    E: Executor,
    S: AsyncReadExtTrait + AsyncWriteExtTrait + Send,
{
    for operation in startup_plan(config).into_iter().flatten() {
        match operation {
            StartupOperation::AddressSet => {
                address_set_async(
                    executor,
                    io,
                    Duration::from_secs(2),
                    config.write_timeout,
                    config.buffer_config,
                )
                .await?;
            }
            StartupOperation::InterfaceClear => {
                if_clear_async(executor, io, config.write_timeout).await?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::transport::BufferConfig;
    use std::collections::VecDeque;

    struct TranscriptIo {
        reads: VecDeque<Vec<u8>>,
        writes: Vec<Vec<u8>>,
        read_buffer_sizes: Vec<usize>,
        flush_calls: usize,
    }

    impl TranscriptIo {
        fn with_read(bytes: Vec<u8>) -> Self {
            Self {
                reads: [bytes].into(),
                writes: Vec::new(),
                read_buffer_sizes: Vec::new(),
                flush_calls: 0,
            }
        }
    }

    impl AsyncReadExtTrait for TranscriptIo {
        async fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> Result<usize> {
            self.read_buffer_sizes.push(buf.len());
            let bytes = self.reads.pop_front().expect("unexpected serial read");
            buf[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }
    }

    impl AsyncWriteExtTrait for TranscriptIo {
        async fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> Result<()> {
            self.writes.push(buf.to_vec());
            Ok(())
        }

        async fn flush(&mut self) -> Result<()> {
            self.flush_calls += 1;
            Ok(())
        }
    }

    #[tokio::test]
    async fn invalid_buffer_bounds_fail_before_serial_device_open() {
        let config = SerialConfig::new("grafton-visca-invalid-buffer-bounds-serial-device")
            .if_clear_on_connect(false)
            .buffer_config(BufferConfig {
                recv_buffer_size: 65,
                max_buffer_size: 64,
            });

        let result = Serial::connect(config).await;

        assert!(matches!(
            result,
            Err(Error::InvalidRequest(actual))
                if actual.as_ref() == "transport receive buffer cannot exceed maximum buffer"
        ));
    }

    #[tokio::test]
    async fn zero_io_timeouts_fail_before_serial_device_open() {
        for (config, message) in [
            (
                SerialConfig::new("grafton-visca-zero-read-timeout-serial-device")
                    .if_clear_on_connect(false)
                    .read_timeout(Duration::ZERO),
                "transport read timeout must be non-zero",
            ),
            (
                SerialConfig::new("grafton-visca-zero-write-timeout-serial-device")
                    .if_clear_on_connect(false)
                    .write_timeout(Duration::ZERO),
                "transport write timeout must be non-zero",
            ),
        ] {
            assert!(matches!(
                Serial::connect(config).await,
                Err(Error::InvalidRequest(actual)) if actual.as_ref() == message
            ));
        }
    }

    #[tokio::test]
    async fn startup_with_address_set_and_if_clear_transmits_address_set_first() {
        let executor = TokioExecutor::from_current().expect("Tokio runtime is present");
        let mut io = TranscriptIo::with_read(vec![0x88, 0x30, 0x02, 0xFF]);
        let config = SerialConfig::new("/dev/test")
            .address_set_on_connect(true)
            .if_clear_on_connect(true)
            .buffer_config(BufferConfig {
                recv_buffer_size: 4,
                max_buffer_size: 32,
            });

        perform_startup_handshakes_for_test(&executor, &mut io, &config)
            .await
            .expect("startup handshakes succeed");

        assert_eq!(
            io.writes,
            [
                vec![0x88, 0x30, 0x01, 0xFF],
                vec![0x88, 0x01, 0x00, 0x01, 0xFF],
            ],
            "the Tokio serial startup transcript must address the bus before clearing it"
        );
        assert_eq!(io.read_buffer_sizes, [4]);
        assert_eq!(
            io.flush_calls, 0,
            "Tokio serial startup must not enter the unbounded device flush"
        );
    }

    struct PendingWriteIo;

    impl AsyncReadExtTrait for PendingWriteIo {
        async fn read<'a>(&'a mut self, _buf: &'a mut [u8]) -> Result<usize> {
            std::future::pending().await
        }
    }

    impl AsyncWriteExtTrait for PendingWriteIo {
        async fn write_all<'a>(&'a mut self, _buf: &'a [u8]) -> Result<()> {
            std::future::pending().await
        }

        async fn flush(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn startup_uses_the_configured_write_timeout() {
        let executor = TokioExecutor::from_current().expect("Tokio runtime is present");
        let mut io = PendingWriteIo;
        let config = SerialConfig::new("/dev/test")
            .address_set_on_connect(false)
            .if_clear_on_connect(true)
            .write_timeout(Duration::from_millis(5));

        let bounded = tokio::time::timeout(
            Duration::from_millis(100),
            perform_startup_handshakes_for_test(&executor, &mut io, &config),
        )
        .await;
        assert!(matches!(bounded, Ok(Err(Error::Timeout))));
    }
}
