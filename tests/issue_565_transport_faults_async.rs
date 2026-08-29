//! Issue #565: 1.x transport fault tolerance, through the async facade.
//!
//! The blocking twin of this file is `issue_565_transport_faults_blocking.rs`.
//! Both owners must answer these transport boundaries identically: raw receive
//! faults while an ACK is unconfirmed poison without replay, sequence-
//! correlated Sony receive faults retry on the same sequence, stream writes
//! preserve session poison, socketless frames still work, occupied sockets
//! remain exact evidence, and write-racing replies match on the first read
//! pump.
//!
//! The engine's deferred-ACK latch (#297) is *not* observable from here: these
//! owners apply a write and its result back to back, so nothing can reach the
//! engine in between. That guarantee is pinned in `runtime::engine::tests`
//! instead, where inputs can be interleaved directly (#636).

#![cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    collections::VecDeque,
    future::Future,
    sync::{Arc, Mutex},
};

use grafton_visca::{
    completion::AppliedOnly,
    profile::ProfileSpec,
    profiles::SonyFR7,
    request::builtin::{FocusStop, ZoomStop},
    transport::{
        AddressingMode, AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig,
    },
    CameraId, Error, Executor, Session, SessionConfig,
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

#[derive(Debug)]
struct Script {
    steps: VecDeque<OnSend>,
    trailing: Vec<Vec<u8>>,
    faults: VecDeque<(usize, Error)>,
    writes: Vec<Vec<u8>>,
    sends: usize,
    last_sequence: Option<[u8; 4]>,
}

/// A scripted async camera whose reads may fail.
#[derive(Debug)]
struct FaultTransport {
    config: TransportConfig,
    semantics: SendSemantics,
    sony: bool,
    script: Arc<Mutex<Script>>,
    reads: flume::Sender<Result<Vec<u8>, Error>>,
    replies: flume::Receiver<Result<Vec<u8>, Error>>,
}

impl FaultTransport {
    fn new(semantics: SendSemantics, steps: Vec<OnSend>) -> Self {
        let (reads, replies) = flume::unbounded();
        Self {
            config: TransportConfig::default(),
            semantics,
            sony: false,
            script: Arc::new(Mutex::new(Script {
                steps: steps.into(),
                trailing: Vec::new(),
                faults: VecDeque::new(),
                writes: Vec::new(),
                sends: 0,
                last_sequence: None,
            })),
            reads,
            replies,
        }
    }

    fn probe(&self) -> Probe {
        Probe {
            script: Arc::clone(&self.script),
        }
    }

    /// Replies used once the explicit script is exhausted.
    fn with_trailing_reply(self, replies: Vec<Vec<u8>>) -> Self {
        self.script.lock().expect("script lock").trailing = replies;
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
    fn with_read_fault(self, send_number: usize, error: Error) -> Self {
        self.script
            .lock()
            .expect("script lock")
            .faults
            .push_back((send_number, error));
        self
    }
}

#[derive(Clone, Debug)]
struct Probe {
    script: Arc<Mutex<Script>>,
}

impl Probe {
    fn writes(&self) -> Vec<Vec<u8>> {
        self.script.lock().expect("script lock").writes.clone()
    }
}

impl HasTransportConfig for FaultTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for FaultTransport {
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
        let sony = self.sony;
        let queued = {
            let mut script = self.script.lock().expect("script lock");
            script.sends = script.sends.saturating_add(1);
            let sends = script.sends;
            let previous_sequence = script.last_sequence;
            let step = script
                .steps
                .pop_front()
                .unwrap_or_else(|| OnSend::Reply(script.trailing.clone()));
            match step {
                OnSend::Fail(error) => Err(error),
                OnSend::Reply(replies) => {
                    script.writes.push(bytes.to_vec());
                    if sony {
                        script.last_sequence = bytes
                            .get(4..8)
                            .and_then(|sequence| sequence.try_into().ok());
                    }
                    let mut queued: Vec<Result<Vec<u8>, Error>> = Vec::new();
                    while script
                        .faults
                        .front()
                        .is_some_and(|(number, _)| *number == sends)
                    {
                        let (_, error) = script.faults.pop_front().expect("checked above");
                        queued.push(Err(error));
                    }
                    queued.extend(
                        replies
                            .into_iter()
                            .map(|reply| Ok(frame_reply(bytes, &reply, sony))),
                    );
                    Ok(queued)
                }
                OnSend::ReplyWithSequences(replies) => {
                    script.writes.push(bytes.to_vec());
                    if sony {
                        script.last_sequence = bytes
                            .get(4..8)
                            .and_then(|sequence| sequence.try_into().ok());
                    }
                    let mut queued: Vec<Result<Vec<u8>, Error>> = Vec::new();
                    while script
                        .faults
                        .front()
                        .is_some_and(|(number, _)| *number == sends)
                    {
                        let (_, error) = script.faults.pop_front().expect("checked above");
                        queued.push(Err(error));
                    }
                    queued.extend(replies.into_iter().map(|(sequence, reply)| {
                        let sequence = match sequence {
                            ReplySequence::Current => Some(&bytes[4..8]),
                            ReplySequence::Previous => previous_sequence
                                .as_ref()
                                .map(|sequence| sequence.as_slice()),
                        };
                        Ok(frame_reply_with_sequence(&reply, sony, sequence))
                    }));
                    Ok(queued)
                }
            }
        };
        let reads = self.reads.clone();
        async move {
            // The camera answers from inside the write, so the reply is already
            // queued by the time this future resolves (#297).
            for reply in queued? {
                let _ = reads.send(reply);
            }
            Ok(())
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn recv_into<'a>(
        &'a mut self,
        dst: &'a mut [u8],
    ) -> impl Future<Output = Result<usize, Error>> + Send {
        async move {
            let bytes = self
                .replies
                .recv_async()
                .await
                .map_err(|_| Error::ConnectionClosed { reason: None })??;
            dst[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }
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

/// A raw receive fault while a successfully sent command awaits its ACK leaves
/// both acceptance and future reply ownership uncertain. The owner must poison
/// the session, require a replacement, and never replay the command.
async fn raw_transient_receive_fault_poisons_without_retry<E: Executor>(executor: E) {
    let transport = FaultTransport::new(SendSemantics::Datagram, vec![OnSend::Reply(Vec::new())])
        .with_trailing_reply(standard_reply())
        .with_read_fault(
            1,
            Error::Io(Arc::new(std::io::Error::from(
                std::io::ErrorKind::ConnectionRefused,
            ))),
        );
    let probe = transport.probe();
    let session = Session::open(transport, session_config(), executor)
        .await
        .expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let error = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("submission")
        .applied()
        .await
        .expect_err("a raw receive fault must poison the session");
    assert!(matches!(error, Error::UnsequencedCommandUnconfirmed));
    assert!(error.requires_new_session());
    assert_eq!(
        probe.writes().len(),
        1,
        "an unconfirmed raw command is never replayed after a receive fault"
    );
}

/// A sequence-correlated Sony receive fault can retry the same logical request
/// and keep the session usable, exactly as 1.x's `SchedulerEvent::NetworkError`
/// did. The classic trigger is UDP `recv` reporting ECONNREFUSED after an ICMP
/// port-unreachable for an earlier datagram.
async fn sony_transient_receive_fault_retries_same_sequence_and_keeps_session<E: Executor>(
    executor: E,
) {
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
    let session = Session::open(transport, sony_session_config(), executor)
        .await
        .expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("submission")
        .applied()
        .await
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

    camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .await
        .expect("the session is still usable")
        .applied()
        .await
        .expect("later work still completes after the bounded retry");

    session.shutdown().await.expect("owner shutdown");
}

/// A failed datagram write fails exactly one command and the session keeps
/// running, which is the per-command write behavior 1.x had.
async fn datagram_write_failure_fails_one_command_and_keeps_the_session<E: Executor>(executor: E) {
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
    let session = Session::open(transport, session_config(), executor)
        .await
        .expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let failing = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("the submission is admitted before its write");
    let error = failing
        .applied()
        .await
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
        .await
        .expect("the session survives an isolated datagram write failure")
        .applied()
        .await
        .expect("later work still completes");
    assert_eq!(probe.writes().len(), 1, "a failed write is not recorded");

    session.shutdown().await.expect("owner shutdown");
}

/// A failed *stream* write is a session verdict on purpose: the byte-stream
/// position becomes unknowable, so every affected caller must learn a
/// replacement session is needed (#564), with the transport cause preserved.
/// The two raw commands target different registered cameras so the raw
/// same-target unacknowledged gate does not hide the session-wide stream
/// boundary.
async fn stream_write_failure_poisons_and_names_the_transport_cause<E: Executor>(executor: E) {
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
    let session = Session::open(transport, multi_target_session_config(), executor)
        .await
        .expect("owner session");
    let first_camera = session
        .camera_for::<NonDefaultCompileTimeProfile>(CameraId::CAMERA_1)
        .expect("camera one view");
    let second_camera = session
        .camera_for::<NonDefaultCompileTimeProfile>(CameraId::CAMERA_2)
        .expect("camera two view");

    let held = first_camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("first submission");
    let failing = second_camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .await
        .expect("the second submission is admitted before its write");

    let error = failing
        .applied()
        .await
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
        .await
        .expect_err("the rest of the session is poisoned too");
    assert!(
        matches!(poisoned, Error::StreamPoisoned { .. }),
        "expected the session-wide poison, got {poisoned:?}"
    );
    assert!(poisoned.requires_new_session());
    assert_eq!(probe.writes().len(), 1, "a failed write is not recorded");
}

/// `90 40 FF` / `90 50 FF` carry no socket nibble and must still complete the
/// command instead of failing the session with `InvalidResponse`.
async fn socketless_ack_and_completion_still_complete_a_command<E: Executor>(executor: E) {
    let transport = FaultTransport::new(
        SendSemantics::Datagram,
        vec![OnSend::Reply(vec![
            ACK_NO_SOCKET.to_vec(),
            COMPLETE_NO_SOCKET.to_vec(),
        ])],
    )
    .with_trailing_reply(vec![ACK_NO_SOCKET.to_vec(), COMPLETE_NO_SOCKET.to_vec()]);
    let probe = transport.probe();
    let session = Session::open(transport, session_config(), executor)
        .await
        .expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("submission")
        .applied()
        .await
        .expect("a socketless ACK and completion must still complete the command");
    assert_eq!(probe.writes().len(), 1, "no retry was needed");

    session.shutdown().await.expect("owner shutdown");
}

/// A malformed datagram is one bad receive, not session death. The owner must
/// discard it and still apply the valid reply that follows on the same
/// transport, without retrying the command or carrying the bad bytes into the
/// next datagram.
async fn malformed_datagram_is_ignored_and_later_valid_reply_succeeds<E: Executor>(executor: E) {
    let transport = FaultTransport::new(
        SendSemantics::Datagram,
        vec![OnSend::Reply(vec![
            vec![0x90, 0x61, 0xff], // malformed error frame: missing error code
            ACK_SOCKET_ONE.to_vec(),
            COMPLETE_SOCKET_ONE.to_vec(),
        ])],
    );
    let probe = transport.probe();
    let session = Session::open(transport, session_config(), executor)
        .await
        .expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("submission")
        .applied()
        .await
        .expect("a later valid datagram must succeed after malformed input");
    assert_eq!(
        probe.writes().len(),
        1,
        "malformed input must not trigger a retry"
    );

    session.shutdown().await.expect("owner shutdown");
}

/// A sequence-correlated Sony camera repeating a socket nibble it already
/// handed out must not cause the second command to be silently remapped:
/// sequence correlation preserves request identity while the occupied socket
/// ACK is ignored. Once the first command completes and frees S1, the second
/// command can advance on an exact ACK naming the available S2.
async fn ack_naming_an_occupied_socket_is_ignored_until_available<E: Executor>(executor: E) {
    let transport = FaultTransport::new(
        SendSemantics::Datagram,
        vec![
            OnSend::Reply(vec![ACK_SOCKET_ONE.to_vec()]),
            OnSend::ReplyWithSequences(vec![
                // The first ACK for the second command names occupied S1.
                // It is ignored, never remapped to S2. Completion of the
                // first command frees S1; the later exact S2 ACK can then
                // advance the second command before its exact completion.
                (ReplySequence::Current, ACK_SOCKET_ONE.to_vec()),
                (ReplySequence::Previous, COMPLETE_SOCKET_ONE.to_vec()),
                (ReplySequence::Current, ACK_SOCKET_TWO.to_vec()),
                (ReplySequence::Current, COMPLETE_SOCKET_TWO.to_vec()),
            ]),
        ],
    )
    .with_sony();
    let probe = transport.probe();
    let session = Session::open(transport, sony_session_config(), executor)
        .await
        .expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    let first = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("first submission");
    let second = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .await
        .expect("second submission");

    first.applied().await.expect("first operation applied");
    second
        .applied()
        .await
        .expect("the later exact ACK must complete the second operation");
    assert_eq!(
        probe.writes().len(),
        2,
        "neither command needed a retransmit after the occupied ACK was ignored"
    );

    session.shutdown().await.expect("owner shutdown");
}

/// Issue #297: the camera answers from inside the write, so its ACK and
/// completion are both already queued when the write future resolves. The
/// owner must match them on its very first read pump — the command settles
/// without waiting for its ACK deadline and without a second write.
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
async fn ack_answered_from_inside_the_write_is_matched_on_the_first_pump<E: Executor>(executor: E) {
    let transport = FaultTransport::new(
        SendSemantics::Datagram,
        vec![OnSend::Reply(standard_reply())],
    )
    .with_trailing_reply(standard_reply());
    let probe = transport.probe();
    let session = Session::open(transport, session_config(), executor)
        .await
        .expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("submission")
        .applied()
        .await
        .expect("an immediately answered command must not wait for its ACK deadline");
    assert_eq!(
        probe.writes().len(),
        1,
        "a command answered inside its own write must never be rewritten"
    );

    session.shutdown().await.expect("owner shutdown");
}

/// One independent libtest case per scenario per runtime.
///
/// These scenarios used to be awaited in sequence inside a single
/// `#[tokio::test]`: the first panic ended the test and every later scenario
/// went unreported, so a green run proved only "nothing failed before the first
/// failure". Generating one case each keeps the failures independent and names
/// the scenario that failed.
macro_rules! runtime_matrix {
    ($($scenario:ident),+ $(,)?) => {
        $(
            mod $scenario {
                #[cfg(feature = "runtime-tokio")]
                #[tokio::test]
                async fn tokio() {
                    let executor =
                        grafton_visca::TokioRuntime::from_current().expect("Tokio runtime");
                    super::$scenario(executor).await;
                }

                #[cfg(feature = "runtime-smol")]
                #[test]
                fn smol() {
                    smol::block_on(super::$scenario(grafton_visca::SmolRuntime::new()));
                }
            }
        )+
    };
}

runtime_matrix!(
    raw_transient_receive_fault_poisons_without_retry,
    sony_transient_receive_fault_retries_same_sequence_and_keeps_session,
    datagram_write_failure_fails_one_command_and_keeps_the_session,
    stream_write_failure_poisons_and_names_the_transport_cause,
    socketless_ack_and_completion_still_complete_a_command,
    malformed_datagram_is_ignored_and_later_valid_reply_succeeds,
    ack_naming_an_occupied_socket_is_ignored_until_available,
    ack_answered_from_inside_the_write_is_matched_on_the_first_pump,
);
