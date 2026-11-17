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
//! - **Errors.** [`TimedIo::verdict`] gives each medium and direction one
//!   retry, idle or fault verdict. [`bounded_io`] re-arms retries and early idle
//!   results within the original precise deadline, for reads and writes alike.
//!   Unix TCP `TimedOut` faults retain their OS cause; `WouldBlock` is idle.
//! - **Writes.** [`write_all_bounded`] keeps partial progress and reports
//!   expiry as [`Error::io_timeout`]. A zero-byte timed serial write also means
//!   its device timeout elapsed; a zero-byte socket write is `WriteZero`.
//!   Claiming more bytes than supplied is always [`Error::Io`].
//! - **Reads.** [`read_once_bounded`] retries `Interrupted` and early idle
//!   results within the same deadline, re-arming the remaining budget each
//!   time. [`TimedIo`] distinguishes idle from connection faults. A timeout
//!   is reported only after the precise deadline. Successful zero-byte stream
//!   reads are EOF, including serial startup, through [`super::stream_read`].
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

/// Medium and direction of a bounded operation. Unix stream socket timeouts
/// distinguish an elapsed OS timer (`WouldBlock`) from a dead connection
/// (`TimedOut`), on writes as well as reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TimedIo {
    StreamSocketRead,
    StreamSocketWrite,
    Datagram,
    /// A zero-byte timed serial write also means the device timer elapsed.
    #[cfg(feature = "transport-serial")]
    SerialDevice,
}

/// The one interpretation of an I/O error inside the blocking transport loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IoVerdict {
    Retry,
    Idle,
    Fault,
}

impl TimedIo {
    pub(crate) const fn verdict(self, error: io::ErrorKind) -> IoVerdict {
        match (self, error) {
            (_, io::ErrorKind::Interrupted) => IoVerdict::Retry,
            (_, io::ErrorKind::WouldBlock) => IoVerdict::Idle,
            (Self::Datagram, io::ErrorKind::TimedOut) => IoVerdict::Idle,
            #[cfg(feature = "transport-serial")]
            (Self::SerialDevice, io::ErrorKind::TimedOut) => IoVerdict::Idle,
            (Self::StreamSocketRead | Self::StreamSocketWrite, io::ErrorKind::TimedOut) => {
                if cfg!(unix) {
                    IoVerdict::Fault
                } else {
                    IoVerdict::Idle
                }
            }
            _ => IoVerdict::Fault,
        }
    }
}

/// Sets the per-call timeout of one I/O handle before its next syscall.
pub(crate) type Arm<Io> = fn(&mut Io, Duration) -> io::Result<()>;

/// Keep a syscall error unboxed until its verdict is known. Retrying an idle
/// timer or signal interruption must not allocate an `Error::Io` source.
#[derive(Debug)]
pub(crate) enum AttemptError {
    Io(io::Error),
    Other(Error),
}

impl From<io::Error> for AttemptError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<Error> for AttemptError {
    fn from(error: Error) -> Self {
        Self::Other(error)
    }
}

/// Run one bounded operation. An attempt returns `None` for partial progress or
/// a discarded datagram, and `Some` when the operation finishes. Signal retries
/// and idle results both recheck the precise deadline before another syscall.
/// Arming failures and non-I/O errors retain their original error unchanged.
pub(crate) fn bounded_io<Io: ?Sized, T>(
    io: &mut Io,
    arm: Arm<Io>,
    deadline: Deadline,
    kind: TimedIo,
    mut now: impl FnMut() -> Instant,
    mut attempt: impl FnMut(&mut Io) -> std::result::Result<Option<T>, AttemptError>,
) -> Result<T> {
    loop {
        let remaining = deadline.remaining_or(now(), Error::io_timeout)?;
        arm(io, remaining)?;
        match attempt(io) {
            Ok(Some(value)) => return Ok(value),
            Ok(None) => {}
            Err(AttemptError::Io(error)) => match kind.verdict(error.kind()) {
                IoVerdict::Retry | IoVerdict::Idle => {}
                IoVerdict::Fault => return Err(error.into()),
            },
            Err(AttemptError::Other(error)) => return Err(error),
        }
    }
}

/// Write all of `bytes` before `deadline`, retaining one whole-frame budget.
pub(crate) fn write_all_bounded<Io: Write + ?Sized>(
    io: &mut Io,
    arm: Arm<Io>,
    bytes: &[u8],
    deadline: Deadline,
    kind: TimedIo,
) -> Result<()> {
    write_before(io, arm, bytes, deadline, kind, Instant::now)
}

/// The write attempt with a clock seam shared with reads and datagrams.
fn write_before<Io: Write + ?Sized>(
    io: &mut Io,
    arm: Arm<Io>,
    bytes: &[u8],
    deadline: Deadline,
    kind: TimedIo,
    now: impl FnMut() -> Instant,
) -> Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }
    let mut written = 0;
    bounded_io(io, arm, deadline, kind, now, |io| {
        let pending = bytes.len() - written;
        match io.write(&bytes[written..])? {
            0 => {
                #[cfg(feature = "transport-serial")]
                if kind == TimedIo::SerialDevice {
                    return Err(Error::io_timeout().into());
                }
                Err(io::Error::new(io::ErrorKind::WriteZero, "write made no progress").into())
            }
            count if count <= pending => {
                written += count;
                Ok((written == bytes.len()).then_some(()))
            }
            count => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("write reported {count} bytes for a {pending}-byte buffer"),
            )
            .into()),
        }
    })
}

/// Read within `timeout`, retrying signal interruptions and early idle results
/// until that deadline, then reporting [`Error::io_timeout`] according to `kind`.
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

/// A read attempt using the shared bounded loop and its clock seam.
pub(crate) fn read_before<Io: Read + ?Sized>(
    io: &mut Io,
    arm: Arm<Io>,
    dst: &mut [u8],
    deadline: Deadline,
    kind: TimedIo,
    now: impl FnMut() -> Instant,
) -> Result<usize> {
    bounded_io(io, arm, deadline, kind, now, |io| Ok(Some(io.read(dst)?)))
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
        write_all_bounded(io, ScriptedIo::arm, bytes, deadline, TimedIo::Datagram)
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
            TimedIo::Datagram,
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

    /// Local fake: a coarse device returns idle before the exact deadline.
    struct EarlyIdleRead {
        clock: std::rc::Rc<std::cell::Cell<Instant>>,
        armed: Vec<Duration>,
        reads: usize,
        data_on_retry: bool,
        idle_kind: ErrorKind,
    }

    impl EarlyIdleRead {
        fn arm(&mut self, timeout: Duration) -> io::Result<()> {
            self.armed.push(timeout);
            Ok(())
        }
    }

    impl Read for EarlyIdleRead {
        fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
            self.reads += 1;
            if self.reads == 2 && self.data_on_retry {
                dst[..3].copy_from_slice(b"ACK");
                return Ok(3);
            }
            let elapsed = if self.reads == 1 {
                Duration::from_micros(9_600)
            } else {
                *self.armed.last().expect("armed before reading")
            };
            self.clock.set(self.clock.get() + elapsed);
            Err(self.idle_kind.into())
        }
    }

    #[test]
    fn idle_reads_rearm_until_the_precise_deadline() {
        let budget = Duration::from_micros(10_500);
        for (kind, idle_kind) in [
            (TimedIo::Datagram, ErrorKind::TimedOut),
            (TimedIo::StreamSocketRead, ErrorKind::WouldBlock),
            #[cfg(feature = "transport-serial")]
            (TimedIo::SerialDevice, ErrorKind::TimedOut),
        ] {
            for data_on_retry in [false, true] {
                let started = Instant::now();
                let clock = std::rc::Rc::new(std::cell::Cell::new(started));
                let mut io = EarlyIdleRead {
                    clock: clock.clone(),
                    armed: Vec::new(),
                    reads: 0,
                    data_on_retry,
                    idle_kind,
                };
                let deadline = Deadline::after(started, budget, "read_timeout").expect("deadline");
                let mut dst = [0; 16];
                let result = read_before(
                    &mut io,
                    EarlyIdleRead::arm,
                    &mut dst,
                    deadline,
                    kind,
                    || clock.get(),
                );
                assert_eq!(
                    io.reads, 2,
                    "an early idle must re-arm the remaining budget"
                );
                assert_eq!(io.armed, [budget, Duration::from_micros(900)]);
                if data_on_retry {
                    assert_eq!(result.expect("data after early idle"), 3);
                    assert_eq!(&dst[..3], b"ACK");
                } else {
                    assert!(matches!(result, Err(Error::Timeout { .. })));
                    assert_eq!(clock.get().duration_since(started), budget);
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
    fn an_early_idle_write_can_finish_within_the_original_budget() {
        let started = Instant::now();
        let budget = Duration::from_millis(10);
        let deadline = Deadline::after(started, budget, "write_timeout").expect("deadline");
        let mut samples = [started, started + Duration::from_millis(4)].into_iter();
        let mut io = ScriptedIo {
            write_results: vec![Err(ErrorKind::WouldBlock.into()), Ok(5)],
            ..ScriptedIo::default()
        };
        write_before(
            &mut io,
            ScriptedIo::arm,
            b"VISCA",
            deadline,
            TimedIo::StreamSocketWrite,
            || samples.next().expect("one deadline check per attempt"),
        )
        .expect("write after early idle");
        assert_eq!(io.armed, [budget, Duration::from_millis(6)]);
        assert_eq!(io.written, b"VISCA");
    }

    #[test]
    fn stream_writes_retain_connection_timeouts_and_recheck_idle_deadlines() {
        for error_kind in [
            ErrorKind::TimedOut,
            ErrorKind::WouldBlock,
            ErrorKind::BrokenPipe,
        ] {
            let started = Instant::now();
            let budget = Duration::from_millis(10);
            let deadline = Deadline::after(started, budget, "write_timeout").expect("deadline");
            let mut samples = [started, started + budget].into_iter();
            let mut io = ScriptedIo {
                write_results: vec![Err(io::Error::new(error_kind, "original write cause"))],
                ..ScriptedIo::default()
            };
            let error = write_before(
                &mut io,
                ScriptedIo::arm,
                b"VISCA",
                deadline,
                TimedIo::StreamSocketWrite,
                || samples.next().expect("one deadline check per attempt"),
            )
            .expect_err("scripted write fails");
            if error_kind == ErrorKind::BrokenPipe
                || cfg!(unix) && error_kind == ErrorKind::TimedOut
            {
                assert!(
                    matches!(error, Error::Io { ref source } if source.kind() == error_kind && source.to_string() == "original write cause")
                );
            } else {
                assert!(matches!(error, Error::Timeout { .. }));
            }
            assert_eq!(io.armed, [budget], "no syscall after the precise deadline");
        }
    }

    #[test]
    fn the_verdict_table_distinguishes_retries_idle_and_faults() {
        for kind in [
            TimedIo::StreamSocketRead,
            TimedIo::StreamSocketWrite,
            TimedIo::Datagram,
            #[cfg(feature = "transport-serial")]
            TimedIo::SerialDevice,
        ] {
            for (error, expected) in [
                (ErrorKind::Interrupted, IoVerdict::Retry),
                (ErrorKind::WouldBlock, IoVerdict::Idle),
                (
                    ErrorKind::TimedOut,
                    if cfg!(unix)
                        && matches!(kind, TimedIo::StreamSocketRead | TimedIo::StreamSocketWrite)
                    {
                        IoVerdict::Fault
                    } else {
                        IoVerdict::Idle
                    },
                ),
                (ErrorKind::BrokenPipe, IoVerdict::Fault),
                (ErrorKind::ConnectionReset, IoVerdict::Fault),
            ] {
                assert_eq!(kind.verdict(error), expected, "{kind:?}: {error:?}");
            }
        }
    }
}
