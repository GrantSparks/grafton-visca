//! Issue #566: retry coverage through the async facade.
//!
//! The blocking twin is `tests/issue_566_retry_recovery_blocking.rs`; the two
//! assert the same observable behavior against the two owners, because a retry
//! policy that only holds on one of them is not a policy. The conclusive camera
//! refusals stay on raw VISCA, while the ambiguous ACK/completion and
//! cancellation cases use Sony's sequence envelope: raw traffic cannot safely
//! identify a later reply after a successful write has gone unresolved.

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
    profiles::SonyFR7,
    raw,
    request::{self, builtin::ZoomStop},
    transport::{
        AsyncTransport, HasTransportConfig, ReceiveOutcome, SendSemantics, TransportConfig,
    },
    types::ZoomPosition,
    CameraId, ControlClass, Error, Executor, InquiryRoute, Request, RetryClass, Session,
    SessionConfig, TimeoutClass,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
const COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x51, 0xff];
/// `0x41` — the camera refuses to execute right now.
const NOT_EXECUTABLE: &[u8] = &[0x90, 0x60, 0x41, 0xff];
/// `0x05` — the camera has no free command socket.
const NO_SOCKET: &[u8] = &[0x90, 0x60, 0x05, 0xff];
/// `0x02` — malformed command syntax, transient only for generated inquiries.
const SYNTAX_ERROR: &[u8] = &[0x90, 0x60, 0x02, 0xff];
const ZOOM_POSITION_REPLY: &[u8] = &[0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xff];
const ZOOM_POSITION_INQUIRY: &[u8] = &[0x81, 0x09, 0x04, 0x47, 0xff];

fn decode_custom_inquiry(payload: &[u8]) -> Result<Vec<u8>, Error> {
    Ok(payload.to_vec())
}

fn custom_zoom_inquiry() -> raw::Inquiry<Vec<u8>> {
    raw::Inquiry::from_fn(
        ZOOM_POSITION_INQUIRY,
        InquiryRoute::RAW,
        decode_custom_inquiry,
        TimeoutClass::Inquiry,
        RetryClass::Inquiry,
        ControlClass::Normal,
    )
    .expect("valid custom inquiry")
}

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

/// A plain command in the movement retry class and quick timeout class.
///
/// `0x41` is replayed for ordinary movement. A typed STOP is deliberately not
/// this vehicle: a STOP the camera finds not executable (for example a focus
/// STOP under auto-focus) is reported at once rather than rewritten.
#[derive(Debug)]
struct MovementCommand;

impl Request for MovementCommand {
    type Class = request::Plain;
    const MAX_SIZE: usize = 3;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
    const RETRY_CLASS: RetryClass = RetryClass::Movement;
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
    sony: bool,
    script: Arc<Mutex<Script>>,
    reads: flume::Sender<Vec<u8>>,
    replies: flume::Receiver<Vec<u8>>,
}

impl ScriptTransport {
    fn new(steps: Vec<Vec<Vec<u8>>>) -> Self {
        let (reads, replies) = flume::unbounded();
        Self {
            config: TransportConfig::default(),
            sony: false,
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

    /// Use Sony's sequence-bearing envelope for requests and replies.
    fn with_sony(mut self) -> Self {
        self.sony = true;
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
        let sony = self.sony;
        let replies = {
            let mut script = self.script.lock().expect("script lock");
            script.writes.push(bytes.to_vec());
            script
                .steps
                .pop_front()
                .unwrap_or_else(|| script.trailing.clone())
        };
        let replies = replies
            .into_iter()
            .map(|reply| frame_reply(bytes, &reply, sony))
            .collect::<Vec<_>>();
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
    ) -> impl Future<Output = Result<ReceiveOutcome, Error>> + Send {
        async move {
            // A silent camera simply never answers; the owner races this
            // against its own deadline.
            let bytes = self
                .replies
                .recv_async()
                .await
                .map_err(|_| Error::connection_closed(None))?;
            Ok(ReceiveOutcome::copy_message(&bytes, dst))
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
    let mut frame = Vec::with_capacity(payload.len() + 8);
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
        .execute(&MovementCommand)
        .await
        .expect("a refused movement command must be replayed, not failed");
    assert_eq!(probe.writes().len(), 2);
    session.shutdown().expect("owner shutdown");

    // A typed STOP refused with `0x41` reports the camera's conclusive
    // refusal at once; resending cannot change the standing condition.
    let transport =
        ScriptTransport::new(vec![vec![NOT_EXECUTABLE.to_vec()]]).with_trailing(standard_reply());
    let probe = transport.probe();
    let session = Session::open(transport, session_config(), executor.clone())
        .await
        .expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");
    let error = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("submission")
        .applied_with_timeout(Duration::from_secs(5))
        .await
        .expect_err("a refused STOP surfaces the camera's refusal");
    assert!(
        matches!(error, Error::CommandNotExecutable),
        "expected the camera's own refusal, got {error:?}"
    );
    assert_eq!(probe.writes().len(), 1, "a refused STOP is not rewritten");
    session.shutdown().expect("owner shutdown");

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
    session.shutdown().expect("owner shutdown");
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
    session.shutdown().expect("owner shutdown");
}

/// Issue #566: the generated async noun path carries closed built-in inquiry
/// provenance through generic public lowering. A raw/custom inquiry with the
/// same bytes remains terminal on `0x02`, so the distinction is not inferred
/// from the request wire.
async fn a_builtin_inquiry_syntax_error_is_replayed_but_custom_syntax_is_terminal<E: Executor>(
    executor: E,
) {
    let transport = ScriptTransport::new(vec![
        vec![SYNTAX_ERROR.to_vec()],
        vec![ZOOM_POSITION_REPLY.to_vec()],
    ]);
    let probe = transport.probe();
    let session = Session::open(transport, session_config(), executor.clone())
        .await
        .expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let position = camera
        .zoom()
        .position()
        .await
        .expect("a generated inquiry must retry transient 0x02");
    assert_eq!(position, ZoomPosition::new(0x1234).expect("zoom position"));
    assert_eq!(
        probe.writes().len(),
        2,
        "the built-in inquiry is reissued once"
    );
    session.shutdown().expect("owner shutdown");

    let transport = ScriptTransport::new(vec![
        vec![SYNTAX_ERROR.to_vec()],
        vec![ZOOM_POSITION_REPLY.to_vec()],
    ]);
    let probe = transport.probe();
    let session = Session::open(transport, session_config(), executor)
        .await
        .expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let custom = custom_zoom_inquiry();
    let error = camera
        .inquire(&custom)
        .await
        .expect_err("a custom inquiry must surface the camera's syntax verdict");
    assert!(matches!(error, Error::SyntaxError));
    assert_eq!(
        probe.writes().len(),
        1,
        "a custom inquiry must not reach the scripted resend"
    );
    session.shutdown().expect("owner shutdown");
}

/// Issue #566: a sequence-correlated Sony movement request survives a lost ACK
/// (same-sequence retransmission, docs/visca_reference.md §5.3). A camera
/// that ACKs and then goes silent is never rewritten (#795): the ACK proved
/// acceptance. Raw VISCA replays neither because it has no request identity.
async fn a_silent_sony_camera_is_retried_only_before_its_ack<E: Executor>(executor: E) {
    let transport = ScriptTransport::new(vec![Vec::new(), standard_reply()])
        .with_sony()
        .with_trailing(standard_reply());
    let probe = transport.probe();
    let session = Session::open(transport, sony_session_config(), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");
    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("submission")
        .applied_with_timeout(Duration::from_secs(5))
        .await
        .expect("a lost ACK must be retried for a movement request");
    let writes = probe.writes();
    assert_eq!(writes.len(), 2, "the lost ACK reissues the frame");
    assert_eq!(
        sony_sequence(&writes[0]),
        sony_sequence(&writes[1]),
        "an ACK retry preserves the logical Sony request sequence"
    );
    session.shutdown().expect("owner shutdown");

    let transport = ScriptTransport::new(vec![vec![ACK_SOCKET_ONE.to_vec()], standard_reply()])
        .with_sony()
        .with_trailing(standard_reply());
    let probe = transport.probe();
    let session = Session::open(transport, sony_session_config(), executor)
        .await
        .expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");
    let error = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("submission")
        .applied_with_timeout(Duration::from_secs(30))
        .await
        .expect_err("a post-ACK completion timeout is terminal");
    assert_eq!(
        error.failure_context(),
        Some(grafton_visca::FailureContext::new(
            grafton_visca::FailureStage::Terminal,
            grafton_visca::Certainty::Unconfirmed
        )),
        "{error:?}"
    );
    assert_eq!(
        probe.writes().len(),
        1,
        "an acknowledged command is never rewritten"
    );
    session.shutdown().expect("owner shutdown");
}

/// Issue #566: `Error::CancellationUnconfirmed` reaches the caller when a
/// sequence-correlated Sony command and its cancellation both remain silent.
async fn an_unresolvable_cancellation_reaches_the_caller<E: Executor>(executor: E) {
    // The camera never answers anything: neither the command nor the cancel.
    let transport = ScriptTransport::new(vec![Vec::new()]).with_sony();
    let probe = transport.probe();
    let session = Session::open(transport, sony_session_config(), executor)
        .await
        .expect("owner session");
    let camera = session.camera::<SonyFR7>().expect("camera view");

    let mut operation = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("submission");
    let error = operation
        .cancel_with_timeout(Duration::from_secs(5))
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
    session.shutdown().expect("owner shutdown");
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
    a_builtin_inquiry_syntax_error_is_replayed_but_custom_syntax_is_terminal,
    a_silent_sony_camera_is_retried_only_before_its_ack,
    an_unresolvable_cancellation_reaches_the_caller,
);
