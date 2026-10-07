//! The blocking worker and the async owner produce the same wire transcript,
//! outcomes, and metrics for the same scripted camera (D24, #780).
//!
//! Both facades run the shared owner shell core. Each scenario drives one
//! session per facade against a reactive camera that answers every write the
//! same way, so the comparison is independent of scheduling: replies depend
//! only on the bytes written, never on when the owner reads them.

#![cfg(all(
    feature = "blocking",
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
#![allow(clippy::expect_used)]

#[path = "common/fake_camera.rs"]
mod fake_camera;
#[macro_use]
#[path = "common/matrix.rs"]
mod matrix;
#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use grafton_visca::{
    completion::AppliedOnly,
    observability::MetricsSnapshot,
    profile::ProfileSpec,
    request::builtin::{FocusStop, ZoomDrive, ZoomStop},
    Error, Executor, SessionConfig,
};

use fake_camera::{frames, FakeCamera};
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

/// A deterministic raw VISCA camera with two command sockets. Its replies
/// depend only on the bytes written and on which sockets it holds busy.
///
/// With `garbage_first`, a malformed datagram is sent ahead of the first
/// reply.
fn scripted_camera(completion: Completion, garbage_first: bool) -> FakeCamera {
    let mut garbage_first = garbage_first;
    let mut sockets = [false; 2];
    FakeCamera::new(move |bytes, answer| {
        if std::mem::take(&mut garbage_first) {
            answer.reply(vec![0x90, 0x41]);
        }
        match bytes {
            // Inquiry: answer the zoom position 0x1234.
            [0x81, 0x09, ..] => {
                answer.reply(frames::inquiry_reply(&[0x01, 0x02, 0x03, 0x04]));
            }
            // Cancel of socket `s`: the command it held ends cancelled.
            [0x81, cancel, 0xff] if cancel & 0xf0 == 0x20 => {
                let socket = cancel & 0x0f;
                if let Some(busy) = sockets.get_mut(usize::from(socket) - 1) {
                    *busy = false;
                }
                answer.reply(frames::canceled(socket));
            }
            [0x81, 0x01, rest @ ..] => {
                let Some(index) = sockets.iter().position(|busy| !busy) else {
                    answer.reply(frames::buffer_full());
                    return;
                };
                let socket = u8::try_from(index + 1).expect("two sockets");
                answer.reply(frames::ack(socket));
                let held = (matches!(completion, Completion::HoldZoomDrive)
                    && matches!(rest, [0x04, 0x07, drive, 0xff] if *drive != 0x00))
                    || (matches!(completion, Completion::HoldPanTiltStop)
                        && matches!(rest, [0x06, 0x01, _, _, 0x03, 0x03, 0xff]));
                if held {
                    sockets[index] = true;
                } else {
                    answer.reply(frames::complete(socket));
                    if matches!(completion, Completion::HoldPanTiltStop)
                        && matches!(rest, [0x04, 0x08, 0x00, 0xff])
                    {
                        sockets[0] = false;
                        answer.reply(frames::complete(1));
                    }
                }
            }
            _ => {}
        }
    })
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

    fn camera(self) -> FakeCamera {
        let (completion, garbage) = match self {
            Self::Sequential | Self::QueuedSubmissions => (Completion::Immediate, false),
            Self::Cancellation | Self::UrgentStopWhileExecuting => {
                (Completion::HoldZoomDrive, false)
            }
            Self::MalformedDatagram => (Completion::Immediate, true),
            Self::Halt => (Completion::HoldPanTiltStop, false),
        };
        scripted_camera(completion, garbage)
    }
}

fn run_blocking(scenario: Scenario) -> Run {
    use grafton_visca::blocking::Session;

    let fake = scenario.camera();
    let session = Session::open(fake.blocking_wire(), config()).expect("blocking session");
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
            fake.wait_for_writes(1);
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
    Run {
        writes: fake.writes(),
        outcomes,
        metrics,
    }
}

async fn run_async<E: Executor>(scenario: Scenario, executor: &E) -> Run {
    use grafton_visca::Session;

    let fake = scenario.camera();
    let session = Session::open(fake.async_wire(), config(), executor.clone())
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
            fake.wait_for_writes_async(executor, 1).await;
            outcomes.push(outcome(applied_async(&camera, &ZoomStop).await));
            outcomes.push(outcome(zoom.cancel().await));
        }
    }
    let metrics = session.metrics().await.expect("metrics");
    session.close().await.expect("close");
    Run {
        writes: fake.writes(),
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

/// Runs a blocking session on its own thread so the async side can wait for
/// it without stalling the runtime, whichever runtime that is.
async fn off_thread<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let (done, result) = flume::bounded(1);
    std::thread::spawn(move || {
        // The receiver outlives the thread unless the test already failed.
        let _ = done.send(work());
    });
    result
        .recv_async()
        .await
        .expect("the blocking scenario thread finished")
}

/// One scenario on both facades: the same wire transcript, outcomes, and
/// metrics.
async fn agree_on<E: Executor>(scenario: Scenario, executor: E) {
    let blocking = off_thread(move || run_blocking(scenario)).await;
    let asynchronous = run_async(scenario, &executor).await;
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

async fn sequential_commands_then_an_inquiry<E: Executor>(executor: E) {
    agree_on(Scenario::Sequential, executor).await;
}

async fn queued_submissions<E: Executor>(executor: E) {
    agree_on(Scenario::QueuedSubmissions, executor).await;
}

async fn cancellation<E: Executor>(executor: E) {
    agree_on(Scenario::Cancellation, executor).await;
}

async fn urgent_stop_while_executing<E: Executor>(executor: E) {
    agree_on(Scenario::UrgentStopWhileExecuting, executor).await;
}

async fn malformed_datagram<E: Executor>(executor: E) {
    agree_on(Scenario::MalformedDatagram, executor).await;
}

async fn halt<E: Executor>(executor: E) {
    agree_on(Scenario::Halt, executor).await;
}

// The Tokio cases keep the multi-thread runtime the comparison has always run
// on, so the async owner is also checked under work stealing.
runtime_matrix!(
    multi_thread:
    sequential_commands_then_an_inquiry,
    queued_submissions,
    cancellation,
    urgent_stop_while_executing,
    malformed_datagram,
    halt
);

#[cfg(feature = "dyn-api")]
async fn dynamic_halt_reports_match_typed_facades_with_independent_stop_dispatch<E: Executor>(
    executor: E,
) {
    let blocking = off_thread(|| {
        let fake = Scenario::Halt.camera();
        let session = grafton_visca::blocking::Session::open(fake.blocking_wire(), config())
            .expect("session");
        let camera = session.camera_dyn().expect("dynamic blocking camera");
        let outcomes = vec![outcome(camera.motion().stop_all_motion())];
        let metrics = session.metrics().expect("metrics");
        session.close().expect("close");
        Run {
            writes: fake.writes(),
            outcomes,
            metrics,
        }
    })
    .await;
    let fake = Scenario::Halt.camera();
    let session = grafton_visca::Session::open(fake.async_wire(), config(), executor.clone())
        .await
        .expect("session");
    let camera = session.camera_dyn().expect("dynamic camera");
    let outcomes = vec![outcome(camera.motion().stop_all_motion().await)];
    let metrics = session.metrics().await.expect("metrics");
    session.shutdown().expect("shutdown");
    let asynchronous = Run {
        writes: fake.writes(),
        outcomes,
        metrics,
    };
    assert_eq!(blocking.outcomes, Scenario::Halt.expected_outcomes());
    assert_eq!(blocking, asynchronous);
    assert_eq!(asynchronous, run_async(Scenario::Halt, &executor).await);
}

#[cfg(feature = "dyn-api")]
runtime_matrix!(multi_thread: dynamic_halt_reports_match_typed_facades_with_independent_stop_dispatch);
