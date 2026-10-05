//! Sans-I/O serial startup: Address Set and I/F Clear.
//!
//! [`SerialStartup`] is the single implementation of the startup protocol —
//! operation order, attempt budgets, retry classification and the handling
//! of noise, idle reads and failures. The blocking and Tokio serial
//! transports each drive it with a loop that performs only the I/O it asks
//! for ([`Action`]), so both facades give the same guarantees on the same
//! bus. The clock is passed in, so the async driver uses its executor's clock
//! and stays deterministic under the test executor.
//!
//! The policy itself is documented on [`crate::transport::serial`].

use std::time::{Duration, Instant};

use tracing::{debug, trace, warn};

#[cfg(test)]
use crate::transport::serial::Startup;
use crate::{
    camera_id::CameraId,
    command::{
        bytes::VISCA_TERMINATOR,
        encode::WireEncode,
        system::{AddressSetCommand, InterfaceClearCommand},
    },
    error::{Error, Result},
    protocol::framer::{FramingMode, ProtocolFramer},
    timeout::Deadline,
    transport::serial::Config,
};

/// Timing of the serial startup protocol.
///
/// [`StartupTiming::VISCA`] is the only production value; tests drive the
/// same production state machine with shorter budgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StartupTiming {
    /// Budget for the I/F Clear write plus its settle delay.
    pub(crate) if_clear_operation: Duration,
    /// Required quiet time after I/F Clear.
    pub(crate) if_clear_settle: Duration,
    /// Budget for one Address Set attempt: its write and its reply.
    pub(crate) address_set_attempt: Duration,
    /// Address Set attempts before startup fails.
    pub(crate) address_set_attempts: u8,
    /// Pause before another Address Set attempt.
    pub(crate) address_set_retry_delay: Duration,
    /// Pause between reads that produced no reply, within an attempt.
    pub(crate) idle_pause: Duration,
}

impl StartupTiming {
    /// The serial startup timing used by every serial transport.
    pub(crate) const VISCA: Self = Self {
        if_clear_operation: Duration::from_secs(2),
        if_clear_settle: Duration::from_millis(100),
        address_set_attempt: Duration::from_secs(2),
        address_set_attempts: 3,
        address_set_retry_delay: Duration::from_millis(100),
        idle_pause: Duration::from_millis(10),
    };
}

/// The camera count reported by a final Address Set reply.
///
/// The reply is `88 30 0p FF`, where `p` is the final assigned address plus
/// one. Network Change (`z0 38 FF`) and every other frame is not a reply.
fn address_set_reply(frame: &[u8]) -> Option<u8> {
    match frame {
        [0x88, 0x30, next_address @ 0x02..=0x08, VISCA_TERMINATOR] => Some(next_address - 1),
        _ => None,
    }
}

/// The I/O the startup asks its driver to perform next.
#[derive(Debug)]
pub(crate) enum Action<'a> {
    /// Write the whole frame within `timeout`, then report through
    /// [`SerialStartup::wrote`].
    Write { frame: &'a [u8], timeout: Duration },
    /// Perform one read of up to [`SerialStartup::read_capacity`] bytes
    /// within `timeout`, then report through [`SerialStartup::read`].
    Read { timeout: Duration },
    /// Sleep, then ask for the next action.
    Sleep(Duration),
    /// Discard everything received so far, so no startup reply, echo or bus
    /// notification reaches the session; a failure ends startup.
    DiscardInput,
    /// Startup finished; on success [`SerialStartup::addressed_cameras`]
    /// reports Address Set's result.
    Done(Result<()>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operation {
    AddressSet,
    InterfaceClear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Write,
    Read,
    Pause,
    Settle,
}

#[derive(Debug)]
enum State {
    /// Start the next requested operation.
    Next,
    /// Begin Address Set attempt `attempt` (1-based) at the next clock sample.
    AttemptStart { attempt: u8 },
    /// Inside one operation (or Address Set attempt) bounded by `deadline`.
    Running {
        operation: Operation,
        attempt: u8,
        deadline: Deadline,
        phase: Phase,
    },
    /// Waiting before the next Address Set attempt.
    RetryDelay { next_attempt: u8 },
    /// Every operation succeeded; discard the remaining input once.
    Discard,
    /// Finished with this result.
    Finished(Result<()>),
}

/// One encoded broadcast frame.
#[derive(Debug, Clone, Copy)]
struct Frame {
    bytes: [u8; 16],
    len: usize,
}

impl Frame {
    fn encode(command: &impl WireEncode, name: &str) -> Result<Self> {
        let mut bytes = [0; 16];
        let len = command
            .write_into(CameraId::CAMERA_1, &mut bytes)
            .map_err(|error| {
                Error::TransportError(format!("Failed to encode {name}: {error}").into())
            })?;
        Ok(Self { bytes, len })
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// The serial startup protocol as a state machine.
#[derive(Debug)]
pub(crate) struct SerialStartup {
    operations: std::vec::IntoIter<Operation>,
    timing: StartupTiming,
    read_timeout: Duration,
    write_timeout: Duration,
    read_capacity: usize,
    performed_io: bool,
    addressed_cameras: Option<u8>,
    framer: ProtocolFramer,
    address_set: Frame,
    if_clear: Frame,
    state: State,
}

impl SerialStartup {
    /// Plan the startup `config` requests: Address Set first, then I/F Clear.
    pub(crate) fn new(config: &Config, timing: StartupTiming) -> Result<Self> {
        let operations = [
            config.startup.address_set.then_some(Operation::AddressSet),
            config
                .startup
                .interface_clear
                .then_some(Operation::InterfaceClear),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        Ok(Self {
            operations: operations.into_iter(),
            timing,
            read_timeout: config.read_timeout,
            write_timeout: config.write_timeout,
            read_capacity: config.buffer_config.recv_buffer_size,
            performed_io: false,
            addressed_cameras: None,
            // Serial VISCA has no Sony envelope; raw framing stays
            // authoritative even when noise begins with Sony type bytes.
            framer: ProtocolFramer::new_with_config_and_mode(
                config.buffer_config,
                FramingMode::RawVisca,
            ),
            address_set: Frame::encode(&AddressSetCommand::new(), "Address Set")?,
            if_clear: Frame::encode(&InterfaceClearCommand::new(), "IF Clear")?,
            state: State::Next,
        })
    }

    /// The camera count Address Set reported, once it completed.
    pub(crate) fn addressed_cameras(&self) -> Option<u8> {
        self.addressed_cameras
    }

    /// The read buffer size a driver must supply.
    pub(crate) fn read_capacity(&self) -> usize {
        self.read_capacity
    }

    /// The next I/O to perform, given the driver's clock.
    pub(crate) fn next(&mut self, now: Instant) -> Action<'_> {
        loop {
            match self.state {
                State::Next => match self.operations.next() {
                    None if self.performed_io => self.state = State::Discard,
                    None => self.state = State::Finished(Ok(())),
                    Some(Operation::AddressSet) => {
                        self.state = State::AttemptStart { attempt: 1 };
                    }
                    Some(Operation::InterfaceClear) => {
                        debug!("Sending I/F Clear");
                        self.start(Operation::InterfaceClear, 1, now);
                    }
                },
                State::AttemptStart { attempt } => {
                    debug!("Address Set attempt {attempt}");
                    self.framer.clear();
                    self.start(Operation::AddressSet, attempt, now);
                }
                State::RetryDelay { next_attempt } => {
                    self.state = State::AttemptStart {
                        attempt: next_attempt,
                    };
                    return Action::Sleep(self.timing.address_set_retry_delay);
                }
                State::Discard => {
                    self.state = State::Finished(Ok(()));
                    return Action::DiscardInput;
                }
                State::Finished(ref result) => return Action::Done(result.clone()),
                State::Running {
                    operation,
                    attempt,
                    deadline,
                    phase,
                } => {
                    let remaining = deadline.remaining_at(now);
                    match (operation, phase) {
                        (_, Phase::Write) if remaining.is_zero() => {
                            self.expired(operation, attempt)
                        }
                        (Operation::AddressSet, Phase::Write) => {
                            return Action::Write {
                                frame: self.address_set.as_slice(),
                                timeout: self.write_timeout.min(remaining),
                            };
                        }
                        (Operation::InterfaceClear, Phase::Write) => {
                            return Action::Write {
                                frame: self.if_clear.as_slice(),
                                timeout: self.write_timeout.min(remaining),
                            };
                        }
                        (Operation::InterfaceClear, Phase::Settle) => {
                            // The settle delay belongs to the same bounded
                            // operation: a stalled write cannot consume it.
                            if remaining < self.timing.if_clear_settle {
                                self.fail(Error::connect_timeout());
                            } else {
                                self.state = State::Next;
                                return Action::Sleep(self.timing.if_clear_settle);
                            }
                        }
                        (Operation::AddressSet, _) if remaining.is_zero() => {
                            self.expired(operation, attempt);
                        }
                        (Operation::AddressSet, Phase::Read) => {
                            return Action::Read {
                                timeout: self.read_timeout.min(remaining),
                            };
                        }
                        (Operation::AddressSet, Phase::Pause) => {
                            self.set_phase(Phase::Read);
                            return Action::Sleep(self.timing.idle_pause.min(remaining));
                        }
                        (Operation::AddressSet, Phase::Settle)
                        | (Operation::InterfaceClear, Phase::Read | Phase::Pause) => {
                            unreachable!("phase {phase:?} is never entered by {operation:?}")
                        }
                    }
                }
            }
        }
    }

    /// Report the result of an [`Action::Write`].
    ///
    /// A failed or timed-out write ends startup: how much of the broadcast
    /// reached the bus is unknowable, and resending after a partial frame
    /// would put a malformed concatenation on the daisy chain.
    pub(crate) fn wrote(&mut self, result: Result<()>) {
        match (result, &self.state) {
            (Err(error), _) => self.fail(error),
            (
                Ok(()),
                State::Running {
                    operation: Operation::InterfaceClear,
                    ..
                },
            ) => self.set_phase(Phase::Settle),
            (Ok(()), _) => self.set_phase(Phase::Read),
        }
    }

    /// Report the result of an [`Action::Read`].
    ///
    /// An idle read (a timeout, `WouldBlock`, `Interrupted`, or no bytes) and
    /// a chunk that completes no reply pause within the attempt; any other
    /// read error ends startup with that error unchanged.
    pub(crate) fn read(&mut self, result: Result<&[u8]>) {
        let chunk = match result {
            Ok(chunk) if !chunk.is_empty() => chunk,
            Ok(_) => {
                self.set_phase(Phase::Pause);
                return;
            }
            // The driver already classified an idle read (an armed timeout
            // or interruption on a device, an executor timeout in async) as
            // `Error::Timeout`; every other error is a real read failure.
            Err(Error::Timeout { .. }) => {
                self.set_phase(Phase::Pause);
                return;
            }
            Err(error) => {
                self.fail(error);
                return;
            }
        };
        trace!("Address Set response: {chunk:02X?}");
        match self.process_chunk(chunk) {
            Ok(Some(camera_count)) => {
                debug!("Address Set complete, {camera_count} cameras found");
                self.addressed_cameras = Some(camera_count);
                self.state = State::Next;
            }
            Ok(None) => self.set_phase(Phase::Pause),
            // The framer could not resynchronize within its bounds: this
            // attempt's input is unusable, but the frame was fully written,
            // so another attempt is safe.
            Err(error @ Error::ResponseTooLarge { .. }) => {
                warn!(?error, "Address Set attempt discarded unframeable input");
                if let State::Running { attempt, .. } = self.state {
                    self.expired(Operation::AddressSet, attempt);
                }
            }
            Err(error) => self.fail(error),
        }
    }

    /// Feed one chunk through the bounded raw framer.
    fn process_chunk(&mut self, chunk: &[u8]) -> Result<Option<u8>> {
        self.framer.push_slice_with_resync(chunk)?;
        for frame in self.framer.drain_frames() {
            match frame {
                Ok(frame) => {
                    if let Some(camera_count) = address_set_reply(&frame) {
                        return Ok(Some(camera_count));
                    }
                }
                // Raw framing has already discarded through the terminator;
                // treat the oversized frame as bus noise.
                Err(Error::ResponseTooLarge { max_size }) => {
                    warn!(
                        max_size,
                        "Discarded oversized serial Address Set noise frame"
                    );
                }
                Err(error) => return Err(error),
            }
        }
        Ok(None)
    }

    /// An attempt's budget is spent without a completed operation.
    ///
    /// A fully written Address Set that received no reply may be retried; once
    /// every attempt is spent startup fails with
    /// [`Error::MaxRetriesExceeded`]. An expired I/F Clear fails at once.
    fn expired(&mut self, operation: Operation, attempt: u8) {
        match operation {
            Operation::AddressSet if attempt < self.timing.address_set_attempts => {
                warn!(attempt, "Address Set attempt got no reply, retrying");
                self.state = State::RetryDelay {
                    next_attempt: attempt + 1,
                };
            }
            Operation::AddressSet => {
                debug!("Address Set got no reply after {attempt} attempts");
                self.fail(Error::MaxRetriesExceeded);
            }
            Operation::InterfaceClear => self.fail(Error::connect_timeout()),
        }
    }

    /// Begin one bounded operation (or Address Set attempt) at `now`.
    fn start(&mut self, operation: Operation, attempt: u8, now: Instant) {
        let budget = match operation {
            Operation::AddressSet => self.timing.address_set_attempt,
            Operation::InterfaceClear => self.timing.if_clear_operation,
        };
        match Deadline::after(now, budget, "serial startup budget") {
            Ok(deadline) => {
                self.performed_io = true;
                self.state = State::Running {
                    operation,
                    attempt,
                    deadline,
                    phase: Phase::Write,
                };
            }
            Err(error) => self.fail(error),
        }
    }

    fn set_phase(&mut self, next: Phase) {
        if let State::Running { ref mut phase, .. } = self.state {
            *phase = next;
        }
    }

    fn fail(&mut self, error: Error) {
        self.state = State::Finished(Err(error));
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::transport::BufferConfig;

    #[test]
    fn address_set_replies_report_the_camera_count() {
        assert_eq!(
            address_set_reply(&[0x88, 0x30, 0x02, VISCA_TERMINATOR]),
            Some(1)
        );
        assert_eq!(
            address_set_reply(&[0x88, 0x30, 0x04, VISCA_TERMINATOR]),
            Some(3)
        );
        assert_eq!(
            address_set_reply(&[0x88, 0x30, 0x08, VISCA_TERMINATOR]),
            Some(7)
        );
        // Network Change and a z0-sourced lookalike are not replies.
        assert_eq!(address_set_reply(&[0x90, 0x38, VISCA_TERMINATOR]), None);
        assert_eq!(
            address_set_reply(&[0x90, 0x30, 0x02, VISCA_TERMINATOR]),
            None
        );
        assert_eq!(
            address_set_reply(&[0x88, 0x30, 0x09, VISCA_TERMINATOR]),
            None
        );
        assert_eq!(address_set_reply(&[0x88, 0x30, 0x08]), None);
        assert_eq!(address_set_reply(&[]), None);
    }

    fn startup(config: Config) -> SerialStartup {
        SerialStartup::new(&config, StartupTiming::VISCA).expect("encodable startup frames")
    }

    #[test]
    fn no_requested_operation_performs_no_io() {
        let mut startup = startup(Config::new("/dev/test"));
        assert!(matches!(startup.next(Instant::now()), Action::Done(Ok(()))));
    }

    #[test]
    fn fragmented_replies_and_noise_are_framed_within_bounds() {
        let config = Config::new("/dev/test")
            .startup(Startup::default().with_address_set(true))
            .buffer_config(BufferConfig {
                recv_buffer_size: BufferConfig::MIN_RECV_BUFFER_SIZE,
                max_buffer_size: 64,
            });
        let mut startup = startup(config);
        let now = Instant::now();
        assert!(matches!(startup.next(now), Action::Write { .. }));
        startup.wrote(Ok(()));
        for chunk in [
            &[0x55; BufferConfig::MIN_RECV_BUFFER_SIZE][..],
            &[0x90, 0x38],
            &[VISCA_TERMINATOR, 0x88, 0x30],
        ] {
            assert!(matches!(startup.next(now), Action::Read { .. }));
            startup.read(Ok(chunk));
            assert!(matches!(startup.next(now), Action::Sleep(_)));
        }
        assert!(matches!(startup.next(now), Action::Read { .. }));
        startup.read(Ok(&[0x08, VISCA_TERMINATOR]));
        assert!(matches!(startup.next(now), Action::DiscardInput));
        assert!(matches!(startup.next(now), Action::Done(Ok(()))));
        assert_eq!(startup.addressed_cameras(), Some(7));
    }
}

/// Startup scenarios run through both the blocking and the Tokio driver, so
/// the two facades are held to one transcript.
#[cfg(test)]
pub(crate) mod scenarios {
    use std::{io::ErrorKind, time::Duration};

    use super::StartupTiming;
    use crate::{
        transport::serial::{Config, Startup},
        Error,
    };

    /// One scripted read.
    #[derive(Debug, Clone)]
    pub(crate) enum Read {
        Bytes(Vec<u8>),
        /// The read's timeout elapsed with nothing received.
        Idle,
        Fails(ErrorKind),
    }

    /// One scripted write.
    #[derive(Debug, Clone, Copy)]
    pub(crate) enum Write {
        Accepted,
        TimesOut,
    }

    /// The expected startup result.
    #[derive(Debug, Clone, Copy)]
    pub(crate) enum Outcome {
        Ok(Option<u8>),
        Timeout,
        Io(ErrorKind),
        MaxRetries,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct Scenario {
        pub(crate) name: &'static str,
        pub(crate) config: Config,
        pub(crate) reads: Vec<Read>,
        pub(crate) writes: Vec<Write>,
        pub(crate) expect_writes: Vec<Vec<u8>>,
        pub(crate) outcome: Outcome,
        pub(crate) discards_input: bool,
    }

    /// Production timing with budgets short enough for real-time drivers.
    pub(crate) const TIMING: StartupTiming = StartupTiming {
        if_clear_operation: Duration::from_millis(200),
        if_clear_settle: Duration::from_millis(5),
        address_set_attempt: Duration::from_millis(40),
        address_set_attempts: 3,
        address_set_retry_delay: Duration::from_millis(5),
        idle_pause: Duration::from_millis(1),
    };

    const ADDRESS_SET: [u8; 4] = [0x88, 0x30, 0x01, 0xFF];
    const IF_CLEAR: [u8; 5] = [0x88, 0x01, 0x00, 0x01, 0xFF];

    fn reply(cameras: u8) -> Read {
        Read::Bytes(vec![0x88, 0x30, cameras + 1, 0xFF])
    }

    fn config(address_set: bool, interface_clear: bool) -> Config {
        Config::new("/dev/test").startup(
            Startup::default()
                .with_address_set(address_set)
                .with_interface_clear(interface_clear),
        )
    }

    pub(crate) fn all() -> Vec<Scenario> {
        vec![
            Scenario {
                name: "the default startup performs no I/O",
                config: config(false, false),
                reads: vec![],
                writes: vec![],
                expect_writes: vec![],
                outcome: Outcome::Ok(None),
                discards_input: false,
            },
            Scenario {
                name: "Address Set precedes I/F Clear and input is discarded",
                config: config(true, true),
                reads: vec![reply(1)],
                writes: vec![],
                expect_writes: vec![ADDRESS_SET.to_vec(), IF_CLEAR.to_vec()],
                outcome: Outcome::Ok(Some(1)),
                discards_input: true,
            },
            Scenario {
                name: "I/F Clear alone",
                config: config(false, true),
                reads: vec![],
                writes: vec![],
                expect_writes: vec![IF_CLEAR.to_vec()],
                outcome: Outcome::Ok(None),
                discards_input: true,
            },
            Scenario {
                name: "an idle read keeps the attempt",
                config: config(true, false),
                reads: vec![Read::Idle, reply(3)],
                writes: vec![],
                expect_writes: vec![ADDRESS_SET.to_vec()],
                outcome: Outcome::Ok(Some(3)),
                discards_input: true,
            },
            // #797: the blocking driver used to resend Address Set here.
            Scenario {
                name: "a timed-out Address Set write is not resent",
                config: config(true, false),
                reads: vec![reply(1)],
                writes: vec![Write::TimesOut],
                expect_writes: vec![ADDRESS_SET.to_vec()],
                outcome: Outcome::Timeout,
                discards_input: false,
            },
            // #797: the blocking driver used to wrap this as `TransportError`.
            Scenario {
                name: "a read error is returned unchanged",
                config: config(true, false),
                reads: vec![Read::Fails(ErrorKind::BrokenPipe)],
                writes: vec![],
                expect_writes: vec![ADDRESS_SET.to_vec()],
                outcome: Outcome::Io(ErrorKind::BrokenPipe),
                discards_input: false,
            },
            Scenario {
                name: "no reply spends every attempt",
                config: config(true, false),
                reads: vec![],
                writes: vec![Write::Accepted; 3],
                expect_writes: vec![ADDRESS_SET.to_vec(); 3],
                outcome: Outcome::MaxRetries,
                discards_input: false,
            },
        ]
    }

    /// Assert a driver's result against the scenario.
    pub(crate) fn check(scenario: &Scenario, result: &Result<Option<u8>, Error>) {
        let name = scenario.name;
        let matched = match (scenario.outcome, result) {
            (Outcome::Ok(expected), Ok(actual)) => expected == *actual,
            (Outcome::Timeout, Err(Error::Timeout { .. })) => true,
            (Outcome::Io(kind), Err(Error::Io(error))) => error.kind() == kind,
            (Outcome::MaxRetries, Err(Error::MaxRetriesExceeded)) => true,
            _ => false,
        };
        assert!(
            matched,
            "{name}: expected {:?}, got {result:?}",
            scenario.outcome
        );
    }
}
