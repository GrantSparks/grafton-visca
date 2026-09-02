//! Issue #714: raw lost-ACK recovery has the same bounded-wait and Urgent
//! safety-lane behavior on the async owner as on the blocking owner.

#![cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
#![allow(clippy::expect_used)]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use grafton_visca::{
    completion::AppliedOnly,
    profile::ProfileSpec,
    raw::{self, RawReplyShape},
    request::builtin::{FocusStop, ZoomDrive},
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    AffectedAxes, ControlClass, Error, Executor, RetryClass, Session, SessionConfig, TimeoutClass,
};
use profile_fixtures::NonDefaultCompileTimeProfile;

const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
const ACK_SOCKET_TWO: &[u8] = &[0x90, 0x42, 0xff];
const COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x51, 0xff];
const COMPLETE_SOCKET_TWO: &[u8] = &[0x90, 0x52, 0xff];
const RAW_ZOOM_STOP: [u8; 6] = [0x81, 0x01, 0x04, 0x07, 0x00, 0xff];

#[derive(Debug)]
struct Script {
    steps: VecDeque<Vec<Vec<u8>>>,
    writes: Vec<Vec<u8>>,
}

#[derive(Debug)]
struct AsyncScriptTransport {
    config: TransportConfig,
    script: Arc<Mutex<Script>>,
    reply_tx: flume::Sender<Vec<u8>>,
    replies: flume::Receiver<Vec<u8>>,
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

impl AsyncScriptTransport {
    fn new(steps: Vec<Vec<Vec<u8>>>) -> (Self, Probe) {
        let script = Arc::new(Mutex::new(Script {
            steps: steps.into(),
            writes: Vec::new(),
        }));
        let (reply_tx, replies) = flume::unbounded();
        (
            Self {
                config: TransportConfig::default(),
                script: Arc::clone(&script),
                reply_tx,
                replies,
            },
            Probe { script },
        )
    }
}

impl HasTransportConfig for AsyncScriptTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for AsyncScriptTransport {
    async fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let replies = {
            let mut script = self.script.lock().expect("script lock");
            script.writes.push(bytes.to_vec());
            script.steps.pop_front().unwrap_or_default()
        };
        for reply in replies {
            self.reply_tx
                .send_async(reply)
                .await
                .map_err(|_| Error::ConnectionClosed { reason: None })?;
        }
        Ok(())
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        let reply = self
            .replies
            .recv_async()
            .await
            .map_err(|_| Error::ConnectionClosed { reason: None })?;
        dst[..reply.len()].copy_from_slice(&reply);
        Ok(reply.len())
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

fn session_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("two-socket raw profile"),
    )
}

fn ordinary_operation() -> raw::AppliedOnly {
    let policy = raw::Policy::new(TimeoutClass::Quick, RetryClass::Never, ControlClass::Normal)
        .expect("raw policy")
        .with_reply_shape(RawReplyShape::AckThenCompletion);
    raw::AppliedOnly::with_policy(RAW_ZOOM_STOP, AffectedAxes::ZOOM, policy)
        .expect("ordinary raw operation")
}

async fn wait_for_writes<E: Executor>(
    probe: &Probe,
    expected: usize,
    timeout: Duration,
    executor: &E,
) {
    let started = Instant::now();
    while probe.writes().len() < expected {
        assert!(
            started.elapsed() < timeout,
            "timed out waiting for write {expected}"
        );
        executor.sleep(Duration::from_millis(1)).await;
    }
}

async fn run_lost_ack_regressions<E: Executor>(executor: E) {
    // Ordinary work stays queued until the predecessor's 100 ms ACK deadline
    // plus its one-second ambiguity window, then writes without TransportBusy.
    let (transport, probe) = AsyncScriptTransport::new(vec![
        vec![],
        vec![ACK_SOCKET_ONE.to_vec(), COMPLETE_SOCKET_ONE.to_vec()],
    ]);
    let session = Session::open(transport, session_config(), executor.clone())
        .await
        .expect("async owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");
    let predecessor = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .await
        .expect("predecessor admission");
    wait_for_writes(&probe, 1, Duration::from_millis(50), &executor).await;
    let first_written_at = Instant::now();
    executor.sleep(Duration::from_millis(150)).await;
    let ordinary = ordinary_operation();
    let successor = camera
        .submit::<AppliedOnly, _>(&ordinary)
        .await
        .expect("ordinary successor admission");
    wait_for_writes(&probe, 2, Duration::from_secs(2), &executor).await;
    let release_elapsed = first_written_at.elapsed();
    assert!(
        release_elapsed >= Duration::from_millis(950),
        "ordinary successor wrote before quarantine release: {release_elapsed:?}"
    );
    assert!(
        release_elapsed < Duration::from_secs(2),
        "ordinary successor missed quarantine release: {release_elapsed:?}"
    );
    assert!(matches!(
        predecessor.applied().await,
        Err(Error::UnsequencedCommandUnconfirmed)
    ));
    successor
        .applied()
        .await
        .expect("ordinary successor settles after release");
    session.shutdown().await.expect("owner shutdown");

    // Urgent work crosses the pre-ACK candidate immediately. Every ACK queued
    // by the second send is read while both candidates are open and binds to
    // neither, so both receipts eventually report the unconfirmed outcome.
    let (transport, probe) = AsyncScriptTransport::new(vec![
        vec![],
        vec![
            ACK_SOCKET_ONE.to_vec(),
            ACK_SOCKET_TWO.to_vec(),
            COMPLETE_SOCKET_TWO.to_vec(),
        ],
    ]);
    let session = Session::open(transport, session_config(), executor.clone())
        .await
        .expect("async owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");
    let predecessor = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .await
        .expect("predecessor admission");
    wait_for_writes(&probe, 1, Duration::from_millis(50), &executor).await;
    let urgent_started = Instant::now();
    let urgent = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .await
        .expect("urgent admission");
    wait_for_writes(&probe, 2, Duration::from_millis(50), &executor).await;
    assert!(
        urgent_started.elapsed() < Duration::from_millis(50),
        "urgent command missed its physical pacing bound"
    );
    assert!(matches!(
        urgent.applied().await,
        Err(Error::UnsequencedCommandUnconfirmed)
    ));
    assert!(matches!(
        predecessor.applied().await,
        Err(Error::UnsequencedCommandUnconfirmed)
    ));
    session.shutdown().await.expect("owner shutdown");
}

#[cfg(feature = "blocking")]
mod parity {
    use super::*;
    use grafton_visca::{
        blocking::{Session as BlockingSession, SessionConfig as BlockingSessionConfig},
        command::CommandKind,
        transport::BlockingTransport,
    };

    #[derive(Debug)]
    struct SilentBlockingTransport {
        config: TransportConfig,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl HasTransportConfig for SilentBlockingTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl BlockingTransport for SilentBlockingTransport {
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
            Ok(())
        }

        fn recv_into_with_timeout(
            &mut self,
            _dst: &mut [u8],
            _timeout: Duration,
        ) -> Result<usize, Error> {
            Err(Error::Timeout)
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    fn blocking_transcript() -> Vec<Vec<u8>> {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let transport = SilentBlockingTransport {
            config: TransportConfig::default(),
            writes: Arc::clone(&writes),
        };
        let session = BlockingSession::open(
            transport,
            BlockingSessionConfig::new(
                ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
                    .expect("raw profile"),
            ),
        )
        .expect("blocking session");
        let camera = session
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera view");
        let _predecessor = camera
            .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
            .expect("blocking predecessor");
        let _urgent = camera
            .submit::<AppliedOnly, _>(&FocusStop)
            .expect("blocking urgent");
        let transcript = writes.lock().expect("writes lock").clone();
        session.shutdown().expect("blocking shutdown");
        transcript
    }

    pub(super) async fn assert_owner_transcript_parity<E: Executor>(executor: E) {
        let blocking = blocking_transcript();
        let (transport, probe) = AsyncScriptTransport::new(vec![vec![], vec![]]);
        let session = Session::open(transport, session_config(), executor.clone())
            .await
            .expect("async session");
        let camera = session
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera view");
        let _predecessor = camera
            .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
            .await
            .expect("async predecessor");
        wait_for_writes(&probe, 1, Duration::from_millis(50), &executor).await;
        let _urgent = camera
            .submit::<AppliedOnly, _>(&FocusStop)
            .await
            .expect("async urgent");
        wait_for_writes(&probe, 2, Duration::from_millis(50), &executor).await;
        let asynchronous = probe.writes();
        session.shutdown().await.expect("async shutdown");

        assert_eq!(asynchronous, blocking);
    }
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_lost_ack_wait_and_urgent_lane_are_bounded() {
    run_lost_ack_regressions(grafton_visca::TokioRuntime::from_current().expect("tokio runtime"))
        .await;
}

#[cfg(all(feature = "runtime-tokio", feature = "blocking"))]
#[tokio::test]
async fn tokio_blocking_and_async_owners_emit_the_same_urgent_transcript() {
    parity::assert_owner_transcript_parity(
        grafton_visca::TokioRuntime::from_current().expect("tokio runtime"),
    )
    .await;
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_lost_ack_wait_and_urgent_lane_are_bounded() {
    smol::block_on(run_lost_ack_regressions(grafton_visca::SmolRuntime::new()));
}

#[cfg(all(feature = "runtime-smol", feature = "blocking"))]
#[test]
fn smol_blocking_and_async_owners_emit_the_same_urgent_transcript() {
    smol::block_on(parity::assert_owner_transcript_parity(
        grafton_visca::SmolRuntime::new(),
    ));
}
