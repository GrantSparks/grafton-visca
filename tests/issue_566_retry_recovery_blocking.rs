//! Issue #566: retry coverage through the blocking facade.
//!
//! The engine tests in `src/runtime/engine/tests.rs` pin the state machine.
//! This file pins what a caller actually observes: which camera refusals are
//! replayed and which are surfaced, that a sequence-correlated Sony movement
//! command survives a lost ACK and a silent post-ACK camera, and that an
//! unresolvable Sony cancellation reaches the caller as
//! `Error::CancellationUnconfirmed`. Raw commands deliberately retain their
//! stricter unsequenced ambiguity verdict.
//!
//! The async twin is `tests/issue_566_retry_recovery_async.rs`.

#![cfg(feature = "blocking")]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    command::CommandKind,
    completion::AppliedOnly,
    profile::ProfileSpec,
    profiles::SonyFR7,
    request::{self, builtin::ZoomStop},
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    CameraId, ControlClass, Error, Request, RetryClass, TimeoutClass,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

/// A plain command in the standard retry class.
///
/// The distinction this file pins is the retry class, so the command is
/// declared here rather than borrowed from a built-in whose profile support
/// would be a second variable.
#[derive(Debug)]
struct StandardCommand;

impl Request for StandardCommand {
    type Class = request::Plain;
    const MAX_SIZE: usize = 3;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
    const RETRY_CLASS: RetryClass = RetryClass::Standard;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn write_into(&self, target: CameraId, out: &mut [u8]) -> Result<usize, Error> {
        out[..3].copy_from_slice(&[target.to_address_byte(), 0x01, 0xff]);
        Ok(3)
    }
}

const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
const COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x51, 0xff];
/// `0x41` — the camera refuses to execute right now.
const NOT_EXECUTABLE: &[u8] = &[0x90, 0x60, 0x41, 0xff];
/// `0x05` — the camera has no free command socket.
const NO_SOCKET: &[u8] = &[0x90, 0x60, 0x05, 0xff];

/// A camera whose answer to the n-th write is scripted.
#[derive(Debug)]
struct ScriptTransport {
    config: TransportConfig,
    sony: bool,
    script: VecDeque<Vec<Vec<u8>>>,
    trailing: Vec<Vec<u8>>,
    reads: VecDeque<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl ScriptTransport {
    fn new(script: Vec<Vec<Vec<u8>>>) -> Self {
        Self {
            config: TransportConfig::default(),
            sony: false,
            script: script.into(),
            trailing: Vec::new(),
            reads: VecDeque::new(),
            writes: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Answer every write past the explicit script with these frames.
    fn with_trailing(mut self, trailing: Vec<Vec<u8>>) -> Self {
        self.trailing = trailing;
        self
    }

    /// Use Sony's sequence-bearing VISCA-over-IP envelope for scripted
    /// replies. Raw VISCA remains the default so the camera-refusal tests
    /// continue to exercise raw protocol evidence.
    fn with_sony(mut self) -> Self {
        self.sony = true;
        self
    }

    fn probe(&self) -> Probe {
        Probe {
            writes: Arc::clone(&self.writes),
        }
    }
}

#[derive(Clone, Debug)]
struct Probe {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl Probe {
    fn writes(&self) -> Vec<Vec<u8>> {
        self.writes.lock().expect("writes lock").clone()
    }
}

impl HasTransportConfig for ScriptTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for ScriptTransport {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        let replies = self
            .script
            .pop_front()
            .unwrap_or_else(|| self.trailing.clone());
        self.reads.extend(
            replies
                .into_iter()
                .map(|reply| frame_reply(bytes, &reply, self.sony)),
        );
        Ok(())
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        self.recv_into_with_timeout(dst, Duration::from_millis(1))
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, Error> {
        let Some(bytes) = self.reads.pop_front() else {
            // A silent camera must not spin the owner's pump; a short pause is
            // what a real socket read would do while its deadline runs down.
            std::thread::sleep(timeout.min(Duration::from_millis(2)));
            return Err(Error::Timeout);
        };
        dst[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
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

fn frame_reply(request: &[u8], payload: &[u8], sony: bool) -> Vec<u8> {
    if !sony {
        return payload.to_vec();
    }
    assert!(
        request.len() >= 8,
        "Sony request must carry its sequence header"
    );
    let mut frame = Vec::with_capacity(8 + payload.len());
    frame.extend_from_slice(&[0x01, 0x11]);
    frame.extend_from_slice(
        &u16::try_from(payload.len())
            .expect("Sony reply payload length")
            .to_be_bytes(),
    );
    frame.extend_from_slice(&request[4..8]);
    frame.extend_from_slice(payload);
    frame
}

fn sony_sequence(frame: &[u8]) -> u32 {
    assert!(
        frame.len() >= 8,
        "Sony write must carry its sequence header"
    );
    u32::from_be_bytes(frame[4..8].try_into().expect("Sony sequence bytes"))
}

fn standard_reply() -> Vec<Vec<u8>> {
    vec![ACK_SOCKET_ONE.to_vec(), COMPLETE_SOCKET_ONE.to_vec()]
}

/// Issue #566: `0x41` is retried for a movement-class request. Nothing drove
/// this code through a facade before.
#[test]
fn a_refused_movement_command_is_replayed_and_then_succeeds() {
    let transport = ScriptTransport::new(vec![vec![NOT_EXECUTABLE.to_vec()], standard_reply()])
        .with_trailing(standard_reply());
    let probe = transport.probe();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission")
        .applied_with_timeout(Duration::from_secs(5))
        .expect("a refused movement command must be replayed, not failed");

    assert_eq!(
        probe.writes().len(),
        2,
        "the refusal is transient for a movement request, so the frame is reissued"
    );
    session.shutdown().expect("owner shutdown");
}

/// Issue #566: the same `0x41` is terminal for a standard-class request, which
/// is the whole point of the `movement_not_executable` distinction.
#[test]
fn a_refused_standard_command_surfaces_the_refusal_without_replay() {
    let transport =
        ScriptTransport::new(vec![vec![NOT_EXECUTABLE.to_vec()]]).with_trailing(standard_reply());
    let probe = transport.probe();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let error = camera
        .execute(&StandardCommand)
        .expect_err("a standard request must surface the camera's refusal");
    assert!(
        matches!(error, Error::CommandNotExecutable),
        "expected the camera's own refusal, got {error:?}"
    );
    assert_eq!(
        probe.writes().len(),
        1,
        "a standard request must not replay a refusal"
    );

    // The session is unharmed by a refused command.
    camera
        .execute(&StandardCommand)
        .expect("the session survives a refused command");
    session.shutdown().expect("owner shutdown");
}

/// Issue #566: `0x05` (`NoSocket`) is a capacity answer, so it is replayed for
/// a standard request too — unlike `0x41`.
#[test]
fn a_no_socket_answer_is_replayed_for_a_standard_command() {
    let transport = ScriptTransport::new(vec![vec![NO_SOCKET.to_vec()], standard_reply()])
        .with_trailing(standard_reply());
    let probe = transport.probe();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .execute(&StandardCommand)
        .expect("a camera with no free socket must be retried, not failed");
    assert_eq!(
        probe.writes().len(),
        2,
        "a capacity answer is replayed regardless of retry class"
    );
    session.shutdown().expect("owner shutdown");
}

/// Issue #566: a sequence-correlated Sony movement request survives a lost
/// ACK. Raw VISCA has no request identity after a successful write, so replay
/// there would be unsafe and is covered by the poison-path tests instead.
#[test]
fn a_movement_command_survives_a_lost_ack() {
    // The first write draws no answer at all; the ACK deadline lapses and the
    // frame is reissued.
    let transport = ScriptTransport::new(vec![Vec::new(), standard_reply()])
        .with_trailing(standard_reply())
        .with_sony();
    let probe = transport.probe();
    let session = Session::open(transport, sony_session_config()).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission")
        .applied_with_timeout(Duration::from_secs(5))
        .expect("a lost ACK must be retried for a movement request");
    assert_eq!(probe.writes().len(), 2, "the lost ACK reissues the frame");
    let writes = probe.writes();
    assert_eq!(
        sony_sequence(&writes[0]),
        sony_sequence(&writes[1]),
        "a Sony retry must preserve the logical request sequence"
    );
    session.shutdown().expect("owner shutdown");
}

/// Issue #566: a sequence-correlated Sony post-ACK completion timeout is
/// retried. `prepared.rs` hard-coded `completion_timeout: false`, so a camera
/// that ACKed and then went silent failed on its first deadline with no second
/// attempt.
#[test]
fn a_silent_camera_after_its_ack_is_retried() {
    let transport = ScriptTransport::new(vec![vec![ACK_SOCKET_ONE.to_vec()], standard_reply()])
        .with_trailing(standard_reply())
        .with_sony();
    let probe = transport.probe();
    let session = Session::open(transport, sony_session_config()).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission")
        .applied_with_timeout(Duration::from_secs(10))
        .expect("a post-ACK completion timeout must be retried");
    assert_eq!(
        probe.writes().len(),
        2,
        "the completion timeout reissues the frame"
    );
    let writes = probe.writes();
    assert_eq!(
        sony_sequence(&writes[0]),
        sony_sequence(&writes[1]),
        "a Sony retry must preserve the logical request sequence"
    );
    session.shutdown().expect("owner shutdown");
}

/// Issue #566: `Error::CancellationUnconfirmed` reaches the caller through a
/// sequence-correlated Sony owner. A raw command with the same ambiguity must
/// poison its session as `UnsequencedCommandUnconfirmed` instead.
#[test]
fn an_unresolvable_cancellation_reaches_the_caller() {
    // The camera never answers anything: neither the command nor the cancel.
    let transport = ScriptTransport::new(vec![Vec::new()]);
    let probe = transport.probe();
    let session =
        Session::open(transport.with_sony(), sony_session_config()).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    let operation = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission");
    let cancellation = operation.cancel().expect("cancellation intent is recorded");

    let error = cancellation
        .outcome(Duration::from_secs(5))
        .expect_err("a camera that never answers cannot confirm a cancellation");
    assert!(
        matches!(error, Error::CancellationUnconfirmed),
        "expected the ambiguity verdict, got {error:?}"
    );
    assert!(
        !probe.writes().is_empty(),
        "the original frame was transmitted"
    );
    session.shutdown().expect("owner shutdown");
}
