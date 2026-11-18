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
        AddressedBus, IoBounds,
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
}

/// Helper methods for creating tokio serial transports.
impl Serial {
    async fn open(config: SerialConfig, io_bounds: IoBounds) -> Result<Self> {
        let stream = port_builder(&config)
            .open_native_async()
            .map_err(|error| open_failed(&config, error))?;
        let mut adapter = TokioSerialAdapter { stream };

        let executor = TokioExecutor::from_current()?;
        let addressed_cameras = perform_startup(
            &executor,
            &mut adapter,
            &config,
            io_bounds,
            StartupTiming::VISCA,
        )
        .await?;

        let addressed_bus =
            addressed_cameras.map(|cameras| AddressedBus::new(config.port.clone(), cameras));
        Ok(Self::new(adapter, io_bounds).with_addressed_bus(addressed_bus))
    }

    /// Connect on an explicitly selected Tokio runtime.
    ///
    /// `TokioRuntime::from_handle` uses this path so opening the async serial
    /// descriptor and its optional timer-driven handshake both bind to the
    /// same runtime that will own the session actor.
    pub(crate) fn connect_on(
        handle: tokio::runtime::Handle,
        config: SerialConfig,
        io_bounds: IoBounds,
    ) -> impl Future<Output = Result<Self>> + Send {
        // Polling through this wrapper retains the caller-owned future while
        // still installing the selected handle before opening the async
        // descriptor and at every handshake poll.
        TokioBoundFuture::new(handle, Self::open(config, io_bounds))
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
    Box::pin(async move {
        crate::timeout::within(future, exec.sleep(duration))
            .await
            .ok_or_else(Error::io_timeout)
    })
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
    io_bounds: IoBounds,
    timing: StartupTiming,
) -> impl Future<Output = Result<Option<u8>>> + Send + 'a
where
    E: Executor,
    S: AsyncReadExtTrait + AsyncWriteExtTrait + DiscardInput + Send + ?Sized,
{
    async move {
        let mut startup = SerialStartup::new(config, io_bounds, timing)?;
        let mut buffer = vec![0; startup.read_capacity()];
        loop {
            match startup.next(exec.now()) {
                Action::Write(write) => {
                    let written = bounded(exec, write.timeout, io.write_all(write.frame()))
                        .await
                        .and_then(|written| written);
                    write.complete(written);
                }
                Action::Read(read_action) => {
                    let read = bounded(exec, read_action.timeout, io.read(&mut buffer))
                        .await
                        .and_then(|read| read);
                    read_action.complete(read.map(|read| &buffer[..read]));
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
        HasTransportConfig,
    };
    use std::collections::VecDeque;

    fn default_bounds() -> IoBounds {
        IoBounds::SERIAL_DEFAULT
    }

    async fn open_serial(config: SerialConfig) -> Result<Serial> {
        use crate::runtime::{Runtime, TokioRuntime};
        TokioRuntime::from_current()?
            .connect_serial(config, default_bounds())
            .await
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn transport_handle_preserves_the_serial_addressed_bus_identity() {
        use crate::{transport::TransportIdentity, TokioRuntime, TransportHandle};

        let (_master, stream) = SerialStream::pair().expect("pseudo-terminal pair");
        let bus = AddressedBus::new("test pseudo-terminal", 3);
        let transport = Serial::new(TokioSerialAdapter { stream }, default_bounds())
            .with_addressed_bus(Some(bus.clone()));
        let handle = TransportHandle::<TokioRuntime>::Serial(Box::new(transport));

        assert_eq!(
            handle.identity(),
            TransportIdentity::Serial { bus: Some(&bus) }
        );
    }

    /// The actual Tokio serial connector preserves the Address Set topology
    /// verdict when a configured camera is beyond the reported chain.
    #[cfg(unix)]
    #[tokio::test]
    async fn registered_cameras_must_have_been_addressed() {
        use serialport::SerialPort as _;
        use std::io::{Read as _, Write as _};

        use crate::{
            camera::CameraConfig, profiles::PtzOpticsG2, transport::serial::Startup,
            types::IndividualCameraId, wire::VISCA_TERMINATOR,
        };

        let (mut master, slave) = serialport::TTYPort::pair().expect("pseudo-terminal pair");
        let path = slave.name().expect("pseudo-terminal path");
        master
            .set_timeout(Duration::from_secs(5))
            .expect("master timeout");
        let bus = std::thread::spawn(move || {
            let mut frame = [0; 4];
            master.read_exact(&mut frame).expect("Address Set frame");
            assert_eq!(frame, [0x88, 0x30, 0x01, VISCA_TERMINATOR]);
            master
                .write_all(&[0x88, 0x30, 0x02, VISCA_TERMINATOR])
                .expect("Address Set reply");
            master
        });

        let config = CameraConfig::<PtzOpticsG2>::serial_configured(
            SerialConfig::new(path.clone()).with_startup(Startup::default().with_address_set(true)),
            default_bounds(),
        )
        .with_camera_id(IndividualCameraId::CAMERA_2);
        let runtime = crate::runtime::TokioRuntime::from_current().expect("Tokio runtime");
        let result = crate::r#async::CameraSession::connect(&config, runtime).await;
        let _master = bus.join().expect("bus thread");
        drop(slave);
        assert!(matches!(
            result,
            Err(Error::CameraNotAddressed { port, camera, addressed: 1 })
                if port == path && camera == IndividualCameraId::CAMERA_2
        ));
    }

    /// Scripted serial I/O: queued read results and a write log.
    #[derive(Default)]
    struct TranscriptIo {
        reads: VecDeque<Result<Vec<u8>>>,
        write_results: VecDeque<Result<()>>,
        writes: Vec<Vec<u8>>,
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
                        ScriptRead::Eof => Ok(Vec::new()),
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

            let result = perform_startup(
                &executor,
                &mut io,
                &scenario.config,
                default_bounds(),
                scenarios::TIMING,
            )
            .await;

            scenarios::check(&scenario, &result);
            if matches!(scenario.outcome, scenarios::Outcome::Closed) {
                assert_eq!(io.reads.len(), 1, "EOF stops before the next read");
            }
            assert_eq!(io.writes, scenario.expect_writes, "{}", scenario.name);
            assert_eq!(
                io.input_discards == 1,
                scenario.discards_input,
                "{}",
                scenario.name
            );
        }
    }

    #[tokio::test]
    async fn an_absent_port_is_a_connection_failure_naming_the_port() {
        let config = SerialConfig::new("/dev/grafton-visca-absent-serial-device");
        let error = open_serial(config)
            .await
            .expect_err("an absent port cannot open");
        assert!(matches!(
            error,
            Error::ConnectionFailed { ref addr, ref source }
                if addr == "/dev/grafton-visca-absent-serial-device"
                    && source.kind() == std::io::ErrorKind::NotFound
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
            .with_startup(Startup::default().with_interface_clear(true));
        let bounds = default_bounds()
            .with_write_timeout(Duration::from_millis(5))
            .expect("short write timeout");

        let bounded = tokio::time::timeout(
            Duration::from_millis(100),
            perform_startup(&executor, &mut io, &config, bounds, StartupTiming::VISCA),
        )
        .await;
        let error = bounded
            .expect("startup respects its write budget")
            .expect_err("the stalled serial write times out");
        assert!(matches!(error, Error::Timeout { .. }));
        assert_eq!(error.recovery(), crate::Recovery::Reconcile);
        assert_eq!(
            error.failure_context(),
            Error::io_timeout().failure_context()
        );
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

        let first = open_serial(config.clone())
            .await
            .expect("first opener succeeds");
        assert_eq!(first.io_bounds(), &default_bounds());

        let second = open_serial(config)
            .await
            .expect_err("the port is held exclusively");
        assert!(matches!(
            second,
            Error::ConnectionFailed { ref addr, .. } if *addr == path
        ));
    }
}
