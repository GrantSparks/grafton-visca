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
        system_wire::{AddressSetCommand, InterfaceClearCommand},
    },
    error::{Error, Result},
    protocol::framer::{BufferOverflow, Framer, OversizedRawFrame, RawFramer},
    timeout::Deadline,
    transport::{
        serial::{Config, SERIAL_PORT_CLOSED},
        stream_read,
    },
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

/// The closed protocol verdict of one Address Set input chunk.
enum AddressSetChunk {
    Reply(u8),
    ChainTooLong(u8),
    Pending,
    Overflow(BufferOverflow),
}

/// The I/O the startup asks its driver to perform next.
#[derive(Debug)]
pub(crate) enum Action<'a> {
    /// Write the whole frame within its timeout, then complete the write.
    Write(PendingWrite<'a>),
    /// Perform one read of up to [`SerialStartup::read_capacity`] bytes within
    /// its timeout, then complete the read.
    Read(PendingRead<'a>),
    /// Sleep, then ask for the next action.
    Sleep(Duration),
    /// Discard everything received so far, so no startup reply, echo or bus
    /// notification reaches the session; a failure ends startup.
    DiscardInput,
    /// Startup finished; on success [`SerialStartup::addressed_cameras`]
    /// reports Address Set's result.
    Done(Result<()>),
}

/// The successful continuation of the write issued by the startup.
#[derive(Debug)]
enum WriteCompletion {
    AddressSet { attempt: u8, deadline: Deadline },
    InterfaceClear,
}

/// A write issued by one startup. Only its driver can report the result.
#[derive(Debug)]
pub(crate) struct PendingWrite<'a> {
    startup: &'a mut SerialStartup,
    completion: WriteCompletion,
    pub(crate) timeout: Duration,
}

impl PendingWrite<'_> {
    pub(crate) fn frame(&self) -> &[u8] {
        match self.completion {
            WriteCompletion::AddressSet { .. } => self.startup.address_set.as_slice(),
            WriteCompletion::InterfaceClear => self.startup.if_clear.as_slice(),
        }
    }

    /// A failed write is never retried because its partial bus progress is unknown.
    pub(crate) fn complete(self, result: Result<()>) {
        self.startup.state = match result {
            Err(error) => State::Finished(Err(error)),
            Ok(()) => match self.completion {
                WriteCompletion::AddressSet { attempt, deadline } => State::AddressSet {
                    attempt,
                    deadline,
                    phase: AddressPhase::Read,
                },
                WriteCompletion::InterfaceClear => State::SettleInterfaceClear,
            },
        };
    }
}

/// A read for the current Address Set attempt.
#[derive(Debug)]
pub(crate) struct PendingRead<'a> {
    startup: &'a mut SerialStartup,
    attempt: u8,
    deadline: Deadline,
    pub(crate) timeout: Duration,
}

impl PendingRead<'_> {
    /// Idle reads pause within the same attempt. Other I/O errors stay exact.
    pub(crate) fn complete(self, result: Result<&[u8]>) {
        self.startup
            .complete_read(self.attempt, self.deadline, result);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operation {
    AddressSet,
    InterfaceClear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddressPhase {
    Read,
    Pause,
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
    /// I/F Clear was written. Wait its reserved settle delay before proceeding.
    SettleInterfaceClear,
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
    port: String,
    operations: std::vec::IntoIter<Operation>,
    timing: StartupTiming,
    read_timeout: Duration,
    write_timeout: Duration,
    if_clear_write_budget: Duration,
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
    pub(crate) fn new(
        config: &Config,
        io_bounds: crate::transport::IoBounds,
        timing: StartupTiming,
    ) -> Result<Self> {
        let if_clear_write_budget = timing
            .if_clear_operation
            .checked_sub(timing.if_clear_settle)
            .filter(|budget| !budget.is_zero())
            .ok_or_else(Error::connect_timeout)?;
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
            port: config.port.clone(),
            operations: operations.into_iter(),
            timing,
            read_timeout: io_bounds.read_timeout(),
            write_timeout: io_bounds.write_timeout(),
            if_clear_write_budget,
            read_capacity: io_bounds.buffer_config().recv_buffer_size(),
            performed_io: false,
            addressed_cameras: None,
            // Serial VISCA has no Sony envelope; raw framing stays
            // authoritative even when noise begins with Sony type bytes.
            framer: RawFramer::new(io_bounds.buffer_config()),
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
    #[cfg(all(feature = "test-utils", feature = "transport-serial"))]
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
                        // The operation starts with this action. Its write may
                        // spend only the portion not reserved for the settle.
                        let timeout = self.write_timeout.min(self.if_clear_write_budget);
                        self.performed_io = true;
                        return Action::Write(PendingWrite {
                            startup: self,
                            completion: WriteCompletion::InterfaceClear,
                            timeout,
                        });
                    }
                },
                State::AttemptStart { attempt } => {
                    debug!("Address Set attempt {attempt}");
                    self.framer.clear();
                    let deadline = match Deadline::after(
                        now,
                        self.timing.address_set_attempt,
                        "serial startup budget",
                    ) {
                        Ok(deadline) => deadline,
                        Err(error) => {
                            self.fail(error);
                            continue;
                        }
                    };
                    let remaining = deadline.remaining_at(now);
                    if remaining.is_zero() {
                        self.expire_address_set(attempt);
                        continue;
                    }
                    let timeout = self.write_timeout.min(remaining);
                    self.performed_io = true;
                    return Action::Write(PendingWrite {
                        startup: self,
                        completion: WriteCompletion::AddressSet { attempt, deadline },
                        timeout,
                    });
                }
                State::SettleInterfaceClear => {
                    self.state = State::Next;
                    return Action::Sleep(self.timing.if_clear_settle);
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
                        AddressPhase::Read => {
                            let timeout = self.read_timeout.min(remaining);
                            return Action::Read(PendingRead {
                                startup: self,
                                attempt,
                                deadline,
                                timeout,
                            });
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
            }
        }
    }

    fn complete_read(&mut self, attempt: u8, deadline: Deadline, result: Result<&[u8]>) {
        let chunk = match result
            .and_then(|chunk| stream_read(chunk.len(), SERIAL_PORT_CLOSED).map(|_| chunk))
        {
            Ok(chunk) => chunk,
            Err(Error::Timeout { .. }) => {
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
            AddressSetChunk::Reply(camera_count) => {
                debug!("Address Set complete, {camera_count} cameras found");
                self.addressed_cameras = Some(camera_count);
                self.state = State::Next;
            }
            AddressSetChunk::Pending => {
                self.state = State::AddressSet {
                    attempt,
                    deadline,
                    phase: AddressPhase::Pause,
                }
            }
            AddressSetChunk::Overflow(BufferOverflow { max_size }) => {
                warn!(max_size, "Address Set attempt discarded unframeable input");
                self.expire_address_set(attempt);
            }
            AddressSetChunk::ChainTooLong(camera_count) => self.fail(Error::SerialChainTooLong {
                port: self.port.clone().into(),
                reported: camera_count,
            }),
        }
    }

    /// Feed one chunk through the bounded raw framer.
    fn process_chunk(&mut self, chunk: &[u8]) -> AddressSetChunk {
        if let Err(overflow) = self.framer.push_slice(chunk) {
            return AddressSetChunk::Overflow(overflow);
        }
        while let Some(frame) = self.framer.visit_next_frame(address_set_reply) {
            match frame {
                Ok(Some(camera_count)) => {
                    if usize::from(camera_count) > crate::target::TargetSlot::COUNT {
                        return AddressSetChunk::ChainTooLong(camera_count);
                    }
                    return AddressSetChunk::Reply(camera_count);
                }
                Ok(None) => {}
                // Raw framing has already discarded through the terminator;
                // treat the oversized frame as bus noise.
                Err(OversizedRawFrame { max_size, .. }) => {
                    warn!(
                        max_size,
                        "Discarded oversized serial Address Set noise frame"
                    );
                }
            }
        }
        AddressSetChunk::Pending
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

    fn startup(config: Config, buffers: BufferConfig) -> SerialStartup {
        let bounds = crate::transport::IoBounds::default()
            .with_buffer_config(buffers)
            .expect("valid bounds");
        SerialStartup::new(&config, bounds, StartupTiming::VISCA).expect("encodable startup frames")
    }

    fn accept_write(startup: &mut SerialStartup, now: Instant) {
        let Action::Write(write) = startup.next(now) else {
            panic!("expected a write");
        };
        write.complete(Ok(()));
    }

    fn feed_read(startup: &mut SerialStartup, now: Instant, bytes: &[u8]) {
        let Action::Read(read) = startup.next(now) else {
            panic!("expected a read");
        };
        read.complete(Ok(bytes));
    }

    #[test]
    fn interface_clear_reserves_the_settle_and_honors_a_late_success() {
        let timing = StartupTiming::VISCA;
        let config =
            Config::new("/dev/test").with_startup(Startup::default().with_interface_clear(true));
        let bounds = crate::transport::IoBounds::SERIAL_DEFAULT
            .with_write_timeout(timing.if_clear_operation)
            .expect("valid write timeout");
        let mut startup = SerialStartup::new(&config, bounds, timing).expect("clear startup");
        let started = Instant::now();
        let Action::Write(write) = startup.next(started) else {
            panic!("expected I/F Clear");
        };

        assert_eq!(write.frame(), [0x88, 0x01, 0x00, 0x01, VISCA_TERMINATOR]);
        assert_eq!(
            write.timeout,
            timing.if_clear_operation - timing.if_clear_settle
        );
        write.complete(Ok(()));

        // The driver can resume after the write timer's deadline even on
        // success. A completed broadcast must always settle.
        let written = started + Duration::from_millis(1950);
        assert!(
            matches!(startup.next(written), Action::Sleep(delay) if delay == timing.if_clear_settle)
        );
        let settled = written + timing.if_clear_settle;
        assert!(matches!(startup.next(settled), Action::DiscardInput));
        assert!(matches!(startup.next(settled), Action::Done(Ok(()))));
    }

    #[test]
    fn interface_clear_refuses_a_budget_with_no_time_to_write() {
        let config =
            Config::new("/dev/test").with_startup(Startup::default().with_interface_clear(true));
        for budget in [Duration::from_millis(99), Duration::from_millis(100)] {
            let timing = StartupTiming {
                if_clear_operation: budget,
                ..StartupTiming::VISCA
            };
            let error =
                SerialStartup::new(&config, crate::transport::IoBounds::SERIAL_DEFAULT, timing)
                    .expect_err("no write fits before settle");
            assert_eq!(
                error.failure_context(),
                Error::connect_timeout().failure_context()
            );
            assert_eq!(error.recovery(), Error::connect_timeout().recovery());
        }
    }

    #[test]
    fn no_requested_operation_performs_no_io() {
        let mut startup = startup(Config::new("/dev/test"), BufferConfig::for_serial());
        assert!(matches!(startup.next(Instant::now()), Action::Done(Ok(()))));
    }

    #[test]
    fn fragmented_replies_and_noise_are_framed_within_bounds() {
        let config =
            Config::new("/dev/test").with_startup(Startup::default().with_address_set(true));
        let buffers = BufferConfig {
            recv_buffer_size: BufferConfig::MIN_RECV_BUFFER_SIZE,
            max_buffer_size: 64,
        };
        let mut startup = startup(config, buffers);
        let now = Instant::now();
        accept_write(&mut startup, now);
        for chunk in [
            &[0x55; BufferConfig::MIN_RECV_BUFFER_SIZE][..],
            &[0x90, 0x38],
            &[VISCA_TERMINATOR, 0x88, 0x30],
        ] {
            feed_read(&mut startup, now, chunk);
            assert!(matches!(startup.next(now), Action::Sleep(_)));
        }
        feed_read(&mut startup, now, &[0x08, VISCA_TERMINATOR]);
        assert!(matches!(startup.next(now), Action::DiscardInput));
        assert!(matches!(startup.next(now), Action::Done(Ok(()))));
        assert_eq!(startup.addressed_cameras(), Some(7));
    }

    #[test]
    fn an_overlong_chain_is_a_final_reply_and_is_not_retried() {
        for next_address in 0x09..=0x0f {
            let mut startup = startup(
                Config::new("/dev/test").with_startup(Startup::default().with_address_set(true)),
                BufferConfig::for_serial(),
            );
            let now = Instant::now();
            accept_write(&mut startup, now);
            feed_read(
                &mut startup,
                now,
                &[0x88, 0x30, next_address, VISCA_TERMINATOR],
            );
            assert!(
                matches!(startup.next(now), Action::Done(Err(Error::SerialChainTooLong { port, reported }))
                if port == "/dev/test" && reported == next_address - 1)
            );
        }
    }

    #[test]
    fn overflow_retries_the_entire_attempt_without_salvaging_its_suffix() {
        let config =
            Config::new("/dev/test").with_startup(Startup::default().with_address_set(true));
        let buffers = BufferConfig {
            recv_buffer_size: BufferConfig::MIN_RECV_BUFFER_SIZE,
            max_buffer_size: 64,
        };
        let capacity = buffers.framer_capacity();
        let mut startup = startup(config, buffers);
        let now = Instant::now();
        accept_write(&mut startup, now);
        let mut overflowing = vec![0x55; capacity];
        overflowing.extend_from_slice(&[0x88, 0x30, 0x02, VISCA_TERMINATOR]);
        feed_read(&mut startup, now, &overflowing);
        assert_eq!(startup.addressed_cameras(), None);
        assert!(matches!(startup.next(now), Action::Sleep(_)));
        accept_write(&mut startup, now);
        feed_read(&mut startup, now, &[0x88, 0x30, 0x04, VISCA_TERMINATOR]);
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
        /// A successful read of zero bytes means the serial stream closed.
        Eof,
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
        Closed,
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
        Config::new("/dev/test").with_startup(
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
            Scenario {
                name: "EOF ends Address Set without another read or broadcast",
                config: config(true, false),
                reads: vec![Read::Eof, reply(1)],
                writes: vec![],
                expect_writes: vec![ADDRESS_SET.to_vec()],
                outcome: Outcome::Closed,
                discards_input: false,
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
            (Outcome::Io(kind), Err(Error::Io { source: error })) => error.kind() == kind,
            (
                Outcome::Closed,
                Err(Error::ConnectionClosed {
                    source: Some(source),
                }),
            ) => {
                matches!(source.as_ref(), Error::Io { source }
                    if source.kind() == ErrorKind::UnexpectedEof
                        && source.to_string() == crate::transport::serial::SERIAL_PORT_CLOSED)
            }
            (Outcome::MaxRetries, Err(Error::MaxRetriesExceeded)) => true,
            (Outcome::InvalidChain(count), Err(Error::SerialChainTooLong { port, reported })) => {
                port.as_ref() == scenario.config.port && *reported == count
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
