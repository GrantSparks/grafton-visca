//! Owner-backed cancellation acceptance for the built-in PTZOptics G2 profile.
//!
//! The G2 profile deliberately has no VISCA socket-cancel support.  These
//! tests exercise the root async session facade rather than the legacy camera
//! runtime and keep the wire transcript observable at the transport boundary.

#![cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]

use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use grafton_visca::{
    completion::AppliedOnly,
    profile::{OperationalTuning, ProfileSpec},
    profiles::PtzOpticsG2,
    request::builtin::FocusStop,
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    CancellationOutcome, Error, Executor, Session, SessionConfig,
};

const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
const FOCUS_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x00, 0xff];
const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
const ACK_SOCKET_TWO: &[u8] = &[0x90, 0x42, 0xff];
const COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x51, 0xff];

#[derive(Debug)]
struct CancellationTransport {
    config: TransportConfig,
    responses: flume::Receiver<Vec<u8>>,
    response_tx: flume::Sender<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    reads: Arc<AtomicUsize>,
    send_count: usize,
    auto_complete_after_first: bool,
}

impl CancellationTransport {
    fn new(auto_complete_after_first: bool) -> (Self, CancellationProbe) {
        let (response_tx, responses) = flume::unbounded();
        let writes = Arc::new(Mutex::new(Vec::new()));
        let reads = Arc::new(AtomicUsize::new(0));
        (
            Self {
                config: TransportConfig::default(),
                responses,
                response_tx: response_tx.clone(),
                writes: Arc::clone(&writes),
                reads: Arc::clone(&reads),
                send_count: 0,
                auto_complete_after_first,
            },
            CancellationProbe {
                response_tx,
                writes,
                reads,
            },
        )
    }
}

#[derive(Clone, Debug)]
struct CancellationProbe {
    response_tx: flume::Sender<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    reads: Arc<AtomicUsize>,
}

impl CancellationProbe {
    fn push(&self, bytes: Vec<u8>) {
        self.response_tx
            .send(bytes)
            .expect("owner response channel remains connected");
    }

    fn push_ack_and_completion(&self) {
        let mut bytes = ACK_SOCKET_ONE.to_vec();
        bytes.extend_from_slice(COMPLETE_SOCKET_ONE);
        self.push(bytes);
    }

    fn writes(&self) -> Vec<Vec<u8>> {
        self.writes.lock().expect("writes lock").clone()
    }

    fn read_count(&self) -> usize {
        self.reads.load(Ordering::Acquire)
    }
}

impl HasTransportConfig for CancellationTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for CancellationTransport {
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
        self.send_count = self.send_count.saturating_add(1);
        let send_number = self.send_count;
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        let response_tx = self.response_tx.clone();
        let auto_complete = self.auto_complete_after_first && send_number > 1;
        async move {
            if auto_complete {
                let mut response = ACK_SOCKET_ONE.to_vec();
                response.extend_from_slice(COMPLETE_SOCKET_ONE);
                response_tx
                    .send_async(response)
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
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
                .responses
                .recv_async()
                .await
                .map_err(|_| Error::ConnectionClosed { reason: None })?;
            let length = bytes.len();
            dst[..length].copy_from_slice(&bytes);
            self.reads.fetch_add(1, Ordering::AcqRel);
            Ok(length)
        }
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

fn g2_config(maximum_command_sockets: Option<u8>) -> SessionConfig {
    let profile = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("built-in G2 profile");
    let config = SessionConfig::new(profile);
    match maximum_command_sockets {
        Some(maximum) => config
            .with_tuning(OperationalTuning::new().maximum_command_sockets(maximum))
            .expect("G2 tuning remains within the built-in profile bounds"),
        None => config,
    }
}

async fn wait_for_writes<E: Executor>(executor: &E, probe: &CancellationProbe, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while probe.writes().len() < count {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {count} writes"
        );
        executor.sleep(Duration::from_millis(5)).await;
    }
}

async fn wait_for_reads<E: Executor>(executor: &E, probe: &CancellationProbe, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while probe.read_count() < count {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {count} reads"
        );
        executor.sleep(Duration::from_millis(1)).await;
    }
}

async fn queued_cancel_is_local<E: Executor>(executor: E) {
    let (transport, probe) = CancellationTransport::new(false);
    let session = Session::open(transport, g2_config(None), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    // G2 has two command sockets.  Keep both occupied before admitting the
    // third typed operation so its cancellation is unambiguously pre-wire.
    let first = camera.zoom().stop().await.expect("first operation");
    wait_for_writes(&executor, &probe, 1).await;
    probe.push(ACK_SOCKET_ONE.to_vec());
    wait_for_reads(&executor, &probe, 1).await;
    let second = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .await
        .expect("second operation");
    wait_for_writes(&executor, &probe, 2).await;
    probe.push(ACK_SOCKET_TWO.to_vec());
    wait_for_reads(&executor, &probe, 2).await;

    let queued = camera.pan_tilt().home().await.expect("queued operation");
    let cancellation = queued.cancel().await.expect("queued cancellation");
    assert!(matches!(
        cancellation.outcome(Duration::from_secs(1)).await,
        Ok(CancellationOutcome::Cancelled)
    ));
    assert_eq!(
        probe.writes(),
        vec![ZOOM_STOP.to_vec(), FOCUS_STOP.to_vec()]
    );

    first.detach();
    second.detach();
    session.shutdown().await.expect("owner shutdown");
}

async fn sent_cancel_is_not_supported<E: Executor>(executor: E, ack_before_cancel: bool) {
    let (transport, probe) = CancellationTransport::new(true);
    let session = Session::open(transport, g2_config(Some(1)), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    let original = camera.zoom().stop().await.expect("transmitted operation");
    wait_for_writes(&executor, &probe, 1).await;

    if ack_before_cancel {
        // The response is delivered as a separate owner input turn.  Once the
        // read is complete, a short owner-clock yield lets the ACK transition
        // the exact original operation into its executing socket phase.
        probe.push(ACK_SOCKET_ONE.to_vec());
        wait_for_reads(&executor, &probe, 1).await;
        executor.sleep(Duration::from_millis(5)).await;
    }

    let rejected = original
        .cancel()
        .await
        .expect_err("G2 sent cancellation must be rejected by profile policy");
    assert!(matches!(rejected.error(), Error::NotSupported));
    // The rejection hands the operation handle back (#612).
    let original = rejected
        .into_operation()
        .expect("a rejected cancellation returns the operation handle");
    assert_eq!(probe.writes(), vec![ZOOM_STOP.to_vec()]);

    if ack_before_cancel {
        probe.push(COMPLETE_SOCKET_ONE.to_vec());
    } else {
        probe.push_ack_and_completion();
    }

    executor
        .timeout(Duration::from_secs(2), original.applied())
        .await
        .expect("recovered handle observer deadline")
        .expect("the recovered handle still observes the original operation");

    // A one-socket tuning makes this next operation wait for the original
    // terminal frame.  Its successful applied wait therefore proves the
    // rejected cancellation did not remove or terminalize the original entry.
    let next = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .await
        .expect("next operation admission");
    executor
        .timeout(Duration::from_secs(2), next.applied())
        .await
        .expect("next operation observer deadline")
        .expect("next operation applied after original terminal");
    assert_eq!(
        probe.writes(),
        vec![ZOOM_STOP.to_vec(), FOCUS_STOP.to_vec()]
    );

    session.shutdown().await.expect("owner shutdown");
}

async fn terminal_race_reports_completed<E: Executor>(executor: E) {
    let (transport, probe) = CancellationTransport::new(false);
    let session = Session::open(transport, g2_config(Some(1)), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    let original = camera.zoom().stop().await.expect("transmitted operation");
    wait_for_writes(&executor, &probe, 1).await;
    // One receive turn carries both frames.  The owner drains both before it
    // can service the later cancellation boundary, making completion the race
    // winner even though the profile does not support socket cancellation.
    probe.push_ack_and_completion();
    wait_for_reads(&executor, &probe, 1).await;
    for _ in 0..5 {
        executor.sleep(Duration::from_millis(1)).await;
    }

    let cancellation = original
        .cancel()
        .await
        .expect("terminal completion wins cancellation race");
    assert!(matches!(
        cancellation.outcome(Duration::from_secs(1)).await,
        Ok(CancellationOutcome::Completed)
    ));
    assert_eq!(probe.writes(), vec![ZOOM_STOP.to_vec()]);
    session.shutdown().await.expect("owner shutdown");
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_root_owner_cancellation_acceptance_matrix() {
    let executor = grafton_visca::TokioRuntime::from_current().expect("Tokio runtime");
    queued_cancel_is_local(executor.clone()).await;
    sent_cancel_is_not_supported(executor.clone(), false).await;
    sent_cancel_is_not_supported(executor.clone(), true).await;
    terminal_race_reports_completed(executor).await;
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_root_owner_cancellation_acceptance_matrix() {
    smol::block_on(async {
        let executor = grafton_visca::SmolRuntime::new();
        queued_cancel_is_local(executor).await;
        sent_cancel_is_not_supported(executor, false).await;
        sent_cancel_is_not_supported(executor, true).await;
        terminal_race_reports_completed(executor).await;
    });
}
