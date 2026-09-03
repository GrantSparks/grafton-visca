//! Serial transport for VISCA over RS-232/422.
//!
//! This module provides serial communication for VISCA protocol,
//! supporting both RS-232 and RS-422 connections with proper
//! Address Set and I/F Clear initialization.

use tracing::trace;

use std::time::{Duration, Instant};

use crate::{
    command::CommandKind,
    error::{Error, Result},
    transport::{
        builder::{AddressingMode, TransportConfig},
        serial::{
            handshake::blocking_handshake::{address_set_blocking, if_clear_blocking},
            startup_plan, Config as SerialConfig, StartupOperation,
        },
        BlockingTransport, HasTransportConfig,
    },
};

/// Serial transport implementation for blocking I/O.
///
/// This transport operates at the stream level, reading/writing raw bytes.
/// Framing and retry logic are handled by the runtime layer.
///
/// The blocking transport uses exclusive ownership (`&mut self`) for all operations,
/// matching the design of TCP and UDP blocking transports. This eliminates the
/// need for internal synchronization and prevents self-deadlock issues.
pub struct SerialTransport {
    port: Box<dyn serialport::SerialPort + Send>,
    config: SerialConfig,
    transport_config: TransportConfig,
}

impl std::fmt::Debug for SerialTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerialTransport")
            .field("port", &self.config.port)
            .field("baud_rate", &self.config.baud_rate)
            .finish_non_exhaustive()
    }
}

/// RAII guard that restores the serial port timeout on drop.
///
/// This ensures timeout restoration happens even if an early return via `?`
/// occurs in the write/read path.
struct TimeoutGuard<'a> {
    port: &'a mut dyn serialport::SerialPort,
    original_timeout: Duration,
}

impl<'a> TimeoutGuard<'a> {
    /// Create a new timeout guard, saving the current timeout and setting a new one.
    fn new(port: &'a mut dyn serialport::SerialPort, new_timeout: Duration) -> Result<Self> {
        let original_timeout = port.timeout();
        port.set_timeout(new_timeout)
            .map_err(|e| Error::TransportError(format!("Failed to set timeout: {e}").into()))?;
        Ok(Self {
            port,
            original_timeout,
        })
    }
}

impl Drop for TimeoutGuard<'_> {
    fn drop(&mut self) {
        // Best-effort restoration - log if it fails but don't panic
        if let Err(e) = self.port.set_timeout(self.original_timeout) {
            trace!("Failed to restore serial port timeout: {e}");
        }
    }
}

impl SerialTransport {
    /// Create a new serial transport with the given configuration.
    pub fn new(config: SerialConfig) -> Result<Self> {
        // Build and validate the transport configuration before opening the
        // serial device. Canonical CameraConfig does this during preflight,
        // but this lower-level initializer also serves direct library paths.
        let transport_config = TransportConfig {
            read_timeout: config.read_timeout,
            write_timeout: config.write_timeout,
            buffer_config: config.buffer_config,
            addressing: AddressingMode::Serial, // Serial transport uses Serial addressing
            ..Default::default()
        };
        transport_config.validate()?;

        // Open serial port
        let mut port = serialport::new(&config.port, config.baud_rate)
            .timeout(config.read_timeout)
            .open()
            .map_err(|e| {
                Error::TransportError(format!("Failed to open serial port: {e}").into())
            })?;

        perform_startup_handshakes(&mut *port, &config)?;

        Ok(Self {
            port,
            config,
            transport_config,
        })
    }
}

/// Perform the requested serial bus startup operations in protocol order.
fn perform_startup_handshakes(
    port: &mut dyn serialport::SerialPort,
    config: &SerialConfig,
) -> Result<()> {
    for operation in startup_plan(config).into_iter().flatten() {
        match operation {
            StartupOperation::AddressSet => {
                address_set_blocking(
                    port,
                    Duration::from_secs(2),
                    config.write_timeout,
                    config.buffer_config,
                )?;
            }
            StartupOperation::InterfaceClear => {
                if_clear_blocking(port, config.write_timeout)?;
            }
        }
    }

    Ok(())
}

impl HasTransportConfig for SerialTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.transport_config
    }

    fn standard_transport_kind(&self) -> Option<crate::camera::TransportKind> {
        Some(crate::camera::TransportKind::Serial)
    }
}

impl BlockingTransport for SerialTransport {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: CommandKind,
        timeout: Duration,
    ) -> Result<()> {
        // Apply write timeout using RAII guard for guaranteed restoration
        let guard = TimeoutGuard::new(&mut *self.port, timeout)?;
        let started = Instant::now();
        let mut written = 0;

        while written < bytes.len() {
            // Keep one deadline across partial writes. `write_all` would give
            // each low-level write the complete timeout again.
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(Error::Timeout);
            }
            guard.port.set_timeout(remaining).map_err(|error| {
                Error::TransportError(format!("Failed to set write timeout: {error}").into())
            })?;

            match guard.port.write(&bytes[written..]) {
                Ok(0) => {
                    return Err(Error::TransportError(
                        "Serial write made no progress".into(),
                    ));
                }
                Ok(count) if count <= bytes.len() - written => written += count,
                Ok(count) => {
                    return Err(Error::TransportError(
                        format!(
                            "Serial write reported {count} bytes for a {}-byte buffer",
                            bytes.len() - written
                        )
                        .into(),
                    ));
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
                {
                    return Err(Error::Timeout);
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    return Err(Error::TransportError(
                        format!("Serial write error: {error}").into(),
                    ));
                }
            }
        }

        if timeout.saturating_sub(started.elapsed()).is_zero() {
            return Err(Error::Timeout);
        }

        // Do not call `SerialPort::flush`: on POSIX it is `tcdrain`, which can
        // remain blocked after the write timeout. The subsequent VISCA reply
        // wait is the protocol-level confirmation that the queued bytes left.
        trace!("Queued {} serial bytes: {:02X?}", bytes.len(), bytes);
        // TimeoutGuard restores original timeout on drop
        Ok(())
    }

    fn recv_into_with_timeout(&mut self, dst: &mut [u8], timeout: Duration) -> Result<usize> {
        // Use RAII guard for guaranteed timeout restoration
        let guard = TimeoutGuard::new(&mut *self.port, timeout)?;

        // Read into the provided buffer
        match guard.port.read(dst) {
            Ok(n) => {
                trace!("Read {} bytes from serial port", n);
                Ok(n)
            }
            Err(io_err)
                if io_err.kind() == std::io::ErrorKind::TimedOut
                    || io_err.kind() == std::io::ErrorKind::WouldBlock =>
            {
                Err(Error::Timeout)
            }
            Err(io_err) => Err(io_err.into()),
        }
        // TimeoutGuard restores original timeout on drop
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(AddressingMode::Serial)
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::transport::serial::handshake::blocking_handshake::{
        address_set_blocking, if_clear_blocking,
    };
    use crate::transport::BufferConfig;
    use serialport::{ClearBuffer, DataBits, FlowControl, Parity, SerialPort, StopBits};
    use std::io::{self, ErrorKind, Read};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    };
    use std::thread;
    use std::{cell::RefCell, collections::VecDeque};

    #[test]
    fn test_serial_config_default() {
        let config = SerialConfig::default();
        assert_eq!(config.baud_rate, 9600);
        assert_eq!(config.camera_address, 1);
        assert!(config.if_clear_on_connect);
        assert!(!config.address_set_on_connect);
        assert_eq!(config.buffer_config, BufferConfig::for_serial());
    }

    #[test]
    fn invalid_buffer_bounds_fail_before_serial_device_open() {
        let config = SerialConfig::new("grafton-visca-invalid-buffer-bounds-serial-device")
            .if_clear_on_connect(false)
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
                SerialTransport::new(config),
                Err(Error::InvalidRequest(actual)) if actual.as_ref() == message
            ));
        }
    }

    /// One deterministic fake serial read outcome.
    enum ReadStep {
        Error(ErrorKind),
        Bytes(Vec<u8>),
    }

    /// One deterministic fake serial write outcome.
    enum WriteStep {
        Partial { bytes: usize, delay: Duration },
    }

    /// A mock serial port for testing that records all operations.
    struct TestSerialPort {
        /// Current timeout setting.
        timeout: RefCell<Duration>,
        /// Every timeout applied through the serial-port API.
        timeout_history: RefCell<Vec<Duration>>,
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
        /// Fail once when this timeout is requested, then let Drop retry it.
        fail_next_timeout_set_to: RefCell<Option<Duration>>,
    }

    impl TestSerialPort {
        fn new(initial_timeout: Duration) -> Self {
            Self {
                timeout: RefCell::new(initial_timeout),
                timeout_history: RefCell::new(Vec::new()),
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
            if let Some(WriteStep::Partial { bytes, delay }) = step {
                thread::sleep(delay);
                return Ok(bytes.min(buf.len()));
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
                    "simulated timeout restoration error",
                ));
            }

            *self.timeout.borrow_mut() = timeout;
            self.timeout_history.borrow_mut().push(timeout);
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

        fn clear(&self, _: ClearBuffer) -> serialport::Result<()> {
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
        let config = SerialConfig {
            port: "/dev/test".to_string(),
            if_clear_on_connect: false,
            address_set_on_connect: false,
            ..Default::default()
        };
        let transport_config = TransportConfig {
            read_timeout: config.read_timeout,
            write_timeout: config.write_timeout,
            buffer_config: config.buffer_config,
            addressing: AddressingMode::Serial,
            ..Default::default()
        };
        SerialTransport {
            port: Box::new(port),
            config,
            transport_config,
        }
    }

    #[test]
    fn address_set_keeps_one_attempt_after_an_early_read_timeout() {
        let configured_read_timeout = Duration::from_millis(50);
        let configured_write_timeout = Duration::from_millis(7);
        let mut port = TestSerialPort::new(configured_read_timeout).with_read_steps([
            ReadStep::Error(ErrorKind::TimedOut),
            ReadStep::Bytes(vec![0x88, 0x30, 0x02, VISCA_TERMINATOR]),
        ]);

        let result = address_set_blocking(
            &mut port,
            Duration::from_secs(1),
            configured_write_timeout,
            BufferConfig::for_serial(),
        );

        assert!(matches!(result, Ok(1)));
        assert_eq!(
            port.write_calls.load(Ordering::SeqCst),
            1,
            "an early idle timeout must not spend an Address Set retry"
        );
        assert_eq!(
            port.read_timeouts.borrow().as_slice(),
            &[configured_read_timeout, configured_read_timeout]
        );
        assert_eq!(port.timeout(), configured_read_timeout);
    }

    #[test]
    fn address_set_caps_a_long_port_read_timeout_to_the_remaining_deadline() {
        let configured_read_timeout = Duration::from_secs(1);
        let configured_write_timeout = Duration::from_millis(7);
        let attempt_timeout = Duration::from_millis(50);
        let mut port = TestSerialPort::new(configured_read_timeout)
            .with_read_steps([ReadStep::Bytes(vec![0x88, 0x30, 0x02, VISCA_TERMINATOR])]);

        let result = address_set_blocking(
            &mut port,
            attempt_timeout,
            configured_write_timeout,
            BufferConfig::for_serial(),
        );

        assert!(matches!(result, Ok(1)));
        let read_timeouts = port.read_timeouts.borrow();
        assert_eq!(read_timeouts.len(), 1);
        assert!(
            read_timeouts[0] <= attempt_timeout,
            "the read must not be allowed to outlive the attempt budget"
        );
        assert!(
            read_timeouts[0] < configured_read_timeout,
            "the configured port timeout is longer than the remaining budget"
        );
        assert_eq!(
            port.timeout_history
                .borrow()
                .iter()
                .filter(|&&timeout| timeout == configured_write_timeout)
                .count(),
            1,
            "Address Set write uses the configured write timeout"
        );
        assert_eq!(port.timeout(), configured_read_timeout);
    }

    #[test]
    fn address_set_does_not_start_a_second_partial_write_after_its_deadline() {
        let configured_read_timeout = Duration::from_millis(50);
        let mut port =
            TestSerialPort::new(configured_read_timeout).with_write_steps([WriteStep::Partial {
                bytes: 1,
                delay: Duration::from_millis(100),
            }]);

        let result = address_set_blocking(
            &mut port,
            Duration::from_millis(50),
            Duration::from_secs(1),
            BufferConfig::for_serial(),
        );

        assert!(matches!(result, Err(Error::Timeout)));
        assert_eq!(
            port.write_calls.load(Ordering::SeqCst),
            1,
            "the expired budget must prevent the follow-up low-level write"
        );
        assert!(port.read_timeouts.borrow().is_empty());
        assert_eq!(port.timeout(), configured_read_timeout);
    }

    #[test]
    fn address_set_with_a_zero_budget_performs_no_io() {
        let configured_read_timeout = Duration::from_millis(50);
        let mut port = TestSerialPort::new(configured_read_timeout);

        let result = address_set_blocking(
            &mut port,
            Duration::ZERO,
            Duration::from_millis(7),
            BufferConfig::for_serial(),
        );

        assert!(matches!(result, Err(Error::Timeout)));
        assert_eq!(port.write_calls.load(Ordering::SeqCst), 0);
        assert!(port.read_timeouts.borrow().is_empty());
        assert!(port.timeout_history.borrow().is_empty());
        assert_eq!(port.timeout(), configured_read_timeout);
    }

    #[test]
    fn blocking_handshakes_do_not_call_an_unbounded_serial_flush() {
        let configured_timeout = Duration::from_millis(50);
        let configured_write_timeout = Duration::from_millis(7);
        let mut address_port = TestSerialPort::new(configured_timeout)
            .with_flush_error(ErrorKind::TimedOut)
            .with_read_steps([ReadStep::Bytes(vec![0x88, 0x30, 0x02, VISCA_TERMINATOR])]);

        assert!(matches!(
            address_set_blocking(
                &mut address_port,
                Duration::from_secs(1),
                configured_write_timeout,
                BufferConfig::for_serial(),
            ),
            Ok(1)
        ));
        assert_eq!(address_port.flush_calls.load(Ordering::SeqCst), 0);

        let mut clear_port =
            TestSerialPort::new(configured_timeout).with_flush_error(ErrorKind::TimedOut);
        assert!(if_clear_blocking(&mut clear_port, configured_write_timeout).is_ok());
        assert_eq!(clear_port.flush_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn startup_with_address_set_and_if_clear_transmits_address_set_first() {
        let mut port = TestSerialPort::new(Duration::from_millis(50))
            .with_read_steps([ReadStep::Bytes(vec![0x88, 0x30, 0x02, VISCA_TERMINATOR])]);
        let config = SerialConfig::new("/dev/test")
            .address_set_on_connect(true)
            .if_clear_on_connect(true)
            .write_timeout(Duration::from_millis(7))
            .buffer_config(BufferConfig {
                recv_buffer_size: 4,
                max_buffer_size: 32,
            });

        perform_startup_handshakes(&mut port, &config).expect("startup handshakes succeed");

        assert_eq!(
            port.writes.borrow().as_slice(),
            [
                vec![0x88, 0x30, 0x01, VISCA_TERMINATOR],
                vec![0x88, 0x01, 0x00, 0x01, VISCA_TERMINATOR],
            ],
            "the blocking serial startup transcript must address the bus before clearing it"
        );
        assert_eq!(port.read_buffer_sizes.borrow().as_slice(), &[4]);
    }

    #[test]
    fn address_set_surfaces_timeout_restoration_failure_after_a_successful_write() {
        let configured_read_timeout = Duration::from_millis(50);
        let mut port = TestSerialPort::new(configured_read_timeout)
            .with_next_timeout_set_failure(configured_read_timeout);

        let result = address_set_blocking(
            &mut port,
            Duration::from_secs(1),
            Duration::from_millis(7),
            BufferConfig::for_serial(),
        );

        assert!(matches!(
            result,
            Err(Error::TransportError(message))
                if message
                    .as_ref()
                    .contains("Failed to restore serial handshake timeout")
        ));
        assert_eq!(port.write_calls.load(Ordering::SeqCst), 1);
        assert!(port.read_timeouts.borrow().is_empty());
        assert_eq!(
            port.timeout(),
            configured_read_timeout,
            "Drop retries restoration after the surfaced failure"
        );
    }

    // =========================================================================
    // Deadlock regression test
    // =========================================================================

    /// This test verifies that send_with_timeout does NOT deadlock.
    ///
    /// The old implementation would self-deadlock because:
    /// 1. send_with_timeout locked the mutex
    /// 2. Then called send_raw which tried to lock the same mutex
    ///
    /// This test spawns a thread to call send_with_timeout and waits with a timeout.
    /// If the implementation deadlocks, the test will fail on timeout.
    #[test]
    fn test_send_with_timeout_does_not_deadlock() {
        let port = TestSerialPort::new(Duration::from_secs(5));
        let mut transport = create_test_transport(port);

        let (tx, rx) = mpsc::channel();

        // Spawn a thread to call send_with_timeout
        let handle = thread::spawn(move || {
            let result = transport.send_with_timeout(
                &[0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR],
                CommandKind::Command,
                Duration::from_millis(100),
            );
            tx.send(result).ok();
            transport // Return transport so we can inspect it
        });

        // Wait for completion with a short timeout
        // If the old mutex-based implementation is in place, this will timeout
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(result) => {
                assert!(
                    result.is_ok(),
                    "send_with_timeout should succeed: {:?}",
                    result
                );
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("DEADLOCK DETECTED: send_with_timeout did not complete within 500ms");
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                panic!("Thread disconnected unexpectedly");
            }
        }

        // Clean up
        let _transport = handle.join().expect("Thread should complete");
    }

    // =========================================================================
    // Timeout restoration tests
    // =========================================================================

    #[test]
    fn test_send_with_timeout_restores_timeout_on_success() {
        let original_timeout = Duration::from_secs(5);
        let write_timeout = Duration::from_millis(100);

        let port = TestSerialPort::new(original_timeout);
        let mut transport = create_test_transport(port);

        // Override the write timeout in config
        transport.config.write_timeout = write_timeout;

        // Send should succeed
        let result = transport.send_with_timeout(
            &[0x81, 0x01, VISCA_TERMINATOR],
            CommandKind::Command,
            write_timeout,
        );
        assert!(result.is_ok());

        // Verify the timeout was changed and then restored
        // Access the port to check its final timeout
        assert_eq!(
            transport.port.timeout(),
            original_timeout,
            "Timeout should be restored to original after successful send"
        );
    }

    #[test]
    fn test_send_with_timeout_restores_timeout_on_write_error() {
        let original_timeout = Duration::from_secs(5);
        let write_timeout = Duration::from_millis(100);

        let port = TestSerialPort::new(original_timeout).with_write_error(ErrorKind::BrokenPipe);
        let mut transport = create_test_transport(port);
        transport.config.write_timeout = write_timeout;

        // Send should fail
        let result = transport.send_with_timeout(
            &[0x81, 0x01, VISCA_TERMINATOR],
            CommandKind::Command,
            write_timeout,
        );
        assert!(result.is_err());

        // Timeout should still be restored via RAII guard
        assert_eq!(
            transport.port.timeout(),
            original_timeout,
            "Timeout should be restored even after write error"
        );
    }

    #[test]
    fn send_with_timeout_does_not_call_unbounded_serial_flush() {
        let original_timeout = Duration::from_secs(5);
        let write_timeout = Duration::from_millis(100);

        let port = TestSerialPort::new(original_timeout).with_flush_error(ErrorKind::BrokenPipe);
        let flush_calls = Arc::clone(&port.flush_calls);
        let mut transport = create_test_transport(port);
        transport.config.write_timeout = write_timeout;

        // The configured flush failure is never observed because command
        // submission must not enter an unbounded device-drain syscall.
        let result = transport.send_with_timeout(
            &[0x81, 0x01, VISCA_TERMINATOR],
            CommandKind::Command,
            write_timeout,
        );
        assert!(result.is_ok());
        assert_eq!(flush_calls.load(Ordering::SeqCst), 0);

        assert_eq!(
            transport.port.timeout(),
            original_timeout,
            "Timeout should be restored after the bounded write"
        );
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

        assert!(matches!(result, Err(Error::Timeout)));
        assert_eq!(
            write_calls.load(Ordering::SeqCst),
            1,
            "an expired whole-write budget must prevent a follow-up syscall"
        );
        assert_eq!(flush_calls.load(Ordering::SeqCst), 0);
        assert_eq!(transport.port.timeout(), original_timeout);
    }

    #[test]
    fn test_recv_into_with_timeout_restores_timeout_on_success() {
        let original_timeout = Duration::from_secs(5);
        let read_timeout = Duration::from_millis(100);

        let mut port = TestSerialPort::new(original_timeout);
        port.read_data = RefCell::new(vec![0x90, 0x50, VISCA_TERMINATOR]);

        let mut transport = create_test_transport(port);

        let mut buf = [0u8; 16];
        let result = transport.recv_into_with_timeout(&mut buf, read_timeout);
        assert!(result.is_ok());

        // Timeout should be restored
        assert_eq!(
            transport.port.timeout(),
            original_timeout,
            "Timeout should be restored after successful recv"
        );
    }

    #[test]
    fn test_timeout_guard_records_changes() {
        let original_timeout = Duration::from_secs(5);
        let write_timeout = Duration::from_millis(100);

        let port = TestSerialPort::new(original_timeout);
        let mut transport = create_test_transport(port);
        transport.config.write_timeout = write_timeout;

        // Send some data
        let _ = transport.send_with_timeout(
            &[0x81, 0x01, VISCA_TERMINATOR],
            CommandKind::Command,
            write_timeout,
        );

        // We can't directly access timeout_history through the boxed trait object,
        // but we can verify the current timeout is correct
        assert_eq!(transport.port.timeout(), original_timeout);
    }

    // =========================================================================
    // Data verification tests
    // =========================================================================

    #[test]
    fn test_send_with_timeout_writes_correct_data() {
        let port = TestSerialPort::new(Duration::from_secs(5));
        let mut transport = create_test_transport(port);

        let data = vec![0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR];
        let result =
            transport.send_with_timeout(&data, CommandKind::Command, Duration::from_millis(100));
        assert!(result.is_ok());

        // Note: We can't directly access written_data through trait object,
        // but the test verifies the write path doesn't panic/error
    }

    #[test]
    fn test_recv_into_with_timeout_reads_data() {
        let mut port = TestSerialPort::new(Duration::from_secs(5));
        port.read_data = RefCell::new(vec![0x90, 0x50, VISCA_TERMINATOR]);

        let mut transport = create_test_transport(port);

        let mut buf = [0u8; 16];
        let result = transport.recv_into_with_timeout(&mut buf, Duration::from_millis(100));
        assert!(result.is_ok());
        let n = result.unwrap();
        assert_eq!(n, 3);
        assert_eq!(&buf[..n], &[0x90, 0x50, VISCA_TERMINATOR]);
    }

    // =========================================================================
    // TimeoutGuard unit tests
    // =========================================================================

    #[test]
    fn test_timeout_guard_sets_and_restores_timeout() {
        let mut port = TestSerialPort::new(Duration::from_secs(10));
        let new_timeout = Duration::from_millis(500);

        {
            let guard = TimeoutGuard::new(&mut port, new_timeout).unwrap();
            // Inside the guard, timeout should be the new value
            assert_eq!(guard.port.timeout(), new_timeout);
        }
        // After guard is dropped, timeout should be restored
        assert_eq!(port.timeout(), Duration::from_secs(10));
    }

    #[test]
    fn test_timeout_guard_restores_on_early_return() {
        // This simulates what happens when a guard goes out of scope
        // due to an early return (via ?)
        let mut port = TestSerialPort::new(Duration::from_secs(10));

        fn operation_that_fails(port: &mut dyn SerialPort) -> Result<()> {
            let _guard = TimeoutGuard::new(port, Duration::from_millis(100))?;
            // Simulate early return
            Err(Error::Timeout)?;
            #[allow(unreachable_code)]
            Ok(())
        }

        let _ = operation_that_fails(&mut port);
        // Timeout should still be restored
        assert_eq!(port.timeout(), Duration::from_secs(10));
    }
}
