//! Dropping an unobserved async movement handle stops the camera (#567).
//!
//! Every terminal method on `Operation` consumes the handle, so a handle that
//! merely goes out of scope is one no caller ever resolved.  These tests pin
//! the wire transcript for that case: an abandoned movement operation must be
//! followed by the typed STOP for its axes, while `detach`, `cancel`, and
//! ordinary completion must not add one.

#![cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]

use std::{
    future::Future,
    panic::AssertUnwindSafe,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use grafton_visca::{
    command::{PanTiltDirection, PresetNumber},
    completion::AppliedOnly,
    profile::{OperationalTuning, ProfileSpec},
    profiles::PtzOpticsG2,
    request::builtin::ZoomStop,
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    types::{PanSpeed, TiltSpeed},
    CancellationOutcome, Error, Executor, Session, SessionConfig,
};

const PAN_TILT_DRIVE_UP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0c, 0x0a, 0x03, 0x01, 0xff];
const PAN_TILT_STOP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0c, 0x0a, 0x03, 0x03, 0xff];
const ZOOM_TELE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xff];
const ZOOM_STOP_FRAME: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
const PAN_TILT_HOME: &[u8] = &[0x81, 0x01, 0x06, 0x04, 0xff];
const FOCUS_AUTO: &[u8] = &[0x81, 0x01, 0x04, 0x38, 0x02, 0xff];

const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
const COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x51, 0xff];
const ACK_SOCKET_TWO: &[u8] = &[0x90, 0x42, 0xff];
const COMPLETE_SOCKET_TWO: &[u8] = &[0x90, 0x52, 0xff];

/// Records every write and answers it from a fixed socket policy.
///
/// With `hold_first_write` the opening command is acknowledged but never
/// completed, so it is genuinely in flight when its handle is dropped; the
/// STOP that follows is then answered on the camera's second command socket.
#[derive(Debug)]
struct DropStopTransport {
    config: TransportConfig,
    responses: flume::Receiver<Vec<u8>>,
    response_tx: flume::Sender<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    send_count: usize,
    hold_first_write: bool,
}

impl DropStopTransport {
    fn new(hold_first_write: bool) -> (Self, DropStopProbe) {
        let (response_tx, responses) = flume::unbounded();
        let writes = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                config: TransportConfig::default(),
                responses,
                response_tx,
                writes: Arc::clone(&writes),
                send_count: 0,
                hold_first_write: false,
            }
            .with_hold(hold_first_write),
            DropStopProbe { writes },
        )
    }

    fn with_hold(mut self, hold_first_write: bool) -> Self {
        self.hold_first_write = hold_first_write;
        self
    }

    fn response_for(&self, send_number: usize) -> Option<Vec<u8>> {
        if self.hold_first_write {
            if send_number == 1 {
                // Acknowledged on socket one and deliberately never completed.
                return Some(ACK_SOCKET_ONE.to_vec());
            }
            let mut bytes = ACK_SOCKET_TWO.to_vec();
            bytes.extend_from_slice(COMPLETE_SOCKET_TWO);
            return Some(bytes);
        }
        let mut bytes = ACK_SOCKET_ONE.to_vec();
        bytes.extend_from_slice(COMPLETE_SOCKET_ONE);
        Some(bytes)
    }
}

#[derive(Clone, Debug)]
struct DropStopProbe {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl DropStopProbe {
    fn writes(&self) -> Vec<Vec<u8>> {
        self.writes.lock().expect("writes lock").clone()
    }
}

impl HasTransportConfig for DropStopTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for DropStopTransport {
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
        self.send_count = self.send_count.saturating_add(1);
        let response = self.response_for(self.send_count);
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        let response_tx = self.response_tx.clone();
        async move {
            if let Some(response) = response {
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

async fn wait_for_writes<E: Executor>(executor: &E, probe: &DropStopProbe, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while probe.writes().len() < count {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {count} writes; saw {:?}",
            probe.writes()
        );
        executor.sleep(Duration::from_millis(5)).await;
    }
}

/// Gives the owner actor a generous chance to emit any further write, so an
/// absent STOP is a real absence and not a race.
async fn settle<E: Executor>(executor: &E) {
    for _ in 0..20 {
        executor.sleep(Duration::from_millis(5)).await;
    }
}

/// Dropping a pan/tilt drive that the camera has acknowledged but not
/// completed sends the typed pan/tilt STOP.
async fn drop_of_in_flight_pan_tilt_stops<E: Executor>(executor: E) {
    let (transport, probe) = DropStopTransport::new(true);
    let session = Session::open(transport, g2_config(None), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    let operation = camera
        .pan_tilt()
        .move_direction(
            PanTiltDirection::Up,
            PanSpeed::new(12).expect("pan speed"),
            TiltSpeed::new(10).expect("tilt speed"),
        )
        .await
        .expect("pan/tilt drive");
    wait_for_writes(&executor, &probe, 1).await;
    assert_eq!(probe.writes(), vec![PAN_TILT_DRIVE_UP.to_vec()]);

    drop(operation);

    wait_for_writes(&executor, &probe, 2).await;
    assert_eq!(
        probe.writes(),
        vec![PAN_TILT_DRIVE_UP.to_vec(), PAN_TILT_STOP.to_vec()],
        "dropping an unobserved pan/tilt drive must stop the axis"
    );

    session.shutdown().await.expect("owner shutdown");
}

/// The same rule holds for the zoom axis and its own typed STOP.
async fn drop_of_in_flight_zoom_stops<E: Executor>(executor: E) {
    let (transport, probe) = DropStopTransport::new(true);
    let session = Session::open(transport, g2_config(None), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    let operation = camera.zoom().tele().await.expect("zoom tele");
    wait_for_writes(&executor, &probe, 1).await;

    drop(operation);

    wait_for_writes(&executor, &probe, 2).await;
    assert_eq!(
        probe.writes(),
        vec![ZOOM_TELE.to_vec(), ZOOM_STOP_FRAME.to_vec()],
        "dropping an unobserved zoom drive must stop the axis"
    );

    session.shutdown().await.expect("owner shutdown");
}

/// `detach` is the explicit opt-out and must leave the camera moving.
async fn detach_suppresses_the_drop_stop<E: Executor>(executor: E) {
    let (transport, probe) = DropStopTransport::new(true);
    let session = Session::open(transport, g2_config(None), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    camera
        .pan_tilt()
        .move_direction(
            PanTiltDirection::Up,
            PanSpeed::new(12).expect("pan speed"),
            TiltSpeed::new(10).expect("tilt speed"),
        )
        .await
        .expect("pan/tilt drive")
        .detach();
    wait_for_writes(&executor, &probe, 1).await;
    settle(&executor).await;

    assert_eq!(
        probe.writes(),
        vec![PAN_TILT_DRIVE_UP.to_vec()],
        "detach must not emit a STOP"
    );

    session.shutdown().await.expect("owner shutdown");
}

/// An observed operation is resolved by its caller, so its trailing drop adds
/// nothing to the wire.
async fn completed_and_settled_operations_add_no_stop<E: Executor>(executor: E) {
    let (transport, probe) = DropStopTransport::new(false);
    let session = Session::open(transport, g2_config(None), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    camera
        .zoom()
        .tele()
        .await
        .expect("zoom tele")
        .applied()
        .await
        .expect("zoom tele applied");
    camera
        .pan_tilt()
        .home()
        .await
        .expect("pan/tilt home")
        .settled()
        .await
        .expect("pan/tilt home settled");
    settle(&executor).await;

    assert_eq!(
        probe.writes(),
        vec![ZOOM_TELE.to_vec(), PAN_TILT_HOME.to_vec()],
        "observed operations must not emit a STOP"
    );

    session.shutdown().await.expect("owner shutdown");
}

/// Explicit cancellation is the caller's own terminal decision; the drop STOP
/// must not add a second one behind it.
async fn cancelled_operations_add_no_stop<E: Executor>(executor: E) {
    let (transport, probe) = DropStopTransport::new(true);
    let session = Session::open(transport, g2_config(Some(1)), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    // The single command socket is held by an acknowledged, never-completed
    // drive, so the second operation is still queued and cancels locally.
    let holder = camera
        .pan_tilt()
        .move_direction(
            PanTiltDirection::Up,
            PanSpeed::new(12).expect("pan speed"),
            TiltSpeed::new(10).expect("tilt speed"),
        )
        .await
        .expect("pan/tilt drive");
    wait_for_writes(&executor, &probe, 1).await;

    let queued = camera.zoom().tele().await.expect("queued zoom tele");
    let cancellation = queued.cancel().await.expect("queued cancellation");
    assert!(matches!(
        cancellation.outcome(Duration::from_secs(1)).await,
        Ok(CancellationOutcome::Cancelled)
    ));
    settle(&executor).await;

    assert_eq!(
        probe.writes(),
        vec![PAN_TILT_DRIVE_UP.to_vec()],
        "a cancelled operation must not emit a STOP"
    );

    holder.detach();
    session.shutdown().await.expect("owner shutdown");
}

/// A dropped STOP does not answer itself, and non-movement work is untouched.
async fn stops_and_non_movement_work_are_unaffected<E: Executor>(executor: E) {
    let (transport, probe) = DropStopTransport::new(false);
    let session = Session::open(transport, g2_config(None), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    // Dropping an unobserved STOP must not enqueue another STOP.
    drop(
        camera
            .submit::<AppliedOnly, _>(&ZoomStop)
            .await
            .expect("zoom stop"),
    );
    // A plain configuration command has no operation handle at all.
    camera.focus().auto().await.expect("focus auto");
    settle(&executor).await;

    assert_eq!(
        probe.writes(),
        vec![ZOOM_STOP_FRAME.to_vec(), FOCUS_AUTO.to_vec()],
        "an abandoned STOP and plain commands add nothing"
    );

    session.shutdown().await.expect("owner shutdown");
}

/// A preset recall drives every profile-selected axis, so abandoning it stops
/// all of them.
async fn drop_of_preset_recall_stops_every_affected_axis<E: Executor>(executor: E) {
    let (transport, probe) = DropStopTransport::new(false);
    let session = Session::open(transport, g2_config(None), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    drop(
        camera
            .presets()
            .recall(PresetNumber::new(3).expect("preset number"))
            .await
            .expect("preset recall"),
    );
    wait_for_writes(&executor, &probe, 2).await;
    settle(&executor).await;

    let writes = probe.writes();
    let stops = &writes[1..];
    assert!(
        stops.contains(&PAN_TILT_STOP.to_vec()),
        "preset recall drives pan/tilt; saw {writes:?}"
    );
    assert!(
        stops.contains(&ZOOM_STOP_FRAME.to_vec()),
        "preset recall drives zoom; saw {writes:?}"
    );

    session.shutdown().await.expect("owner shutdown");
}

/// The safety case the issue is about: a panic unwinding past a live handle
/// still stops the camera.
async fn drop_during_panic_unwind_stops<E: Executor>(executor: E) {
    let (transport, probe) = DropStopTransport::new(true);
    let session = Session::open(transport, g2_config(None), executor.clone())
        .await
        .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("G2 camera view");

    let operation = camera
        .pan_tilt()
        .move_direction(
            PanTiltDirection::Up,
            PanSpeed::new(12).expect("pan speed"),
            TiltSpeed::new(10).expect("tilt speed"),
        )
        .await
        .expect("pan/tilt drive");
    wait_for_writes(&executor, &probe, 1).await;

    let unwound = std::panic::catch_unwind(AssertUnwindSafe(move || {
        let _live_movement = operation;
        panic!("issue 567: simulated failure while the camera is moving");
    }));
    assert!(unwound.is_err(), "the test panic must have unwound");

    wait_for_writes(&executor, &probe, 2).await;
    assert_eq!(
        probe.writes(),
        vec![PAN_TILT_DRIVE_UP.to_vec(), PAN_TILT_STOP.to_vec()],
        "a panic must not leave hardware moving"
    );

    session.shutdown().await.expect("owner shutdown");
}

async fn run_drop_stop_matrix<E: Executor>(executor: E) {
    drop_of_in_flight_pan_tilt_stops(executor.clone()).await;
    drop_of_in_flight_zoom_stops(executor.clone()).await;
    detach_suppresses_the_drop_stop(executor.clone()).await;
    completed_and_settled_operations_add_no_stop(executor.clone()).await;
    cancelled_operations_add_no_stop(executor.clone()).await;
    stops_and_non_movement_work_are_unaffected(executor.clone()).await;
    drop_of_preset_recall_stops_every_affected_axis(executor.clone()).await;
    drop_during_panic_unwind_stops(executor).await;
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_dropping_a_movement_operation_stops_the_camera() {
    let executor = grafton_visca::TokioRuntime::from_current().expect("Tokio runtime");
    run_drop_stop_matrix(executor).await;
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_dropping_a_movement_operation_stops_the_camera() {
    smol::block_on(run_drop_stop_matrix(grafton_visca::SmolRuntime::new()));
}
