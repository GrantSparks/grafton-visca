//! Tokio serial transport.
//!
//! The port is opened and started up as documented on
//! [`crate::transport::serial`]; this module contains only the Tokio I/O
//! driver for that shared behaviour.

use std::{future::Future, pin::Pin, time::Duration};

use tokio_serial::{SerialPortBuilderExt, SerialStream};

use crate::{
    error::{Error, Result},
    executor::{Executor, TokioBoundFuture, TokioExecutor},
    transport::{
        async_io::{AsyncReadExt as AsyncReadExtTrait, AsyncWriteExt as AsyncWriteExtTrait},
        serial::{
            handshake::{Action, SerialStartup, StartupTiming},
            open_failed, port_builder, Config as SerialConfig,
        },
        AddressedBus,
    },
};

/// Serial transport for async VISCA communication using tokio.
///
/// This is a type alias for the generic Serial transport specialized for tokio's TokioSerialAdapter.
pub type Serial = crate::transport::async_serial::Serial<TokioSerialAdapter>;

/// Wrapper around tokio-serial's SerialStream to implement our async I/O traits.
#[derive(Debug)]
pub struct TokioSerialAdapter {
    stream: SerialStream,
}

/// Discarding the input received so far, which the startup asks for once its
/// operations have finished.
pub(crate) trait DiscardInput {
    fn discard_input(&mut self) -> Result<()>;
}

impl DiscardInput for TokioSerialAdapter {
    fn discard_input(&mut self) -> Result<()> {
        tokio_serial::SerialPort::clear(&self.stream, tokio_serial::ClearBuffer::Input)
            .map_err(|error| Error::from(std::io::Error::from(error)))
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
    /// Open the configured port and perform the requested startup.
    ///
    /// The configuration is validated before the device is opened. Opening
    /// writes nothing to the bus unless [`SerialConfig::startup`] requests
    /// Address Set or I/F Clear. See [`crate::transport::serial`] for the
    /// open and startup behaviour shared with the blocking transport.
    #[allow(clippy::manual_async_fn)]
    pub fn connect(config: SerialConfig) -> impl Future<Output = Result<Self>> + Send {
        async move {
            let transport_config = config.transport_config();
            transport_config.validate()?;

            let stream = port_builder(&config)
                .open_native_async()
                .map_err(|error| open_failed(&config, error))?;
            let mut adapter = TokioSerialAdapter { stream };

            let executor = TokioExecutor::from_current()?;
            let addressed_cameras =
                perform_startup(&executor, &mut adapter, &config, StartupTiming::VISCA).await?;

            let addressed_bus =
                addressed_cameras.map(|cameras| AddressedBus::new(config.port.clone(), cameras));
            Ok(Self::new(adapter, transport_config).with_addressed_bus(addressed_bus))
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
        Self::connect(SerialConfig::new(port)).await
    }
}

type SendFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Erase the executor's RPITIT futures before they enter the startup state
/// machine. Older compilers cannot always prove the equivalent higher-ranked
/// lifetime bound through nested opaque futures.
fn bounded<'a, E, F, T>(exec: &'a E, duration: Duration, future: F) -> SendFuture<'a, Result<T>>
where
    E: Executor,
    F: Future<Output = T> + Send + 'a,
    T: Send + 'a,
{
    Box::pin(exec.timeout(duration, future))
}

fn sleep<E: Executor>(exec: &E, duration: Duration) -> SendFuture<'_, ()> {
    Box::pin(exec.sleep(duration))
}

/// Drive the shared serial startup with async I/O on `exec`'s clock,
/// returning the camera count Address Set reported.
#[allow(clippy::manual_async_fn)]
fn perform_startup<'a, E, S>(
    exec: &'a E,
    io: &'a mut S,
    config: &'a SerialConfig,
    timing: StartupTiming,
) -> impl Future<Output = Result<Option<u8>>> + Send + 'a
where
    E: Executor,
    S: AsyncReadExtTrait + AsyncWriteExtTrait + DiscardInput + Send + ?Sized,
{
    async move {
        let mut startup = SerialStartup::new(config, timing)?;
        let mut buffer = vec![0; startup.read_capacity()];
        loop {
            match startup.next(exec.now()) {
                Action::Write { frame, timeout } => {
                    let written = bounded(exec, timeout, io.write_all(frame))
                        .await
                        .and_then(|written| written);
                    startup.wrote(written);
                }
                Action::Read { timeout } => {
                    let read = bounded(exec, timeout, io.read(&mut buffer))
                        .await
                        .and_then(|read| read);
                    startup.read(read.map(|read| &buffer[..read]));
                }
                Action::Sleep(duration) => sleep(exec, duration).await,
                Action::DiscardInput => io.discard_input()?,
                Action::Done(result) => return result.map(|()| startup.addressed_cameras()),
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::transport::{
        serial::{
            handshake::scenarios::{self, Read as ScriptRead, Write as ScriptWrite},
            Startup,
        },
        AddressingMode, BufferConfig, HasTransportConfig,
    };
    use std::collections::VecDeque;

    /// Scripted serial I/O: queued read results and a write log.
    #[derive(Default)]
    struct TranscriptIo {
        reads: VecDeque<Result<Vec<u8>>>,
        write_results: VecDeque<Result<()>>,
        writes: Vec<Vec<u8>>,
        flush_calls: usize,
        input_discards: usize,
    }

    impl AsyncReadExtTrait for TranscriptIo {
        async fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> Result<usize> {
            match self.reads.pop_front() {
                Some(Ok(bytes)) => {
                    buf[..bytes.len()].copy_from_slice(&bytes);
                    Ok(bytes.len())
                }
                Some(Err(error)) => Err(error),
                None => std::future::pending().await,
            }
        }
    }

    impl AsyncWriteExtTrait for TranscriptIo {
        async fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> Result<()> {
            self.writes.push(buf.to_vec());
            match self.write_results.pop_front() {
                Some(Err(error)) => Err(error),
                Some(Ok(())) | None => Ok(()),
            }
        }

        async fn flush(&mut self) -> Result<()> {
            self.flush_calls += 1;
            Ok(())
        }
    }

    impl DiscardInput for TranscriptIo {
        fn discard_input(&mut self) -> Result<()> {
            self.input_discards += 1;
            Ok(())
        }
    }

    /// The startup transcripts shared with the blocking driver.
    #[tokio::test]
    async fn shared_startup_scenarios() {
        let executor = TokioExecutor::from_current().expect("Tokio runtime is present");
        for scenario in scenarios::all() {
            let mut io = TranscriptIo {
                reads: scenario
                    .reads
                    .iter()
                    .map(|read| match read {
                        ScriptRead::Bytes(bytes) => Ok(bytes.clone()),
                        ScriptRead::Idle => Err(Error::io_timeout()),
                        ScriptRead::Fails(kind) => Err(std::io::Error::from(*kind).into()),
                    })
                    .collect(),
                write_results: scenario
                    .writes
                    .iter()
                    .map(|write| match write {
                        ScriptWrite::Accepted => Ok(()),
                        ScriptWrite::TimesOut => Err(Error::io_timeout()),
                    })
                    .collect(),
                ..TranscriptIo::default()
            };

            let result =
                perform_startup(&executor, &mut io, &scenario.config, scenarios::TIMING).await;

            scenarios::check(&scenario, &result);
            assert_eq!(io.writes, scenario.expect_writes, "{}", scenario.name);
            assert_eq!(
                io.input_discards == 1,
                scenario.discards_input,
                "{}",
                scenario.name
            );
            assert_eq!(
                io.flush_calls, 0,
                "{}: startup must not enter the unbounded device flush",
                scenario.name
            );
        }
    }

    #[tokio::test]
    async fn invalid_buffer_bounds_fail_before_serial_device_open() {
        let config = SerialConfig::new("grafton-visca-invalid-buffer-bounds-serial-device")
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
                    .read_timeout(Duration::ZERO),
                "transport read timeout must be non-zero",
            ),
            (
                SerialConfig::new("grafton-visca-zero-write-timeout-serial-device")
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
    async fn an_absent_port_is_a_connection_failure_naming_the_port() {
        let config = SerialConfig::new("/dev/grafton-visca-absent-serial-device");
        let error = Serial::connect(config)
            .await
            .expect_err("an absent port cannot open");
        assert!(matches!(
            error,
            Error::ConnectionFailed { ref addr, .. }
                if addr == "/dev/grafton-visca-absent-serial-device"
        ));
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

    impl DiscardInput for PendingWriteIo {
        fn discard_input(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn startup_uses_the_configured_write_timeout() {
        let executor = TokioExecutor::from_current().expect("Tokio runtime is present");
        let mut io = PendingWriteIo;
        let config = SerialConfig::new("/dev/test")
            .startup(Startup::default().with_interface_clear(true))
            .write_timeout(Duration::from_millis(5));

        let bounded = tokio::time::timeout(
            Duration::from_millis(100),
            perform_startup(&executor, &mut io, &config, StartupTiming::VISCA),
        )
        .await;
        assert!(matches!(bounded, Ok(Err(Error::Timeout { .. }))));
    }

    /// The same exclusive-access open and reported configuration as the
    /// blocking transport.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_held_port_cannot_be_opened_twice() {
        use serialport::SerialPort;

        let (_master, slave) = serialport::TTYPort::pair().expect("pseudo-terminal pair");
        let path = slave.name().expect("pseudo-terminal path");
        let config = SerialConfig::new(path.clone());

        let first = Serial::connect(config.clone())
            .await
            .expect("first opener succeeds");
        let reported = first.transport_config();
        assert_eq!(reported, &config.transport_config());
        assert_eq!(reported.addressing, AddressingMode::Serial);
        assert_eq!(reported.tcp_nodelay, None);
        assert_eq!(reported.tcp_keepalive, None);

        let second = Serial::connect(config)
            .await
            .expect_err("the port is held exclusively");
        assert!(matches!(
            second,
            Error::ConnectionFailed { ref addr, .. } if *addr == path
        ));
    }
}
