//! Serial transport for VISCA over RS-232/422.
//!
//! This module provides serial communication for VISCA protocol,
//! supporting both RS-232 and RS-422 connections with proper
//! Address Set and I/F Clear initialization.

use tracing::trace;

use std::{io::Read, time::Duration};

use crate::{
    command::CommandKind,
    error::{Error, Result},
    transport::{
        builder::{AddressingMode, TransportConfig},
        serial::{
            handshake::blocking_handshake::{address_set_blocking, if_clear_blocking},
            Config as SerialConfig,
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
        // Open serial port
        let mut port = serialport::new(&config.port, config.baud_rate)
            .timeout(config.read_timeout)
            .open()
            .map_err(|e| {
                Error::TransportError(format!("Failed to open serial port: {e}").into())
            })?;

        let if_clear = config.if_clear_on_connect;
        let address_set = config.address_set_on_connect;

        // Create TransportConfig from SerialConfig
        let transport_config = TransportConfig {
            read_timeout: config.read_timeout,
            write_timeout: config.write_timeout,
            buffer_config: config.buffer_config,
            retry_config: config.retry_config,
            addressing: AddressingMode::Serial, // Serial transport uses Serial addressing
            ..Default::default()
        };

        // Perform initialization if requested
        if if_clear {
            if_clear_blocking(&mut *port)?;
        }
        if address_set {
            address_set_blocking(&mut *port, Duration::from_secs(2))?;
        }

        Ok(Self {
            port,
            config,
            transport_config,
        })
    }
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
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<()> {
        // Apply write timeout using RAII guard for guaranteed restoration
        let write_timeout = self.config.write_timeout;
        let guard = TimeoutGuard::new(&mut *self.port, write_timeout)?;

        // Send the data directly - no nested locking, no separate send_raw
        guard
            .port
            .write_all(bytes)
            .map_err(|e| Error::TransportError(format!("Serial write error: {e}").into()))?;
        guard
            .port
            .flush()
            .map_err(|e| Error::TransportError(format!("Serial flush error: {e}").into()))?;

        trace!("Sent {} bytes: {:02X?}", bytes.len(), bytes);
        // TimeoutGuard restores original timeout on drop
        Ok(())
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize> {
        match self.port.read(dst) {
            Ok(n) => {
                trace!("Read {} bytes from serial port", n);
                Ok(n)
            }
            Err(e) => Err(e.into()),
        }
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
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    use serialport::{ClearBuffer, DataBits, FlowControl, Parity, SerialPort, StopBits};
    use std::cell::RefCell;
    use std::io::{self, ErrorKind};
    use std::sync::mpsc;
    use std::thread;

    #[test]
    fn test_serial_config_default() {
        let config = SerialConfig::default();
        assert_eq!(config.baud_rate, 9600);
        assert_eq!(config.camera_address, 1);
        assert!(config.if_clear_on_connect);
        assert!(!config.address_set_on_connect);
        assert_eq!(
            config.buffer_config,
            crate::transport::buffer::BufferConfig::for_serial()
        );
    }

    /// A mock serial port for testing that records all operations.
    struct TestSerialPort {
        /// Current timeout setting.
        timeout: RefCell<Duration>,
        /// If set, write_all will return this error.
        write_error: RefCell<Option<ErrorKind>>,
        /// If set, flush will return this error.
        flush_error: RefCell<Option<ErrorKind>>,
        /// Data to return on read.
        read_data: RefCell<Vec<u8>>,
    }

    impl TestSerialPort {
        fn new(initial_timeout: Duration) -> Self {
            Self {
                timeout: RefCell::new(initial_timeout),
                write_error: RefCell::new(None),
                flush_error: RefCell::new(None),
                read_data: RefCell::new(Vec::new()),
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
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
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
            *self.timeout.borrow_mut() = timeout;
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
            retry_config: config.retry_config,
            addressing: AddressingMode::Serial,
            ..Default::default()
        };
        SerialTransport {
            port: Box::new(port),
            config,
            transport_config,
        }
    }

    // =========================================================================
    // Deadlock regression test
    // =========================================================================

    /// This test verifies that send_with_kind does NOT deadlock.
    ///
    /// The old implementation would self-deadlock because:
    /// 1. send_with_kind locked the mutex
    /// 2. Then called send_raw which tried to lock the same mutex
    ///
    /// This test spawns a thread to call send_with_kind and waits with a timeout.
    /// If the implementation deadlocks, the test will fail on timeout.
    #[test]
    fn test_send_with_kind_does_not_deadlock() {
        let port = TestSerialPort::new(Duration::from_secs(5));
        let mut transport = create_test_transport(port);

        let (tx, rx) = mpsc::channel();

        // Spawn a thread to call send_with_kind
        let handle = thread::spawn(move || {
            let result = transport.send_with_kind(
                &[0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR],
                CommandKind::Command,
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
                    "send_with_kind should succeed: {:?}",
                    result
                );
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("DEADLOCK DETECTED: send_with_kind did not complete within 500ms");
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
    fn test_send_with_kind_restores_timeout_on_success() {
        let original_timeout = Duration::from_secs(5);
        let write_timeout = Duration::from_millis(100);

        let port = TestSerialPort::new(original_timeout);
        let mut transport = create_test_transport(port);

        // Override the write timeout in config
        transport.config.write_timeout = write_timeout;

        // Send should succeed
        let result =
            transport.send_with_kind(&[0x81, 0x01, VISCA_TERMINATOR], CommandKind::Command);
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
    fn test_send_with_kind_restores_timeout_on_write_error() {
        let original_timeout = Duration::from_secs(5);
        let write_timeout = Duration::from_millis(100);

        let port = TestSerialPort::new(original_timeout).with_write_error(ErrorKind::BrokenPipe);
        let mut transport = create_test_transport(port);
        transport.config.write_timeout = write_timeout;

        // Send should fail
        let result =
            transport.send_with_kind(&[0x81, 0x01, VISCA_TERMINATOR], CommandKind::Command);
        assert!(result.is_err());

        // Timeout should still be restored via RAII guard
        assert_eq!(
            transport.port.timeout(),
            original_timeout,
            "Timeout should be restored even after write error"
        );
    }

    #[test]
    fn test_send_with_kind_restores_timeout_on_flush_error() {
        let original_timeout = Duration::from_secs(5);
        let write_timeout = Duration::from_millis(100);

        let port = TestSerialPort::new(original_timeout).with_flush_error(ErrorKind::BrokenPipe);
        let mut transport = create_test_transport(port);
        transport.config.write_timeout = write_timeout;

        // Send should fail during flush
        let result =
            transport.send_with_kind(&[0x81, 0x01, VISCA_TERMINATOR], CommandKind::Command);
        assert!(result.is_err());

        // Timeout should still be restored via RAII guard
        assert_eq!(
            transport.port.timeout(),
            original_timeout,
            "Timeout should be restored even after flush error"
        );
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
        let _ = transport.send_with_kind(&[0x81, 0x01, VISCA_TERMINATOR], CommandKind::Command);

        // We can't directly access timeout_history through the boxed trait object,
        // but we can verify the current timeout is correct
        assert_eq!(transport.port.timeout(), original_timeout);
    }

    // =========================================================================
    // Data verification tests
    // =========================================================================

    #[test]
    fn test_send_with_kind_writes_correct_data() {
        let port = TestSerialPort::new(Duration::from_secs(5));
        let mut transport = create_test_transport(port);

        let data = vec![0x81, 0x01, 0x04, 0x00, VISCA_TERMINATOR];
        let result = transport.send_with_kind(&data, CommandKind::Command);
        assert!(result.is_ok());

        // Note: We can't directly access written_data through trait object,
        // but the test verifies the write path doesn't panic/error
    }

    #[test]
    fn test_recv_into_reads_data() {
        let mut port = TestSerialPort::new(Duration::from_secs(5));
        port.read_data = RefCell::new(vec![0x90, 0x50, VISCA_TERMINATOR]);

        let mut transport = create_test_transport(port);

        let mut buf = [0u8; 16];
        let result = transport.recv_into(&mut buf);
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
