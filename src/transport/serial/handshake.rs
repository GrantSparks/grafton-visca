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
        bytes::{constants::system, VISCA_TERMINATOR},
        encode::WireEncode,
        system::{AddressSetCommand, InterfaceClearCommand},
    },
    error::{Error, Result},
    protocol::framer::{Framer, FramingError, RawFramer},
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
        [address, register, next_address @ 0x02..=0x0f, VISCA_TERMINATOR]
            if *address == CameraId::BROADCAST.to_address_byte()
                && *register == system::ADDRESS_SET_REGISTER =>
        {
            Some(next_address - 1)
        }
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
enum AddressPhase {
    Write,
    Read,
    Pause,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClearPhase {
    Write,
    Settle,
}

#[derive(Debug)]
enum State {
    /// Start the next requested operation.
    Next,
    /// Begin Address Set attempt `attempt` (1-based) at the next clock sample.
    AttemptStart { attempt: u8 },
    /// One Address Set attempt; its counter never belongs to I/F Clear.
    AddressSet {
        attempt: u8,
        deadline: Deadline,
        phase: AddressPhase,
    },
    /// One bounded I/F Clear write and settle delay.
    InterfaceClear {
        deadline: Deadline,
        phase: ClearPhase,
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
                Error::transport_error(error.with_context(format!("Failed to encode {name}")))
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
    framer: RawFramer,
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
            framer: RawFramer::new(config.buffer_config),
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

    /// Retained input inspected by the bounded serial-startup fuzz driver.
    #[cfg(feature = "test-utils")]
    pub(crate) fn retained_len(&self) -> usize {
        self.framer.buffered_len()
    }

    /// The next I/O to perform, given the driver's clock.
    pub(crate) fn next(&mut self, now: Instant) -> Action<'_> {
        loop {
            match self.state {
                State::Next => match self.operations.next() {
                    None if self.performed_io => self.state = State::Discard,
                    None => self.state = State::Finished(Ok(())),
                    Some(Operation::AddressSet) => self.state = State::AttemptStart { attempt: 1 },
                    Some(Operation::InterfaceClear) => {
                        debug!("Sending I/F Clear");
                        self.begin(now, self.timing.if_clear_operation, |deadline| {
                            State::InterfaceClear {
                                deadline,
                                phase: ClearPhase::Write,
                            }
                        });
                    }
                },
                State::AttemptStart { attempt } => {
                    debug!("Address Set attempt {attempt}");
                    self.framer.clear();
                    self.begin(now, self.timing.address_set_attempt, |deadline| {
                        State::AddressSet {
                            attempt,
                            deadline,
                            phase: AddressPhase::Write,
                        }
                    });
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
                State::AddressSet {
                    attempt,
                    deadline,
                    phase,
                } => {
                    let remaining = deadline.remaining_at(now);
                    if remaining.is_zero() {
                        self.expire_address_set(attempt);
                        continue;
                    }
                    match phase {
                        AddressPhase::Write => {
                            return Action::Write {
                                frame: self.address_set.as_slice(),
                                timeout: self.write_timeout.min(remaining),
                            }
                        }
                        AddressPhase::Read => {
                            return Action::Read {
                                timeout: self.read_timeout.min(remaining),
                            }
                        }
                        AddressPhase::Pause => {
                            self.state = State::AddressSet {
                                attempt,
                                deadline,
                                phase: AddressPhase::Read,
                            };
                            return Action::Sleep(self.timing.idle_pause.min(remaining));
                        }
                    }
                }
                State::InterfaceClear { deadline, phase } => {
                    let remaining = deadline.remaining_at(now);
                    match phase {
                        ClearPhase::Write if remaining.is_zero() => {
                            self.fail(Error::connect_timeout())
                        }
                        ClearPhase::Write => {
                            return Action::Write {
                                frame: self.if_clear.as_slice(),
                                timeout: self.write_timeout.min(remaining),
                            }
                        }
                        ClearPhase::Settle => {
                            if remaining < self.timing.if_clear_settle {
                                self.fail(Error::connect_timeout());
                            } else {
                                self.state = State::Next;
                                return Action::Sleep(self.timing.if_clear_settle);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Report the result of the last requested write.
    /// A failed write is never retried because its partial bus progress is unknown.
    pub(crate) fn wrote(&mut self, result: Result<()>) {
        match (result, &self.state) {
            (Err(error), _) => self.fail(error),
            (
                Ok(()),
                State::AddressSet {
                    attempt,
                    deadline,
                    phase: AddressPhase::Write,
                },
            ) => {
                self.state = State::AddressSet {
                    attempt: *attempt,
                    deadline: *deadline,
                    phase: AddressPhase::Read,
                };
            }
            (
                Ok(()),
                State::InterfaceClear {
                    deadline,
                    phase: ClearPhase::Write,
                },
            ) => {
                self.state = State::InterfaceClear {
                    deadline: *deadline,
                    phase: ClearPhase::Settle,
                };
            }
            (Ok(()), _) => self.fail(Error::InvalidState(
                "serial startup received a write result without a pending write".into(),
            )),
        }
    }

    /// Report the result of the last requested Address Set read.
    /// Idle reads pause within the same attempt; all other I/O errors stay exact.
    pub(crate) fn read(&mut self, result: Result<&[u8]>) {
        let State::AddressSet {
            attempt,
            deadline,
            phase: AddressPhase::Read,
        } = self.state
        else {
            self.fail(Error::InvalidState(
                "serial startup received a read result without a pending read".into(),
            ));
            return;
        };
        let chunk = match result {
            Ok(chunk) if !chunk.is_empty() => chunk,
            Ok(_) | Err(Error::Timeout { .. }) => {
                self.state = State::AddressSet {
                    attempt,
                    deadline,
                    phase: AddressPhase::Pause,
                };
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
            Ok(None) => {
                self.state = State::AddressSet {
                    attempt,
                    deadline,
                    phase: AddressPhase::Pause,
                }
            }
            Err(error @ Error::ResponseTooLarge { .. }) => {
                warn!(?error, "Address Set attempt discarded unframeable input");
                self.expire_address_set(attempt);
            }
            Err(error) => self.fail(error),
        }
    }

    /// Feed one chunk through the bounded raw framer.
    fn process_chunk(&mut self, chunk: &[u8]) -> Result<Option<u8>> {
        self.framer.push_slice(chunk)?;
        while let Some(frame) = self.framer.visit_next_frame(address_set_reply) {
            match frame {
                Ok(Some(camera_count)) => {
                    if usize::from(camera_count) > crate::session_config::MAX_REGISTERED_TARGETS {
                        return Err(Error::InvalidRequest(
                                format!("serial chain reports {camera_count} devices; VISCA addresses at most 7").into(),
                            ));
                    }
                    return Ok(Some(camera_count));
                }
                Ok(None) => {}
                // Raw framing has already discarded through the terminator;
                // treat the oversized frame as bus noise.
                Err(FramingError::OversizedRawFrame { max_size }) => {
                    warn!(
                        max_size,
                        "Discarded oversized serial Address Set noise frame"
                    );
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(None)
    }

    /// Retry only a fully written Address Set whose bounded reply was absent.
    fn expire_address_set(&mut self, attempt: u8) {
        if attempt < self.timing.address_set_attempts {
            warn!(attempt, "Address Set attempt got no reply, retrying");
            self.state = State::RetryDelay {
                next_attempt: attempt + 1,
            };
        } else {
            debug!("Address Set got no reply after {attempt} attempts");
            self.fail(Error::MaxRetriesExceeded);
        }
    }

    /// Construct the operation-specific write state only after its deadline is valid.
    fn begin(&mut self, now: Instant, budget: Duration, running: impl FnOnce(Deadline) -> State) {
        match Deadline::after(now, budget, "serial startup budget") {
            Ok(deadline) => {
                self.performed_io = true;
                self.state = running(deadline);
            }
            Err(error) => self.fail(error),
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
            Some(8)
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

    #[test]
    fn an_overlong_chain_is_a_final_reply_and_is_not_retried() {
        for next_address in 0x09..=0x0f {
            let mut startup = startup(
                Config::new("/dev/test").startup(Startup::default().with_address_set(true)),
            );
            let now = Instant::now();
            assert!(matches!(startup.next(now), Action::Write { .. }));
            startup.wrote(Ok(()));
            assert!(matches!(startup.next(now), Action::Read { .. }));
            startup.read(Ok(&[0x88, 0x30, next_address, VISCA_TERMINATOR]));
            assert!(
                matches!(startup.next(now), Action::Done(Err(Error::InvalidRequest(message)))
                if message.contains(&format!("reports {} devices", next_address - 1)))
            );
        }
    }

    #[test]
    fn overflow_retries_the_entire_attempt_without_salvaging_its_suffix() {
        let config = Config::new("/dev/test")
            .startup(Startup::default().with_address_set(true))
            .buffer_config(BufferConfig {
                recv_buffer_size: BufferConfig::MIN_RECV_BUFFER_SIZE,
                max_buffer_size: 64,
            });
        let capacity = config.buffer_config.framer_capacity();
        let mut startup = startup(config);
        let now = Instant::now();
        assert!(matches!(startup.next(now), Action::Write { .. }));
        startup.wrote(Ok(()));
        assert!(matches!(startup.next(now), Action::Read { .. }));
        let mut overflowing = vec![0x55; capacity];
        overflowing.extend_from_slice(&[0x88, 0x30, 0x02, VISCA_TERMINATOR]);
        startup.read(Ok(&overflowing));
        assert_eq!(startup.addressed_cameras(), None);
        assert!(matches!(startup.next(now), Action::Sleep(_)));
        assert!(matches!(startup.next(now), Action::Write { .. }));
        startup.wrote(Ok(()));
        assert!(matches!(startup.next(now), Action::Read { .. }));
        startup.read(Ok(&[0x88, 0x30, 0x04, VISCA_TERMINATOR]));
        assert!(matches!(startup.next(now), Action::DiscardInput));
        assert!(matches!(startup.next(now), Action::Done(Ok(()))));
        assert_eq!(startup.addressed_cameras(), Some(3));
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
        InvalidChain(u8),
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
                name: "an overlong chain fails after one Address Set write",
                config: config(true, false),
                reads: vec![reply(8)],
                writes: vec![],
                expect_writes: vec![ADDRESS_SET.to_vec()],
                outcome: Outcome::InvalidChain(8),
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
            (Outcome::InvalidChain(count), Err(Error::InvalidRequest(message))) => {
                message.as_ref()
                    == format!("serial chain reports {count} devices; VISCA addresses at most 7")
            }
            _ => false,
        };
        assert!(
            matched,
            "{name}: expected {:?}, got {result:?}",
            scenario.outcome
        );
    }
}
