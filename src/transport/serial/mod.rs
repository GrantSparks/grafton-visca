//! Serial (RS-232/RS-422) VISCA transports: configuration and the behaviour
//! shared by the blocking and Tokio serial transports.
//!
//! # Opening a port
//!
//! Both facades open the port through one builder, so they behave the same:
//!
//! - **Exclusive access.** The port is opened for exclusive use (`TIOCEXCL`
//!   plus an exclusive `flock` on Unix). A VISCA serial bus has one owner:
//!   a second writer would interleave bytes with this session's frames and
//!   could reset the bus with its own broadcasts.
//! - **Open failure** is [`Error::ConnectionFailed`] naming the port and
//!   carrying the platform error, so its [`std::io::ErrorKind`] survives.
//! - **Reported configuration.** The transport reports
//!   [`Config::transport_config`]: serial addressing and no TCP socket
//!   options.
//! - **Writes on open.** Only those [`Config::startup`] requests; the default
//!   writes nothing. After any startup operation the received input is
//!   discarded, so no startup reply, echo or bus notification reaches the
//!   session.
//! - **Addressed cameras.** When Address Set runs, the transport reports the
//!   camera count through `HasTransportConfig::addressed_bus`, and session
//!   startup checks every registered camera against it before any protocol
//!   I/O, on every entry point.
//!
//! # Startup protocol
//!
//! Address Set and I/F Clear run through one sans-I/O state machine for both
//! facades. Address Set (when requested) runs before I/F Clear.
//!
//! - **Address Set** makes up to three attempts, each with one 2-second
//!   budget covering its write and its reply. Partial or noisy input never
//!   buys an attempt more time. A fully written attempt that gets no reply
//!   (or whose input cannot be framed) is retried after 100 ms; after the
//!   last attempt startup fails with [`Error::MaxRetriesExceeded`].
//! - **A failed or timed-out write is never retried**, for either command:
//!   how much of the broadcast reached the bus is unknowable, and resending
//!   after a partial frame would put a malformed concatenation on the daisy
//!   chain. Startup fails with the write's error.
//! - **Read errors** other than an idle read (timeout, `WouldBlock`,
//!   `Interrupted`) end startup with the read's own error, unchanged.
//! - **I/F Clear** writes its broadcast and then waits a 100 ms settle delay,
//!   both within one 2-second budget.
//!
//! Serial `flush` is never called: on POSIX it is `tcdrain`, which can block
//! beyond every timeout. The camera's reply and the settle delay are the
//! protocol-level confirmation that the queued bytes left.

mod config;
pub(crate) mod handshake;

pub use config::{Config, Startup};

use crate::Error;

/// The end-of-stream reason every serial transport reports.
pub(crate) const SERIAL_PORT_CLOSED: &str = "serial port closed";

/// Serial ports are opened for exclusive access on every facade.
const EXCLUSIVE_ACCESS: bool = true;

/// The shortest timeout that is safe to pass to a serial-device backend.
///
/// Windows interprets a zero-millisecond serial timeout as *no timeout*.
/// Derived owner budgets can legitimately be shorter than a millisecond, so
/// clamp only at this device boundary; the owner still checks its precise
/// deadline after every I/O operation.
#[cfg(feature = "transport-serial")]
const MIN_DEVICE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1);

/// Round a serial-device timeout up to the backend's smallest safe value.
#[cfg(feature = "transport-serial")]
fn device_timeout(timeout: std::time::Duration) -> std::time::Duration {
    timeout.max(MIN_DEVICE_TIMEOUT)
}

/// The one port builder both facades open with exclusive access. No timeout
/// is configured here: every blocking read and write arms its own.
pub(crate) fn port_builder(config: &Config) -> serialport::SerialPortBuilder {
    serialport::new(&config.port, config.baud_rate).exclusive(EXCLUSIVE_ACCESS)
}

/// The one open-failure error both facades report.
pub(crate) fn open_failed(config: &Config, error: serialport::Error) -> Error {
    crate::transport::connect::connection_failed(&config.port, std::io::Error::from(error))
}

/// A blocking serial device as the shared bounded-I/O helpers see it.
#[cfg(feature = "transport-serial")]
pub(crate) type DevicePort = dyn serialport::SerialPort + Send;

/// Arm a blocking serial device's single read/write timeout.
#[cfg(feature = "transport-serial")]
pub(crate) fn set_device_timeout(
    port: &mut DevicePort,
    timeout: std::time::Duration,
) -> std::io::Result<()> {
    port.set_timeout(device_timeout(timeout))
        .map_err(std::io::Error::from)
}
