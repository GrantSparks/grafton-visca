//! A refused cancellation must never strand the caller (#612, #777).
//!
//! `PtzOpticsG2` is the one built-in profile without VISCA socket-cancel
//! support, so cancelling one of its already-written commands is refused with
//! [`Error::NotSupported`]. Dropping a handle is exactly `detach` — no STOP is
//! emitted — so a refusal that also swallowed the handle would leave a caller
//! watching a moving axis with nothing left to observe it through.
//!
//! `cancel` borrows the handle, so a refusal is a plain error and the handle is
//! untouched. These tests pin that contract: the same handle still observes
//! the original operation the engine deliberately left running, a second
//! cancel is refused the same way, and the documented recourse — an explicit
//! typed STOP — reaches the wire while the original is still in flight.

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
    profile::ProfileSpec,
    profiles::PtzOpticsG2,
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    CancellationOutcome, Error, Executor, Session, SessionConfig,
};

const ZOOM_TELE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xff];
const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
const COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x51, 0xff];
const ACK_SOCKET_TWO: &[u8] = &[0x90, 0x42, 0xff];
const COMPLETE_SOCKET_TWO: &[u8] = &[0x90, 0x52, 0xff];

/// A transport that records every write and replays exactly the frames a test
/// pushes, so the wire transcript is the assertion surface.
#[derive(Debug)]
struct ScriptedTransport {
    config: TransportConfig,
    responses: flume::Receiver<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    reads: Arc<AtomicUsize>,
}

impl ScriptedTransport {
    fn new() -> (Self, TransportProbe) {
        let (response_tx, responses) = flume::unbounded();
        let writes = Arc::new(Mutex::new(Vec::new()));
        let reads = Arc::new(AtomicUsize::new(0));
        (
            Self {
                config: TransportConfig::default(),
                responses,
                writes: Arc::clone(&writes),
                reads: Arc::clone(&reads),
            },
            TransportProbe {
                response_tx,
                writes,
                reads,
            },
        )
    }
}

#[derive(Clone, Debug)]
struct TransportProbe {
    response_tx: flume::Sender<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    reads: Arc<AtomicUsize>,
}

impl TransportProbe {
    fn push(&self, bytes: &[u8]) {
        self.response_tx
            .send(bytes.to_vec())
            .expect("owner response channel remains connected");
    }

    fn writes(&self) -> Vec<Vec<u8>> {
        self.writes.lock().expect("writes lock").clone()
    }

    fn read_count(&self) -> usize {
        self.reads.load(Ordering::Acquire)
    }
}

impl HasTransportConfig for ScriptedTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for ScriptedTransport {
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        async { Ok(()) }
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
                .map_err(|_| Error::connection_closed(None))?;
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

fn g2_config() -> SessionConfig {
    let profile = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("built-in G2 profile");
    SessionConfig::new(profile)
}

async fn wait_for_writes<E: Executor>(executor: &E, probe: &TransportProbe, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while probe.writes().len() < count {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {count} writes"
        );
        executor.sleep(Duration::from_millis(1)).await;
    }
}

async fn wait_for_reads<E: Executor>(executor: &E, probe: &TransportProbe, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while probe.read_count() < count {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {count} reads"
        );
        executor.sleep(Duration::from_millis(1)).await;
    }
}

/// The whole contract on one moving axis: refuse, keep observing, stop.
async fn refused_cancel_leaves_the_handle_observing<E: Executor>(executor: E) {
    let (transport, probe) = ScriptedTransport::new();
    let session = Session::open(transport, g2_config(), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    // A continuous zoom: the axis keeps moving until something stops it.
    let mut moving = camera
        .zoom()
        .tele()
        .await
        .expect("continuous zoom admitted");
    wait_for_writes(&executor, &probe, 1).await;
    probe.push(ACK_SOCKET_ONE);
    wait_for_reads(&executor, &probe, 1).await;

    // The G2 has no socket-cancel, so the owner refuses. `cancel` borrows the
    // handle, so the refusal leaves it in the caller's hands.
    let refused = moving
        .cancel()
        .await
        .expect_err("G2 sent cancellation must be rejected by profile policy");
    assert!(matches!(refused, Error::NotSupported));

    // A refusal installs no intent, so a retry is refused the same way.
    let refused = moving
        .cancel()
        .await
        .expect_err("the retry is refused on the same profile grounds");
    assert!(matches!(refused, Error::NotSupported));

    // No cancellation frame was ever written, and the original is still live.
    assert_eq!(probe.writes(), vec![ZOOM_TELE.to_vec()]);

    // The documented recourse for a profile without socket-cancel: an
    // explicit typed STOP, which the second command socket dispatches while
    // the original is still in flight.
    let mut stop = camera.zoom().stop().await.expect("typed stop admitted");
    wait_for_writes(&executor, &probe, 2).await;
    assert_eq!(
        probe.writes(),
        vec![ZOOM_TELE.to_vec(), ZOOM_STOP.to_vec()],
        "the STOP that ends physical movement must reach the wire"
    );
    probe.push(ACK_SOCKET_TWO);
    probe.push(COMPLETE_SOCKET_TWO);
    executor
        .timeout(Duration::from_secs(2), stop.applied())
        .await
        .expect("stop observer deadline")
        .expect("the stop applies while the refused original is still running");

    // The handle still reports the original operation's own terminal state,
    // and a cancel after it concluded answers from that state.
    probe.push(COMPLETE_SOCKET_ONE);
    executor
        .timeout(Duration::from_secs(2), moving.applied())
        .await
        .expect("handle observer deadline")
        .expect("the handle still observes the original operation");
    assert!(matches!(
        moving.cancel().await,
        Ok(CancellationOutcome::Completed)
    ));
    assert_eq!(probe.writes().len(), 2, "a concluded cancel sends nothing");

    session.shutdown().await.expect("owner shutdown");
}

/// A handle can be detached after a refused cancellation, and the refusal
/// leaves the engine's view of the original request untouched.
async fn handle_can_be_detached_after_a_refused_cancel<E: Executor>(executor: E) {
    let (transport, probe) = ScriptedTransport::new();
    let session = Session::open(transport, g2_config(), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    let mut moving = camera
        .zoom()
        .tele()
        .await
        .expect("continuous zoom admitted");
    wait_for_writes(&executor, &probe, 1).await;

    assert!(matches!(moving.cancel().await, Err(Error::NotSupported)));
    moving.detach();

    // Detaching relinquishes observation only, exactly as it does for a handle
    // that was never offered to `cancel`: the original still completes.
    probe.push(ACK_SOCKET_ONE);
    probe.push(COMPLETE_SOCKET_ONE);
    wait_for_reads(&executor, &probe, 2).await;
    let mut next = camera.zoom().stop().await.expect("typed stop admitted");
    wait_for_writes(&executor, &probe, 2).await;
    probe.push(ACK_SOCKET_ONE);
    probe.push(COMPLETE_SOCKET_ONE);
    executor
        .timeout(Duration::from_secs(2), next.applied())
        .await
        .expect("stop observer deadline")
        .expect("stop applied after the detached original terminalized");
    assert_eq!(
        probe.writes(),
        vec![ZOOM_TELE.to_vec(), ZOOM_STOP.to_vec()],
        "no cancellation frame is ever written for a profile without support"
    );

    session.shutdown().await.expect("owner shutdown");
}

/// A still-queued command cancels locally on every profile.
async fn queued_cancel_concludes_cancelled_on_every_profile<E: Executor>(executor: E) {
    let (transport, probe) = ScriptedTransport::new();
    let session = Session::open(transport, g2_config(), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    // Occupy both G2 command sockets so the third submission is unambiguously
    // still queued when it is cancelled.
    let first = camera.zoom().tele().await.expect("first operation");
    wait_for_writes(&executor, &probe, 1).await;
    probe.push(ACK_SOCKET_ONE);
    wait_for_reads(&executor, &probe, 1).await;

    let second = camera.focus().stop().await.expect("second operation");
    wait_for_writes(&executor, &probe, 2).await;
    probe.push(ACK_SOCKET_TWO);
    wait_for_reads(&executor, &probe, 2).await;

    let mut queued = camera.pan_tilt().home().await.expect("queued operation");
    assert!(matches!(
        queued.cancel_with_timeout(Duration::from_secs(1)).await,
        Ok(CancellationOutcome::Cancelled)
    ));
    assert_eq!(
        probe.writes().len(),
        2,
        "the queued command never reached the wire"
    );

    first.detach();
    second.detach();
    session.shutdown().await.expect("owner shutdown");
}

/// The dynamic projection erases the completion marker but not the contract:
/// a refused cancellation leaves the dynamic handle observing too.
#[cfg(feature = "dyn-api")]
async fn dyn_refused_cancel_leaves_the_handle_observing<E: Executor>(executor: E) {
    use grafton_visca::{dynapi::DynSessionCamera, request::builtin::ZoomDrive};

    let (transport, probe) = ScriptedTransport::new();
    let session = Session::open(transport, g2_config(), executor.clone())
        .await
        .expect("owner session");
    let camera = DynSessionCamera::from_session(&session).expect("dynamic G2 camera");

    let mut moving = camera
        .submit_applied(&ZoomDrive::Tele)
        .await
        .expect("continuous zoom admitted");
    wait_for_writes(&executor, &probe, 1).await;
    probe.push(ACK_SOCKET_ONE);
    wait_for_reads(&executor, &probe, 1).await;

    assert!(matches!(moving.cancel().await, Err(Error::NotSupported)));

    probe.push(COMPLETE_SOCKET_ONE);
    executor
        .timeout(Duration::from_secs(2), moving.applied())
        .await
        .expect("handle observer deadline")
        .expect("the dynamic handle still observes the original operation");
    assert_eq!(probe.writes(), vec![ZOOM_TELE.to_vec()]);

    session.shutdown().await.expect("owner shutdown");
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_refused_cancellation_is_recoverable() {
    let executor = grafton_visca::TokioRuntime::from_current().expect("Tokio runtime");
    refused_cancel_leaves_the_handle_observing(executor.clone()).await;
    handle_can_be_detached_after_a_refused_cancel(executor.clone()).await;
    queued_cancel_concludes_cancelled_on_every_profile(executor.clone()).await;
    #[cfg(feature = "dyn-api")]
    dyn_refused_cancel_leaves_the_handle_observing(executor).await;
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_refused_cancellation_is_recoverable() {
    smol::block_on(async {
        let executor = grafton_visca::SmolRuntime::new();
        refused_cancel_leaves_the_handle_observing(executor).await;
        handle_can_be_detached_after_a_refused_cancel(executor).await;
        queued_cancel_concludes_cancelled_on_every_profile(executor).await;
        #[cfg(feature = "dyn-api")]
        dyn_refused_cancel_leaves_the_handle_observing(executor).await;
    });
}
