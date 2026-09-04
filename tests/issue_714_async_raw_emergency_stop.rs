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
#[cfg(feature = "blocking")]
use grafton_visca::{transport::AddressingMode, CameraId, OperationalTuning};
use profile_fixtures::NonDefaultCompileTimeProfile;

const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
const ACK_SOCKET_TWO: &[u8] = &[0x90, 0x42, 0xff];
#[cfg(feature = "blocking")]
const CAMERA_TWO_ACK_SOCKET_ONE: &[u8] = &[0xa0, 0x41, 0xff];
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
        Self::new_with_config(steps, TransportConfig::default())
    }

    #[cfg(feature = "blocking")]
    fn new_serial(steps: Vec<Vec<Vec<u8>>>) -> (Self, Probe) {
        Self::new_with_config(
            steps,
            TransportConfig {
                addressing: AddressingMode::Serial,
                ..TransportConfig::default()
            },
        )
    }

    fn new_with_config(steps: Vec<Vec<Vec<u8>>>, config: TransportConfig) -> (Self, Probe) {
        let script = Arc::new(Mutex::new(Script {
            steps: steps.into(),
            writes: Vec::new(),
        }));
        let (reply_tx, replies) = flume::unbounded();
        (
            Self {
                config,
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

    #[cfg(feature = "blocking")]
    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(self.config.addressing)
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

#[cfg(feature = "blocking")]
fn two_camera_session_config(command_spacing: Duration) -> SessionConfig {
    let mut config = session_config();
    config
        .register_target(
            CameraId::CAMERA_2,
            ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
                .expect("two-socket raw profile"),
        )
        .expect("second serial target");
    config
        .with_tuning(OperationalTuning::new().command_spacing(command_spacing))
        .expect("test command spacing")
}

fn ordinary_operation() -> raw::AppliedOnly {
    let policy = raw::Policy::new(TimeoutClass::Quick, RetryClass::Never, ControlClass::Normal)
        .expect("raw policy")
        .with_reply_shape(RawReplyShape::AckThenCompletion);
    raw::AppliedOnly::with_policy(RAW_ZOOM_STOP, AffectedAxes::ZOOM, policy)
        .expect("ordinary raw operation")
}

#[cfg(feature = "blocking")]
fn ordinary_operation_for(target: CameraId) -> raw::AppliedOnly {
    let mut wire = RAW_ZOOM_STOP;
    wire[0] = target.to_address_byte();
    raw::AppliedOnly::with_policy(
        wire,
        AffectedAxes::ZOOM,
        raw::Policy::new(TimeoutClass::Quick, RetryClass::Never, ControlClass::Normal)
            .expect("raw policy")
            .with_reply_shape(RawReplyShape::AckThenCompletion),
    )
    .expect("targeted ordinary raw operation")
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

    /// Give the exact same per-write reply script to the blocking facade.
    /// This prevents parity from being accidentally tested against a similar
    /// but separately maintained transport fixture.
    impl BlockingTransport for AsyncScriptTransport {
        fn send_with_timeout(
            &mut self,
            bytes: &[u8],
            _kind: CommandKind,
            _timeout: Duration,
        ) -> Result<(), Error> {
            let replies = {
                let mut script = self.script.lock().expect("script lock");
                script.writes.push(bytes.to_vec());
                script.steps.pop_front().unwrap_or_default()
            };
            for reply in replies {
                self.reply_tx
                    .try_send(reply)
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
            }
            Ok(())
        }

        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            _timeout: Duration,
        ) -> Result<usize, Error> {
            let reply = self.replies.try_recv().map_err(|_| Error::Timeout)?;
            dst[..reply.len()].copy_from_slice(&reply);
            Ok(reply.len())
        }

        fn addressing_mode_hint(&self) -> Option<AddressingMode> {
            Some(self.config.addressing)
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

    /// One five-write public-facade transcript spans both #744 defects:
    /// camera-two's paced cancellation must not reject camera-one's normal
    /// request, and the later urgent stop must bind its ACK through that
    /// normal request's lost-ACK `PreAck` hold.
    fn blocking_pending_cancel_and_preack_transcript() -> Vec<Vec<u8>> {
        const SPACING: Duration = Duration::from_millis(20);

        let (transport, probe) = AsyncScriptTransport::new_serial(vec![
            vec![CAMERA_TWO_ACK_SOCKET_ONE.to_vec()],
            vec![],
            vec![],
            vec![],
            vec![ACK_SOCKET_ONE.to_vec(), COMPLETE_SOCKET_ONE.to_vec()],
        ]);
        let session = BlockingSession::open(transport, two_camera_session_config(SPACING))
            .expect("blocking multi-camera session");
        let camera_one = session
            .camera_for::<NonDefaultCompileTimeProfile>(CameraId::CAMERA_1)
            .expect("camera one view");
        let camera_two = session
            .camera_for::<NonDefaultCompileTimeProfile>(CameraId::CAMERA_2)
            .expect("camera two view");

        let predecessor = camera_two
            .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
            .expect("camera two predecessor");
        let _socket_successor = camera_two
            .submit::<AppliedOnly, _>(&ordinary_operation_for(CameraId::CAMERA_2))
            .expect("camera two ACK drain");
        let _cancellation = predecessor.cancel().expect("paced cancellation");
        let ordinary = camera_one
            .submit::<AppliedOnly, _>(&ordinary_operation())
            .expect("camera one ordinary first write");
        assert!(matches!(
            ordinary.applied(),
            Err(Error::UnsequencedCommandUnconfirmed)
        ));
        let stop = camera_one
            .submit::<AppliedOnly, _>(&FocusStop)
            .expect("urgent stop crosses camera-one PreAck hold");
        stop.applied()
            .expect("the urgent stop ACK is attributable and completes");

        let transcript = probe.writes();
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

    pub(super) async fn assert_pending_cancel_and_preack_transcript_parity<E: Executor>(
        executor: E,
    ) {
        const SPACING: Duration = Duration::from_millis(20);

        let blocking = blocking_pending_cancel_and_preack_transcript();
        let (transport, probe) = AsyncScriptTransport::new_serial(vec![
            vec![CAMERA_TWO_ACK_SOCKET_ONE.to_vec()],
            vec![],
            vec![],
            vec![],
            vec![ACK_SOCKET_ONE.to_vec(), COMPLETE_SOCKET_ONE.to_vec()],
        ]);
        let session = Session::open(
            transport,
            two_camera_session_config(SPACING),
            executor.clone(),
        )
        .await
        .expect("async multi-camera session");
        let camera_one = session
            .camera_for::<NonDefaultCompileTimeProfile>(CameraId::CAMERA_1)
            .expect("camera one view");
        let camera_two = session
            .camera_for::<NonDefaultCompileTimeProfile>(CameraId::CAMERA_2)
            .expect("camera two view");

        let predecessor = camera_two
            .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
            .await
            .expect("camera two predecessor");
        wait_for_writes(&probe, 1, Duration::from_secs(1), &executor).await;
        let _socket_successor = camera_two
            .submit::<AppliedOnly, _>(&ordinary_operation_for(CameraId::CAMERA_2))
            .await
            .expect("camera two successor");
        wait_for_writes(&probe, 2, Duration::from_secs(1), &executor).await;
        let _cancellation = predecessor.cancel().await.expect("paced cancellation");
        let ordinary = camera_one
            .submit::<AppliedOnly, _>(&ordinary_operation())
            .await
            .expect("camera one ordinary admission");
        wait_for_writes(&probe, 4, Duration::from_secs(1), &executor).await;
        assert!(matches!(
            ordinary.applied().await,
            Err(Error::UnsequencedCommandUnconfirmed)
        ));
        let stop = camera_one
            .submit::<AppliedOnly, _>(&FocusStop)
            .await
            .expect("urgent stop crosses camera-one PreAck hold");
        wait_for_writes(&probe, 5, Duration::from_secs(1), &executor).await;
        stop.applied()
            .await
            .expect("the urgent stop ACK is attributable and completes");

        let asynchronous = probe.writes();
        session.shutdown().await.expect("async shutdown");
        assert_eq!(
            asynchronous, blocking,
            "both owners must emit the identical #744 transcript"
        );
        assert_eq!(
            asynchronous,
            vec![
                vec![0x82, 0x01, 0x04, 0x07, 0x02, 0xff],
                vec![0x82, 0x01, 0x04, 0x07, 0x00, 0xff],
                vec![0x82, 0x21, 0xff],
                RAW_ZOOM_STOP.to_vec(),
                vec![0x81, 0x01, 0x04, 0x08, 0x00, 0xff],
            ],
            "the transcript retains the cancellation, camera-one write, and acknowledged urgent stop"
        );
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

#[cfg(all(feature = "runtime-tokio", feature = "blocking"))]
#[tokio::test]
async fn tokio_blocking_and_async_owners_match_the_pending_cancel_recovery_transcript() {
    parity::assert_pending_cancel_and_preack_transcript_parity(
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

#[cfg(all(feature = "runtime-smol", feature = "blocking"))]
#[test]
fn smol_blocking_and_async_owners_match_the_pending_cancel_recovery_transcript() {
    smol::block_on(parity::assert_pending_cancel_and_preack_transcript_parity(
        grafton_visca::SmolRuntime::new(),
    ));
}
