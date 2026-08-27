//! Issue #566: retry coverage through the async facade.
//!
//! The blocking twin is `tests/issue_566_retry_recovery_blocking.rs`; the two
//! assert the same observable behavior against the two owners, because a retry
//! policy that only holds on one of them is not a policy.

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
    time::Duration,
};

use grafton_visca::{
    completion::AppliedOnly,
    profile::ProfileSpec,
    request::{self, builtin::ZoomStop},
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    CameraId, ControlClass, Error, Executor, Request, RetryClass, Session, SessionConfig,
    TimeoutClass,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
const COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x51, 0xff];
/// `0x41` — the camera refuses to execute right now.
const NOT_EXECUTABLE: &[u8] = &[0x90, 0x60, 0x41, 0xff];
/// `0x05` — the camera has no free command socket.
const NO_SOCKET: &[u8] = &[0x90, 0x60, 0x05, 0xff];

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

#[derive(Debug)]
struct Script {
    steps: VecDeque<Vec<Vec<u8>>>,
    trailing: Vec<Vec<u8>>,
    writes: Vec<Vec<u8>>,
}

/// A camera whose answer to the n-th write is scripted.
#[derive(Debug)]
struct ScriptTransport {
    config: TransportConfig,
    script: Arc<Mutex<Script>>,
    reads: flume::Sender<Vec<u8>>,
    replies: flume::Receiver<Vec<u8>>,
}

impl ScriptTransport {
    fn new(steps: Vec<Vec<Vec<u8>>>) -> Self {
        let (reads, replies) = flume::unbounded();
        Self {
            config: TransportConfig::default(),
            script: Arc::new(Mutex::new(Script {
                steps: steps.into(),
                trailing: Vec::new(),
                writes: Vec::new(),
            })),
            reads,
            replies,
        }
    }

    /// Answer every write past the explicit script with these frames.
    fn with_trailing(self, trailing: Vec<Vec<u8>>) -> Self {
        self.script.lock().expect("script lock").trailing = trailing;
        self
    }

    fn probe(&self) -> Probe {
        Probe {
            script: Arc::clone(&self.script),
        }
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

impl HasTransportConfig for ScriptTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for ScriptTransport {
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
        let replies = {
            let mut script = self.script.lock().expect("script lock");
            script.writes.push(bytes.to_vec());
            script
                .steps
                .pop_front()
                .unwrap_or_else(|| script.trailing.clone())
        };
        let reads = self.reads.clone();
        async move {
            for reply in replies {
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
            // A silent camera simply never answers; the owner races this
            // against its own deadline.
            let bytes = self
                .replies
                .recv_async()
                .await
                .map_err(|_| Error::ConnectionClosed { reason: None })?;
            dst[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }
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

fn standard_reply() -> Vec<Vec<u8>> {
    vec![ACK_SOCKET_ONE.to_vec(), COMPLETE_SOCKET_ONE.to_vec()]
}

/// Issue #566: `0x41` is retried for a movement-class request and is terminal
/// for a standard one.
async fn a_camera_refusal_is_replayed_only_for_movement<E: Executor>(executor: E) {
    let transport = ScriptTransport::new(vec![vec![NOT_EXECUTABLE.to_vec()], standard_reply()])
        .with_trailing(standard_reply());
    let probe = transport.probe();
    let session = Session::open(transport, session_config(), executor.clone())
        .await
        .expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("submission")
        .applied_with_timeout(Duration::from_secs(5))
        .await
        .expect("a refused movement command must be replayed, not failed");
    assert_eq!(probe.writes().len(), 2);
    session.shutdown().await.expect("owner shutdown");

    let transport =
        ScriptTransport::new(vec![vec![NOT_EXECUTABLE.to_vec()]]).with_trailing(standard_reply());
    let probe = transport.probe();
    let session = Session::open(transport, session_config(), executor)
        .await
        .expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let error = camera
        .execute(&StandardCommand)
        .await
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
    session.shutdown().await.expect("owner shutdown");
}

/// Issue #566: `0x05` (`NoSocket`) is a capacity answer, so it is replayed for
/// a standard request too — unlike `0x41`.
async fn a_no_socket_answer_is_replayed_for_a_standard_command<E: Executor>(executor: E) {
    let transport = ScriptTransport::new(vec![vec![NO_SOCKET.to_vec()], standard_reply()])
        .with_trailing(standard_reply());
    let probe = transport.probe();
    let session = Session::open(transport, session_config(), executor)
        .await
        .expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .execute(&StandardCommand)
        .await
        .expect("a camera with no free socket must be retried, not failed");
    assert_eq!(probe.writes().len(), 2);
    session.shutdown().await.expect("owner shutdown");
}

/// Issue #566: a movement request survives a lost ACK, and a command survives a
/// camera that ACKs and then goes silent. The rewrite gated ACK retries to
/// `RetryClass::Standard` and disabled completion retries outright.
async fn a_silent_camera_is_retried_before_and_after_its_ack<E: Executor>(executor: E) {
    let transport =
        ScriptTransport::new(vec![Vec::new(), standard_reply()]).with_trailing(standard_reply());
    let probe = transport.probe();
    let session = Session::open(transport, session_config(), executor.clone())
        .await
        .expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");
    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("submission")
        .applied_with_timeout(Duration::from_secs(5))
        .await
        .expect("a lost ACK must be retried for a movement request");
    assert_eq!(probe.writes().len(), 2, "the lost ACK reissues the frame");
    session.shutdown().await.expect("owner shutdown");

    let transport = ScriptTransport::new(vec![vec![ACK_SOCKET_ONE.to_vec()], standard_reply()])
        .with_trailing(standard_reply());
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
        .applied_with_timeout(Duration::from_secs(10))
        .await
        .expect("a post-ACK completion timeout must be retried");
    assert_eq!(
        probe.writes().len(),
        2,
        "the completion timeout reissues the frame"
    );
    session.shutdown().await.expect("owner shutdown");
}

/// Issue #566: `Error::CancellationUnconfirmed` had no path through the owner
/// or the facade — it was reachable only from the engine's own tests.
async fn an_unresolvable_cancellation_reaches_the_caller<E: Executor>(executor: E) {
    // The camera never answers anything: neither the command nor the cancel.
    let transport = ScriptTransport::new(vec![Vec::new()]);
    let probe = transport.probe();
    let session = Session::open(transport, session_config(), executor)
        .await
        .expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let operation = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("submission");
    let cancellation = operation
        .cancel()
        .await
        .expect("cancellation intent is recorded");
    let error = cancellation
        .outcome(Duration::from_secs(5))
        .await
        .expect_err("a camera that never answers cannot confirm a cancellation");
    assert!(
        matches!(error, Error::CancellationUnconfirmed),
        "expected the ambiguity verdict, got {error:?}"
    );
    assert!(
        !probe.writes().is_empty(),
        "the original frame was transmitted"
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
    a_camera_refusal_is_replayed_only_for_movement,
    a_no_socket_answer_is_replayed_for_a_standard_command,
    a_silent_camera_is_retried_before_and_after_its_ack,
    an_unresolvable_cancellation_reaches_the_caller,
);
