#![cfg(all(feature = "blocking", feature = "runtime-tokio"))]
//! Hardware tests for several concurrent camera sessions on a PTZOptics G2
//! bench: sessions to different cameras must stay independent, with no
//! cross-talk, no shared stalls and a correct restore.
//!
//! **WARNING: `hwc04_concurrent_drive_and_stop` physically moves every listed
//! camera.** Every test is ignored by default and meant to be run by an
//! operator who can see the cameras.
//!
//! Gating, in order, for every test:
//! 1. `#[ignore]`: the test runs only with `--ignored`.
//! 2. `VISCA_CAMERA_IPS` must list at least two distinct camera addresses,
//!    comma-separated, otherwise the test prints a skip line and returns
//!    (passing). Every camera is driven with the `PtzOpticsG2` profile; raw
//!    VISCA runs over TCP port 5678 and UDP port 1259.
//! 3. `hwc04_concurrent_drive_and_stop` additionally requires
//!    `VISCA_HW_ALLOW_MOTION=1`. `hwc05_one_camera_fault_does_not_stall_others`
//!    additionally requires `VISCA_HW_FAULT_FLAG` and `VISCA_HW_FAULT_CAMERA`.
//!    A missing variable prints a skip line and returns (passing).
//!
//! Scenarios:
//! - `hwc01_concurrent_inquiries_tokio`: one async TCP session per camera on
//!   one multi-thread Tokio runtime; all cameras run 20 rounds of the inquiry
//!   set (power, zoom position, focus position, pan/tilt position, focus
//!   mode) concurrently. No motion.
//! - `hwc02_concurrent_inquiries_blocking_threads`: the same with one blocking
//!   TCP session per camera, each on its own OS thread, released together by
//!   a barrier. No motion.
//! - `hwc03_mixed_transports`: as `hwc02`, with cameras alternating between
//!   TCP (first, third, ...) and UDP (second, fourth, ...). No motion.
//! - `hwc04_concurrent_drive_and_stop`: every camera runs, released together
//!   by a barrier per drive, a pan-right drive, a zoom-tele drive and a
//!   zoom-wide drive, each stopped after 250 ms; then each camera must report
//!   rest, and preset 1 is recalled on every camera (`RESTORED` per camera).
//! - `hwc05_one_camera_fault_does_not_stall_others`: the other cameras keep
//!   running the inquiry set while the operator injects a client-side TCP
//!   fault on one camera. No motion.
//!
//! What the inquiry scenarios assert, per camera: every reply decodes; every
//! round equals that camera's own baseline, read sequentially before the
//! concurrent phase (the baselines must distinguish the cameras, otherwise
//! attribution is unprovable and the test fails with
//! `ATTRIBUTION_INDISTINGUISHABLE`); the session metrics count no
//! acknowledgement, completion or inquiry timeout; and the session is still
//! running. Focus position identifies a camera only in manual focus, since
//! auto focus may move it between rounds.
//!
//! `hwc05` asserts that every unfaulted session sees no error, stays attributed
//! to its own camera, counts no timeout, and keeps every inquiry within the
//! profile's inquiry reply deadline (p50 and max are printed). The faulted
//! session must report only the documented stalled-stream classes
//! (`Error::Timeout`, `Error::InquiryCorrelationLost`, or an error with
//! `requires_new_session() == true`) while the fault is active, and must answer
//! an inquiry again within 30 s of the fault being lifted, on the same session
//! or, when that session cannot recover, on a fresh one.
//!
//! Safety rules the motion test keeps (as in `hardware_motion_test`):
//! - Pan/tilt moves only through `move_direction` at pan speed 1 and tilt
//!   speed 1; zoom only through `tele()` / `wide()`. Each drive is stopped
//!   within about 300 ms of wall time. No home, reset, absolute move, preset
//!   set/reset, power, menu or settings change is ever issued.
//! - A drop guard per camera sends pan/tilt STOP and zoom STOP and then
//!   recalls preset 1 (best effort) on every exit path; a failed drive fires
//!   that camera's guard at once. Preset 1 must be a safe, previously stored
//!   position on every listed camera.
//! - A command that returned an error is never retried by the test body; the
//!   guard's STOP is idempotent and is the safety action, and a preset recall
//!   is attempted at most once per camera.
//!
//! Output: every observation is one greppable line on stderr of the form
//! `HW|t=<elapsed ms since test start>|cam=<ip>|<observation>`; lines about the
//! whole bench omit the `cam=` field.
//!
//! Run with (placeholder addresses; list every camera under test):
//! ```sh
//! # Inquiry scenarios (no motion):
//! VISCA_CAMERA_IPS=192.0.2.10,192.0.2.11,192.0.2.12 \
//!   cargo test --test hardware_concurrent_test --features runtime-tokio \
//!   -- --ignored --nocapture --test-threads=1 \
//!   hwc01_concurrent_inquiries_tokio hwc02_concurrent_inquiries_blocking_threads \
//!   hwc03_mixed_transports
//!
//! # Concurrent drive and stop (moves every listed camera):
//! VISCA_CAMERA_IPS=192.0.2.10,192.0.2.11,192.0.2.12 VISCA_HW_ALLOW_MOTION=1 \
//!   cargo test --test hardware_concurrent_test --features runtime-tokio \
//!   -- --ignored --nocapture --test-threads=1 hwc04_concurrent_drive_and_stop
//!
//! # One-camera fault (no motion):
//! VISCA_CAMERA_IPS=192.0.2.10,192.0.2.11,192.0.2.12 \
//!   VISCA_HW_FAULT_FLAG=/tmp/hwc05.fault VISCA_HW_FAULT_CAMERA=192.0.2.11 \
//!   cargo test --test hardware_concurrent_test --features runtime-tokio \
//!   -- --ignored --nocapture --test-threads=1 \
//!   hwc05_one_camera_fault_does_not_stall_others
//! ```
//!
//! For `hwc05`, wait for the `cam=<fault camera>|READY_FOR_FAULT` line, then
//! inject a client-side fault on that camera's TCP port only (for example
//! `iptables -I OUTPUT -d 192.0.2.11 -p tcp --dport 5678 -j DROP`) and create
//! the flag file within 60 s. Hold the fault for at least 5 s, then lift it
//! (delete the rule) and remove the flag file within 180 s of creating it.

#[macro_use]
#[path = "common/hardware.rs"]
mod hardware;
#[path = "common/hardware_rest.rs"]
mod hardware_rest;

use std::{
    cell::Cell,
    collections::HashSet,
    env, fmt,
    future::Future,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Barrier,
    },
    thread::{self, sleep},
    time::{Duration, Instant},
};

use grafton_visca::{
    blocking::{Camera, CameraSession, Connect, Operation},
    camera::{profiles::PtzOpticsG2, IdleWait, PanTiltPosition},
    command::PanTiltDirection,
    completion,
    runtime::TokioRuntime,
    types::{FocusPosition, PanSpeed, TiltSpeed, ZoomPosition},
    AffectedAxes, Camera as AsyncCamera, CameraSession as AsyncCameraSession,
    Connect as AsyncConnect, Error, FocusMode, MetricsSnapshot, SessionStatus,
};

use hardware::{
    close_session, observe, observe_error, opted_in, report_restored, restore_and_report, Checks,
    Hw, HwLog, MotionGuard, HANDLE_WAIT,
};
use hardware_rest::{sample_rest, REST_INTERVAL, REST_SAMPLES};

const TCP_PORT: u16 = 5678;
const UDP_PORT: u16 = 1259;

/// Inquiry-set rounds each camera runs in the inquiry scenarios.
const ROUNDS: usize = 20;
/// How long a drive runs before its STOP is sent.
const DRIVE: Duration = Duration::from_millis(250);
/// Bound for the protocol idle wait after each STOP.
const IDLE_WAIT: Duration = Duration::from_secs(2);

/// Fault handshake bounds for `hwc05`.
const FAULT_APPEAR_TIMEOUT: Duration = Duration::from_secs(60);
const FAULT_HOLD_LIMIT: Duration = Duration::from_secs(180);
const FAULT_POLL: Duration = Duration::from_millis(500);
const RECOVERY_WINDOW: Duration = Duration::from_secs(30);
const RECOVERY_POLL: Duration = Duration::from_secs(1);
/// Pause between unfaulted inquiry rounds in `hwc05`.
const UNFAULTED_ROUND_PAUSE: Duration = Duration::from_millis(100);
/// Upper bound on the unfaulted loops should the faulted side never stop them.
const UNFAULTED_LIMIT: Duration = Duration::from_secs(360);

// ---------------------------------------------------------------------------
// Output and gating
// ---------------------------------------------------------------------------

/// Per-camera output: `HW|t=..|cam=<ip>|<observation>`.
#[derive(Debug, Clone)]
struct CamHw {
    hw: Hw,
    ip: Arc<str>,
}

impl CamHw {
    fn new(hw: Hw, ip: &str) -> Self {
        Self { hw, ip: ip.into() }
    }
}

impl HwLog for CamHw {
    fn log(&self, args: fmt::Arguments<'_>) {
        hw!(self.hw, "cam={}|{args}", self.ip);
    }
}

/// Reads `VISCA_CAMERA_IPS`; prints a skip line unless it lists at least two
/// addresses. A repeated address is an operator error, since two sessions to
/// one camera would make attribution meaningless.
fn camera_ips(hw: &Hw, test: &str) -> Option<Vec<String>> {
    let raw = env::var("VISCA_CAMERA_IPS").unwrap_or_default();
    let ips: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|ip| !ip.is_empty())
        .map(str::to_owned)
        .collect();
    if ips.len() < 2 {
        hw!(
            hw,
            "SKIP {test}: VISCA_CAMERA_IPS needs at least 2 comma-separated addresses (got {})",
            ips.len()
        );
        eprintln!("Skipped: VISCA_CAMERA_IPS needs at least 2 addresses");
        return None;
    }
    let distinct: HashSet<&str> = ips.iter().map(String::as_str).collect();
    assert_eq!(
        distinct.len(),
        ips.len(),
        "VISCA_CAMERA_IPS lists an address more than once: {ips:?}"
    );
    hw!(hw, "cameras={ips:?} profile=PtzOpticsG2");
    Some(ips)
}

// ---------------------------------------------------------------------------
// Transports and sessions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transport {
    Tcp,
    Udp,
}

impl Transport {
    fn label(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
        }
    }

    fn address(self, ip: &str) -> String {
        match self {
            Self::Tcp => format!("{ip}:{TCP_PORT}"),
            Self::Udp => format!("{ip}:{UDP_PORT}"),
        }
    }
}

fn open_blocking(cam: &CamHw, transport: Transport) -> CameraSession<PtzOpticsG2> {
    let address = transport.address(&cam.ip);
    hw!(cam, "connect.{}={address}", transport.label());
    let session = match transport {
        Transport::Tcp => Connect::open_tcp::<PtzOpticsG2>(address),
        Transport::Udp => Connect::open_udp::<PtzOpticsG2>(address),
    };
    session.unwrap_or_else(|error| panic!("cam={}: open session failed: {error}", cam.ip))
}

/// The inquiry set, read once.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Snapshot {
    power_on: bool,
    zoom: ZoomPosition,
    focus: FocusPosition,
    pan_tilt: PanTiltPosition,
    focus_mode: FocusMode,
}

/// The fields that identify one camera at rest. Focus position counts only in
/// manual focus, since auto focus may move it between rounds.
type Identity = (
    bool,
    ZoomPosition,
    PanTiltPosition,
    FocusMode,
    Option<FocusPosition>,
);

impl Snapshot {
    fn identity(&self) -> Identity {
        (
            self.power_on,
            self.zoom,
            self.pan_tilt,
            self.focus_mode,
            (self.focus_mode == FocusMode::Manual).then_some(self.focus),
        )
    }
}

/// Runs one inquiry, records its latency and prints a failure.
fn timed<T>(
    cam: &CamHw,
    label: &str,
    latencies: &mut Vec<Duration>,
    inquiry: impl FnOnce() -> Result<T, Error>,
) -> Result<T, Error> {
    let started = Instant::now();
    let result = inquiry();
    latencies.push(started.elapsed());
    if let Err(error) = &result {
        observe_error(cam, label, error);
    }
    result
}

fn read_blocking(
    camera: &Camera<PtzOpticsG2>,
    cam: &CamHw,
    latencies: &mut Vec<Duration>,
) -> Result<Snapshot, Error> {
    Ok(Snapshot {
        power_on: timed(cam, "power", latencies, || camera.power().state())?,
        zoom: timed(cam, "zoom", latencies, || camera.zoom().position())?,
        focus: timed(cam, "focus", latencies, || camera.focus().position())?,
        pan_tilt: timed(cam, "pan_tilt", latencies, || camera.pan_tilt().position())?,
        focus_mode: timed(cam, "focus_mode", latencies, || camera.focus().mode())?,
    })
}

/// Async counterpart of [`timed`]; the future does not run before it is
/// awaited here, so the latency covers exactly one inquiry.
async fn timed_async<T>(
    cam: &CamHw,
    label: &str,
    latencies: &mut Vec<Duration>,
    inquiry: impl Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    let started = Instant::now();
    let result = inquiry.await;
    latencies.push(started.elapsed());
    if let Err(error) = &result {
        observe_error(cam, label, error);
    }
    result
}

async fn read_async(
    camera: &AsyncCamera<PtzOpticsG2>,
    cam: &CamHw,
    latencies: &mut Vec<Duration>,
) -> Result<Snapshot, Error> {
    Ok(Snapshot {
        power_on: timed_async(cam, "power", latencies, camera.power().state()).await?,
        zoom: timed_async(cam, "zoom", latencies, camera.zoom().position()).await?,
        focus: timed_async(cam, "focus", latencies, camera.focus().position()).await?,
        pan_tilt: timed_async(cam, "pan_tilt", latencies, camera.pan_tilt().position()).await?,
        focus_mode: timed_async(cam, "focus_mode", latencies, camera.focus().mode()).await?,
    })
}

/// One camera's session with its metrics and inquiry baseline, both taken
/// before any concurrent work starts.
struct Member {
    cam: CamHw,
    session: CameraSession<PtzOpticsG2>,
    baseline: Snapshot,
    before: MetricsSnapshot,
}

impl Member {
    fn open(hw: Hw, ip: &str, transport: Transport) -> Self {
        let cam = CamHw::new(hw, ip);
        let session = open_blocking(&cam, transport);
        let before = session
            .session()
            .metrics()
            .unwrap_or_else(|error| panic!("cam={ip}: metrics failed: {error}"));
        let baseline = read_blocking(session.camera(), &cam, &mut Vec::new());
        observe(&cam, "baseline", &baseline);
        let baseline = baseline
            .unwrap_or_else(|error| panic!("cam={ip}: baseline inquiry set failed: {error}"));
        Self {
            cam,
            session,
            baseline,
            before,
        }
    }

    /// Opens every camera in turn, so the baselines are read sequentially.
    fn open_all(hw: Hw, ips: &[String], transport: impl Fn(usize) -> Transport) -> Vec<Self> {
        ips.iter()
            .enumerate()
            .map(|(index, ip)| Self::open(hw, ip, transport(index)))
            .collect()
    }
}

struct AsyncMember {
    cam: CamHw,
    session: AsyncCameraSession<PtzOpticsG2>,
    baseline: Snapshot,
    before: MetricsSnapshot,
}

impl AsyncMember {
    async fn open(hw: Hw, ip: &str, runtime: TokioRuntime) -> Self {
        let cam = CamHw::new(hw, ip);
        let address = Transport::Tcp.address(ip);
        hw!(cam, "connect.tcp={address}");
        let session = AsyncConnect::open_tcp::<PtzOpticsG2, _>(address, runtime)
            .await
            .unwrap_or_else(|error| panic!("cam={ip}: open async session failed: {error}"));
        let before = session
            .session()
            .metrics()
            .await
            .unwrap_or_else(|error| panic!("cam={ip}: metrics failed: {error}"));
        let baseline = read_async(session.camera(), &cam, &mut Vec::new()).await;
        observe(&cam, "baseline", &baseline);
        let baseline = baseline
            .unwrap_or_else(|error| panic!("cam={ip}: baseline inquiry set failed: {error}"));
        Self {
            cam,
            session,
            baseline,
            before,
        }
    }
}

// ---------------------------------------------------------------------------
// Tallies and evaluation
// ---------------------------------------------------------------------------

/// What one camera saw during its concurrent rounds.
#[derive(Debug, Default)]
struct Tally {
    rounds: usize,
    decoded: usize,
    failed: usize,
    mismatched: usize,
    latencies: Vec<Duration>,
}

impl Tally {
    fn record(
        &mut self,
        cam: &CamHw,
        baseline: &Snapshot,
        result: Result<Snapshot, Error>,
        verbose: bool,
    ) {
        let round = self.rounds;
        self.rounds += 1;
        match result {
            Ok(snapshot) => {
                self.decoded += 1;
                if snapshot.identity() != baseline.identity() {
                    self.mismatched += 1;
                    hw!(
                        cam,
                        "ATTRIBUTION_MISMATCH round={round} got={snapshot:?} baseline={baseline:?}"
                    );
                } else if verbose {
                    hw!(cam, "round[{round}]=Ok({snapshot:?})");
                }
            }
            Err(_) => {
                self.failed += 1;
                hw!(cam, "round[{round}].failed");
            }
        }
    }

    /// Median and maximum inquiry latency; zero when nothing was measured.
    fn latency(&self) -> (Duration, Duration) {
        let mut sorted = self.latencies.clone();
        sorted.sort_unstable();
        let p50 = sorted.get(sorted.len() / 2).copied().unwrap_or_default();
        let max = sorted.last().copied().unwrap_or_default();
        (p50, max)
    }
}

/// Sum of the expired-deadline counters.
fn timeouts(metrics: &MetricsSnapshot) -> u64 {
    metrics.ack_timeouts + metrics.completion_timeouts + metrics.inquiry_timeouts
}

/// Prints one camera's results and records its checks: every reply decoded,
/// every round attributed to its own camera, no timeout and a live session.
fn evaluate(
    cam: &CamHw,
    tally: &Tally,
    before: &MetricsSnapshot,
    after: &Result<MetricsSnapshot, Error>,
    checks: &mut Checks,
) {
    let (p50, max) = tally.latency();
    hw!(
        cam,
        "summary rounds={} decoded={} failed={} mismatched={} inquiries={} latency_p50_ms={:.1} latency_max_ms={:.1}",
        tally.rounds,
        tally.decoded,
        tally.failed,
        tally.mismatched,
        tally.latencies.len(),
        p50.as_secs_f64() * 1000.0,
        max.as_secs_f64() * 1000.0
    );
    observe(cam, "metrics.before", &Ok::<_, Error>(*before));
    observe(cam, "metrics.after", after);
    let new_timeouts = after
        .as_ref()
        .map(|after| timeouts(after).saturating_sub(timeouts(before)));
    hw!(cam, "metrics.new_timeouts={new_timeouts:?}");
    checks.check(
        cam,
        "every_reply_decoded",
        tally.rounds > 0 && tally.failed == 0 && tally.decoded == tally.rounds,
    );
    checks.check(
        cam,
        "replies_attributed_to_own_camera",
        tally.mismatched == 0,
    );
    checks.check(cam, "no_timeouts", matches!(new_timeouts, Ok(0)));
    checks.check(
        cam,
        "session_running",
        after
            .as_ref()
            .is_ok_and(|after| after.session == SessionStatus::Running),
    );
}

/// Attribution is provable only when no two cameras share an identity.
fn check_baselines_distinct<'a>(
    hw: &Hw,
    baselines: impl IntoIterator<Item = (&'a CamHw, &'a Snapshot)>,
    checks: &mut Checks,
) {
    let baselines: Vec<_> = baselines.into_iter().collect();
    let mut distinct = true;
    for (index, (first, first_baseline)) in baselines.iter().enumerate() {
        for (second, second_baseline) in &baselines[index + 1..] {
            if first_baseline.identity() == second_baseline.identity() {
                distinct = false;
                hw!(
                    hw,
                    "ATTRIBUTION_INDISTINGUISHABLE cam={} cam={} identity={:?}",
                    first.ip,
                    second.ip,
                    first_baseline.identity()
                );
            }
        }
    }
    checks.check(hw, "baselines_distinguish_cameras", distinct);
}

/// Shared body of the blocking inquiry scenarios: one session per camera on
/// its own OS thread, all released together by a barrier.
fn blocking_inquiry_scenario(hw: Hw, ips: &[String], transport: impl Fn(usize) -> Transport) {
    let members = Member::open_all(hw, ips, transport);
    let mut checks = Checks::default();
    check_baselines_distinct(
        &hw,
        members.iter().map(|member| (&member.cam, &member.baseline)),
        &mut checks,
    );

    let barrier = Barrier::new(members.len());
    let tallies: Vec<Tally> = thread::scope(|scope| {
        let barrier = &barrier;
        let handles: Vec<_> = members
            .iter()
            .map(|member| {
                scope.spawn(move || {
                    let mut tally = Tally::default();
                    barrier.wait();
                    hw!(member.cam, "rounds.start");
                    for _ in 0..ROUNDS {
                        let result = read_blocking(
                            member.session.camera(),
                            &member.cam,
                            &mut tally.latencies,
                        );
                        tally.record(&member.cam, &member.baseline, result, true);
                    }
                    tally
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("inquiry thread panicked"))
            .collect()
    });

    for (member, tally) in members.into_iter().zip(&tallies) {
        let after = member.session.session().metrics();
        evaluate(&member.cam, tally, &member.before, &after, &mut checks);
        close_session(&member.cam, member.session);
    }
    checks.assert_all();
}

// ---------------------------------------------------------------------------
// HWC-01..03: concurrent inquiries
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn hwc01_concurrent_inquiries_tokio() {
    let hw = Hw::new();
    let Some(ips) = camera_ips(&hw, "hwc01_concurrent_inquiries_tokio") else {
        return;
    };
    let runtime = TokioRuntime::from_current().expect("tokio runtime");
    let mut members = Vec::with_capacity(ips.len());
    for ip in &ips {
        members.push(AsyncMember::open(hw, ip, runtime.clone()).await);
    }
    let mut checks = Checks::default();
    check_baselines_distinct(
        &hw,
        members.iter().map(|member| (&member.cam, &member.baseline)),
        &mut checks,
    );

    let barrier = Arc::new(tokio::sync::Barrier::new(members.len()));
    let handles: Vec<_> = members
        .iter()
        .map(|member| {
            let camera = member.session.camera().clone();
            let cam = member.cam.clone();
            let baseline = member.baseline;
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                let mut tally = Tally::default();
                barrier.wait().await;
                hw!(cam, "rounds.start");
                for _ in 0..ROUNDS {
                    let result = read_async(&camera, &cam, &mut tally.latencies).await;
                    tally.record(&cam, &baseline, result, true);
                }
                tally
            })
        })
        .collect();
    let mut tallies = Vec::with_capacity(handles.len());
    for handle in handles {
        tallies.push(handle.await.expect("inquiry task panicked"));
    }

    for (member, tally) in members.into_iter().zip(&tallies) {
        let after = member.session.session().metrics().await;
        evaluate(&member.cam, tally, &member.before, &after, &mut checks);
        let close = member.session.close().await;
        observe(&member.cam, "session.close", &close);
    }
    checks.assert_all();
}

#[test]
#[ignore]
fn hwc02_concurrent_inquiries_blocking_threads() {
    let hw = Hw::new();
    let Some(ips) = camera_ips(&hw, "hwc02_concurrent_inquiries_blocking_threads") else {
        return;
    };
    blocking_inquiry_scenario(hw, &ips, |_| Transport::Tcp);
}

#[test]
#[ignore]
fn hwc03_mixed_transports() {
    let hw = Hw::new();
    let Some(ips) = camera_ips(&hw, "hwc03_mixed_transports") else {
        return;
    };
    blocking_inquiry_scenario(hw, &ips, |index| {
        if index % 2 == 0 {
            Transport::Tcp
        } else {
            Transport::Udp
        }
    });
}

// ---------------------------------------------------------------------------
// HWC-04: concurrent drive and stop
// ---------------------------------------------------------------------------

/// One barrier-synchronized drive, run on every camera at once.
#[derive(Debug, Clone, Copy)]
enum Drive {
    PanRight,
    ZoomTele,
    ZoomWide,
}

impl Drive {
    const ALL: [Self; 3] = [Self::PanRight, Self::ZoomTele, Self::ZoomWide];

    fn label(self) -> &'static str {
        match self {
            Self::PanRight => "pan_tilt.right",
            Self::ZoomTele => "zoom.tele",
            Self::ZoomWide => "zoom.wide",
        }
    }

    fn check_name(self) -> &'static str {
        match self {
            Self::PanRight => "pan_right_stop_applied",
            Self::ZoomTele => "zoom_tele_stop_applied",
            Self::ZoomWide => "zoom_wide_stop_applied",
        }
    }

    fn run(
        self,
        camera: &Camera<PtzOpticsG2>,
        cam: &CamHw,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error> {
        let label = self.label();
        match self {
            Self::PanRight => drive_then_stop(
                camera,
                cam,
                label,
                AffectedAxes::PAN_TILT,
                || {
                    camera
                        .pan_tilt()
                        .move_direction(PanTiltDirection::Right, pan_speed, tilt_speed)
                },
                || camera.pan_tilt().stop(),
            ),
            Self::ZoomTele => drive_then_stop(
                camera,
                cam,
                label,
                AffectedAxes::ZOOM,
                || camera.zoom().tele(),
                || camera.zoom().stop(),
            ),
            Self::ZoomWide => drive_then_stop(
                camera,
                cam,
                label,
                AffectedAxes::ZOOM,
                || camera.zoom().wide(),
                || camera.zoom().stop(),
            ),
        }
    }
}

/// Submits a drive, sends its STOP after [`DRIVE`], and returns whether the
/// STOP was applied. The drive outcome and the protocol idle wait are printed,
/// not asserted; physical rest is sampled separately.
fn drive_then_stop<D: completion::Kind, S: completion::Kind>(
    camera: &Camera<PtzOpticsG2>,
    cam: &CamHw,
    label: &str,
    axes: AffectedAxes,
    drive: impl FnOnce() -> Result<Operation<D>, Error>,
    stop: impl FnOnce() -> Result<Operation<S>, Error>,
) -> Result<(), Error> {
    let drive_started = Instant::now();
    let mut drive = drive()?;
    hw!(cam, "{label}.submitted id={:?}", drive.id());
    sleep(DRIVE);
    let mut stop = stop()?;
    hw!(
        cam,
        "{label}.drive_wall_ms_before_stop_submitted={}",
        drive_started.elapsed().as_millis()
    );
    let stop_result = stop.applied();
    observe(cam, &format!("{label}.stop.applied"), &stop_result);
    let drive_result = drive.applied_with_timeout(HANDLE_WAIT);
    observe(cam, &format!("{label}.drive.applied"), &drive_result);
    let idle = camera
        .motion()
        .wait_until_idle(IdleWait::new(axes, IDLE_WAIT));
    observe(cam, &format!("{label}.wait_until_idle"), &idle);
    stop_result
}

/// One camera's part of `hwc04`. It waits on the barrier before every drive
/// whatever happened earlier, so a failing camera never blocks the others.
fn concurrent_motion(camera: &Camera<PtzOpticsG2>, cam: &CamHw, barrier: &Barrier) -> Checks {
    let mut checks = Checks::default();
    let recall_attempted = Cell::new(false);

    let before = camera.pan_tilt().position();
    observe(cam, "pan_tilt.before", &before);
    let speeds = PanSpeed::new(1).and_then(|pan| TiltSpeed::new(1).map(|tilt| (pan, tilt)));
    observe(cam, "speeds", &speeds);
    checks.check(cam, "ready_before_motion", before.is_ok() && speeds.is_ok());
    let mut speeds = before.and(speeds).ok();

    let mut guard = Some(MotionGuard {
        camera,
        hw: cam.clone(),
        recall_attempted: &recall_attempted,
    });

    for drive in Drive::ALL {
        barrier.wait();
        let applied = match speeds {
            Some((pan_speed, tilt_speed)) => {
                let result = drive.run(camera, cam, pan_speed, tilt_speed);
                if let Err(error) = &result {
                    observe_error(cam, drive.label(), error);
                    // Stop this camera and recall preset 1 now rather than
                    // after the remaining drives; it takes no further part.
                    speeds = None;
                    drop(guard.take());
                }
                result.is_ok()
            }
            None => {
                hw!(cam, "{}.skipped", drive.label());
                false
            }
        };
        checks.check(cam, drive.check_name(), applied);
    }

    let stable = sample_rest(cam, "pan_tilt+zoom", REST_SAMPLES, REST_INTERVAL, || {
        Ok((camera.pan_tilt().position()?, camera.zoom().position()?))
    });
    checks.check(cam, "rest_stable", stable);

    if recall_attempted.get() {
        report_restored(camera, cam);
    } else {
        restore_and_report(camera, cam, &recall_attempted);
    }
    drop(guard);
    checks
}

#[test]
#[ignore]
fn hwc04_concurrent_drive_and_stop() {
    const TEST: &str = "hwc04_concurrent_drive_and_stop";
    let hw = Hw::new();
    let Some(ips) = camera_ips(&hw, TEST) else {
        return;
    };
    if !opted_in(&hw, TEST, "VISCA_HW_ALLOW_MOTION") {
        return;
    }
    let sessions: Vec<(CamHw, CameraSession<PtzOpticsG2>)> = ips
        .iter()
        .map(|ip| {
            let cam = CamHw::new(hw, ip);
            let session = open_blocking(&cam, Transport::Tcp);
            (cam, session)
        })
        .collect();

    let barrier = Barrier::new(sessions.len());
    let outcomes: Vec<thread::Result<Checks>> = thread::scope(|scope| {
        let barrier = &barrier;
        let handles: Vec<_> = sessions
            .iter()
            .map(|(cam, session)| {
                scope.spawn(move || concurrent_motion(session.camera(), cam, barrier))
            })
            .collect();
        handles.into_iter().map(|handle| handle.join()).collect()
    });

    for (cam, session) in sessions {
        close_session(&cam, session);
    }
    for outcome in outcomes {
        outcome.expect("motion thread panicked").assert_all();
    }
}

// ---------------------------------------------------------------------------
// HWC-05: one camera's fault does not stall the others
// ---------------------------------------------------------------------------

/// Sets the flag when dropped, so the unfaulted loops stop on every exit path
/// of the faulted side.
struct SetOnDrop<'a>(&'a AtomicBool);

impl Drop for SetOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// What the faulted camera's session reported across the fault.
#[derive(Debug, Default)]
struct FaultReport {
    flag_appeared: bool,
    fault_lifted: bool,
    errors: usize,
    undocumented_errors: usize,
    recovered: bool,
}

/// The documented outcomes of a client-side stall on a raw-VISCA TCP stream:
/// the inquiry's reply-deadline timeout, the owed-reply correlation lock, or a
/// session-ending error once the stream itself fails.
fn is_documented_fault_error(error: &Error) -> bool {
    matches!(
        error,
        Error::Timeout { .. } | Error::InquiryCorrelationLost { .. }
    ) || error.requires_new_session()
}

/// Polls `present` until the flag file's existence matches it or `limit`
/// passes; returns whether it matched.
fn wait_for_flag(flag: &Path, present: bool, limit: Duration) -> bool {
    let started = Instant::now();
    while flag.exists() != present {
        if started.elapsed() >= limit {
            return false;
        }
        sleep(Duration::from_millis(100));
    }
    true
}

/// The faulted camera's side of `hwc05`: the `READY_FOR_FAULT` handshake,
/// observation while the fault is active, then recovery.
fn faulted_camera(
    member: Member,
    flag: &Path,
    barrier: &Barrier,
    stop: &AtomicBool,
) -> FaultReport {
    let _stop = SetOnDrop(stop);
    let cam = member.cam;
    let session = member.session;
    let mut report = FaultReport::default();

    barrier.wait();
    hw!(cam, "READY_FOR_FAULT flag={}", flag.display());
    report.flag_appeared = wait_for_flag(flag, true, FAULT_APPEAR_TIMEOUT);
    if !report.flag_appeared {
        hw!(
            cam,
            "fault.flag_not_seen within_s={}",
            FAULT_APPEAR_TIMEOUT.as_secs()
        );
        close_session(&cam, session);
        return report;
    }
    hw!(cam, "fault.flag_seen");

    // Observe the session while the fault is active. Once an error proves the
    // session is gone, stop using it.
    let fault_started = Instant::now();
    let mut session_dead = false;
    let mut index = 0usize;
    while flag.exists() && fault_started.elapsed() < FAULT_HOLD_LIMIT {
        if !session_dead {
            let result = session.camera().power().state();
            observe(&cam, &format!("during_fault[{index}]"), &result);
            if let Err(error) = &result {
                report.errors += 1;
                let documented = is_documented_fault_error(error);
                if !documented {
                    report.undocumented_errors += 1;
                }
                hw!(cam, "during_fault[{index}].documented_class={documented}");
                session_dead = error.requires_new_session();
            }
            index += 1;
        }
        sleep(FAULT_POLL);
    }
    report.fault_lifted = !flag.exists();
    hw!(
        cam,
        "fault.observation_ended lifted={} held_ms={} errors={} session_dead={session_dead}",
        report.fault_lifted,
        fault_started.elapsed().as_millis(),
        report.errors
    );
    if !report.fault_lifted {
        close_session(&cam, session);
        return report;
    }

    // Recovery on the same session first: a stalled stream reopens its
    // inquiries once the owed reply, or any later answer, arrives.
    let mut recovered_via = None;
    let recovery_started = Instant::now();
    if !session_dead {
        let mut attempt = 0usize;
        while recovery_started.elapsed() < RECOVERY_WINDOW {
            let result = session.camera().power().state();
            observe(&cam, &format!("recovery.same_session[{attempt}]"), &result);
            attempt += 1;
            match result {
                Ok(_) => {
                    recovered_via = Some(format!("same_session attempts={attempt}"));
                    break;
                }
                Err(error) if error.requires_new_session() => break,
                Err(_) => sleep(RECOVERY_POLL),
            }
        }
    }
    close_session(&cam, session);

    // Otherwise a fresh session must answer.
    if recovered_via.is_none() {
        let address = Transport::Tcp.address(&cam.ip);
        let mut attempt = 0usize;
        while recovered_via.is_none() && recovery_started.elapsed() < 2 * RECOVERY_WINDOW {
            match Connect::open_tcp::<PtzOpticsG2>(address.clone()) {
                Ok(fresh) => {
                    let result = fresh.camera().power().state();
                    observe(&cam, &format!("recovery.new_session[{attempt}]"), &result);
                    if result.is_ok() {
                        recovered_via = Some(format!("new_session attempts={}", attempt + 1));
                    }
                    close_session(&cam, fresh);
                }
                Err(error) => {
                    observe_error(&cam, &format!("recovery.open[{attempt}]"), &error);
                }
            }
            attempt += 1;
            if recovered_via.is_none() {
                sleep(RECOVERY_POLL);
            }
        }
    }
    match &recovered_via {
        Some(via) => hw!(cam, "RECOVERED via={via}"),
        None => hw!(cam, "NOT_RECOVERED"),
    }
    report.recovered = recovered_via.is_some();
    report
}

/// An unfaulted camera's side of `hwc05`: inquiry rounds until the faulted
/// side finishes.
fn unfaulted_camera(member: &Member, barrier: &Barrier, stop: &AtomicBool) -> Tally {
    let mut tally = Tally::default();
    barrier.wait();
    hw!(member.cam, "rounds.start");
    let started = Instant::now();
    while !stop.load(Ordering::SeqCst) && started.elapsed() < UNFAULTED_LIMIT {
        let result = read_blocking(member.session.camera(), &member.cam, &mut tally.latencies);
        tally.record(&member.cam, &member.baseline, result, false);
        sleep(UNFAULTED_ROUND_PAUSE);
    }
    tally
}

#[test]
#[ignore]
fn hwc05_one_camera_fault_does_not_stall_others() {
    const TEST: &str = "hwc05_one_camera_fault_does_not_stall_others";
    let hw = Hw::new();
    let Some(ips) = camera_ips(&hw, TEST) else {
        return;
    };
    let fault = (
        env::var_os("VISCA_HW_FAULT_FLAG").map(PathBuf::from),
        env::var("VISCA_HW_FAULT_CAMERA").ok(),
    );
    let (Some(flag), Some(fault_ip)) = fault else {
        hw!(
            hw,
            "SKIP {TEST}: VISCA_HW_FAULT_FLAG and VISCA_HW_FAULT_CAMERA must both be set"
        );
        eprintln!("Skipped: VISCA_HW_FAULT_FLAG and VISCA_HW_FAULT_CAMERA not set");
        return;
    };
    let fault_ip = fault_ip.trim().to_owned();
    assert!(
        ips.contains(&fault_ip),
        "VISCA_HW_FAULT_CAMERA={fault_ip} is not listed in VISCA_CAMERA_IPS={ips:?}"
    );
    assert!(
        !flag.exists(),
        "fault flag {} already exists; remove it before starting",
        flag.display()
    );
    hw!(hw, "fault.camera={fault_ip} flag={}", flag.display());

    let (mut faulted, unfaulted): (Vec<Member>, Vec<Member>) =
        Member::open_all(hw, &ips, |_| Transport::Tcp)
            .into_iter()
            .partition(|member| *member.cam.ip == *fault_ip);
    let faulted = faulted.pop().expect("the fault camera is listed");

    let barrier = Barrier::new(ips.len());
    let stop = AtomicBool::new(false);
    let (report, tallies) = thread::scope(|scope| {
        let barrier = &barrier;
        let stop = &stop;
        let flag = flag.as_path();
        let faulted = scope.spawn(move || faulted_camera(faulted, flag, barrier, stop));
        let handles: Vec<_> = unfaulted
            .iter()
            .map(|member| scope.spawn(move || unfaulted_camera(member, barrier, stop)))
            .collect();
        let tallies: Vec<Tally> = handles
            .into_iter()
            .map(|handle| handle.join().expect("unfaulted thread panicked"))
            .collect();
        (faulted.join().expect("faulted thread panicked"), tallies)
    });

    let mut checks = Checks::default();
    for (member, tally) in unfaulted.into_iter().zip(&tallies) {
        let after = member.session.session().metrics();
        evaluate(&member.cam, tally, &member.before, &after, &mut checks);
        let bound = member.session.camera().profile().timing().inquiry_timeout();
        let (_, max) = tally.latency();
        hw!(member.cam, "latency.bound_ms={}", bound.as_millis());
        checks.check(&member.cam, "latency_within_inquiry_deadline", max <= bound);
        close_session(&member.cam, member.session);
    }

    let fault_cam = CamHw::new(hw, &fault_ip);
    hw!(fault_cam, "fault.report={report:?}");
    checks.check(&fault_cam, "fault_flag_appeared", report.flag_appeared);
    checks.check(&fault_cam, "fault_lifted", report.fault_lifted);
    checks.check(
        &fault_cam,
        "faulted_session_reported_error",
        report.errors > 0,
    );
    checks.check(
        &fault_cam,
        "faulted_errors_documented_class",
        report.undocumented_errors == 0,
    );
    checks.check(&fault_cam, "faulted_camera_recovered", report.recovered);
    checks.assert_all();
}
