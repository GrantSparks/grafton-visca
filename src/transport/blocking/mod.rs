//! Blocking transport implementations and the bounded-I/O helpers shared by
//! every blocking transport (TCP, UDP and serial).
//!
//! # Policy
//!
//! The helpers here are the single implementation of these rules:
//!
//! - **Arm before every syscall.** Every bounded read or write sets its own
//!   per-call OS or device timeout immediately before the syscall, through an
//!   [`Arm`] function. Nothing relies on a timeout left behind by an earlier
//!   operation, so nothing is restored afterwards. Failing to arm is reported
//!   as [`Error::Io`] carrying the platform error, on sockets and serial
//!   devices alike.
//! - **Budgets.** A multi-syscall operation uses one [`Deadline`]; partial
//!   progress never buys a fresh timeout, and a budget too large for the
//!   monotonic clock is rejected as [`Error::InvalidParameter`], exactly as
//!   configuration validation rejects it.
//! - **Writes.** [`write_all_bounded`] retries `Interrupted` in place and reports
//!   expiry as [`Error::io_timeout`]. A zero-byte timed serial write also means
//!   its device timeout elapsed; a zero-byte socket write is `WriteZero`.
//!   Claiming more bytes than supplied is always [`Error::Io`].
//! - **Reads.** [`read_once_bounded`] retries `Interrupted` within the same
//!   deadline, re-arming the remaining budget each time. Other idle results
//!   are classified with [`TimedIo`]. Successful zero-byte stream reads are
//!   EOF, including serial startup, through [`super::stream_read`].
//!
//! All of this is synchronous; the blocking facade stays executor-free.

use std::{
    io::{self, Read, Write},
    time::{Duration, Instant},
};

use crate::{timeout::Deadline, Error, Result};

pub mod tcp;
pub mod udp;

// Re-exports
pub use tcp::Tcp;
pub use udp::Udp;

#[cfg(test)]
mod tests;

/// Which bounded operation produced an I/O error, for deciding whether it
/// ended idle (nothing transferred) rather than failing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TimedIo {
    /// A write on any transport, or a read whose `TimedOut` can only mean the
    /// armed timeout (datagram sockets, serial devices).
    Bounded,
    /// A serial-device operation, where a zero-byte timed write reports an
    /// elapsed device timeout rather than a socket's `WriteZero`.
    #[cfg(feature = "transport-serial")]
    SerialDevice,
    /// A read from a connected TCP stream. Unix reports an expired
    /// `SO_RCVTIMEO` as `EAGAIN` / `WouldBlock`, while `ETIMEDOUT` is the
    /// connection-level result of exhausted keepalive probes and must reach
    /// the owner as session death (#719). Other platforms may report the armed
    /// read timeout itself as `TimedOut`.
    StreamSocketRead,
}

impl TimedIo {
    /// Whether `error` means the operation ended idle: its armed timeout
    /// elapsed, or a signal interrupted it before any byte moved.
    pub(crate) fn ended_idle(self, error: &io::Error) -> bool {
        match (self, error.kind()) {
            (_, io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => true,
            (Self::Bounded, io::ErrorKind::TimedOut) => true,
            #[cfg(feature = "transport-serial")]
            (Self::SerialDevice, io::ErrorKind::TimedOut) => true,
            (Self::StreamSocketRead, io::ErrorKind::TimedOut) => cfg!(not(unix)),
            _ => false,
        }
    }
}

/// Sets the per-call timeout of one I/O handle before its next syscall.
pub(crate) type Arm<Io> = fn(&mut Io, Duration) -> io::Result<()>;

/// Write all of `bytes` before `deadline`.
///
/// Each syscall is armed with the remaining budget, so a partial write keeps
/// the one whole-frame deadline. See the module policy for the
/// error contract.
pub(crate) fn write_all_bounded<Io: Write + ?Sized>(
    io: &mut Io,
    arm: Arm<Io>,
    bytes: &[u8],
    deadline: Deadline,
    kind: TimedIo,
) -> Result<()> {
    let mut written = 0;
    while written < bytes.len() {
        let remaining = deadline.remaining_or(Instant::now(), Error::io_timeout)?;
        arm(io, remaining)?;

        let pending = bytes.len() - written;
        match io.write(&bytes[written..]) {
            Ok(0) => {
                #[cfg(feature = "transport-serial")]
                if kind == TimedIo::SerialDevice {
                    return Err(Error::io_timeout());
                }
                return Err(
                    io::Error::new(io::ErrorKind::WriteZero, "write made no progress").into(),
                );
            }
            Ok(count) if count <= pending => written += count,
            Ok(count) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("write reported {count} bytes for a {pending}-byte buffer"),
                )
                .into());
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if kind.ended_idle(&error) => {
                return Err(Error::io_timeout());
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Read within `timeout`, retrying signal interruptions within that deadline
/// and reporting other idle ends as [`Error::io_timeout`] according to `kind`.
/// A zero budget ends idle before arming or reading; an unrepresentable
/// monotonic-clock budget is rejected as [`Error::InvalidParameter`].
pub(crate) fn read_once_bounded<Io: Read + ?Sized>(
    io: &mut Io,
    arm: Arm<Io>,
    dst: &mut [u8],
    timeout: Duration,
    kind: TimedIo,
) -> Result<usize> {
    let deadline = Deadline::after(Instant::now(), timeout, "read_timeout")?;
    read_before(io, arm, dst, deadline, kind, Instant::now)
}

/// The bounded read loop; its clock seam pins interruptions at the deadline.
fn read_before<Io: Read + ?Sized>(
    io: &mut Io,
    arm: Arm<Io>,
    dst: &mut [u8],
    deadline: Deadline,
    kind: TimedIo,
    mut now: impl FnMut() -> Instant,
) -> Result<usize> {
    loop {
        let remaining = deadline.remaining_or(now(), Error::io_timeout)?;
        arm(io, remaining)?;
        match io.read(dst) {
            Ok(read) => return Ok(read),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if kind.ended_idle(&error) => return Err(Error::io_timeout()),
            Err(error) => return Err(error.into()),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod helper_tests {
    use std::io::{self, ErrorKind, Write};

    use super::*;

    /// A handle whose writes are scripted and whose armed timeouts are logged.
    #[derive(Debug, Default)]
    struct ScriptedIo {
        armed: Vec<Duration>,
        written: Vec<u8>,
        write_results: Vec<io::Result<usize>>,
    }

    impl ScriptedIo {
        fn arm(&mut self, timeout: Duration) -> io::Result<()> {
            self.armed.push(timeout);
            Ok(())
        }
    }

    impl Write for ScriptedIo {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let result = if self.write_results.is_empty() {
                Ok(buf.len())
            } else {
                self.write_results.remove(0)
            };
            if let Ok(count) = result {
                self.written.extend_from_slice(&buf[..count.min(buf.len())]);
            }
            result
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn write(io: &mut ScriptedIo, bytes: &[u8], budget: Duration) -> Result<()> {
        let deadline = Deadline::after(Instant::now(), budget, "write_timeout")?;
        write_all_bounded(io, ScriptedIo::arm, bytes, deadline, TimedIo::Bounded)
    }

    #[test]
    fn an_over_reported_write_is_an_io_error_not_a_panic() {
        let mut io = ScriptedIo {
            write_results: vec![Ok(9)],
            ..ScriptedIo::default()
        };
        let error = write(&mut io, b"VISCA", Duration::from_secs(1))
            .expect_err("a write claiming more than it was given is rejected");
        assert!(matches!(
            error,
            Error::Io { source: ref io_error } if io_error.kind() == ErrorKind::InvalidData
        ));
    }

    #[test]
    fn a_zero_progress_write_is_write_zero() {
        let mut io = ScriptedIo {
            write_results: vec![Ok(0)],
            ..ScriptedIo::default()
        };
        assert!(matches!(
            write(&mut io, b"VISCA", Duration::from_secs(1)),
            Err(Error::Io { source: ref io_error }) if io_error.kind() == ErrorKind::WriteZero
        ));
    }

    #[test]
    fn every_syscall_is_armed_and_partial_writes_continue() {
        let mut io = ScriptedIo {
            write_results: vec![Err(ErrorKind::Interrupted.into()), Ok(2), Ok(3)],
            ..ScriptedIo::default()
        };
        write(&mut io, b"VISCA", Duration::from_secs(1)).expect("whole frame written");
        assert_eq!(io.written, b"VISCA");
        assert_eq!(
            io.armed.len(),
            3,
            "one arm before each of the three syscalls"
        );
    }

    #[test]
    fn an_unrepresentable_budget_is_rejected_before_any_syscall() {
        let mut io = ScriptedIo::default();
        assert!(matches!(
            write(&mut io, b"VISCA", Duration::MAX),
            Err(Error::InvalidParameter {
                parameter: "write_timeout",
                ..
            })
        ));
        assert!(io.armed.is_empty());
        assert!(io.written.is_empty());
    }

    /// Local fake: advance the read deadline when the interrupted syscall
    /// returns, independently of scheduler load and wall-clock sleeps.
    struct InterruptedRead {
        clock: std::rc::Rc<std::cell::Cell<Instant>>,
        spent: Duration,
        interrupted: bool,
        reads: usize,
        armed: Vec<Duration>,
    }

    impl InterruptedRead {
        fn arm(&mut self, timeout: Duration) -> io::Result<()> {
            self.armed.push(timeout);
            Ok(())
        }
    }

    impl Read for InterruptedRead {
        fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
            self.reads += 1;
            if !self.interrupted {
                self.interrupted = true;
                self.clock.set(self.clock.get() + self.spent);
                return Err(ErrorKind::Interrupted.into());
            }
            dst[..3].copy_from_slice(b"ACK");
            Ok(3)
        }
    }

    #[test]
    fn interrupted_reads_retry_only_with_the_original_budget_remaining() {
        for kind in [
            TimedIo::Bounded,
            TimedIo::StreamSocketRead,
            #[cfg(feature = "transport-serial")]
            TimedIo::SerialDevice,
        ] {
            for spent in [Duration::from_millis(4), Duration::from_millis(10)] {
                let started = Instant::now();
                let clock = std::rc::Rc::new(std::cell::Cell::new(started));
                let mut io = InterruptedRead {
                    clock: clock.clone(),
                    spent,
                    interrupted: false,
                    reads: 0,
                    armed: Vec::new(),
                };
                let budget = Duration::from_millis(10);
                let deadline = Deadline::after(started, budget, "read_timeout").expect("deadline");
                let mut dst = [0; 16];
                let result = read_before(
                    &mut io,
                    InterruptedRead::arm,
                    &mut dst,
                    deadline,
                    kind,
                    || clock.get(),
                );
                if spent < budget {
                    assert_eq!(result.expect("data after EINTR"), 3);
                    assert_eq!(&dst[..3], b"ACK");
                    assert_eq!(io.reads, 2);
                    assert_eq!(io.armed, [budget, budget - spent]);
                } else {
                    assert!(matches!(result, Err(Error::Timeout { .. })));
                    assert_eq!(
                        io.reads, 1,
                        "no syscall after the interrupted read spent the deadline"
                    );
                    assert_eq!(io.armed, [budget]);
                }
            }
        }
    }

    #[cfg(feature = "transport-serial")]
    #[test]
    fn a_zero_progress_timed_serial_write_is_an_unconfirmed_timeout() {
        let mut io = ScriptedIo {
            write_results: vec![Ok(0)],
            ..ScriptedIo::default()
        };
        let deadline = Deadline::after(Instant::now(), Duration::from_secs(1), "write_timeout")
            .expect("deadline");
        let error = write_all_bounded(
            &mut io,
            ScriptedIo::arm,
            b"VISCA",
            deadline,
            TimedIo::SerialDevice,
        )
        .expect_err("device timeout returned zero bytes");
        assert!(matches!(error, Error::Timeout { .. }));
        assert_eq!(
            error.failure_context(),
            Error::io_timeout().failure_context()
        );
        assert_eq!(error.recovery(), crate::Recovery::Reconcile);
        assert_eq!(io.armed.len(), 1);
        assert!(io.written.is_empty());
    }

    #[test]
    fn stream_socket_reads_keep_unix_connection_timeouts_distinct() {
        assert!(TimedIo::StreamSocketRead.ended_idle(&ErrorKind::WouldBlock.into()));
        assert!(TimedIo::StreamSocketRead.ended_idle(&ErrorKind::Interrupted.into()));
        assert_eq!(
            TimedIo::StreamSocketRead.ended_idle(&ErrorKind::TimedOut.into()),
            cfg!(not(unix)),
            "Unix ETIMEDOUT must reach the owner as keepalive/session failure"
        );
        assert!(TimedIo::Bounded.ended_idle(&ErrorKind::TimedOut.into()));
        assert!(!TimedIo::Bounded.ended_idle(&ErrorKind::BrokenPipe.into()));
    }
}
