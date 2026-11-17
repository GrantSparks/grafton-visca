//! Serial (RS-232/RS-422) VISCA transports: configuration and the behaviour
//! shared by the blocking and Tokio serial transports.
//!
//! # Opening a port
//!
//! Both facades open the port through one builder, so they behave the same:
//!
//! - **Exclusive access.** The port is opened for exclusive use (`TIOCEXCL`
//!   plus an exclusive `flock` on Unix; Windows opens a COM port exclusively
//!   by default). A VISCA serial bus has one owner:
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
//!   camera count through `HasTransportConfig::identity`, and session
//!   startup checks every registered camera against it before any protocol
//!   I/O, on every entry point. A registered camera beyond the chain returns
//!   [`Error::CameraNotAddressed`] with its camera id and the reported count.
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
//!   A final reply reporting more than seven cameras fails immediately with
//!   [`Error::SerialChainTooLong`], without repeating Address Set. Both topology
//!   verdicts classify as [`crate::error::ErrorKind::InvalidParameter`]; a silent
//!   chain remains a timeout rather than a topology verdict.
//! - **A failed or timed-out write is never retried**, for either command:
//!   how much of the broadcast reached the bus is unknowable, and resending
//!   after a partial frame would put a malformed concatenation on the daisy
//!   chain. Startup fails with the write's error.
//! - **Reads** retry `Interrupted` within the same read deadline. An idle
//!   timeout or `WouldBlock` keeps the Address Set attempt; other errors end
//!   startup with the read's own error. A successful zero-byte read is EOF:
//!   startup immediately reports [`Error::ConnectionClosed`] with the same
//!   `serial port closed` reason as session reads.
//! - **Timed serial writes** returning zero bytes report a timeout, including
//!   during startup. They may have partially sent an earlier chunk and must
//!   not be resubmitted automatically.
//! - **I/F Clear** writes its broadcast and then waits a 100 ms settle delay,
//!   both within one 2-second budget.
//!
//! Serial `flush` is never called: on POSIX it is `tcdrain`, which can block
//! beyond every timeout. The camera's reply and the settle delay are the
//! protocol-level confirmation that the queued bytes left.

pub(crate) mod handshake;

pub(crate) use super::serial_config::SerialConfig as Config;
#[cfg(test)]
pub(crate) use super::serial_config::SerialStartup as Startup;

use crate::Error;

/// The end-of-stream reason every serial transport reports.
pub(crate) const SERIAL_PORT_CLOSED: &str = "serial port closed";

/// Serial ports are opened for exclusive access on every facade. Unix needs
/// the builder flag; Windows opens a COM port exclusively by default.
#[cfg(unix)]
const EXCLUSIVE_ACCESS: bool = true;

/// The shortest timeout that is safe to pass to a serial-device backend.
///
/// Windows interprets a zero-millisecond serial timeout as *no timeout*.
/// Derived owner budgets can legitimately be shorter than a millisecond, so
/// round up only at this device boundary. The bounded read helper rechecks
/// its precise deadline after every idle result before it returns a timeout.
#[cfg(feature = "transport-serial")]
const MIN_DEVICE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1);

/// Round up to whole milliseconds, with the backend's one-millisecond floor.
#[cfg(feature = "transport-serial")]
pub(crate) fn device_timeout(timeout: std::time::Duration) -> std::time::Duration {
    let timeout = timeout.max(MIN_DEVICE_TIMEOUT);
    let remainder = timeout.subsec_nanos() % 1_000_000;
    if remainder == 0 {
        timeout
    } else {
        timeout.saturating_add(std::time::Duration::from_nanos(u64::from(
            1_000_000 - remainder,
        )))
    }
}

/// The one port builder both facades open with exclusive access. No timeout
/// is configured here: every blocking read and write arms its own.
pub(crate) fn port_builder(config: &Config) -> serialport::SerialPortBuilder {
    let builder = serialport::new(&config.port, config.baud_rate);
    #[cfg(unix)]
    let builder = builder.exclusive(EXCLUSIVE_ACCESS);
    builder
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

#[cfg(all(test, feature = "transport-serial"))]
mod tests {
    use super::device_timeout;
    use std::time::Duration;

    #[test]
    fn device_timeouts_round_fractional_milliseconds_up() {
        for (input, expected) in [
            (Duration::ZERO, Duration::from_millis(1)),
            (Duration::from_nanos(1), Duration::from_millis(1)),
            (Duration::from_millis(1), Duration::from_millis(1)),
            (Duration::from_micros(1_001), Duration::from_millis(2)),
            (Duration::from_micros(9_600), Duration::from_millis(10)),
            (
                Duration::from_secs(1) + Duration::from_nanos(1),
                Duration::from_millis(1_001),
            ),
            (Duration::MAX, Duration::MAX),
        ] {
            assert_eq!(device_timeout(input), expected);
        }
    }
}
