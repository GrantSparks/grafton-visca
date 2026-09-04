//! Issue #630: the engine's four control-class lanes, through async API.
//!
//! The blocking counterpart lives in
//! `tests/issue_630_submission_class_blocking.rs`; both observe the same
//! engine behaviour at the transport boundary. This file additionally checks
//! that the erased `dyn-api` projection carries the same surface.

#![cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]

use std::{
    future::Future,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use grafton_visca::{
    completion::AppliedOnly,
    profile::ProfileSpec,
    profiles::PtzOpticsG2,
    request::builtin::{FocusDrive, FocusModeCommand, ZoomDrive, ZoomStop},
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    Error, Executor, Session, SessionConfig, SubmissionClass,
};

const ZOOM_TELE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xff];
const ZOOM_WIDE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x03, 0xff];
const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
const FOCUS_FAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x02, 0xff];
const FOCUS_NEAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x03, 0xff];

/// A two-socket camera that acknowledges every write immediately and only
/// completes a socket when the test says so, which is what makes "the socket
/// freed, and this is what the owner wrote next" observable.
#[derive(Debug)]
struct LaneTransport {
    config: TransportConfig,
    responses: flume::Receiver<Vec<u8>>,
    response_tx: flume::Sender<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    send_count: usize,
}

impl LaneTransport {
    fn new() -> (Self, LaneProbe) {
        let (response_tx, responses) = flume::unbounded();
        let writes = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                config: TransportConfig::default(),
                responses,
                response_tx: response_tx.clone(),
                writes: Arc::clone(&writes),
                send_count: 0,
            },
            LaneProbe {
                response_tx,
                writes,
            },
        )
    }
}

#[derive(Clone, Debug)]
struct LaneProbe {
    response_tx: flume::Sender<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl LaneProbe {
    /// Completes one command socket, freeing it for the next dispatch.
    fn complete_socket(&self, socket: u8) {
        self.response_tx
            .send(vec![0x90, 0x50 | socket, 0xff])
            .expect("owner response channel remains connected");
    }

    fn writes(&self) -> Vec<Vec<u8>> {
        self.writes.lock().expect("writes lock").clone()
    }
}

impl HasTransportConfig for LaneTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for LaneTransport {
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
        self.send_count = self.send_count.saturating_add(1);
        // Two sockets, assigned round-robin exactly as a real camera would as
        // each one becomes free again.
        let socket = u8::try_from(self.send_count % 2).unwrap_or(1) + 1;
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        let response_tx = self.response_tx.clone();
        async move {
            response_tx
                .send_async(vec![0x90, 0x40 | socket, 0xff])
                .await
                .map_err(|_| Error::ConnectionClosed { reason: None })?;
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
            Ok(length)
        }
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

fn g2_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("built-in G2 profile"),
    )
}

async fn wait_for_writes<E: Executor>(executor: &E, probe: &LaneProbe, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while probe.writes().len() < count {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {count} writes; saw {:?}",
            probe.writes(),
        );
        executor.sleep(Duration::from_millis(2)).await;
    }
}

/// Asserts that no further write appears while the queue is deliberately
/// starved of sockets.
async fn assert_stable_write_count<E: Executor>(executor: &E, probe: &LaneProbe, count: usize) {
    for _ in 0..5 {
        executor.sleep(Duration::from_millis(2)).await;
    }
    assert_eq!(
        probe.writes().len(),
        count,
        "no queued request may be written while every socket is busy",
    );
}

/// A background-class submission yields the freed socket to a user-class
/// submission made after it.
async fn background_yields_to_a_later_user_submission<E: Executor>(executor: E) {
    let (transport, probe) = LaneTransport::new();
    let session = Session::open(transport, g2_config(), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    let first = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .await
        .expect("first socket");
    wait_for_writes(&executor, &probe, 1).await;
    let second = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Far)
        .await
        .expect("second socket");
    wait_for_writes(&executor, &probe, 2).await;

    let background = camera
        .with_submission_class(SubmissionClass::Background)
        .submit::<AppliedOnly, _>(&ZoomDrive::Wide)
        .await
        .expect("background submission is admitted and queued");
    let user = camera
        .with_submission_class(SubmissionClass::User)
        .submit::<AppliedOnly, _>(&FocusDrive::Near)
        .await
        .expect("user submission is admitted and queued");
    assert_stable_write_count(&executor, &probe, 2).await;

    probe.complete_socket(2);
    wait_for_writes(&executor, &probe, 3).await;
    assert_eq!(
        probe.writes()[2],
        FOCUS_NEAR.to_vec(),
        "issue #630: the later user-class request takes the freed socket",
    );

    probe.complete_socket(1);
    wait_for_writes(&executor, &probe, 4).await;
    assert_eq!(
        probe.writes()[3],
        ZOOM_WIDE.to_vec(),
        "the background request is written once nothing outranks it",
    );
    assert_eq!(
        probe.writes()[0..2].to_vec(),
        vec![ZOOM_TELE.to_vec(), FOCUS_FAR.to_vec()],
    );

    first.detach();
    second.detach();
    user.detach();
    background.detach();
    session.shutdown().await.expect("owner shutdown");
}

/// The handle default demotes one handle's traffic; a sibling clone keeps the
/// built-in classification, and a derived view can outrank the default.
async fn handle_default_and_derived_view_override<E: Executor>(executor: E) {
    let (transport, probe) = LaneTransport::new();
    let session = Session::open(transport, g2_config(), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    let first = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .await
        .expect("first socket");
    wait_for_writes(&executor, &probe, 1).await;
    let second = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Far)
        .await
        .expect("second socket");
    wait_for_writes(&executor, &probe, 2).await;

    let mut poller = camera.clone();
    assert_eq!(poller.submission_class(), None);
    poller.set_submission_class(Some(SubmissionClass::Background));
    assert_eq!(poller.submission_class(), Some(SubmissionClass::Background));
    assert_eq!(
        camera.submission_class(),
        None,
        "a clone diverges rather than sharing the default",
    );

    // Both are `SubmissionClass::User` built-ins, so only the handle they came
    // from can separate them.
    let demoted = poller
        .submit::<AppliedOnly, _>(&ZoomDrive::Wide)
        .await
        .expect("demoted submission");
    let ordinary = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Near)
        .await
        .expect("ordinary submission");
    assert_stable_write_count(&executor, &probe, 2).await;

    probe.complete_socket(2);
    wait_for_writes(&executor, &probe, 3).await;
    assert_eq!(
        probe.writes()[2],
        FOCUS_NEAR.to_vec(),
        "issue #630: the demoted handle's earlier request yields the socket",
    );

    probe.complete_socket(1);
    wait_for_writes(&executor, &probe, 4).await;
    assert_eq!(probe.writes()[3], ZOOM_WIDE.to_vec());

    // Now the override: the demoted handle submits first and still wins.
    let raised = poller
        .with_submission_class(SubmissionClass::User)
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .await
        .expect("raised submission");
    let later = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Far)
        .await
        .expect("later ordinary submission");
    assert_stable_write_count(&executor, &probe, 4).await;
    assert_eq!(
        poller.submission_class(),
        Some(SubmissionClass::Background),
        "a derived view does not change the source handle default",
    );

    probe.complete_socket(2);
    wait_for_writes(&executor, &probe, 5).await;
    assert_eq!(
        probe.writes()[4],
        ZOOM_TELE.to_vec(),
        "issue #630: the derived view's class wins over the handle default",
    );

    first.detach();
    second.detach();
    demoted.detach();
    ordinary.detach();
    raised.detach();
    later.detach();
    session.shutdown().await.expect("owner shutdown");
}

/// A typed stop retains its urgent safety class under both QoS override forms.
async fn urgent_cannot_be_demoted_by_any_submission_override<E: Executor>(executor: E) {
    let (transport, probe) = LaneTransport::new();
    let session = Session::open(transport, g2_config(), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    let first = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .await
        .expect("first socket");
    wait_for_writes(&executor, &probe, 1).await;
    let second = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Far)
        .await
        .expect("second socket");
    wait_for_writes(&executor, &probe, 2).await;

    let ordinary = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Near)
        .await
        .expect("ordinary user-class submission");
    let mut poller = camera.clone();
    poller.set_submission_class(Some(SubmissionClass::Background));
    let stop = poller
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("stop from a demoted handle");
    assert_stable_write_count(&executor, &probe, 2).await;

    probe.complete_socket(2);
    wait_for_writes(&executor, &probe, 3).await;
    assert_eq!(
        probe.writes()[2],
        ZOOM_STOP.to_vec(),
        "issue #630: an urgent stop keeps its class under a demoted handle",
    );

    probe.complete_socket(1);
    wait_for_writes(&executor, &probe, 4).await;
    assert_eq!(probe.writes()[3], FOCUS_NEAR.to_vec());

    // A derived view's QoS value is also unable to weaken the stop's intrinsic
    // urgent safety floor.
    let protected_stop = camera
        .with_submission_class(SubmissionClass::Background)
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("stop with background ordinary-traffic QoS");
    let later = camera
        .submit::<AppliedOnly, _>(&FocusDrive::Far)
        .await
        .expect("later user-class submission");
    assert_stable_write_count(&executor, &probe, 4).await;

    probe.complete_socket(2);
    wait_for_writes(&executor, &probe, 5).await;
    assert_eq!(
        probe.writes()[4],
        ZOOM_STOP.to_vec(),
        "issue #630: a derived view cannot demote an urgent stop",
    );

    first.detach();
    second.detach();
    ordinary.detach();
    stop.detach();
    protected_stop.detach();
    later.detach();
    session.shutdown().await.expect("owner shutdown");
}

/// The classified plain-command and inquiry routes reach the same owner.
async fn classified_execute_reaches_the_wire<E: Executor>(executor: E) {
    let (transport, probe) = LaneTransport::new();
    let session = Session::open(transport, g2_config(), executor.clone())
        .await
        .expect("owner session");
    let mut camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    let background = camera.with_submission_class(SubmissionClass::Background);
    let execute = background.execute(&FocusModeCommand::Manual);
    let completion = async {
        wait_for_writes(&executor, &probe, 1).await;
        probe.complete_socket(2);
    };
    let (result, ()) = futures_lite::future::zip(execute, completion).await;
    result.expect("background plain command still executes");

    camera.set_submission_class(Some(SubmissionClass::User));
    let execute = camera.execute(&FocusModeCommand::Auto);
    let completion = async {
        wait_for_writes(&executor, &probe, 2).await;
        probe.complete_socket(1);
    };
    let (result, ()) = futures_lite::future::zip(execute, completion).await;
    result.expect("handle default plain command still executes");

    assert_eq!(probe.writes().len(), 2);
    session.shutdown().await.expect("owner shutdown");
}

/// The erased projection carries the same submission-class surface.
#[cfg(feature = "dyn-api")]
async fn dyn_projection_has_the_same_surface<E: Executor>(executor: E) {
    use grafton_visca::DynSessionCamera;

    let (transport, probe) = LaneTransport::new();
    let session = Session::open(transport, g2_config(), executor.clone())
        .await
        .expect("owner session");
    let dynamic = DynSessionCamera::from_session(&session).expect("dynamic view");

    let first = dynamic
        .submit_applied(&ZoomDrive::Tele)
        .await
        .expect("first socket");
    wait_for_writes(&executor, &probe, 1).await;
    let second = dynamic
        .submit_applied(&FocusDrive::Far)
        .await
        .expect("second socket");
    wait_for_writes(&executor, &probe, 2).await;

    let mut poller = dynamic.clone();
    assert_eq!(poller.submission_class(), None);
    poller.set_submission_class(Some(SubmissionClass::Background));
    assert_eq!(poller.submission_class(), Some(SubmissionClass::Background));
    assert_eq!(dynamic.submission_class(), None);

    let demoted = poller
        .submit_applied(&ZoomDrive::Wide)
        .await
        .expect("demoted submission");
    let ordinary = dynamic
        .with_submission_class(SubmissionClass::User)
        .submit_applied(&FocusDrive::Near)
        .await
        .expect("ordinary submission");
    assert_stable_write_count(&executor, &probe, 2).await;

    probe.complete_socket(2);
    wait_for_writes(&executor, &probe, 3).await;
    assert_eq!(
        probe.writes()[2],
        FOCUS_NEAR.to_vec(),
        "issue #630: the dynamic projection uses the same lanes",
    );

    probe.complete_socket(1);
    wait_for_writes(&executor, &probe, 4).await;
    assert_eq!(probe.writes()[3], ZOOM_WIDE.to_vec());

    // A typed view projected out of the dynamic one inherits the default.
    let typed = poller.camera::<PtzOpticsG2>().expect("typed projection");
    assert_eq!(typed.submission_class(), Some(SubmissionClass::Background));

    first.detach();
    second.detach();
    demoted.detach();
    ordinary.detach();
    session.shutdown().await.expect("owner shutdown");
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_submission_class_matrix() {
    let executor = grafton_visca::TokioRuntime::from_current().expect("Tokio runtime");
    background_yields_to_a_later_user_submission(executor.clone()).await;
    handle_default_and_derived_view_override(executor.clone()).await;
    urgent_cannot_be_demoted_by_any_submission_override(executor.clone()).await;
    classified_execute_reaches_the_wire(executor.clone()).await;
    #[cfg(feature = "dyn-api")]
    dyn_projection_has_the_same_surface(executor).await;
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_submission_class_matrix() {
    smol::block_on(async {
        let executor = grafton_visca::SmolRuntime::new();
        background_yields_to_a_later_user_submission(executor).await;
        handle_default_and_derived_view_override(executor).await;
        urgent_cannot_be_demoted_by_any_submission_override(executor).await;
        classified_execute_reaches_the_wire(executor).await;
        #[cfg(feature = "dyn-api")]
        dyn_projection_has_the_same_surface(executor).await;
    });
}
