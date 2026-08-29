//! Issue #561: blocking submission behavior at the first-write boundary.
//!
//! Public blocking operation handles are returned only after their own first
//! write succeeds. A request that loses the socket race is terminalized as
//! `TransportBusy`; ordinary owner receipts still retain bounded queueing.

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
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
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
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
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

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        self.recv_into_with_timeout(dst, Duration::from_secs(1))
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        _timeout: Duration,
    ) -> Result<usize, Error> {
        let response = self.responses.pop_front().ok_or(Error::Timeout)?;
        dst[..response.len()].copy_from_slice(&response);
        Ok(response.len())
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
/// the transport, and `reads` counts receive calls. Keeping those distinctions
/// lets the public blocking tests assert the exact first-write boundary.
#[derive(Clone, Debug)]
struct IoProbe {
    attempts: Arc<Mutex<Vec<Vec<u8>>>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    reads: Arc<Mutex<usize>>,
}

impl IoProbe {
    fn new() -> Self {
        Self {
            attempts: Arc::new(Mutex::new(Vec::new())),
            writes: Arc::new(Mutex::new(Vec::new())),
            reads: Arc::new(Mutex::new(0)),
        }
    }

    fn attempts(&self) -> Vec<Vec<u8>> {
        self.attempts.lock().expect("attempts lock").clone()
    }

    fn writes(&self) -> Vec<Vec<u8>> {
        self.writes.lock().expect("writes lock").clone()
    }

    fn reads(&self) -> usize {
        *self.reads.lock().expect("reads lock")
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
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
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

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        self.recv_into_with_timeout(dst, Duration::from_secs(1))
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        _timeout: Duration,
    ) -> Result<usize, Error> {
        *self.probe.reads.lock().expect("reads lock") += 1;
        let response = self.responses.pop_front().ok_or(Error::Timeout)?;
        dst[..response.len()].copy_from_slice(&response);
        Ok(response.len())
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
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
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

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        self.recv_into_with_timeout(dst, Duration::from_secs(1))
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        _timeout: Duration,
    ) -> Result<usize, Error> {
        *self.probe.reads.lock().expect("reads lock") += 1;
        let response = self.responses.pop_front().ok_or(Error::Timeout)?;
        dst[..response.len()].copy_from_slice(&response);
        Ok(response.len())
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

/// A third public operation over a two-socket target is rejected immediately,
/// without a write or an escaped handle. Once the first operation completes,
/// the caller can resubmit successfully.
#[test]
fn third_blocking_operation_rejects_until_a_socket_frees() {
    let (transport, writes) = TwoSocketTransport::new(TransportConfig::default());
    let session =
        Session::open(transport.with_sony(), sony_session_config()).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    let first = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first submission");
    let second = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("second submission");
    let error = camera
        .submit::<AppliedOnly, _>(&pan_tilt_stop())
        .expect_err("issue #542: a third operation must not queue before first write");
    assert!(matches!(error, Error::TransportBusy));

    let before = written(&writes);
    assert_eq!(
        before.len(),
        2,
        "only the two successful operation submissions may write"
    );
    assert_eq!(sony_payload(&before[0]), ZOOM_STOP);
    assert_eq!(sony_payload(&before[1]), FOCUS_STOP);

    let metrics = session
        .metrics()
        .expect("metrics after first-write rejection");
    assert_eq!(metrics.admitted, 3);
    assert_eq!(metrics.terminal, 1);
    assert_eq!(metrics.active, 2);
    assert_eq!(metrics.pending, 0);
    assert_eq!(metrics.writes, 2);

    first.applied().expect("first operation applied");
    let replacement = camera
        .submit::<AppliedOnly, _>(&pan_tilt_stop())
        .expect("a resubmission after a socket frees must write");
    let drained = written(&writes);
    assert_eq!(
        drained.len(),
        3,
        "the replacement request writes exactly once"
    );
    assert!(sony_payload(&drained[2]).starts_with(PAN_TILT_STOP_PREFIX));

    second.applied().expect("second operation applied");
    replacement
        .applied()
        .expect("replacement operation applied");

    assert_eq!(written(&writes).len(), 3, "nothing is written twice");
    session.shutdown().expect("owner shutdown");
}

/// A first-write rejection releases the shared admission permit. Repeating
/// the attempt therefore remains a `TransportBusy` rejection rather than
/// turning into a queue-depth rejection, and the original operations remain
/// observable.
#[test]
fn first_write_rejection_releases_admission_capacity() {
    let config = sony_session_config()
        .with_admission_capacity(NonZeroUsize::new(3).expect("non-zero queue depth"));
    let (transport, writes) = TwoSocketTransport::new(TransportConfig::default());
    let session = Session::open(transport.with_sony(), config).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    let first = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first submission");
    let second = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("second submission");

    let error = camera
        .submit::<AppliedOnly, _>(&pan_tilt_stop())
        .expect_err("the third operation must reject while both sockets are occupied");
    assert!(
        matches!(error, Error::TransportBusy),
        "expected a first-write rejection, got {error:?}"
    );
    assert_eq!(
        written(&writes).len(),
        2,
        "a first-write rejection never writes"
    );

    let repeated = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect_err("the released permit must not be mistaken for a writable socket");
    assert!(matches!(repeated, Error::TransportBusy));
    assert_eq!(written(&writes).len(), 2);

    first.applied().expect("first operation applied");
    second.applied().expect("second operation applied");

    // With the sockets and permits released, the same operation is admitted
    // and written normally.
    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("capacity is released by terminal operations")
        .applied()
        .expect("re-admitted operation applied");

    session.shutdown().expect("owner shutdown");
}

/// The same invariant holds when a target is tuned to one command socket: a
/// second operation rejects without writing, and a later resubmission works
/// once the first operation reaches its terminal outcome.
#[test]
fn one_socket_operation_rejection_is_immediate_and_retryable() {
    let config = session_config()
        .with_tuning(OperationalTuning::new().maximum_command_sockets(1))
        .expect("one socket is within the profile limit");
    let (transport, writes) = TwoSocketTransport::new(TransportConfig::default());
    let session = Session::open(transport, config).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let first = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first operation");
    let error = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect_err("the second operation must not wait for the one socket");
    assert!(matches!(error, Error::TransportBusy));
    assert_eq!(written(&writes).len(), 1);

    first.applied().expect("first operation applied");
    let replacement = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("resubmission after the socket frees");
    assert_eq!(written(&writes).len(), 2);
    replacement
        .applied()
        .expect("replacement operation applied");
    session.shutdown().expect("owner shutdown");
}

/// A typed operation that fails during its first datagram send returns that
/// exact transport error without manufacturing a handle. The terminalized
/// entry releases its shared permit, so a one-slot session can immediately
/// admit and complete a later operation.
#[test]
fn typed_operation_first_write_failure_returns_exact_error_and_releases_permit() {
    let config =
        session_config().with_admission_capacity(NonZeroUsize::new(1).expect("one permit"));
    let (transport, probe) = FirstWriteFailureTransport::new(TransportConfig::default());
    let session = Session::open(transport, config).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let error = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect_err("a failed first write must not return an operation handle");
    match &error {
        Error::TransportError(reason) => assert_eq!(reason.as_ref(), FIRST_WRITE_ERROR),
        other => panic!("expected the exact first-write transport error, got {other:?}"),
    }
    assert!(!matches!(error, Error::TransportBusy));
    assert_eq!(probe.attempts().len(), 1, "the failed operation tried once");
    assert!(
        probe.writes().is_empty(),
        "the failed send accepted no bytes"
    );

    let metrics = session.metrics().expect("metrics after failed first write");
    assert_eq!(metrics.admitted, 1);
    assert_eq!(metrics.terminal, 1);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.pending, 0);
    assert_eq!(metrics.writes, 1, "metrics count the attempted write");
    assert_eq!(metrics.write_failures, 1);

    // With only one admission permit, this proves that terminal removal also
    // released the permit rather than merely dropping the public observer.
    let recovered = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("the terminalized request released its admission permit");
    assert_eq!(probe.attempts().len(), 2);
    assert_eq!(probe.writes().len(), 1);
    recovered
        .applied()
        .expect("the later datagram operation must recover normally");

    let metrics = session.metrics().expect("metrics after recovery");
    assert_eq!(metrics.admitted, 2);
    assert_eq!(metrics.terminal, 2);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.pending, 0);
    assert_eq!(metrics.writes, 2);
    assert_eq!(metrics.write_failures, 1);
    assert_eq!(
        probe.reads(),
        2,
        "only the recovered handle pumps its reply"
    );
    session.shutdown().expect("owner shutdown");
}

/// Profile pacing is a submission wait, not permission to pump an earlier
/// request. The Sony FR7 profile's nonzero command minimum makes the second
/// typed submission wait even though the second socket is available; the
/// transport guard catches an implementation that skips that pacing wait.
#[test]
fn typed_operation_waits_for_profile_pacing_without_pumping_peer_replies() {
    let profile = ProfileSpec::from_compile_time::<SonyFR7>().expect("Sony FR7 profile");
    assert_eq!(
        profile.timing().minimum_command_spacing(),
        Duration::from_millis(35),
        "the pacing precondition must be nonzero"
    );
    let (transport, probe) = PacingTransport::new(Duration::from_millis(30));
    let config = SessionConfig::new(profile)
        // The intentional pacing wait must not let the first peer's short
        // profile ACK deadline expire before its explicit handle is observed.
        .with_tuning(OperationalTuning::new().ack_timeout(Duration::from_secs(1)))
        .expect("widened ACK deadline");
    let session = Session::open(transport.with_sony(), config).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    let first = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first operation");
    assert_eq!(probe.attempts().len(), 1);
    assert_eq!(probe.writes().len(), 1);
    assert_eq!(probe.reads(), 0);

    // The second socket is free, but profile pacing is not. A correct submit
    // waits only for this request's dispatch eligibility and does not receive
    // the first operation's queued ACK/completion while waiting.
    let second = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("second operation waits for and then performs its own write");
    assert_eq!(probe.attempts().len(), 2);
    assert_eq!(probe.writes().len(), 2);
    assert_eq!(probe.reads(), 0, "submit must not pump either peer reply");

    first
        .applied()
        .expect("the first operation still owns its queued replies");
    second
        .applied()
        .expect("the second operation still owns its queued replies");
    assert_eq!(
        probe.reads(),
        4,
        "only explicit handle waits consume replies"
    );
    session.shutdown().expect("owner shutdown");
}
