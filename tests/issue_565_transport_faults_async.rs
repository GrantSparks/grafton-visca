//! Issue #565: 1.x transport fault tolerance, through the async facade.
//!
//! The blocking twin of this file is `issue_565_transport_faults_blocking.rs`.
//! Both owners must answer these five cases identically: a transient receive
//! error retries rather than destroying the session, a failed stream write
//! reports its own transport error to the caller that owned it, a socketless
//! ACK/completion still works, an ACK naming an occupied socket is reassigned,
//! and a camera that answers from inside the write is matched on the owner's
//! first read pump.
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
    request::builtin::{FocusStop, ZoomStop},
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    Error, Executor, Session, SessionConfig,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
const COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x51, 0xff];
const COMPLETE_SOCKET_TWO: &[u8] = &[0x90, 0x52, 0xff];
const ACK_NO_SOCKET: &[u8] = &[0x90, 0x40, 0xff];
const COMPLETE_NO_SOCKET: &[u8] = &[0x90, 0x50, 0xff];

/// What the camera does when the n-th frame is written.
#[derive(Debug, Clone)]
enum OnSend {
    /// Accept the write and queue these reads, in order.
    Reply(Vec<Vec<u8>>),
    /// Fail the write itself.
    Fail(Error),
}

#[derive(Debug)]
struct Script {
    steps: VecDeque<OnSend>,
    trailing: Vec<Vec<u8>>,
    faults: VecDeque<(usize, Error)>,
    writes: Vec<Vec<u8>>,
    sends: usize,
}

/// A scripted async camera whose reads may fail.
#[derive(Debug)]
struct FaultTransport {
    config: TransportConfig,
    semantics: SendSemantics,
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
            script: Arc::new(Mutex::new(Script {
                steps: steps.into(),
                trailing: Vec::new(),
                faults: VecDeque::new(),
                writes: Vec::new(),
                sends: 0,
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
        let queued = {
            let mut script = self.script.lock().expect("script lock");
            script.sends = script.sends.saturating_add(1);
            let sends = script.sends;
            let step = script
                .steps
                .pop_front()
                .unwrap_or_else(|| OnSend::Reply(script.trailing.clone()));
            match step {
                OnSend::Fail(error) => Err(error),
                OnSend::Reply(replies) => {
                    script.writes.push(bytes.to_vec());
                    let mut queued: Vec<Result<Vec<u8>, Error>> = Vec::new();
                    while script
                        .faults
                        .front()
                        .is_some_and(|(number, _)| *number == sends)
                    {
                        let (_, error) = script.faults.pop_front().expect("checked above");
                        queued.push(Err(error));
                    }
                    queued.extend(replies.into_iter().map(Ok));
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
}

fn session_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("two-socket runtime profile"),
    )
}

fn standard_reply() -> Vec<Vec<u8>> {
    vec![ACK_SOCKET_ONE.to_vec(), COMPLETE_SOCKET_ONE.to_vec()]
}

/// A transient receive error retries the in-flight command and keeps the
/// session usable, exactly as 1.x's `SchedulerEvent::NetworkError` did.
async fn transient_receive_error_retries_instead_of_destroying_the_session<E: Executor>(
    executor: E,
) {
    let transport = FaultTransport::new(
        SendSemantics::Datagram,
        vec![OnSend::Reply(Vec::new()), OnSend::Reply(standard_reply())],
    )
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

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("submission")
        .applied()
        .await
        .expect("a transient receive error must retry, not fail the session");
    assert_eq!(
        probe.writes().len(),
        2,
        "the in-flight command is written again after the transient fault"
    );

    camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .await
        .expect("the session is still usable")
        .applied()
        .await
        .expect("later work still completes");

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
    );
    let probe = transport.probe();
    let session = Session::open(transport, session_config(), executor)
        .await
        .expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let held = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("first submission");
    let failing = camera
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

/// A camera repeating a socket nibble it already handed out must not cost the
/// second command its ACK deadline.
async fn ack_naming_an_occupied_socket_is_reassigned<E: Executor>(executor: E) {
    let transport = FaultTransport::new(
        SendSemantics::Datagram,
        vec![
            OnSend::Reply(vec![ACK_SOCKET_ONE.to_vec()]),
            OnSend::Reply(vec![
                ACK_SOCKET_ONE.to_vec(),
                COMPLETE_SOCKET_ONE.to_vec(),
                COMPLETE_SOCKET_TWO.to_vec(),
            ]),
        ],
    );
    let probe = transport.probe();
    let session = Session::open(transport, session_config(), executor)
        .await
        .expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

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
        .expect("the reassigned ACK must complete the second operation");
    assert_eq!(
        probe.writes().len(),
        2,
        "neither command needed a retry after reassignment"
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
    transient_receive_error_retries_instead_of_destroying_the_session,
    datagram_write_failure_fails_one_command_and_keeps_the_session,
    stream_write_failure_poisons_and_names_the_transport_cause,
    socketless_ack_and_completion_still_complete_a_command,
    ack_naming_an_occupied_socket_is_reassigned,
    ack_answered_from_inside_the_write_is_matched_on_the_first_pump,
);
