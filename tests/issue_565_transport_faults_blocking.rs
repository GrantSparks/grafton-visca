//! Issue #565: 1.x transport fault tolerance, through the blocking facade.
//!
//! Each test pins one transport boundary: a raw receive fault while an ACK is
//! unconfirmed fails only that command without replay and keeps the session
//! alive (issue #671; the strict opt-in still poisons), sequence-correlated Sony
//! receive faults retry on the same sequence, failed stream writes still
//! report their session-wide poison, socketless frames still work, occupied
//! sockets fall back to the free socket, and write-racing replies are matched on
//! the first read pump.
//!
//! The engine's deferred-ACK latch (#297) is *not* observable from here: these
//! owners apply a write and its result back to back, so nothing can reach the
//! engine in between. That guarantee is pinned in `runtime::engine::tests`
//! instead, where inputs can be interleaved directly (#636).

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
    request::builtin::{FocusStop, ZoomStop},
    transport::{
        AddressingMode, BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig,
    },
    CameraId, Error, OperationalTuning,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
const ACK_SOCKET_TWO: &[u8] = &[0x90, 0x42, 0xff];
const COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x51, 0xff];
const COMPLETE_SOCKET_TWO: &[u8] = &[0x90, 0x52, 0xff];
const ACK_NO_SOCKET: &[u8] = &[0x90, 0x40, 0xff];
const COMPLETE_NO_SOCKET: &[u8] = &[0x90, 0x50, 0xff];

/// What the camera does when the n-th frame is written.
#[derive(Debug, Clone)]
enum OnSend {
    /// Accept the write and queue these reads, in order.
    Reply(Vec<Vec<u8>>),
    /// Accept the write and queue replies whose Sony sequence may come from
    /// this write or the preceding one.
    ReplyWithSequences(Vec<(ReplySequence, Vec<u8>)>),
    /// Fail the write itself.
    Fail(Error),
}

#[derive(Debug, Clone, Copy)]
enum ReplySequence {
    Current,
    Previous,
}

/// A scripted blocking camera whose reads may fail.
#[derive(Debug)]
struct FaultTransport {
    config: TransportConfig,
    semantics: SendSemantics,
    sony: bool,
    script: VecDeque<OnSend>,
    trailing: Vec<Vec<u8>>,
    reads: VecDeque<Result<Vec<u8>, Error>>,
    faults: VecDeque<(usize, Error)>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    sends: usize,
    last_sequence: Option<[u8; 4]>,
}

impl FaultTransport {
    fn new(semantics: SendSemantics, script: Vec<OnSend>) -> Self {
        Self {
            config: TransportConfig::default(),
            semantics,
            sony: false,
            script: script.into(),
            trailing: Vec::new(),
            reads: VecDeque::new(),
            faults: VecDeque::new(),
            writes: Arc::new(Mutex::new(Vec::new())),
            sends: 0,
            last_sequence: None,
        }
    }

    fn probe(&self) -> Probe {
        Probe {
            writes: Arc::clone(&self.writes),
        }
    }

    /// Replies used once the explicit script is exhausted.
    fn with_trailing_reply(mut self, replies: Vec<Vec<u8>>) -> Self {
        self.trailing = replies;
        self
    }

    fn with_sony(mut self) -> Self {
        self.sony = true;
        self
    }

    fn with_serial_addressing(mut self) -> Self {
        self.config.addressing = AddressingMode::Serial;
        self
    }

    /// Inject a read failure before the reply queued by send `send_number`.
    fn with_read_fault(mut self, send_number: usize, error: Error) -> Self {
        self.faults.push_back((send_number, error));
        self
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

impl HasTransportConfig for FaultTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for FaultTransport {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
        self.sends = self.sends.saturating_add(1);
        let step = self
            .script
            .pop_front()
            .unwrap_or_else(|| OnSend::Reply(self.trailing.clone()));
        if let OnSend::Fail(error) = step {
            return Err(error);
        }
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        while self
            .faults
            .front()
            .is_some_and(|(number, _)| *number == self.sends)
        {
            let (_, error) = self.faults.pop_front().expect("checked above");
            self.reads.push_back(Err(error));
        }
        let previous_sequence = self.last_sequence;
        if self.sony {
            self.last_sequence = bytes
                .get(4..8)
                .and_then(|sequence| sequence.try_into().ok());
        }
        match step {
            OnSend::Reply(replies) => {
                for reply in replies {
                    self.reads
                        .push_back(Ok(frame_reply(bytes, &reply, self.sony)));
                }
            }
            OnSend::ReplyWithSequences(replies) => {
                for (sequence, reply) in replies {
                    let sequence = match sequence {
                        ReplySequence::Current => Some(&bytes[4..8]),
                        ReplySequence::Previous => previous_sequence
                            .as_ref()
                            .map(|sequence| sequence.as_slice()),
                    };
                    self.reads
                        .push_back(Ok(frame_reply_with_sequence(&reply, self.sony, sequence)));
                }
            }
            OnSend::Fail(_) => unreachable!("failed sends return before replies are queued"),
        }
        Ok(())
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        self.recv_into_with_timeout(dst, Duration::from_millis(1))
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        _timeout: Duration,
    ) -> Result<usize, Error> {
        let bytes = self.reads.pop_front().ok_or(Error::Timeout)??;
        dst[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }

    fn send_semantics(&self) -> SendSemantics {
        self.semantics
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(self.config.addressing)
    }
}

fn frame_reply(request: &[u8], payload: &[u8], sony: bool) -> Vec<u8> {
    if !sony {
        return payload.to_vec();
    }
    assert!(
        request.len() >= 8,
        "Sony request must carry its sequence header"
    );
    frame_reply_with_sequence(payload, sony, Some(&request[4..8]))
}

fn frame_reply_with_sequence(payload: &[u8], sony: bool, sequence: Option<&[u8]>) -> Vec<u8> {
    if !sony {
        return payload.to_vec();
    }
    let sequence = sequence.expect("Sony reply must carry a sequence");
    assert_eq!(sequence.len(), 4, "Sony sequence has four bytes");
    let mut frame = Vec::with_capacity(8 + payload.len());
    frame.extend_from_slice(&[0x01, 0x11]);
    frame.extend_from_slice(
        &u16::try_from(payload.len())
            .expect("Sony reply payload length")
            .to_be_bytes(),
    );
    frame.extend_from_slice(sequence);
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

fn session_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("two-socket runtime profile"),
    )
}

fn sony_session_config() -> SessionConfig {
    SessionConfig::new(ProfileSpec::from_compile_time::<SonyFR7>().expect("Sony FR7 profile"))
}

fn multi_target_session_config() -> SessionConfig {
    let mut config = session_config();
    config
        .register_target(
            CameraId::CAMERA_2,
            ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
                .expect("two-socket runtime profile"),
        )
        .expect("second raw target");
    config
}

fn standard_reply() -> Vec<Vec<u8>> {
    vec![ACK_SOCKET_ONE.to_vec(), COMPLETE_SOCKET_ONE.to_vec()]
}

/// Issue #671: a raw receive fault while a successfully sent command awaits its
/// ACK leaves that one command's acceptance uncertain, but by default it does
/// not poison the session. The command is never replayed (a raw command may
/// already have reached the camera) and eventually fails
/// `UnsequencedCommandUnconfirmed` — a per-request outcome that does *not*
/// require a new session — while the session stays usable for later work.
#[test]
fn raw_receive_fault_fails_one_command_and_keeps_the_session() {
    let transport = FaultTransport::new(SendSemantics::Datagram, vec![OnSend::Reply(Vec::new())])
        .with_trailing_reply(standard_reply())
        .with_read_fault(
            1,
            Error::Io(Arc::new(std::io::Error::from(
                std::io::ErrorKind::ConnectionRefused,
            ))),
        );
    let probe = transport.probe();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let error = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission")
        .applied()
        .expect_err("the unconfirmed raw command fails on its own");

    assert!(matches!(error, Error::UnsequencedCommandUnconfirmed));
    assert!(
        !error.requires_new_session(),
        "a raw command that fails on a live session is a per-request outcome, not session death"
    );
    assert_eq!(
        probe.writes().len(),
        1,
        "an unconfirmed raw command is never replayed after a receive fault"
    );

    // The session survived the receive fault: later work still completes.
    camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("the session survives a transient receive fault")
        .applied()
        .expect("later work still completes");

    session.shutdown().expect("owner shutdown");
}

/// Issue #671 strict opt-in, end to end: with
/// `OperationalTuning::strict_unconfirmed_poison(true)`, the same receive fault
/// poisons the whole session, surfaced as `StreamPoisoned` (which requires a
/// replacement session). This exercises the full tuning → adapter → engine
/// plumbing of the opt-in through the real facade.
#[test]
fn strict_opt_in_raw_receive_fault_poisons_the_session() {
    let transport = FaultTransport::new(SendSemantics::Datagram, vec![OnSend::Reply(Vec::new())])
        .with_trailing_reply(standard_reply())
        .with_read_fault(
            1,
            Error::Io(Arc::new(std::io::Error::from(
                std::io::ErrorKind::ConnectionRefused,
            ))),
        );
    let session = Session::open(
        transport,
        session_config()
            .with_tuning(OperationalTuning::new().strict_unconfirmed_poison(true))
            .expect("strict tuning is valid"),
    )
    .expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let error = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission")
        .applied()
        .expect_err("strict mode poisons the session on the receive fault");
    assert!(
        matches!(error, Error::StreamPoisoned { .. }),
        "strict mode surfaces the poison as StreamPoisoned, got {error:?}"
    );
    assert!(
        error.requires_new_session(),
        "a poisoned session must require a replacement"
    );
}

/// A sequence-correlated Sony receive fault can retry the same logical request
/// and keep the session usable. The classic trigger is UDP `recv` reporting
/// ECONNREFUSED after an ICMP port-unreachable for an earlier datagram.
#[test]
fn sony_transient_receive_fault_retries_same_sequence_and_keeps_session() {
    let transport = FaultTransport::new(
        SendSemantics::Datagram,
        vec![OnSend::Reply(Vec::new()), OnSend::Reply(standard_reply())],
    )
    .with_sony()
    .with_trailing_reply(standard_reply())
    .with_read_fault(
        1,
        Error::Io(Arc::new(std::io::Error::from(
            std::io::ErrorKind::ConnectionRefused,
        ))),
    );
    let probe = transport.probe();
    let session = Session::open(transport, sony_session_config()).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission")
        .applied()
        .expect("a Sony receive fault retries on its existing sequence");

    let writes = probe.writes();
    assert_eq!(
        writes.len(),
        2,
        "the sequence-correlated request gets one bounded retry"
    );
    assert_eq!(
        sony_sequence(&writes[0]),
        sony_sequence(&writes[1]),
        "a Sony retry must preserve the logical request sequence"
    );

    // The session survived, so later work still runs on it.
    camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("the session is still usable")
        .applied()
        .expect("later work still completes after the bounded retry");

    session.shutdown().expect("owner shutdown");
}

/// A failed datagram write fails exactly one command and the session keeps
/// running: a datagram is framed by its own boundary, so nothing else is
/// affected. This is the per-command write behavior 1.x had.
#[test]
fn datagram_write_failure_fails_one_command_and_keeps_the_session() {
    let transport = FaultTransport::new(
        SendSemantics::Datagram,
        vec![
            OnSend::Fail(Error::Io(Arc::new(std::io::Error::from(
                std::io::ErrorKind::ConnectionRefused,
            )))),
            OnSend::Reply(standard_reply()),
        ],
    )
    .with_trailing_reply(standard_reply());
    let probe = transport.probe();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let error = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect_err("the first write fails on the wire");
    assert!(
        matches!(error, Error::Io(_)),
        "the caller whose datagram write failed sees its own transport error, got {error:?}"
    );
    assert!(
        !error.requires_new_session(),
        "an isolated datagram write failure is not session death"
    );

    camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("the session survives an isolated datagram write failure")
        .applied()
        .expect("later work still completes");
    assert_eq!(probe.writes().len(), 1, "a failed write is not recorded");

    session.shutdown().expect("owner shutdown");
}

/// A failed *stream* write is a session verdict on purpose: the byte-stream
/// position becomes unknowable, so every affected caller must learn that a
/// replacement session is needed (#564). The exact transport cause is carried
/// in the poison reason rather than thrown away. The two raw commands target
/// different registered cameras so the raw same-target unacknowledged gate
/// does not hide the session-wide stream boundary.
#[test]
fn stream_write_failure_poisons_and_names_the_transport_cause() {
    let transport = FaultTransport::new(
        SendSemantics::Stream,
        vec![
            OnSend::Reply(vec![ACK_SOCKET_ONE.to_vec()]),
            OnSend::Fail(Error::Io(Arc::new(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "peer went away mid-frame",
            )))),
        ],
    )
    .with_serial_addressing();
    let probe = transport.probe();
    let session = Session::open(transport, multi_target_session_config()).expect("owner session");
    let first_camera = session
        .camera_for::<NonDefaultCompileTimeProfile>(CameraId::CAMERA_1)
        .expect("camera one view");
    let second_camera = session
        .camera_for::<NonDefaultCompileTimeProfile>(CameraId::CAMERA_2)
        .expect("camera two view");

    let held = first_camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first submission");

    let error = second_camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect_err("the second write fails on the wire");
    let Error::StreamPoisoned { reason } = &error else {
        panic!("a stream write failure is terminal for the session, got {error:?}");
    };
    assert!(
        reason.contains("peer went away mid-frame"),
        "the transport cause must survive in the poison reason: {reason}"
    );
    assert!(error.requires_new_session());

    let poisoned = held
        .applied()
        .expect_err("the rest of the session is poisoned too");
    assert!(
        matches!(poisoned, Error::StreamPoisoned { .. }),
        "expected the session-wide poison, got {poisoned:?}"
    );
    assert!(poisoned.requires_new_session());
    assert_eq!(probe.writes().len(), 1, "a failed write is not recorded");
}

/// A camera that answers `90 40 FF` / `90 50 FF` sends no socket nibble. 1.x
/// assigned a free socket; 2.0 turned the frame into a hard `InvalidResponse`
/// that killed the session.
#[test]
fn socketless_ack_and_completion_still_complete_a_command() {
    let transport = FaultTransport::new(
        SendSemantics::Datagram,
        vec![OnSend::Reply(vec![
            ACK_NO_SOCKET.to_vec(),
            COMPLETE_NO_SOCKET.to_vec(),
        ])],
    )
    .with_trailing_reply(vec![ACK_NO_SOCKET.to_vec(), COMPLETE_NO_SOCKET.to_vec()]);
    let probe = transport.probe();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission")
        .applied()
        .expect("a socketless ACK and completion must still complete the command");
    assert_eq!(probe.writes().len(), 1, "no retry was needed");

    session.shutdown().expect("owner shutdown");
}

/// A sequence-correlated Sony camera that repeats a socket nibble it already
/// handed out must not cause the second command to be silently remapped:
/// sequence correlation preserves the request identity while the occupied
/// socket ACK is ignored. Once the first command completes and frees S1, the
/// second command can advance on an exact ACK naming the available S2.
#[test]
fn ack_naming_an_occupied_socket_is_ignored_until_available() {
    let transport = FaultTransport::new(
        SendSemantics::Datagram,
        vec![
            OnSend::Reply(vec![ACK_SOCKET_ONE.to_vec()]),
            // The camera first answers the second command with occupied S1.
            // That explicit ACK is ignored, never remapped to S2. Completion
            // of the first command frees S1; the later exact S2 ACK can then
            // advance the second command before its exact completion.
            OnSend::ReplyWithSequences(vec![
                (ReplySequence::Current, ACK_SOCKET_ONE.to_vec()),
                (ReplySequence::Previous, COMPLETE_SOCKET_ONE.to_vec()),
                (ReplySequence::Current, ACK_SOCKET_TWO.to_vec()),
                (ReplySequence::Current, COMPLETE_SOCKET_TWO.to_vec()),
            ]),
        ],
    )
    .with_sony();
    let probe = transport.probe();
    let session = Session::open(transport, sony_session_config()).expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    let first = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first submission");
    let second = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("second submission");

    first.applied().expect("first operation applied");
    second
        .applied()
        .expect("the later exact ACK must complete the second operation");
    assert_eq!(
        probe.writes().len(),
        2,
        "neither command needed a retransmit after the occupied ACK was ignored"
    );

    session.shutdown().expect("owner shutdown");
}

/// Issue #297: the camera answers from inside the write, so its ACK and
/// completion are both already queued by the time the write returns. The owner
/// must match them on its very first read pump — the command settles without
/// waiting for its ACK deadline and without a second write.
///
/// This is deliberately **not** a test of the engine's deferred-ACK latch. The
/// shipped owners apply the write and its result back to back, so no frame can
/// reach the engine between them and `Phase::Sending` is unobservable from
/// here; a facade test claiming otherwise passes with the latch replaced by a
/// drop. The latch itself is pinned at engine level, where an `Input` sequence
/// can actually produce that interleaving, by
/// `runtime::engine::tests::ack_racing_its_own_write_result_is_latched_and_applied`,
/// `racing_acks_are_never_attributed_while_two_commands_are_being_written` and
/// `a_second_racing_ack_cannot_steal_the_latch_from_the_first` (#636).
#[test]
fn ack_answered_from_inside_the_write_is_matched_on_the_first_pump() {
    let transport = FaultTransport::new(
        SendSemantics::Datagram,
        vec![OnSend::Reply(standard_reply())],
    )
    .with_trailing_reply(standard_reply());
    let probe = transport.probe();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission")
        .applied()
        .expect("an immediately answered command must not wait for its ACK deadline");
    assert_eq!(
        probe.writes().len(),
        1,
        "a command answered inside its own write must never be rewritten"
    );

    session.shutdown().expect("owner shutdown");
}
