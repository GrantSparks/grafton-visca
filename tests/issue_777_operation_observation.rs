//! Operation handles observe their operation through borrowing waits (#777).
//!
//! Every wait takes `&mut self` and the handle caches what it observes, so a
//! wait that times out or is dropped releases only that wait, a later wait
//! continues from what was already observed, and cancellation is one
//! idempotent intent per handle. These tests drive the root async facade over
//! a scripted raw transport whose wire transcript is the assertion surface.

#![cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
#![allow(clippy::expect_used)]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use futures_lite::future;
use grafton_visca::{
    completion::{AppliedOnly, Targeted},
    profile::ProfileSpec,
    request::builtin::{PanTiltHome, ZoomDrive, ZoomStop},
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    CancellationOutcome, Certainty, Error, Executor, FailureContext, FailureStage, Operation,
    Session, SessionConfig,
};

use profile_fixtures::NonDefaultCompileTimeProfile as Raw;

const ZOOM_TELE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xff];
const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
const PAN_TILT_HOME: &[u8] = &[0x81, 0x01, 0x06, 0x04, 0xff];
const CANCEL_SOCKET_ONE: &[u8] = &[0x81, 0x21, 0xff];
const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
const COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x51, 0xff];
const CANCELLED_SOCKET_ONE: &[u8] = &[0x90, 0x61, 0x04, 0xff];
const SYNTAX_ERROR_SOCKET_ONE: &[u8] = &[0x90, 0x61, 0x02, 0xff];
const PAN_TILT_POSITION_INQUIRY: &[u8] = &[0x81, 0x09, 0x06, 0x12, 0xff];
const PAN_TILT_POSITION: &[u8] = &[
    0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff,
];

/// A short wait that the scripted camera never answers in time.
const SHORT: Duration = Duration::from_millis(20);
/// A generous bound for anything the test expects to happen.
const LONG: Duration = Duration::from_secs(5);

/// Records every write and replays exactly the frames a test pushes, except
/// pan-tilt position inquiries, which it answers with a fixed position once a
/// test enables that.
#[derive(Debug)]
struct ScriptedTransport {
    config: TransportConfig,
    responses: flume::Receiver<Vec<u8>>,
    probe: Probe,
}

#[derive(Clone, Debug)]
struct Probe {
    response_tx: flume::Sender<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    reads: Arc<AtomicUsize>,
    positions: Arc<Mutex<PositionReplies>>,
}

/// Whether position inquiries are answered, and how many written while they
/// were not are still owed a reply.
#[derive(Debug, Default)]
struct PositionReplies {
    answering: bool,
    owed: usize,
}

impl ScriptedTransport {
    fn new() -> (Self, Probe) {
        let (response_tx, responses) = flume::unbounded();
        let probe = Probe {
            response_tx,
            writes: Arc::default(),
            reads: Arc::default(),
            positions: Arc::default(),
        };
        (
            Self {
                config: TransportConfig::default(),
                responses,
                probe: probe.clone(),
            },
            probe,
        )
    }
}

impl Probe {
    fn push(&self, bytes: &[u8]) {
        self.response_tx
            .send(bytes.to_vec())
            .expect("owner response channel remains connected");
    }

    /// Answers every position inquiry from now on, including those already
    /// written and unanswered.
    fn answer_positions(&self) {
        let mut positions = self.positions.lock().expect("positions lock");
        positions.answering = true;
        for _ in 0..std::mem::take(&mut positions.owed) {
            self.push(PAN_TILT_POSITION);
        }
    }

    fn record_write(&self, bytes: &[u8]) {
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        if bytes == PAN_TILT_POSITION_INQUIRY {
            let mut positions = self.positions.lock().expect("positions lock");
            if positions.answering {
                self.push(PAN_TILT_POSITION);
            } else {
                positions.owed += 1;
            }
        }
    }

    /// Writes other than position inquiries.
    fn commands(&self) -> Vec<Vec<u8>> {
        self.writes()
            .into_iter()
            .filter(|bytes| bytes != PAN_TILT_POSITION_INQUIRY)
            .collect()
    }

    fn writes(&self) -> Vec<Vec<u8>> {
        self.writes.lock().expect("writes lock").clone()
    }

    async fn await_writes<E: Executor>(&self, executor: &E, count: usize) {
        let deadline = Instant::now() + LONG;
        while self.writes().len() < count {
            assert!(
                Instant::now() < deadline,
                "timed out awaiting {count} writes"
            );
            executor.sleep(Duration::from_millis(1)).await;
        }
    }

    async fn await_reads<E: Executor>(&self, executor: &E, count: usize) {
        let deadline = Instant::now() + LONG;
        while self.reads.load(Ordering::Acquire) < count {
            assert!(
                Instant::now() < deadline,
                "timed out awaiting {count} reads"
            );
            executor.sleep(Duration::from_millis(1)).await;
        }
    }
}

impl HasTransportConfig for ScriptedTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for ScriptedTransport {
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
        self.probe.record_write(bytes);
        async { Ok(()) }
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        let bytes = self
            .responses
            .recv_async()
            .await
            .map_err(|_| Error::ConnectionClosed { reason: None })?;
        dst[..bytes.len()].copy_from_slice(&bytes);
        self.probe.reads.fetch_add(1, Ordering::AcqRel);
        Ok(bytes.len())
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

async fn open<E: Executor>(executor: &E) -> (Session, Probe) {
    let (transport, probe) = ScriptedTransport::new();
    let profile = ProfileSpec::from_compile_time::<Raw>().expect("raw two-socket profile");
    let session = Session::open(transport, SessionConfig::new(profile), executor.clone())
        .await
        .expect("owner session");
    (session, probe)
}

/// Admits a continuous zoom and acknowledges it on socket one, so the
/// operation is written, accepted, and still running.
async fn running_zoom<E: Executor>(
    executor: &E,
    session: &Session,
    probe: &Probe,
) -> Operation<AppliedOnly> {
    let camera = session.camera::<Raw>().expect("raw camera");
    let writes = probe.writes().len();
    let reads = probe.reads.load(Ordering::Acquire);
    let operation = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .await
        .expect("continuous zoom admitted");
    probe.await_writes(executor, writes + 1).await;
    probe.push(ACK_SOCKET_ONE);
    probe.await_reads(executor, reads + 1).await;
    operation
}

fn debug<T: std::fmt::Debug>(value: &T) -> String {
    format!("{value:?}")
}

/// A wait that times out, and one dropped as the losing branch of a race,
/// release only themselves: the handle observes the later outcome and then
/// answers repeat waits from its cache.
async fn abandoned_waits_keep_the_handle_observing<E: Executor>(executor: E) {
    let (session, probe) = open(&executor).await;
    let mut moving = running_zoom(&executor, &session, &probe).await;

    // The expired wait names the still-running operation and is never
    // retryable: resubmitting would duplicate it (D20, #783).
    let id = moving.id();
    let expired = moving
        .applied_with_timeout(SHORT)
        .await
        .expect_err("nothing concluded the zoom");
    assert!(matches!(expired, Error::ObservationTimeout { operation } if operation == id));
    assert!(!expired.is_retryable());
    let lost = future::or(async { Some(moving.applied().await) }, async {
        executor.sleep(SHORT).await;
        None
    })
    .await;
    assert!(lost.is_none(), "the timer wins the race");
    assert!(
        executor.timeout(SHORT, moving.applied()).await.is_err(),
        "an executor timeout drops the wait future"
    );

    probe.push(COMPLETE_SOCKET_ONE);
    moving
        .applied_with_timeout(LONG)
        .await
        .expect("the handle observes the completion");

    // The cached outcome answers without the owner: even a zero timeout
    // succeeds, and after shutdown the handle still reports it.
    moving
        .applied_with_timeout(Duration::ZERO)
        .await
        .expect("a cached outcome needs no wait");
    session.shutdown().await.expect("owner shutdown");
    moving
        .applied()
        .await
        .expect("the cache outlives the owner");
    assert_eq!(probe.writes(), vec![ZOOM_TELE.to_vec()]);
}

/// Application and settlement are observed on one handle, in sequence, and
/// a settlement wait abandoned during position polling restarts with a fresh
/// proof instead of losing the cached application.
async fn application_then_settlement<E: Executor>(executor: E) {
    let (session, probe) = open(&executor).await;
    let camera = session.camera::<Raw>().expect("raw camera");
    let mut home = camera
        .submit::<Targeted, _>(&PanTiltHome)
        .await
        .expect("home admitted");
    probe.await_writes(&executor, 1).await;
    probe.push(ACK_SOCKET_ONE);

    // Abandoned while awaiting application.
    assert!(matches!(
        home.settled_with_timeout(SHORT).await,
        Err(Error::ObservationTimeout { .. })
    ));
    probe.push(COMPLETE_SOCKET_ONE);
    home.applied().await.expect("home applied");

    // Abandoned while polling: the camera does not answer position inquiries.
    assert!(matches!(
        home.settled_with_timeout(SHORT).await,
        Err(Error::ObservationTimeout { .. })
    ));
    assert!(
        probe.writes().len() > 1,
        "settlement polls the pan-tilt position"
    );

    probe.answer_positions();
    home.settled_with_timeout(LONG)
        .await
        .expect("two equal samples settle home");
    let polled = probe.writes().len();
    home.settled_with_timeout(Duration::ZERO)
        .await
        .expect("settlement is cached");
    home.applied().await.expect("application is cached");
    assert_eq!(probe.writes().len(), polled, "cached waits poll nothing");
    assert_eq!(probe.commands(), vec![PAN_TILT_HOME.to_vec()]);
    session.shutdown().await.expect("owner shutdown");
}

/// A failed outcome is cached like a success, and `cancel` after it reports
/// that failure without writing anything.
async fn a_failed_outcome_is_cached<E: Executor>(executor: E) {
    let (session, probe) = open(&executor).await;
    let mut moving = running_zoom(&executor, &session, &probe).await;
    probe.push(SYNTAX_ERROR_SOCKET_ONE);

    let first = moving.applied().await.expect_err("the camera rejects it");
    let again = moving.applied().await.expect_err("the failure is cached");
    assert_eq!(debug(&first), debug(&again));
    let cancelled = moving.cancel().await.expect_err("the failure decides");
    assert_eq!(debug(&first), debug(&cancelled));
    assert_eq!(probe.writes(), vec![ZOOM_TELE.to_vec()]);
    session.shutdown().await.expect("owner shutdown");
}

/// One handle has one cancellation intent: a timed-out cancel, a dropped
/// cancel, and a repeat cancel all observe the single socket cancel written
/// for the first, and the conclusion is cached.
async fn cancellation_is_one_idempotent_intent<E: Executor>(executor: E) {
    let (session, probe) = open(&executor).await;
    let mut moving = running_zoom(&executor, &session, &probe).await;

    assert!(matches!(
        moving.cancel_with_timeout(SHORT).await,
        Err(Error::ObservationTimeout { .. })
    ));
    probe.await_writes(&executor, 2).await;
    assert!(
        executor.timeout(SHORT, moving.cancel()).await.is_err(),
        "an executor timeout drops the cancel future"
    );

    let reply = async {
        executor.sleep(SHORT).await;
        probe.push(CANCELLED_SOCKET_ONE);
    };
    let (outcome, ()) = future::zip(moving.cancel_with_timeout(LONG), reply).await;
    assert_eq!(
        outcome.expect("cancellation won"),
        CancellationOutcome::Cancelled
    );
    assert_eq!(
        probe.writes(),
        vec![ZOOM_TELE.to_vec(), CANCEL_SOCKET_ONE.to_vec()],
        "every cancel observed the one intent"
    );

    assert!(matches!(
        moving.applied().await,
        Err(Error::CommandCanceled)
    ));
    assert_eq!(
        moving.cancel().await.expect("cached"),
        CancellationOutcome::Cancelled
    );
    assert_eq!(probe.writes().len(), 2);
    session.shutdown().await.expect("owner shutdown");
}

/// An outcome delivered before `cancel` answers it: nothing is written.
async fn cancel_after_the_outcome_sends_nothing<E: Executor>(executor: E) {
    let (session, probe) = open(&executor).await;
    let mut moving = running_zoom(&executor, &session, &probe).await;
    probe.push(COMPLETE_SOCKET_ONE);
    probe.await_reads(&executor, 2).await;

    assert_eq!(
        moving.cancel().await.expect("completed first"),
        CancellationOutcome::Completed
    );
    assert_eq!(probe.writes(), vec![ZOOM_TELE.to_vec()]);
    session.shutdown().await.expect("owner shutdown");
}

/// A cancellation the owner accepted but could not conclude is an error from
/// `cancel`, cached as the intent's answer; the operation keeps running and
/// its own outcome then decides every later answer.
async fn a_failed_cancellation_leaves_the_outcome_observable<E: Executor>(executor: E) {
    let (session, probe) = open(&executor).await;
    let mut moving = running_zoom(&executor, &session, &probe).await;

    // The camera never answers the socket cancel, so the owner's own
    // cancellation observation deadline ends the intent.
    let failed = moving.cancel().await.expect_err("unanswered cancel fails");
    assert_eq!(
        failed.failure_context(),
        Some(FailureContext::new(
            FailureStage::CancellationAttempt,
            Certainty::StillLive
        )),
        "{failed:?}"
    );
    let again = moving.cancel().await.expect_err("the failure is cached");
    assert_eq!(again.failure_context(), failed.failure_context());
    assert_eq!(
        probe.writes(),
        vec![ZOOM_TELE.to_vec(), CANCEL_SOCKET_ONE.to_vec()],
        "a failed intent is not retried"
    );

    probe.push(COMPLETE_SOCKET_ONE);
    moving
        .applied()
        .await
        .expect("the operation's own outcome is still observed");
    assert_eq!(
        moving.cancel().await.expect("the outcome decides"),
        CancellationOutcome::Completed
    );
    session.shutdown().await.expect("owner shutdown");
}

/// An outcome delivered before the owner stopped still answers `cancel`, even
/// though the cancellation request can no longer be sent.
async fn cancel_after_shutdown_reports_the_delivered_outcome<E: Executor>(executor: E) {
    let (session, probe) = open(&executor).await;
    let mut moving = running_zoom(&executor, &session, &probe).await;
    probe.push(COMPLETE_SOCKET_ONE);
    probe.await_reads(&executor, 2).await;
    session.shutdown().await.expect("owner shutdown");

    assert_eq!(
        moving
            .cancel()
            .await
            .expect("the delivered outcome decides"),
        CancellationOutcome::Completed
    );
    moving.applied().await.expect("and is still observable");
}

/// Shutdown ends a pending wait instead of stranding it.
async fn shutdown_ends_pending_waits<E: Executor>(executor: E) {
    let (session, probe) = open(&executor).await;
    let mut moving = running_zoom(&executor, &session, &probe).await;

    let (waited, shutdown) = executor
        .timeout(LONG, future::zip(moving.applied(), session.shutdown()))
        .await
        .expect("neither the wait nor shutdown hangs");
    shutdown.expect("owner shutdown");
    let error = waited.expect_err("the operation never concluded");
    let again = moving.applied().await.expect_err("still unconcluded");
    assert_eq!(debug(&error), debug(&again));
}

/// Dropping a handle while its cancellation is in flight relinquishes only
/// observation; the owner concludes the cancellation and keeps serving.
async fn dropping_a_handle_mid_cancellation<E: Executor>(executor: E) {
    let (session, probe) = open(&executor).await;
    let mut moving = running_zoom(&executor, &session, &probe).await;
    assert!(executor.timeout(SHORT, moving.cancel()).await.is_err());
    probe.await_writes(&executor, 2).await;
    drop(moving);
    probe.push(CANCELLED_SOCKET_ONE);
    probe.await_reads(&executor, 2).await;

    let camera = session.camera::<Raw>().expect("raw camera");
    let mut stop = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("stop admitted");
    probe.await_writes(&executor, 3).await;
    probe.push(ACK_SOCKET_ONE);
    probe.push(COMPLETE_SOCKET_ONE);
    stop.applied().await.expect("the owner keeps serving");
    assert_eq!(
        probe.writes(),
        vec![
            ZOOM_TELE.to_vec(),
            CANCEL_SOCKET_ONE.to_vec(),
            ZOOM_STOP.to_vec()
        ]
    );
    session.shutdown().await.expect("owner shutdown");
}

/// The dynamic projection erases the completion marker, not the contract.
#[cfg(feature = "dyn-api")]
async fn dyn_handles_share_the_contract<E: Executor>(executor: E) {
    use grafton_visca::dynapi::DynSessionCamera;

    let (session, probe) = open(&executor).await;
    let camera = DynSessionCamera::from_session(&session).expect("dynamic camera");
    let mut moving = camera
        .submit_applied(&ZoomDrive::Tele)
        .await
        .expect("continuous zoom admitted");
    probe.await_writes(&executor, 1).await;
    probe.push(ACK_SOCKET_ONE);

    assert!(matches!(
        moving.applied_with_timeout(SHORT).await,
        Err(Error::ObservationTimeout { .. })
    ));
    assert!(matches!(
        moving.cancel_with_timeout(SHORT).await,
        Err(Error::ObservationTimeout { .. })
    ));
    probe.await_writes(&executor, 2).await;
    probe.push(CANCELLED_SOCKET_ONE);
    assert_eq!(
        moving.cancel().await.expect("cancellation won"),
        CancellationOutcome::Cancelled
    );
    assert!(matches!(
        moving.applied().await,
        Err(Error::CommandCanceled)
    ));
    assert_eq!(
        probe.writes(),
        vec![ZOOM_TELE.to_vec(), CANCEL_SOCKET_ONE.to_vec()]
    );

    probe.answer_positions();
    let mut home = camera
        .submit_targeted(&PanTiltHome)
        .await
        .expect("home admitted");
    probe.await_writes(&executor, 3).await;
    probe.push(ACK_SOCKET_ONE);
    probe.push(COMPLETE_SOCKET_ONE);
    home.applied().await.expect("home applied");
    home.settled().await.expect("home settled");
    home.settled_with_timeout(Duration::ZERO)
        .await
        .expect("settlement is cached");
    session.shutdown().await.expect("owner shutdown");
}

async fn contract<E: Executor>(executor: E) {
    abandoned_waits_keep_the_handle_observing(executor.clone()).await;
    application_then_settlement(executor.clone()).await;
    a_failed_outcome_is_cached(executor.clone()).await;
    cancellation_is_one_idempotent_intent(executor.clone()).await;
    cancel_after_the_outcome_sends_nothing(executor.clone()).await;
    a_failed_cancellation_leaves_the_outcome_observable(executor.clone()).await;
    cancel_after_shutdown_reports_the_delivered_outcome(executor.clone()).await;
    shutdown_ends_pending_waits(executor.clone()).await;
    dropping_a_handle_mid_cancellation(executor.clone()).await;
    #[cfg(feature = "dyn-api")]
    dyn_handles_share_the_contract(executor).await;
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_operation_observation() {
    contract(grafton_visca::TokioRuntime::from_current().expect("Tokio runtime")).await;
}

/// The losing branch of a real `tokio::select!` drops its wait future.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_select_losing_branch_keeps_the_handle_observing() {
    let executor = grafton_visca::TokioRuntime::from_current().expect("Tokio runtime");
    let (session, probe) = open(&executor).await;
    let mut moving = running_zoom(&executor, &session, &probe).await;

    tokio::select! {
        biased;
        () = tokio::time::sleep(SHORT) => {}
        result = moving.applied() => panic!("nothing concluded the zoom: {result:?}"),
    }
    probe.push(COMPLETE_SOCKET_ONE);
    tokio::select! {
        result = moving.applied() => result.expect("the handle observes the completion"),
        () = tokio::time::sleep(LONG) => panic!("the completion was lost"),
    }
    session.shutdown().await.expect("owner shutdown");
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_operation_observation() {
    smol::block_on(contract(grafton_visca::SmolRuntime::new()));
}
