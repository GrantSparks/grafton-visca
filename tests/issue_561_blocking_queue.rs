//! Issue #561: blocking submission queues behind busy sockets.
//!
//! A blocking operation handle is returned once the owner has admitted the
//! request (D24). A request that finds every socket busy stays queued and is
//! written when one frees; a transport write failure is reported through the
//! operation's own outcome.

#![cfg(feature = "blocking")]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    command::CommandKind,
    completion::AppliedOnly,
    profile::ProfileSpec,
    profiles::SonyFR7,
    request::builtin::{FocusStop, PanTiltStop, ZoomStop},
    transport::{
        BlockingTransport, HasTransportConfig, ReceiveOutcome, SendSemantics, TransportConfig,
    },
    types::{PanSpeed, TiltSpeed},
    Error, OperationalTuning,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
const FOCUS_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x00, 0xff];
const PAN_TILT_STOP_PREFIX: &[u8] = &[0x81, 0x01, 0x06, 0x01];

/// A two-socket camera: every accepted command is answered with an ACK and a
/// completion on alternating sockets, in the order the commands were written.
#[derive(Debug)]
struct TwoSocketTransport {
    config: TransportConfig,
    responses: VecDeque<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    next_socket: u8,
    sony: bool,
}

impl TwoSocketTransport {
    fn new(config: TransportConfig) -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
        let writes = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                config,
                responses: VecDeque::new(),
                writes: Arc::clone(&writes),
                next_socket: 0,
                sony: false,
            },
            writes,
        )
    }

    fn with_sony(mut self) -> Self {
        self.sony = true;
        self
    }
}

impl HasTransportConfig for TwoSocketTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for TwoSocketTransport {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: CommandKind,
        _timeout: Duration,
    ) -> Result<(), Error> {
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        let socket = (self.next_socket % 2) + 1;
        self.next_socket = self.next_socket.wrapping_add(1);
        let ack = vec![0x90, 0x40 | socket, 0xff];
        let completion = vec![0x90, 0x50 | socket, 0xff];
        if self.sony {
            self.responses.push_back(sony_reply(bytes, &ack));
            self.responses.push_back(sony_reply(bytes, &completion));
        } else {
            self.responses.push_back(ack);
            self.responses.push_back(completion);
        }
        Ok(())
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        _timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        let response = self.responses.pop_front().ok_or(Error::io_timeout())?;
        Ok(ReceiveOutcome::copy_message(&response, dst))
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

fn pan_tilt_stop() -> PanTiltStop {
    PanTiltStop::new(
        PanSpeed::new(1).expect("valid pan speed"),
        TiltSpeed::new(1).expect("valid tilt speed"),
    )
}

fn session_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("two-socket runtime profile"),
    )
}

fn sony_session_config() -> SessionConfig {
    SessionConfig::new(ProfileSpec::from_compile_time::<SonyFR7>().expect("Sony FR7 profile"))
}

fn sony_reply(request: &[u8], payload: &[u8]) -> Vec<u8> {
    assert!(
        request.len() >= 8,
        "Sony request must carry its sequence header"
    );
    let mut response = Vec::with_capacity(8 + payload.len());
    response.extend_from_slice(&[0x01, 0x11]);
    response.extend_from_slice(
        &u16::try_from(payload.len())
            .expect("Sony reply payload length")
            .to_be_bytes(),
    );
    response.extend_from_slice(&request[4..8]);
    response.extend_from_slice(payload);
    response
}

fn sony_payload(frame: &[u8]) -> &[u8] {
    assert!(
        frame.len() >= 8,
        "Sony write must carry its sequence header"
    );
    &frame[8..]
}

fn written(writes: &Arc<Mutex<Vec<Vec<u8>>>>) -> Vec<Vec<u8>> {
    writes.lock().expect("writes lock").clone()
}

const FIRST_WRITE_ERROR: &str = "distinctive first-write transport failure";

/// Shared observations for transports whose ownership moves into `Session`.
/// `attempts` includes failed writes; `writes` contains only sends accepted by
/// the transport.
#[derive(Clone, Debug)]
struct IoProbe {
    attempts: Arc<Mutex<Vec<Vec<u8>>>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl IoProbe {
    fn new() -> Self {
        Self {
            attempts: Arc::new(Mutex::new(Vec::new())),
            writes: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn attempts(&self) -> Vec<Vec<u8>> {
        self.attempts.lock().expect("attempts lock").clone()
    }

    fn writes(&self) -> Vec<Vec<u8>> {
        self.writes.lock().expect("writes lock").clone()
    }
}

/// A datagram transport whose first send fails before accepting bytes, then
/// recovers for a later submission. It records attempted and successful sends
/// separately so an operation cannot hide a failed first write as a no-write
/// path.
#[derive(Debug)]
struct FirstWriteFailureTransport {
    config: TransportConfig,
    responses: VecDeque<Vec<u8>>,
    probe: IoProbe,
    sends: usize,
    next_socket: u8,
}

impl FirstWriteFailureTransport {
    fn new(config: TransportConfig) -> (Self, IoProbe) {
        let probe = IoProbe::new();
        (
            Self {
                config,
                responses: VecDeque::new(),
                probe: probe.clone(),
                sends: 0,
                next_socket: 0,
            },
            probe,
        )
    }

    fn queue_reply(&mut self) {
        let socket = (self.next_socket % 2) + 1;
        self.next_socket = self.next_socket.wrapping_add(1);
        self.responses.push_back(vec![0x90, 0x40 | socket, 0xff]);
        self.responses.push_back(vec![0x90, 0x50 | socket, 0xff]);
    }
}

impl HasTransportConfig for FirstWriteFailureTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for FirstWriteFailureTransport {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: CommandKind,
        _timeout: Duration,
    ) -> Result<(), Error> {
        self.sends = self.sends.saturating_add(1);
        self.probe
            .attempts
            .lock()
            .expect("attempts lock")
            .push(bytes.to_vec());
        if self.sends == 1 {
            return Err(Error::TransportError(FIRST_WRITE_ERROR.into()));
        }
        self.probe
            .writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        self.queue_reply();
        Ok(())
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        _timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        let response = self.responses.pop_front().ok_or(Error::io_timeout())?;
        Ok(ReceiveOutcome::copy_message(&response, dst))
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

/// A datagram transport with a small independent pacing guard. The profile
/// used by the pacing test requires a longer interval, so a skipped profile
/// wait is rejected by this transport without relying on a wall-clock lower
/// bound assertion in the test itself.
#[derive(Debug)]
struct PacingTransport {
    config: TransportConfig,
    responses: VecDeque<Vec<u8>>,
    probe: IoProbe,
    minimum_spacing: Duration,
    next_eligible: Option<Instant>,
    next_socket: u8,
    sony: bool,
}

impl PacingTransport {
    fn new(minimum_spacing: Duration) -> (Self, IoProbe) {
        let probe = IoProbe::new();
        (
            Self {
                config: TransportConfig::default(),
                responses: VecDeque::new(),
                probe: probe.clone(),
                minimum_spacing,
                next_eligible: None,
                next_socket: 0,
                sony: false,
            },
            probe,
        )
    }

    fn with_sony(mut self) -> Self {
        self.sony = true;
        self
    }

    fn queue_reply(&mut self, request: &[u8]) {
        let socket = (self.next_socket % 2) + 1;
        self.next_socket = self.next_socket.wrapping_add(1);
        let ack = vec![0x90, 0x40 | socket, 0xff];
        let completion = vec![0x90, 0x50 | socket, 0xff];
        if self.sony {
            self.responses.push_back(sony_reply(request, &ack));
            self.responses.push_back(sony_reply(request, &completion));
        } else {
            self.responses.push_back(ack);
            self.responses.push_back(completion);
        }
    }
}

impl HasTransportConfig for PacingTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for PacingTransport {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: CommandKind,
        _timeout: Duration,
    ) -> Result<(), Error> {
        let now = Instant::now();
        self.probe
            .attempts
            .lock()
            .expect("attempts lock")
            .push(bytes.to_vec());
        if self.next_eligible.is_some_and(|eligible| now < eligible) {
            return Err(Error::TransportError("transport pacing violation".into()));
        }
        self.probe
            .writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        self.next_eligible = Some(now + self.minimum_spacing);
        self.queue_reply(bytes);
        Ok(())
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        _timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        let response = self.responses.pop_front().ok_or(Error::io_timeout())?;
        Ok(ReceiveOutcome::copy_message(&response, dst))
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

/// A third public operation over a two-socket target is admitted and queued
/// while both sockets are busy, then written once one frees. Nothing is
/// rejected and nothing is written twice.
#[test]
fn a_third_operation_queues_until_a_socket_frees() {
    let (transport, writes) = TwoSocketTransport::new(TransportConfig::default());
    let session =
        Session::open(transport.with_sony(), sony_session_config()).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    let mut first = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first submission");
    let mut second = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("second submission");
    let mut third = camera
        .submit::<AppliedOnly, _>(&pan_tilt_stop())
        .expect("a third operation is admitted and queued");

    first.applied().expect("first operation applied");
    second.applied().expect("second operation applied");
    third.applied().expect("queued operation applied");

    let written = written(&writes);
    assert_eq!(written.len(), 3, "each operation writes exactly once");
    assert_eq!(sony_payload(&written[0]), ZOOM_STOP);
    assert_eq!(sony_payload(&written[1]), FOCUS_STOP);
    assert!(sony_payload(&written[2]).starts_with(PAN_TILT_STOP_PREFIX));

    let metrics = session.metrics().expect("metrics");
    assert_eq!(metrics.admitted, 3);
    assert_eq!(metrics.terminal, 3);
    assert_eq!(metrics.active, 0);
    session.shutdown().expect("owner shutdown");
}

/// The same holds when a target is tuned to one command socket: the second
/// operation waits in the owner's queue and is written only after the first
/// one completes.
#[test]
fn a_one_socket_target_queues_the_second_operation() {
    let config = session_config().with_tuning(OperationalTuning::new().maximum_command_sockets(1));
    let (transport, writes) = TwoSocketTransport::new(TransportConfig::default());
    let session = Session::open(transport, config).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let mut first = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first operation");
    let mut second = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("the second operation is admitted while the one socket is busy");
    first.applied().expect("first operation applied");
    second.applied().expect("queued operation applied");
    assert_eq!(written(&writes), [ZOOM_STOP.to_vec(), FOCUS_STOP.to_vec()]);
    session.shutdown().expect("owner shutdown");
}

/// A typed operation whose datagram send fails is still admitted; the exact
/// transport error is its outcome (D24). The terminalized entry releases its
/// shared permit, so a one-slot session can immediately admit and complete a
/// later operation.
#[test]
fn a_failed_operation_write_is_its_outcome_and_releases_the_permit() {
    let config =
        session_config().with_admission_capacity(NonZeroUsize::new(1).expect("one permit"));
    let (transport, probe) = FirstWriteFailureTransport::new(TransportConfig::default());
    let session = Session::open(transport, config).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let error = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission succeeds at admission")
        .applied()
        .expect_err("the failed write is the operation's outcome");
    match &error {
        Error::TransportError(reason) => assert_eq!(reason.as_ref(), FIRST_WRITE_ERROR),
        other => panic!("expected the exact transport error, got {other:?}"),
    }
    assert_eq!(probe.attempts().len(), 1, "the failed operation tried once");
    assert!(
        probe.writes().is_empty(),
        "the failed send accepted no bytes"
    );

    let metrics = session.metrics().expect("metrics after failed write");
    assert_eq!(metrics.admitted, 1);
    assert_eq!(metrics.terminal, 1);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.pending, 0);
    assert_eq!(metrics.writes, 1, "metrics count the attempted write");
    assert_eq!(metrics.write_failures, 1);

    // With only one admission permit, this proves that terminal removal also
    // released the permit rather than merely dropping the public observer.
    camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("the terminalized request released its admission permit")
        .applied()
        .expect("the later datagram operation must recover normally");
    assert_eq!(probe.attempts().len(), 2);
    assert_eq!(probe.writes().len(), 1);

    let metrics = session.metrics().expect("metrics after recovery");
    assert_eq!(metrics.admitted, 2);
    assert_eq!(metrics.terminal, 2);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.pending, 0);
    assert_eq!(metrics.writes, 2);
    assert_eq!(metrics.write_failures, 1);
    session.shutdown().expect("owner shutdown");
}

/// Profile pacing holds the second write back even though the second socket
/// is free; the transport guard catches an owner that skips that wait.
#[test]
fn typed_operations_respect_profile_pacing() {
    let profile = ProfileSpec::from_compile_time::<SonyFR7>().expect("Sony FR7 profile");
    assert_eq!(
        profile.timing().minimum_command_spacing(),
        Duration::from_millis(35),
        "the pacing precondition must be nonzero"
    );
    let (transport, probe) = PacingTransport::new(Duration::from_millis(30));
    let session =
        Session::open(transport.with_sony(), SessionConfig::new(profile)).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    let mut first = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first operation");
    let mut second = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("second operation");
    first.applied().expect("first operation applied");
    second.applied().expect("second operation applied");
    assert_eq!(probe.attempts().len(), 2, "no write violated pacing");
    assert_eq!(probe.writes().len(), 2);
    session.shutdown().expect("owner shutdown");
}
