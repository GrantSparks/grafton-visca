//! Blocking serial transport for VISCA over RS-232/422.
//!
//! The port is opened and started up as documented on
//! [`crate::transport::serial`]; this module contains only the blocking I/O
//! driver for that shared behaviour.

use std::time::{Duration, Instant};

use tracing::trace;

use crate::{
    command::CommandKind,
    error::Result,
    timeout::Deadline,
    transport::{
        blocking::{read_once_bounded, write_all_bounded, TimedIo},
        builder::{AddressingMode, TransportConfig},
        serial::{
            handshake::{Action, SerialStartup, StartupTiming},
            open_failed, port_builder, set_device_timeout, Config as SerialConfig, DevicePort,
            SERIAL_PORT_CLOSED,
        },
        stream_read, AddressedBus, BlockingTransport, HasTransportConfig,
    },
};

/// Serial transport implementation for blocking I/O.
///
/// This transport operates at the stream level, reading/writing raw bytes.
/// Framing and retry logic are handled by the runtime layer.
pub struct SerialTransport {
    port: Box<dyn serialport::SerialPort + Send>,
    config: SerialConfig,
    transport_config: TransportConfig,
    addressed_bus: Option<AddressedBus>,
}

impl std::fmt::Debug for SerialTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerialTransport")
            .field("port", &self.config.port)
            .field("baud_rate", &self.config.baud_rate)
            .finish_non_exhaustive()
    }
}

impl SerialTransport {
    /// Open the configured port and perform the requested startup.
    ///
    /// The configuration is validated before the device is opened. Opening
    /// writes nothing to the bus unless [`SerialConfig::startup`] requests
    /// Address Set or I/F Clear.
    ///
    /// # Errors
    ///
    /// [`crate::Error::InvalidRequest`] for an invalid configuration,
    /// [`crate::Error::ConnectionFailed`] when the port cannot be opened (including
    /// when another process holds it), and the startup errors documented on
    /// [`crate::transport::serial`].
    pub fn new(config: SerialConfig) -> Result<Self> {
        let transport_config = config.transport_config();
        transport_config.validate()?;

        let mut port = port_builder(&config)
            .open()
            .map_err(|error| open_failed(&config, error))?;
        let addressed_bus = perform_startup(&mut *port, &config, StartupTiming::VISCA)?
            .map(|cameras| AddressedBus::new(config.port.clone(), cameras));

        Ok(Self {
            port,
            config,
            transport_config,
            addressed_bus,
        })
    }
}

/// Drive the shared serial startup with blocking device I/O, returning the
/// camera count Address Set reported.
fn perform_startup(
    port: &mut DevicePort,
    config: &SerialConfig,
    timing: StartupTiming,
) -> Result<Option<u8>> {
    let mut startup = SerialStartup::new(config, timing)?;
    let mut buffer = vec![0; startup.read_capacity()];
    loop {
        match startup.next(Instant::now()) {
            Action::Write { frame, timeout } => {
                let written = Deadline::after(Instant::now(), timeout, "write_timeout").and_then(
                    |deadline| {
                        write_all_bounded(port, set_device_timeout, frame, deadline, timeout)
                    },
                );
                startup.wrote(written);
            }
            Action::Read { timeout } => {
                let read = read_once_bounded(
                    port,
                    set_device_timeout,
                    &mut buffer,
                    timeout,
                    TimedIo::Bounded,
                );
                startup.read(read.map(|read| &buffer[..read]));
            }
            Action::Sleep(duration) => std::thread::sleep(duration),
            Action::DiscardInput => port
                .clear(serialport::ClearBuffer::Input)
                .map_err(std::io::Error::from)?,
            Action::Done(result) => return result.map(|()| startup.addressed_cameras()),
        }
    }
}

impl HasTransportConfig for SerialTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.transport_config
    }

    fn standard_transport_kind(&self) -> Option<crate::camera::TransportKind> {
        Some(crate::camera::TransportKind::Serial)
    }

    fn addressed_bus(&self) -> Option<&AddressedBus> {
        self.addressed_bus.as_ref()
    }
}

impl BlockingTransport for SerialTransport {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: CommandKind,
        timeout: Duration,
    ) -> Result<()> {
        let deadline = Deadline::after(Instant::now(), timeout, "write_timeout")?;
        write_all_bounded(
            &mut *self.port,
            set_device_timeout,
            bytes,
            deadline,
            timeout,
        )?;
        trace!("Queued {} serial bytes: {:02X?}", bytes.len(), bytes);
        Ok(())
    }

    fn recv_into_with_timeout(&mut self, dst: &mut [u8], timeout: Duration) -> Result<usize> {
        let read = read_once_bounded(
            &mut *self.port,
            set_device_timeout,
            dst,
            timeout,
            TimedIo::Bounded,
        )?;
        trace!("Read {read} bytes from serial port");
        stream_read(read, SERIAL_PORT_CLOSED)
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(AddressingMode::Serial)
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::transport::BufferConfig;
    use crate::{
        command::bytes::VISCA_TERMINATOR,
        transport::serial::{
            handshake::scenarios::{self, Read as ScriptRead, Write as ScriptWrite},
            Startup,
        },
        Error,
    };
    use serialport::{ClearBuffer, DataBits, FlowControl, Parity, SerialPort, StopBits};
    use std::io::{self, ErrorKind, Read};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };
    use std::thread;
    use std::{cell::RefCell, collections::VecDeque};

    /// One deterministic fake serial read outcome.
    enum ReadStep {
        Error(ErrorKind),
        Bytes(Vec<u8>),
    }

    /// One deterministic fake serial write outcome.
    enum WriteStep {
        Error(ErrorKind),
        Partial {
            bytes: usize,
            delay: Duration,
        },
        /// Claim more bytes than the buffer held.
        OverReport,
    }

    /// A mock serial port for testing that records all operations.
    struct TestSerialPort {
        /// Current timeout setting.
        timeout: RefCell<Duration>,
        /// Every timeout applied through the serial-port API.
        timeout_history: RefCell<Vec<Duration>>,
        /// A Send-safe observation handle for tests that move the port into a transport.
        timeout_history_observer: Arc<Mutex<Vec<Duration>>>,
        /// If set, write will return this error.
        write_error: RefCell<Option<ErrorKind>>,
        /// If set, flush will return this error.
        flush_error: RefCell<Option<ErrorKind>>,
        /// Data to return on read.
        read_data: RefCell<Vec<u8>>,
        /// Scripted read outcomes, used by handshake deadline tests.
        read_steps: RefCell<VecDeque<ReadStep>>,
        /// Timeout visible to the fake at each read.
        read_timeouts: RefCell<Vec<Duration>>,
        /// Buffer length supplied to each read.
        read_buffer_sizes: RefCell<Vec<usize>>,
        /// Number of low-level writes issued to the fake.
        write_calls: Arc<AtomicUsize>,
        /// Complete byte sequences submitted to the fake, in transmission order.
        writes: RefCell<Vec<Vec<u8>>>,
        /// Number of flush calls issued to the fake.
        flush_calls: Arc<AtomicUsize>,
        /// Scripted write outcomes, used by partial-write deadline tests.
        write_steps: RefCell<VecDeque<WriteStep>>,
        /// Fail once when this timeout is requested.
        fail_next_timeout_set_to: RefCell<Option<Duration>>,
        /// Number of input-buffer discards.
        input_clears: RefCell<usize>,
    }

    impl TestSerialPort {
        fn new(initial_timeout: Duration) -> Self {
            Self {
                timeout: RefCell::new(initial_timeout),
                timeout_history: RefCell::new(Vec::new()),
                timeout_history_observer: Arc::new(Mutex::new(Vec::new())),
                write_error: RefCell::new(None),
                flush_error: RefCell::new(None),
                read_data: RefCell::new(Vec::new()),
                read_steps: RefCell::new(VecDeque::new()),
                read_timeouts: RefCell::new(Vec::new()),
                read_buffer_sizes: RefCell::new(Vec::new()),
                write_calls: Arc::new(AtomicUsize::new(0)),
                writes: RefCell::new(Vec::new()),
                flush_calls: Arc::new(AtomicUsize::new(0)),
                write_steps: RefCell::new(VecDeque::new()),
                fail_next_timeout_set_to: RefCell::new(None),
                input_clears: RefCell::new(0),
            }
        }

        fn with_write_error(self, kind: ErrorKind) -> Self {
            *self.write_error.borrow_mut() = Some(kind);
            self
        }

        fn with_flush_error(self, kind: ErrorKind) -> Self {
            *self.flush_error.borrow_mut() = Some(kind);
            self
        }

        fn with_read_steps(self, steps: impl IntoIterator<Item = ReadStep>) -> Self {
            self.read_steps.borrow_mut().extend(steps);
            self
        }

        fn with_write_steps(self, steps: impl IntoIterator<Item = WriteStep>) -> Self {
            self.write_steps.borrow_mut().extend(steps);
            self
        }

        fn with_next_timeout_set_failure(self, timeout: Duration) -> Self {
            *self.fail_next_timeout_set_to.borrow_mut() = Some(timeout);
            self
        }
    }

    impl std::fmt::Debug for TestSerialPort {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("TestSerialPort")
                .field("timeout", &self.timeout)
                .finish()
        }
    }

    impl Read for TestSerialPort {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.read_timeouts.borrow_mut().push(self.timeout());
            self.read_buffer_sizes.borrow_mut().push(buf.len());

            let step = { self.read_steps.borrow_mut().pop_front() };
            if let Some(step) = step {
                return match step {
                    ReadStep::Error(kind) => Err(io::Error::new(kind, "simulated read error")),
                    ReadStep::Bytes(mut bytes) => {
                        let n = std::cmp::min(buf.len(), bytes.len());
                        buf[..n].copy_from_slice(&bytes[..n]);
                        if n < bytes.len() {
                            bytes.drain(..n);
                            self.read_steps
                                .borrow_mut()
                                .push_front(ReadStep::Bytes(bytes));
                        }
                        Ok(n)
                    }
                };
            }

            let mut data = self.read_data.borrow_mut();
            let n = std::cmp::min(buf.len(), data.len());
            if n > 0 {
                buf[..n].copy_from_slice(&data[..n]);
                data.drain(..n);
            }
            Ok(n)
        }
    }

    impl io::Write for TestSerialPort {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if let Some(kind) = *self.write_error.borrow() {
                return Err(io::Error::new(kind, "simulated write error"));
            }
            self.write_calls.fetch_add(1, Ordering::SeqCst);
            self.writes.borrow_mut().push(buf.to_vec());

            let step = { self.write_steps.borrow_mut().pop_front() };
            if let Some(step) = step {
                return match step {
                    WriteStep::Error(kind) => Err(io::Error::new(kind, "simulated write error")),
                    WriteStep::Partial { bytes, delay } => {
                        thread::sleep(delay);
                        Ok(bytes.min(buf.len()))
                    }
                    WriteStep::OverReport => Ok(buf.len() + 1),
                };
            }

            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            self.flush_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(kind) = *self.flush_error.borrow() {
                return Err(io::Error::new(kind, "simulated flush error"));
            }
            Ok(())
        }
    }

    impl SerialPort for TestSerialPort {
        fn name(&self) -> Option<String> {
            Some("TestPort".to_string())
        }

        fn baud_rate(&self) -> serialport::Result<u32> {
            Ok(9600)
        }

        fn data_bits(&self) -> serialport::Result<DataBits> {
            Ok(DataBits::Eight)
        }

        fn flow_control(&self) -> serialport::Result<FlowControl> {
            Ok(FlowControl::None)
        }

        fn parity(&self) -> serialport::Result<Parity> {
            Ok(Parity::None)
        }

        fn stop_bits(&self) -> serialport::Result<StopBits> {
            Ok(StopBits::One)
        }

        fn timeout(&self) -> Duration {
            *self.timeout.borrow()
        }

        fn set_timeout(&mut self, timeout: Duration) -> serialport::Result<()> {
            let should_fail = self
                .fail_next_timeout_set_to
                .borrow()
                .is_some_and(|expected| expected == timeout);
            if should_fail {
                *self.fail_next_timeout_set_to.borrow_mut() = None;
                return Err(serialport::Error::new(
                    serialport::ErrorKind::Unknown,
                    "simulated timeout setting error",
                ));
            }

            *self.timeout.borrow_mut() = timeout;
            self.timeout_history.borrow_mut().push(timeout);
            self.timeout_history_observer
                .lock()
                .expect("timeout observer lock is not poisoned")
                .push(timeout);
            Ok(())
        }

        fn set_baud_rate(&mut self, _: u32) -> serialport::Result<()> {
            Ok(())
        }

        fn set_data_bits(&mut self, _: DataBits) -> serialport::Result<()> {
            Ok(())
        }

        fn set_flow_control(&mut self, _: FlowControl) -> serialport::Result<()> {
            Ok(())
        }

        fn set_parity(&mut self, _: Parity) -> serialport::Result<()> {
            Ok(())
        }

        fn set_stop_bits(&mut self, _: StopBits) -> serialport::Result<()> {
            Ok(())
        }

        fn write_request_to_send(&mut self, _: bool) -> serialport::Result<()> {
            Ok(())
        }

        fn write_data_terminal_ready(&mut self, _: bool) -> serialport::Result<()> {
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
            Ok(self.read_data.borrow().len() as u32)
        }

        fn bytes_to_write(&self) -> serialport::Result<u32> {
            Ok(0)
        }

        fn clear(&self, buffer: ClearBuffer) -> serialport::Result<()> {
            if matches!(buffer, ClearBuffer::Input) {
                *self.input_clears.borrow_mut() += 1;
            }
            Ok(())
        }

        fn try_clone(&self) -> serialport::Result<Box<dyn SerialPort>> {
            Err(serialport::Error::new(
                serialport::ErrorKind::Unknown,
                "Clone not supported for TestSerialPort",
            ))
        }

        fn set_break(&self) -> serialport::Result<()> {
            Ok(())
        }

        fn clear_break(&self) -> serialport::Result<()> {
            Ok(())
        }
    }

    /// Helper to create a SerialTransport with a test port directly.
    fn create_test_transport(port: TestSerialPort) -> SerialTransport {
        let config = SerialConfig::new("/dev/test");
        let transport_config = config.transport_config();
        SerialTransport {
            port: Box::new(port),
            config,
            transport_config,
            addressed_bus: None,
        }
    }

    /// The production timing with a different Address Set attempt budget.
    fn timing(address_set_attempt: Duration) -> StartupTiming {
        StartupTiming {
            address_set_attempt,
            ..StartupTiming::VISCA
        }
    }

    /// An Address Set startup whose read timeout matches the fake port's.
    fn address_set(read_timeout: Duration, write_timeout: Duration) -> SerialConfig {
        SerialConfig::new("/dev/test")
            .startup(Startup::default().with_address_set(true))
            .read_timeout(read_timeout)
            .write_timeout(write_timeout)
    }

    #[test]
    fn test_serial_config_default() {
        let config = SerialConfig::default();
        assert_eq!(config.baud_rate, 9600);
        assert!(!config.startup.address_set);
        assert!(!config.startup.interface_clear);
        assert_eq!(config.buffer_config, BufferConfig::for_serial());
    }

    #[test]
    fn invalid_buffer_bounds_fail_before_serial_device_open() {
        let config = SerialConfig::new("grafton-visca-invalid-buffer-bounds-serial-device")
            .buffer_config(BufferConfig {
                recv_buffer_size: 65,
                max_buffer_size: 64,
            });

        let result = SerialTransport::new(config);

        assert!(matches!(
            result,
            Err(Error::InvalidRequest(actual))
                if actual.as_ref() == "transport receive buffer cannot exceed maximum buffer"
        ));
    }

    #[test]
    fn zero_io_timeouts_fail_before_serial_device_open() {
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
                SerialTransport::new(config),
                Err(Error::InvalidRequest(actual)) if actual.as_ref() == message
            ));
        }
    }

    #[test]
    fn an_absent_port_is_a_connection_failure_naming_the_port() {
        let config = SerialConfig::new("/dev/grafton-visca-absent-serial-device");
        let error = SerialTransport::new(config).expect_err("an absent port cannot open");
        assert!(matches!(
            error,
            Error::ConnectionFailed { ref addr, .. }
                if addr == "/dev/grafton-visca-absent-serial-device"
        ));
    }

    #[test]
    fn address_set_retries_an_interrupted_write_in_place() {
        let configured_read_timeout = Duration::from_millis(50);
        let mut port = TestSerialPort::new(configured_read_timeout)
            .with_write_steps([WriteStep::Error(ErrorKind::Interrupted)])
            .with_read_steps([ReadStep::Bytes(vec![0x88, 0x30, 0x02, VISCA_TERMINATOR])]);

        perform_startup(
            &mut port,
            &address_set(configured_read_timeout, Duration::from_millis(7)),
            timing(Duration::from_secs(1)),
        )
        .expect("an interrupted handshake write retries in place");
        assert_eq!(port.write_calls.load(Ordering::SeqCst), 2);
    }

    /// #797: an expired partial write leaves an unknowable bus position, so it
    /// ends startup instead of starting another attempt.
    #[test]
    fn a_partial_address_set_write_is_not_resent() {
        let configured_read_timeout = Duration::from_millis(50);
        let mut port =
            TestSerialPort::new(configured_read_timeout).with_write_steps([WriteStep::Partial {
                bytes: 1,
                delay: Duration::from_millis(100),
            }]);

        let result = perform_startup(
            &mut port,
            &address_set(configured_read_timeout, Duration::from_secs(1)),
            timing(Duration::from_millis(50)),
        );

        assert!(matches!(result, Err(Error::Timeout { .. })));
        assert_eq!(
            port.write_calls.load(Ordering::SeqCst),
            1,
            "the expired partial write starts no follow-up syscall and no new attempt"
        );
        assert!(port.read_timeouts.borrow().is_empty());
    }

    #[test]
    fn address_set_discards_an_oversized_noise_frame_before_retrying() {
        let configured_read_timeout = Duration::from_millis(50);
        let mut oversized_noise = vec![0x55; 257];
        oversized_noise.push(VISCA_TERMINATOR);
        let mut port = TestSerialPort::new(configured_read_timeout).with_read_steps([
            ReadStep::Bytes(oversized_noise),
            ReadStep::Error(ErrorKind::TimedOut),
            ReadStep::Bytes(vec![0x88, 0x30, 0x02, VISCA_TERMINATOR]),
        ]);

        perform_startup(
            &mut port,
            &address_set(configured_read_timeout, Duration::from_millis(7)),
            timing(Duration::from_millis(20)),
        )
        .expect("resynchronized Address Set reaches a later attempt");
        assert_eq!(
            port.write_calls.load(Ordering::SeqCst),
            2,
            "the oversized frame spends only one Address Set attempt"
        );
    }

    #[test]
    fn address_set_caps_a_long_read_timeout_to_the_remaining_deadline() {
        let configured_read_timeout = Duration::from_secs(1);
        let configured_write_timeout = Duration::from_millis(7);
        let attempt_timeout = Duration::from_millis(50);
        let mut port = TestSerialPort::new(configured_read_timeout)
            .with_read_steps([ReadStep::Bytes(vec![0x88, 0x30, 0x02, VISCA_TERMINATOR])]);

        perform_startup(
            &mut port,
            &address_set(configured_read_timeout, configured_write_timeout),
            timing(attempt_timeout),
        )
        .expect("Address Set succeeds");

        let read_timeouts = port.read_timeouts.borrow();
        assert_eq!(read_timeouts.len(), 1);
        assert!(
            read_timeouts[0] <= attempt_timeout,
            "the read must not be allowed to outlive the attempt budget"
        );
        let history = port.timeout_history.borrow();
        assert!(
            history[0] <= configured_write_timeout,
            "the Address Set write is bounded by the configured write timeout"
        );
    }

    #[test]
    fn a_zero_attempt_budget_performs_no_io() {
        let configured_read_timeout = Duration::from_millis(50);
        let mut port = TestSerialPort::new(configured_read_timeout);

        let result = perform_startup(
            &mut port,
            &address_set(configured_read_timeout, Duration::from_millis(7)),
            timing(Duration::ZERO),
        );

        assert!(matches!(result, Err(Error::MaxRetriesExceeded)));
        assert_eq!(port.write_calls.load(Ordering::SeqCst), 0);
        assert!(port.read_timeouts.borrow().is_empty());
        assert!(port.timeout_history.borrow().is_empty());
    }

    #[test]
    fn startup_never_calls_an_unbounded_serial_flush() {
        let configured_timeout = Duration::from_millis(50);
        let mut address_port = TestSerialPort::new(configured_timeout)
            .with_flush_error(ErrorKind::TimedOut)
            .with_read_steps([ReadStep::Bytes(vec![0x88, 0x30, 0x02, VISCA_TERMINATOR])]);
        perform_startup(
            &mut address_port,
            &address_set(configured_timeout, Duration::from_millis(7)),
            StartupTiming::VISCA,
        )
        .expect("Address Set succeeds");
        assert_eq!(address_port.flush_calls.load(Ordering::SeqCst), 0);

        let mut clear_port =
            TestSerialPort::new(configured_timeout).with_flush_error(ErrorKind::TimedOut);
        perform_startup(
            &mut clear_port,
            &SerialConfig::new("/dev/test").startup(Startup::default().with_interface_clear(true)),
            StartupTiming::VISCA,
        )
        .expect("I/F Clear succeeds");
        assert_eq!(clear_port.flush_calls.load(Ordering::SeqCst), 0);
    }

    /// #800: failing to arm a timeout is `Error::Io` on serial devices, as it
    /// is on sockets.
    #[test]
    fn failing_to_arm_a_device_timeout_is_an_io_error() {
        let read_timeout = Duration::from_millis(100);
        let port =
            TestSerialPort::new(Duration::from_secs(5)).with_next_timeout_set_failure(read_timeout);
        let mut transport = create_test_transport(port);

        let mut buf = [0u8; 16];
        let result = transport.recv_into_with_timeout(&mut buf, read_timeout);
        assert!(matches!(result, Err(Error::Io(_))));
    }

    /// #798/#800: a zero-byte read is end of stream on every stream transport.
    #[test]
    fn a_zero_byte_read_is_a_closed_serial_port() {
        let mut transport = create_test_transport(TestSerialPort::new(Duration::from_secs(5)));
        let mut buf = [0u8; 16];
        assert!(matches!(
            transport.recv_into_with_timeout(&mut buf, Duration::from_millis(10)),
            Err(Error::ConnectionClosed { reason: Some(ref reason) })
                if reason == SERIAL_PORT_CLOSED
        ));
    }

    /// #800: the shared write loop rejects a write claiming more bytes than
    /// it was given, for serial exactly as for TCP.
    #[test]
    fn an_over_reported_serial_write_is_rejected() {
        let port =
            TestSerialPort::new(Duration::from_secs(5)).with_write_steps([WriteStep::OverReport]);
        let mut transport = create_test_transport(port);
        let result = transport.send_with_timeout(
            &[0x81, 0x01, VISCA_TERMINATOR],
            CommandKind::Command,
            Duration::from_millis(100),
        );
        assert!(matches!(
            result,
            Err(Error::Io(ref error)) if error.kind() == ErrorKind::InvalidData
        ));
    }

    #[test]
    fn a_failed_write_is_reported_as_io() {
        let original_timeout = Duration::from_secs(5);
        let port = TestSerialPort::new(original_timeout).with_write_error(ErrorKind::BrokenPipe);
        let mut transport = create_test_transport(port);

        let result = transport.send_with_timeout(
            &[0x81, 0x01, VISCA_TERMINATOR],
            CommandKind::Command,
            Duration::from_millis(100),
        );
        assert!(matches!(
            result,
            Err(Error::Io(ref error)) if error.kind() == ErrorKind::BrokenPipe
        ));
    }

    /// The startup transcripts shared with the Tokio driver.
    #[test]
    fn shared_startup_scenarios() {
        for scenario in scenarios::all() {
            let reads = scenario.reads.iter().map(|read| match read {
                ScriptRead::Bytes(bytes) => ReadStep::Bytes(bytes.clone()),
                ScriptRead::Idle => ReadStep::Error(ErrorKind::TimedOut),
                ScriptRead::Fails(kind) => ReadStep::Error(*kind),
            });
            let writes = scenario.writes.iter().map(|write| match write {
                ScriptWrite::Accepted => WriteStep::Partial {
                    bytes: usize::MAX,
                    delay: Duration::ZERO,
                },
                ScriptWrite::TimesOut => WriteStep::Error(ErrorKind::TimedOut),
            });
            let mut port = TestSerialPort::new(Duration::from_millis(50))
                .with_read_steps(reads)
                .with_write_steps(writes);

            let result = perform_startup(&mut port, &scenario.config, scenarios::TIMING);

            scenarios::check(&scenario, &result);
            assert_eq!(
                port.writes.borrow().as_slice(),
                scenario.expect_writes.as_slice(),
                "{}",
                scenario.name
            );
            assert_eq!(
                *port.input_clears.borrow() == 1,
                scenario.discards_input,
                "{}",
                scenario.name
            );
        }
    }

    /// Address Set's camera count is checked against the registered cameras
    /// on a real (pseudo-terminal) port: a camera the chain did not address
    /// fails the open, and an addressed one starts a session.
    #[cfg(unix)]
    #[test]
    #[cfg_attr(miri, ignore = "requires a pseudo-terminal")]
    fn registered_cameras_must_have_been_addressed() {
        use std::io::{Read as _, Write as _};

        use crate::{camera::CameraConfig, profiles::PtzOpticsG2, CameraId};

        for (camera, expect_open) in [
            (CameraId::CAMERA_1, true),
            (CameraId::new(2).unwrap(), false),
        ] {
            let (mut master, slave) = serialport::TTYPort::pair().expect("pseudo-terminal pair");
            let path = slave.name().expect("pseudo-terminal path");
            master
                .set_timeout(Duration::from_secs(5))
                .expect("master timeout");
            let bus = thread::spawn(move || {
                // One camera answers Address Set.
                let mut frame = [0u8; 4];
                master.read_exact(&mut frame).expect("Address Set frame");
                assert_eq!(frame, [0x88, 0x30, 0x01, VISCA_TERMINATOR]);
                master
                    .write_all(&[0x88, 0x30, 0x02, VISCA_TERMINATOR])
                    .expect("Address Set reply");
                master
            });

            let result = CameraConfig::<PtzOpticsG2>::serial(path.clone(), 9_600)
                .camera_id(camera)
                .serial_startup(Startup::default().with_address_set(true))
                .open_serial();
            let _master = bus.join().expect("bus thread");
            drop(slave);

            match result {
                Ok(session) => {
                    assert!(expect_open, "camera {camera:?} was not addressed");
                    session.close().expect("clean close");
                }
                Err(error) => {
                    assert!(!expect_open, "camera {camera:?} should open: {error:?}");
                    assert!(matches!(
                        error,
                        Error::ConnectionFailed { ref addr, ref source }
                            if *addr == path
                                && source.kind() == ErrorKind::NotFound
                                && source.to_string()
                                    == "camera 2 was not addressed by Address Set (chain reported 1)"
                    ));
                }
            }
        }
    }

    /// `send_with_timeout` must not deadlock: an earlier implementation
    /// locked an internal mutex and then re-entered it.
    #[test]
    fn test_send_with_timeout_does_not_deadlock() {
        let mut transport = create_test_transport(TestSerialPort::new(Duration::from_secs(5)));
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = thread::spawn(move || {
            let result = transport.send_with_timeout(
                &[0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR],
                CommandKind::Command,
                Duration::from_millis(100),
            );
            tx.send(result).ok();
            transport
        });
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(result) => assert!(
                result.is_ok(),
                "send_with_timeout should succeed: {result:?}"
            ),
            Err(error) => panic!("send_with_timeout did not complete within 500ms: {error}"),
        }
        let _transport = handle.join().expect("Thread should complete");
    }

    #[test]
    fn send_with_timeout_does_not_call_unbounded_serial_flush() {
        let port =
            TestSerialPort::new(Duration::from_secs(5)).with_flush_error(ErrorKind::BrokenPipe);
        let flush_calls = Arc::clone(&port.flush_calls);
        let mut transport = create_test_transport(port);

        // The configured flush failure is never observed because command
        // submission must not enter an unbounded device-drain syscall.
        transport
            .send_with_timeout(
                &[0x81, 0x01, VISCA_TERMINATOR],
                CommandKind::Command,
                Duration::from_millis(100),
            )
            .expect("send succeeds without flushing");
        assert_eq!(flush_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn send_with_timeout_does_not_start_another_partial_write_after_deadline() {
        let original_timeout = Duration::from_secs(5);
        let port = TestSerialPort::new(original_timeout).with_write_steps([WriteStep::Partial {
            bytes: 1,
            delay: Duration::from_millis(40),
        }]);
        let write_calls = Arc::clone(&port.write_calls);
        let flush_calls = Arc::clone(&port.flush_calls);
        let mut transport = create_test_transport(port);

        let result = transport.send_with_timeout(
            &[0x81, 0x01, VISCA_TERMINATOR],
            CommandKind::Command,
            Duration::from_millis(20),
        );

        assert!(matches!(result, Err(Error::Timeout { .. })));
        assert_eq!(
            write_calls.load(Ordering::SeqCst),
            1,
            "an expired whole-write budget must prevent a follow-up syscall"
        );
        assert_eq!(flush_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn send_with_timeout_reports_a_completed_last_chunk_after_deadline() {
        let original_timeout = Duration::from_secs(5);
        let port = TestSerialPort::new(original_timeout).with_write_steps([WriteStep::Partial {
            bytes: 6,
            delay: Duration::from_millis(40),
        }]);
        let write_calls = Arc::clone(&port.write_calls);
        let mut transport = create_test_transport(port);

        // This is the wire form of a pan/tilt stop. The low-level writer accepts
        // the complete frame before returning, even though it returns after the
        // logical deadline. The command is already on the wire and must not be
        // reported as a timeout (which would poison a stream session).
        let stop = [0x81, 0x01, 0x06, 0x01, 0x03, VISCA_TERMINATOR];
        let result =
            transport.send_with_timeout(&stop, CommandKind::Command, Duration::from_millis(20));

        assert!(result.is_ok(), "a fully written frame was delivered");
        assert_eq!(write_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn serial_device_timeouts_are_never_less_than_one_millisecond() {
        let port = TestSerialPort::new(Duration::from_micros(500));
        let history = Arc::clone(&port.timeout_history_observer);
        let mut transport = create_test_transport(port);

        // A budget the Windows serial backend would otherwise truncate to
        // zero milliseconds, on both the write and the read path.
        transport
            .send_with_timeout(
                &[0x81, 0x01, VISCA_TERMINATOR],
                CommandKind::Command,
                Duration::from_micros(500),
            )
            .expect("the fake accepts the bounded write");
        let mut buf = [0u8; 4];
        let _ = transport.recv_into_with_timeout(&mut buf, Duration::from_nanos(1));

        let history = history
            .lock()
            .expect("timeout observer lock is not poisoned");
        assert!(!history.is_empty());
        assert!(
            history
                .iter()
                .all(|timeout| *timeout >= Duration::from_millis(1)),
            "no serial-device timeout may be passed below the one-millisecond floor"
        );
    }

    /// The port is opened through the shared builder in exclusive mode, so a
    /// second opener of the same device fails as a connection failure.
    #[cfg(unix)]
    #[test]
    #[cfg_attr(miri, ignore = "requires a pseudo-terminal")]
    fn a_held_port_cannot_be_opened_twice() {
        let (_master, slave) = serialport::TTYPort::pair().expect("pseudo-terminal pair");
        let path = slave.name().expect("pseudo-terminal path");
        let config = SerialConfig::new(path.clone());

        let first = SerialTransport::new(config.clone()).expect("first opener succeeds");
        assert_eq!(first.transport_config(), &config.transport_config());

        let second = SerialTransport::new(config).expect_err("the port is held exclusively");
        assert!(matches!(
            second,
            Error::ConnectionFailed { ref addr, .. } if *addr == path
        ));
    }
}
