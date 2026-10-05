//! The blocking worker and the async owner produce the same wire transcript,
//! outcomes, and metrics for the same scripted camera (D24, #780).
//!
//! Both facades run the shared owner shell core. Each scenario drives one
//! session per facade against a reactive camera that answers every write the
//! same way, so the comparison is independent of scheduling: replies depend
//! only on the bytes written, never on when the owner reads them.

#![cfg(all(feature = "blocking", feature = "async", feature = "runtime-tokio"))]
#![allow(clippy::expect_used)]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    collections::VecDeque,
    future::Future,
    sync::{Arc, Mutex},
    time::Duration,
};

use grafton_visca::{
    command::CommandKind,
    completion::AppliedOnly,
    observability::MetricsSnapshot,
    profile::ProfileSpec,
    request::builtin::{FocusStop, ZoomDrive, ZoomStop},
    transport::{
        AsyncTransport, BlockingTransport, HasTransportConfig, ReceiveOutcome, SendSemantics,
        TransportConfig,
    },
    Error, SessionConfig, TokioRuntime,
};

use profile_fixtures::NonDefaultCompileTimeProfile as Raw;

/// How the scripted camera answers the commands it accepts.
#[derive(Clone, Copy, Debug)]
enum Completion {
    /// ACK then completion for every command.
    Immediate,
    /// ACK only for a zoom drive, so it stays executing until cancelled.
    HoldZoomDrive,
    /// First STOP completes only after the other two STOPs have been written.
    HoldPanTiltStop,
}

/// A deterministic raw VISCA camera with two command sockets.
#[derive(Debug)]
struct Camera {
    completion: Completion,
    /// A malformed datagram sent ahead of the first reply.
    garbage_first: bool,
    sockets: [bool; 2],
    writes: Vec<Vec<u8>>,
    replies: VecDeque<Vec<u8>>,
}

impl Camera {
    fn new(completion: Completion, garbage_first: bool) -> Self {
        Self {
            completion,
            garbage_first,
            sockets: [false; 2],
            writes: Vec::new(),
            replies: VecDeque::new(),
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        self.writes.push(bytes.to_vec());
        if std::mem::take(&mut self.garbage_first) {
            self.replies.push_back(vec![0x90, 0x41]);
        }
        match bytes {
            // Inquiry: answer the zoom position 0x1234.
            [0x81, 0x09, ..] => self
                .replies
                .push_back(vec![0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xff]),
            // Cancel of socket `s`: the command it held ends cancelled.
            [0x81, cancel, 0xff] if cancel & 0xf0 == 0x20 => {
                let socket = cancel & 0x0f;
                if let Some(busy) = self.sockets.get_mut(usize::from(socket) - 1) {
                    *busy = false;
                }
                self.replies
                    .push_back(vec![0x90, 0x60 | socket, 0x04, 0xff]);
            }
            [0x81, 0x01, rest @ ..] => {
                let Some(index) = self.sockets.iter().position(|busy| !busy) else {
                    self.replies.push_back(vec![0x90, 0x60, 0x03, 0xff]);
                    return;
                };
                let socket = u8::try_from(index + 1).expect("two sockets");
                self.replies.push_back(vec![0x90, 0x40 | socket, 0xff]);
                let held = (matches!(self.completion, Completion::HoldZoomDrive)
                    && matches!(rest, [0x04, 0x07, drive, 0xff] if *drive != 0x00))
                    || (matches!(self.completion, Completion::HoldPanTiltStop)
                        && matches!(rest, [0x06, 0x01, _, _, 0x03, 0x03, 0xff]));
                if held {
                    self.sockets[index] = true;
                } else {
                    self.replies.push_back(vec![0x90, 0x50 | socket, 0xff]);
                    if matches!(self.completion, Completion::HoldPanTiltStop)
                        && matches!(rest, [0x04, 0x08, 0x00, 0xff])
                    {
                        self.sockets[0] = false;
                        self.replies.push_back(vec![0x90, 0x51, 0xff]);
                    }
                }
            }
            _ => {}
        }
    }
}

type SharedCamera = Arc<Mutex<Camera>>;

#[derive(Debug)]
struct BlockingWire {
    config: TransportConfig,
    camera: SharedCamera,
}

impl HasTransportConfig for BlockingWire {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for BlockingWire {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: CommandKind,
        _timeout: Duration,
    ) -> Result<(), Error> {
        self.camera.lock().expect("camera lock").write(bytes);
        Ok(())
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        let next = self.camera.lock().expect("camera lock").replies.pop_front();
        let Some(bytes) = next else {
            std::thread::sleep(timeout.min(Duration::from_millis(1)));
            return Err(Error::io_timeout());
        };
        Ok(ReceiveOutcome::copy_message(&bytes, dst))
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

#[derive(Debug)]
struct AsyncWire {
    config: TransportConfig,
    camera: SharedCamera,
}

impl HasTransportConfig for AsyncWire {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for AsyncWire {
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
        self.camera.lock().expect("camera lock").write(bytes);
        async { Ok(()) }
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
        loop {
            let next = self.camera.lock().expect("camera lock").replies.pop_front();
            if let Some(bytes) = next {
                return Ok(ReceiveOutcome::copy_message(&bytes, dst));
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

/// What one facade observed for a scenario.
#[derive(Debug, PartialEq)]
struct Run {
    writes: Vec<Vec<u8>>,
    outcomes: Vec<String>,
    metrics: MetricsSnapshot,
}

fn config() -> SessionConfig {
    SessionConfig::new(ProfileSpec::from_compile_time::<Raw>().expect("raw three-axis profile"))
}

fn outcome<T: std::fmt::Debug>(result: Result<T, Error>) -> String {
    format!("{result:?}")
}

/// The scenarios, written once per facade with the same calls in the same
/// order.
#[derive(Clone, Copy, Debug)]
enum Scenario {
    /// Commands applied one after another, then an inquiry.
    Sequential,
    /// Three commands submitted before any is observed; the owner queues
    /// them on socket capacity and the raw single-flight correlation.
    QueuedSubmissions,
    /// A held zoom drive cancelled through its handle; a repeated cancel
    /// observes the first intent.
    Cancellation,
    /// A held zoom drive stopped by an urgent typed STOP on the second
    /// socket while it executes.
    UrgentStopWhileExecuting,
    /// A malformed datagram ahead of the first reply is diagnosed and the
    /// session continues.
    MalformedDatagram,
    Halt,
}

impl Scenario {
    const ALL: [Self; 6] = [
        Self::Sequential,
        Self::QueuedSubmissions,
        Self::Cancellation,
        Self::UrgentStopWhileExecuting,
        Self::MalformedDatagram,
        Self::Halt,
    ];

    /// The outcomes both facades must report, which anchors the comparison
    /// against a regression the two would share.
    fn expected_outcomes(self) -> Vec<&'static str> {
        match self {
            Self::Sequential | Self::MalformedDatagram => {
                vec!["Ok(())", "Ok(())", "Ok(ZoomPosition(4660))"]
            }
            Self::QueuedSubmissions => vec!["Ok(())"; 3],
            Self::Cancellation => vec!["Ok(Cancelled)", "Ok(Cancelled)", "Err(CommandCanceled)"],
            Self::UrgentStopWhileExecuting => vec!["Ok(())", "Ok(Cancelled)"],
            Self::Halt => {
                vec!["Ok(HaltReport { pan_tilt: Applied, zoom: Applied, focus: Applied })"]
            }
        }
    }

    fn camera(self) -> SharedCamera {
        let (completion, garbage) = match self {
            Self::Sequential | Self::QueuedSubmissions => (Completion::Immediate, false),
            Self::Cancellation | Self::UrgentStopWhileExecuting => {
                (Completion::HoldZoomDrive, false)
            }
            Self::MalformedDatagram => (Completion::Immediate, true),
            Self::Halt => (Completion::HoldPanTiltStop, false),
        };
        Arc::new(Mutex::new(Camera::new(completion, garbage)))
    }
}

fn run_blocking(scenario: Scenario) -> Run {
    use grafton_visca::blocking::Session;

    let shared = scenario.camera();
    let wire = BlockingWire {
        config: TransportConfig::default(),
        camera: Arc::clone(&shared),
    };
    let session = Session::open(wire, config()).expect("blocking session");
    let camera = session.camera::<Raw>().expect("raw camera");
    let mut outcomes = Vec::new();
    match scenario {
        Scenario::Halt => outcomes.push(outcome(camera.motion().stop_all_motion())),
        Scenario::Sequential | Scenario::MalformedDatagram => {
            outcomes.push(outcome(
                camera
                    .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
                    .and_then(|mut op| op.applied()),
            ));
            outcomes.push(outcome(
                camera
                    .submit::<AppliedOnly, _>(&ZoomStop)
                    .and_then(|mut op| op.applied()),
            ));
            outcomes.push(outcome(camera.zoom().position()));
        }
        Scenario::QueuedSubmissions => {
            let mut handles = [
                camera.submit::<AppliedOnly, _>(&ZoomDrive::Tele),
                camera.submit::<AppliedOnly, _>(&ZoomStop),
                camera.submit::<AppliedOnly, _>(&FocusStop),
            ];
            for handle in &mut handles {
                let result = match handle {
                    Ok(handle) => handle.applied(),
                    Err(error) => Err(error.clone()),
                };
                outcomes.push(outcome(result));
            }
        }
        Scenario::Cancellation => {
            let mut zoom = camera
                .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
                .expect("zoom admitted");
            outcomes.push(outcome(zoom.cancel()));
            outcomes.push(outcome(zoom.cancel()));
            outcomes.push(outcome(zoom.applied()));
        }
        Scenario::UrgentStopWhileExecuting => {
            let mut zoom = camera
                .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
                .expect("zoom admitted");
            await_writes_blocking(&shared, 1);
            outcomes.push(outcome(
                camera
                    .submit::<AppliedOnly, _>(&ZoomStop)
                    .and_then(|mut op| op.applied()),
            ));
            outcomes.push(outcome(zoom.cancel()));
        }
    }
    let metrics = session.metrics().expect("metrics");
    session.close().expect("close joins the worker");
    let writes = shared.lock().expect("camera lock").writes.clone();
    Run {
        writes,
        outcomes,
        metrics,
    }
}

async fn run_async(scenario: Scenario) -> Run {
    use grafton_visca::Session;

    let shared = scenario.camera();
    let wire = AsyncWire {
        config: TransportConfig::default(),
        camera: Arc::clone(&shared),
    };
    let executor = TokioRuntime::from_current().expect("Tokio runtime");
    let session = Session::open(wire, config(), executor)
        .await
        .expect("async session");
    let camera = session.camera::<Raw>().expect("raw camera");
    let mut outcomes = Vec::new();
    match scenario {
        Scenario::Halt => outcomes.push(outcome(camera.motion().stop_all_motion().await)),
        Scenario::Sequential | Scenario::MalformedDatagram => {
            outcomes.push(outcome(applied_async(&camera, &ZoomDrive::Tele).await));
            outcomes.push(outcome(applied_async(&camera, &ZoomStop).await));
            outcomes.push(outcome(camera.zoom().position().await));
        }
        Scenario::QueuedSubmissions => {
            let mut handles = [
                camera.submit::<AppliedOnly, _>(&ZoomDrive::Tele).await,
                camera.submit::<AppliedOnly, _>(&ZoomStop).await,
                camera.submit::<AppliedOnly, _>(&FocusStop).await,
            ];
            for handle in &mut handles {
                let result = match handle {
                    Ok(handle) => handle.applied().await,
                    Err(error) => Err(error.clone()),
                };
                outcomes.push(outcome(result));
            }
        }
        Scenario::Cancellation => {
            let mut zoom = camera
                .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
                .await
                .expect("zoom admitted");
            outcomes.push(outcome(zoom.cancel().await));
            outcomes.push(outcome(zoom.cancel().await));
            outcomes.push(outcome(zoom.applied().await));
        }
        Scenario::UrgentStopWhileExecuting => {
            let mut zoom = camera
                .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
                .await
                .expect("zoom admitted");
            await_writes_async(&shared, 1).await;
            outcomes.push(outcome(applied_async(&camera, &ZoomStop).await));
            outcomes.push(outcome(zoom.cancel().await));
        }
    }
    let metrics = session.metrics().await.expect("metrics");
    session.close().await.expect("close");
    let writes = shared.lock().expect("camera lock").writes.clone();
    Run {
        writes,
        outcomes,
        metrics,
    }
}

async fn applied_async<R>(camera: &grafton_visca::Camera<Raw>, request: &R) -> Result<(), Error>
where
    R: grafton_visca::OperationCommand<AppliedOnly>,
{
    camera
        .submit::<AppliedOnly, _>(request)
        .await?
        .applied()
        .await
}

fn written(shared: &SharedCamera) -> usize {
    shared.lock().expect("camera lock").writes.len()
}

fn await_writes_blocking(shared: &SharedCamera, count: usize) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while written(shared) < count {
        assert!(
            std::time::Instant::now() < deadline,
            "awaiting {count} writes"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

async fn await_writes_async(shared: &SharedCamera, count: usize) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while written(shared) < count {
        assert!(
            std::time::Instant::now() < deadline,
            "awaiting {count} writes"
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blocking_and_async_owners_agree_on_every_scenario() {
    for scenario in Scenario::ALL {
        let blocking = tokio::task::spawn_blocking(move || run_blocking(scenario))
            .await
            .expect("blocking scenario");
        let asynchronous = run_async(scenario).await;
        assert!(
            !blocking.writes.is_empty(),
            "{scenario:?} wrote nothing on the blocking facade"
        );
        assert_eq!(
            blocking.outcomes,
            scenario.expected_outcomes(),
            "{scenario:?} outcomes"
        );
        assert_eq!(blocking, asynchronous, "{scenario:?} diverged");
    }
}

#[cfg(feature = "dyn-api")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dynamic_halt_reports_match_typed_facades_with_independent_stop_dispatch() {
    let blocking = tokio::task::spawn_blocking(|| {
        let shared = Scenario::Halt.camera();
        let session = grafton_visca::blocking::Session::open(
            BlockingWire {
                config: TransportConfig::default(),
                camera: Arc::clone(&shared),
            },
            config(),
        )
        .expect("session");
        let camera = session.camera_dyn().expect("dynamic blocking camera");
        let outcomes = vec![outcome(camera.motion().stop_all_motion())];
        let metrics = session.metrics().expect("metrics");
        session.close().expect("close");
        let writes = shared.lock().expect("camera lock").writes.clone();
        Run {
            writes,
            outcomes,
            metrics,
        }
    })
    .await
    .expect("blocking scenario");
    let shared = Scenario::Halt.camera();
    let session = grafton_visca::Session::open(
        AsyncWire {
            config: TransportConfig::default(),
            camera: Arc::clone(&shared),
        },
        config(),
        TokioRuntime::from_current().expect("runtime"),
    )
    .await
    .expect("session");
    let camera = session.camera_dyn().expect("dynamic camera");
    let outcomes = vec![outcome(camera.motion().stop_all_motion().await)];
    let metrics = session.metrics().await.expect("metrics");
    session.shutdown().expect("shutdown");
    let writes = shared.lock().expect("camera lock").writes.clone();
    let asynchronous = Run {
        writes,
        outcomes,
        metrics,
    };
    assert_eq!(blocking.outcomes, Scenario::Halt.expected_outcomes());
    assert_eq!(blocking, asynchronous);
    assert_eq!(asynchronous, run_async(Scenario::Halt).await);
}
