#![cfg(all(feature = "blocking", feature = "runtime-tokio"))]
//! Hardware motion and recovery tests for PTZOptics G2 cameras.
//!
//! **WARNING: the motion tests physically move the camera.** They are ignored
//! by default and are meant to be run by an operator who can see the camera.
//!
//! Gating, in order, for every test:
//! 1. `#[ignore]`: the test runs only with `--ignored`.
//! 2. `VISCA_CAMERA_IP` must be set, otherwise the test prints a skip line and
//!    returns (passing).
//! 3. Tests that can move the camera additionally require
//!    `VISCA_HW_ALLOW_MOTION=1`, otherwise they skip the same way.
//!    `hw04_halt_idle_report` and `hw04_recover_after_disconnect` never move
//!    the camera and do not need it. `hw04_recover_after_disconnect` also needs
//!    `VISCA_HW_FAULT_FLAG` (a file path the operator creates to signal a
//!    client-side fault injection).
//!
//! Safety rules the motion tests keep:
//! - Pan/tilt moves only through `move_direction` at pan speed 1 and tilt
//!   speed 1; zoom only through `tele()` / `wide()`. Each drive is stopped
//!   within about 300 ms of wall time. No home, reset, absolute move, preset
//!   set/reset, power, menu or settings change is ever issued.
//! - A drop guard sends pan/tilt STOP and zoom STOP and then recalls preset 1
//!   (best effort) on every exit path, including a panic or an early `?`.
//! - Each motion test ends by explicitly recalling preset 1 and printing
//!   `RESTORED pan=.. tilt=.. zoom=..`. Preset 1 must therefore be a safe,
//!   previously stored position on the camera under test.
//! - A command that returned an `Unconfirmed` error is never retried by the
//!   test body. The drop guard sends a fresh STOP unconditionally because STOP
//!   is idempotent and is the safety action; it never repeats a preset recall
//!   that was already attempted.
//!
//! Output: every observation is one greppable line on stderr of the form
//! `HW|t=<elapsed ms since test start>|<observation>`.
//!
//! Run with:
//! ```sh
//! VISCA_CAMERA_IP=192.168.0.110 VISCA_HW_ALLOW_MOTION=1 \
//!   cargo test --test hardware_motion_test --features runtime-tokio \
//!   -- --ignored --nocapture --test-threads=1
//! ```
//!
//! `hw04_recover_after_disconnect` additionally needs
//! `VISCA_HW_FAULT_FLAG=/path/to/flag` and an operator who creates that file
//! after the `READY_FOR_FAULT` line, injects a client-side fault (for example
//! a firewall drop of the camera's TCP port), and later removes the file once
//! the fault is lifted.

use std::{
    cell::Cell,
    env, fmt,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::sleep,
    time::{Duration, Instant},
};

use grafton_visca::{
    blocking::{Camera, Connect},
    camera::{profiles::PtzOpticsG2, IdleWait},
    command::PanTiltDirection,
    runtime::TokioRuntime,
    types::{PanSpeed, TiltSpeed},
    AffectedAxes, Camera as AsyncCamera, Certainty, Connect as AsyncConnect, Error, ErrorKind,
    HaltOutcome, HaltReport, PresetNumber,
};

const TCP_PORT: u16 = 5678;
const UDP_PORT: u16 = 1259;

/// How long a drive runs before its STOP is sent.
const DRIVE: Duration = Duration::from_millis(250);
/// Bound for waiting on a guard or handle observation.
const HANDLE_WAIT: Duration = Duration::from_secs(2);
/// Number of samples and spacing used for the physical-rest observation.
const REST_SAMPLES: usize = 5;
const REST_INTERVAL: Duration = Duration::from_millis(200);

// ---------------------------------------------------------------------------
// Output and gating helpers
// ---------------------------------------------------------------------------

/// Test clock: every output line carries milliseconds since test start.
#[derive(Debug, Clone, Copy)]
struct Hw {
    start: Instant,
}

impl Hw {
    fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    fn log(&self, args: fmt::Arguments<'_>) {
        eprintln!("HW|t={}|{}", self.start.elapsed().as_millis(), args);
    }
}

macro_rules! hw {
    ($hw:expr, $($arg:tt)*) => {
        $hw.log(format_args!($($arg)*))
    };
}

/// Whether the error means the request's physical outcome is unknowable, so
/// it must never be replayed.
fn is_unconfirmed(error: &Error) -> bool {
    error.kind() == ErrorKind::Unconfirmed
        || error
            .failure_context()
            .is_some_and(|context| context.certainty == Certainty::Unconfirmed)
}

/// Prints the Debug of an error plus its classification.
fn observe_error(hw: &Hw, label: &str, error: &Error) {
    hw!(hw, "{label}=Err({error:?})");
    hw!(
        hw,
        "{label}.error kind={:?} requires_new_session={} unconfirmed={} display=\"{error}\"",
        error.kind(),
        error.requires_new_session(),
        is_unconfirmed(error)
    );
}

/// Prints the Debug of a result, with the error classification on failure.
fn observe<T: fmt::Debug>(hw: &Hw, label: &str, result: &Result<T, Error>) {
    match result {
        Ok(value) => hw!(hw, "{label}=Ok({value:?})"),
        Err(error) => observe_error(hw, label, error),
    }
}

/// Reads `VISCA_CAMERA_IP`; prints a skip line when unset.
fn camera_ip(hw: &Hw, test: &str) -> Option<String> {
    if let Ok(ip) = env::var("VISCA_CAMERA_IP") {
        if !ip.trim().is_empty() {
            return Some(ip.trim().to_owned());
        }
    }
    hw!(hw, "SKIP {test}: VISCA_CAMERA_IP not set");
    eprintln!("Skipped: VISCA_CAMERA_IP not set");
    None
}

/// Gate for tests that may move the camera: needs the camera IP and
/// `VISCA_HW_ALLOW_MOTION=1`.
fn motion_gate(hw: &Hw, test: &str) -> Option<String> {
    let ip = camera_ip(hw, test)?;
    if env::var("VISCA_HW_ALLOW_MOTION").as_deref() != Ok("1") {
        hw!(hw, "SKIP {test}: VISCA_HW_ALLOW_MOTION=1 not set");
        eprintln!("Skipped: VISCA_HW_ALLOW_MOTION=1 not set");
        return None;
    }
    Some(ip)
}

/// Named boolean checks collected during a test and asserted only after the
/// camera has been restored and the session closed.
#[derive(Debug, Default)]
struct Checks(Vec<(&'static str, bool)>);

impl Checks {
    fn check(&mut self, hw: &Hw, name: &'static str, ok: bool) {
        hw!(hw, "check.{name}={ok}");
        self.0.push((name, ok));
    }

    fn assert_all(&self) {
        let failed: Vec<&str> = self
            .0
            .iter()
            .filter(|(_, ok)| !ok)
            .map(|(name, _)| *name)
            .collect();
        assert!(failed.is_empty(), "failed checks: {failed:?}");
    }
}

/// Samples `read` `samples` times, `interval` apart, and reports whether every
/// sample succeeded and all samples are equal.
fn sample_rest<T, F>(hw: &Hw, label: &str, samples: usize, interval: Duration, mut read: F) -> bool
where
    T: fmt::Debug + PartialEq,
    F: FnMut() -> Result<T, Error>,
{
    let mut values: Vec<T> = Vec::with_capacity(samples);
    let mut all_ok = true;
    for index in 0..samples {
        if index > 0 {
            sleep(interval);
        }
        let result = read();
        observe(hw, &format!("{label}.sample[{index}]"), &result);
        match result {
            Ok(value) => values.push(value),
            Err(_) => all_ok = false,
        }
    }
    let stable = all_ok && values.windows(2).all(|pair| pair[0] == pair[1]);
    hw!(hw, "{label}.rest_stable={stable}");
    stable
}

/// Prints each axis of a halt report separately, never collapsed.
fn print_halt_report(hw: &Hw, report: &HaltReport) {
    for (axis, outcome) in [
        ("pan_tilt", &report.pan_tilt),
        ("zoom", &report.zoom),
        ("focus", &report.focus),
    ] {
        hw!(hw, "halt.{axis}={outcome:?}");
        if let HaltOutcome::Failed(error) = outcome {
            observe_error(hw, &format!("halt.{axis}.failure"), error);
        }
    }
}

/// A generous sanity bound for the halt call: the profile's movement timeout
/// plus slack. The halt's own deadline is an internal function of the profile
/// timeouts; exceeding this bound means the call is hung, not merely slow.
fn halt_bound(camera: &Camera<PtzOpticsG2>) -> Duration {
    camera.profile().command_timeouts().movement_timeout() + Duration::from_secs(5)
}

// ---------------------------------------------------------------------------
// Blocking safety helpers
// ---------------------------------------------------------------------------

/// Sends pan/tilt STOP and zoom STOP, waits for each to be applied, then
/// recalls preset 1 unless the test already attempted that recall. Every step
/// is best effort: `Drop` cannot report and may run while unwinding.
struct MotionGuard<'a> {
    camera: &'a Camera<PtzOpticsG2>,
    hw: Hw,
    recall_attempted: &'a Cell<bool>,
}

impl Drop for MotionGuard<'_> {
    fn drop(&mut self) {
        let hw = &self.hw;
        hw!(hw, "guard.begin");
        // Submit both STOPs before waiting on either so a slow pan/tilt STOP
        // cannot delay the zoom STOP.
        let pan_tilt = self.camera.pan_tilt().stop();
        let zoom = self.camera.zoom().stop();
        for (label, submitted) in [("guard.pan_tilt_stop", pan_tilt), ("guard.zoom_stop", zoom)] {
            match submitted {
                Ok(mut operation) => {
                    let result = operation.applied_with_timeout(HANDLE_WAIT);
                    observe(hw, label, &result);
                }
                Err(error) => observe_error(hw, &format!("{label}.submit"), &error),
            }
        }
        if !self.recall_attempted.replace(true) {
            restore_preset_one(self.camera, hw, "guard.recall_preset1");
        }
        hw!(hw, "guard.end");
    }
}

/// Recalls preset 1 and waits for its settlement. Never retried.
fn restore_preset_one(camera: &Camera<PtzOpticsG2>, hw: &Hw, label: &str) {
    let result = PresetNumber::new(1)
        .and_then(|preset| camera.presets().recall(preset))
        .and_then(|mut operation| operation.settled());
    observe(hw, &format!("{label}.settled"), &result);
}

/// Reads pan/tilt and zoom positions and prints the `RESTORED` line.
fn report_restored(camera: &Camera<PtzOpticsG2>, hw: &Hw) {
    let pan_tilt = camera.pan_tilt().position();
    let zoom = camera.zoom().position();
    observe(hw, "restored.pan_tilt", &pan_tilt);
    observe(hw, "restored.zoom", &zoom);
    let (pan, tilt) = pan_tilt.map_or_else(
        |_| ("ERR".to_owned(), "ERR".to_owned()),
        |position| (position.pan.to_string(), position.tilt.to_string()),
    );
    let zoom = zoom.map_or_else(
        |_| "ERR".to_owned(),
        |position| position.value().to_string(),
    );
    hw!(hw, "RESTORED pan={pan} tilt={tilt} zoom={zoom}");
}

/// Explicit end-of-test restore: recall preset 1 (marking it attempted so the
/// guard does not repeat it), then print the restored positions.
fn restore_and_report(camera: &Camera<PtzOpticsG2>, hw: &Hw, attempted: &Cell<bool>) {
    attempted.set(true);
    restore_preset_one(camera, hw, "restore.preset1");
    report_restored(camera, hw);
}

fn close_session(hw: &Hw, session: grafton_visca::blocking::CameraSession<PtzOpticsG2>) {
    let result = session.close();
    observe(hw, "session.close", &result);
}

// ---------------------------------------------------------------------------
// HW-01: drive and stop
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn hw01_udp_zoom_drive_and_stop() {
    let hw = Hw::new();
    let Some(ip) = motion_gate(&hw, "hw01_udp_zoom_drive_and_stop") else {
        return;
    };
    let address = format!("{ip}:{UDP_PORT}");
    hw!(hw, "connect.udp={address}");
    let session = Connect::open_udp::<PtzOpticsG2>(address).expect("open UDP session");
    let outcome = zoom_drive_and_stop(session.camera(), &hw);
    close_session(&hw, session);
    outcome.expect("zoom drive and stop").assert_all();
}

fn zoom_drive_and_stop(camera: &Camera<PtzOpticsG2>, hw: &Hw) -> Result<Checks, Error> {
    let mut checks = Checks::default();
    let recall_attempted = Cell::new(false);

    let before = camera.zoom().position();
    observe(hw, "zoom.before", &before);
    before?;

    let _guard = MotionGuard {
        camera,
        hw: *hw,
        recall_attempted: &recall_attempted,
    };

    let drive_started = Instant::now();
    let mut drive = camera.zoom().tele()?;
    hw!(hw, "zoom.tele.submitted id={:?}", drive.id());
    sleep(DRIVE);
    let mut stop = camera.zoom().stop()?;
    hw!(
        hw,
        "zoom.drive_wall_ms_before_stop_submitted={}",
        drive_started.elapsed().as_millis()
    );
    let stop_result = stop.applied();
    observe(hw, "zoom.stop.applied", &stop_result);
    let drive_result = drive.applied_with_timeout(HANDLE_WAIT);
    observe(hw, "zoom.tele.applied", &drive_result);
    stop_result?;

    let idle = camera
        .motion()
        .wait_until_idle(IdleWait::new(AffectedAxes::ZOOM, Duration::from_secs(2)));
    observe(hw, "zoom.wait_until_idle", &idle);

    let stable = sample_rest(hw, "zoom", REST_SAMPLES, REST_INTERVAL, || {
        camera.zoom().position()
    });

    restore_and_report(camera, hw, &recall_attempted);
    checks.check(hw, "zoom_stop_applied", true);
    checks.check(hw, "zoom_rest_stable", stable);
    Ok(checks)
}

#[test]
#[ignore]
fn hw01_tcp_pan_tilt_drive_and_stop() {
    let hw = Hw::new();
    let Some(ip) = motion_gate(&hw, "hw01_tcp_pan_tilt_drive_and_stop") else {
        return;
    };
    let address = format!("{ip}:{TCP_PORT}");
    hw!(hw, "connect.tcp={address}");
    let session = Connect::open_tcp::<PtzOpticsG2>(address).expect("open TCP session");
    let outcome = pan_tilt_drive_and_stop(session.camera(), &hw);
    close_session(&hw, session);
    outcome.expect("pan/tilt drive and stop").assert_all();
}

fn pan_tilt_drive_and_stop(camera: &Camera<PtzOpticsG2>, hw: &Hw) -> Result<Checks, Error> {
    let mut checks = Checks::default();
    let recall_attempted = Cell::new(false);

    let before = camera.pan_tilt().position();
    observe(hw, "pan_tilt.before", &before);
    before?;
    let pan_speed = PanSpeed::new(1)?;
    let tilt_speed = TiltSpeed::new(1)?;

    let _guard = MotionGuard {
        camera,
        hw: *hw,
        recall_attempted: &recall_attempted,
    };

    let drive_started = Instant::now();
    let mut drive =
        camera
            .pan_tilt()
            .move_direction(PanTiltDirection::Right, pan_speed, tilt_speed)?;
    hw!(hw, "pan_tilt.right.submitted id={:?}", drive.id());
    sleep(DRIVE);
    let mut stop = camera.pan_tilt().stop()?;
    hw!(
        hw,
        "pan_tilt.drive_wall_ms_before_stop_submitted={}",
        drive_started.elapsed().as_millis()
    );
    let stop_result = stop.applied();
    observe(hw, "pan_tilt.stop.applied", &stop_result);
    let drive_result = drive.applied_with_timeout(HANDLE_WAIT);
    observe(hw, "pan_tilt.right.applied", &drive_result);
    stop_result?;

    let idle = camera.motion().wait_until_idle(IdleWait::new(
        AffectedAxes::PAN_TILT,
        Duration::from_secs(2),
    ));
    observe(hw, "pan_tilt.wait_until_idle", &idle);

    let stable = sample_rest(hw, "pan_tilt", REST_SAMPLES, REST_INTERVAL, || {
        camera.pan_tilt().position()
    });

    restore_and_report(camera, hw, &recall_attempted);
    checks.check(hw, "pan_tilt_stop_applied", true);
    checks.check(hw, "pan_tilt_rest_stable", stable);
    Ok(checks)
}

// ---------------------------------------------------------------------------
// HW-04: owner halt
// ---------------------------------------------------------------------------

/// `stop_all_motion()` on an idle camera. Sends only STOPs; does not need
/// `VISCA_HW_ALLOW_MOTION`.
#[test]
#[ignore]
fn hw04_halt_idle_report() {
    let hw = Hw::new();
    let Some(ip) = camera_ip(&hw, "hw04_halt_idle_report") else {
        return;
    };
    let address = format!("{ip}:{TCP_PORT}");
    hw!(hw, "connect.tcp={address}");
    let session = Connect::open_tcp::<PtzOpticsG2>(address).expect("open TCP session");
    let camera = session.camera();

    let bound = halt_bound(camera);
    let halt_started = Instant::now();
    let report = camera.motion().stop_all_motion();
    let halt_elapsed = halt_started.elapsed();
    hw!(hw, "halt.elapsed_ms={}", halt_elapsed.as_millis());
    hw!(hw, "halt.bound_ms={}", bound.as_millis());
    match &report {
        Ok(report) => print_halt_report(&hw, report),
        Err(error) => observe_error(&hw, "halt.report", error),
    }

    // The session must still serve an inquiry afterwards. Documented raw
    // holds may delay it, so only its timing and result are recorded.
    let inquiry_started = Instant::now();
    let power = camera.power().state();
    hw!(
        hw,
        "inquiry.power.elapsed_ms={}",
        inquiry_started.elapsed().as_millis()
    );
    observe(&hw, "inquiry.power", &power);

    close_session(&hw, session);

    let mut checks = Checks::default();
    checks.check(&hw, "halt_returned_ok_report", report.is_ok());
    checks.check(&hw, "halt_within_bound", halt_elapsed <= bound);
    if let Ok(report) = &report {
        // The G2 profile declares pan/tilt and zoom STOPs.
        checks.check(
            &hw,
            "halt_pan_tilt_supported",
            !matches!(report.pan_tilt, HaltOutcome::Unsupported),
        );
        checks.check(
            &hw,
            "halt_zoom_supported",
            !matches!(report.zoom, HaltOutcome::Unsupported),
        );
    }
    checks.assert_all();
}

/// D21: unsent pre-halt queued motion is superseded; later submissions run
/// normally. Blocking `submit` returns once the owner has admitted the
/// request (not on completion), so three operations can be outstanding at
/// once on the blocking facade and no async API is needed.
#[test]
#[ignore]
fn hw04_halt_fences_queued_motion() {
    let hw = Hw::new();
    let Some(ip) = motion_gate(&hw, "hw04_halt_fences_queued_motion") else {
        return;
    };
    let address = format!("{ip}:{TCP_PORT}");
    hw!(hw, "connect.tcp={address}");
    let session = Connect::open_tcp::<PtzOpticsG2>(address).expect("open TCP session");
    let outcome = halt_fences_queued_motion(session.camera(), &hw);
    close_session(&hw, session);
    outcome.expect("halt fence test").assert_all();
}

fn classify(hw: &Hw, label: &str, result: &Result<(), Error>) {
    let class = match result {
        Ok(()) => "SENT_APPLIED",
        Err(Error::MotionSuperseded { .. }) => "SUPERSEDED",
        Err(_) => "OTHER_ERROR",
    };
    observe(hw, label, result);
    hw!(hw, "{label}.class={class}");
}

fn halt_fences_queued_motion(camera: &Camera<PtzOpticsG2>, hw: &Hw) -> Result<Checks, Error> {
    let mut checks = Checks::default();
    let recall_attempted = Cell::new(false);
    let pan_speed = PanSpeed::new(1)?;
    let tilt_speed = TiltSpeed::new(1)?;
    let bound = halt_bound(camera);

    let _guard = MotionGuard {
        camera,
        hw: *hw,
        recall_attempted: &recall_attempted,
    };

    // Start a zoom drive and queue more motion behind it without waiting.
    let drive_started = Instant::now();
    let mut tele = camera.zoom().tele()?;
    hw!(hw, "op.zoom_tele.submitted id={:?}", tele.id());
    let mut left =
        camera
            .pan_tilt()
            .move_direction(PanTiltDirection::Left, pan_speed, tilt_speed)?;
    hw!(hw, "op.pan_tilt_left.submitted id={:?}", left.id());
    let mut wide = camera.zoom().wide()?;
    hw!(hw, "op.zoom_wide.submitted id={:?}", wide.id());

    // Halt immediately.
    let halt_started = Instant::now();
    let report = camera.motion().stop_all_motion();
    let halt_elapsed = halt_started.elapsed();
    hw!(
        hw,
        "drive.wall_ms_until_halt_returned={}",
        drive_started.elapsed().as_millis()
    );
    hw!(hw, "halt.elapsed_ms={}", halt_elapsed.as_millis());
    hw!(hw, "halt.bound_ms={}", bound.as_millis());
    match &report {
        Ok(report) => print_halt_report(hw, report),
        Err(error) => observe_error(hw, "halt.report", error),
    }

    // Resolve each handle: superseded means the owner never sent it.
    let tele_result = tele.applied_with_timeout(HANDLE_WAIT);
    classify(hw, "op.zoom_tele.outcome", &tele_result);
    let left_result = left.applied_with_timeout(HANDLE_WAIT);
    classify(hw, "op.pan_tilt_left.outcome", &left_result);
    let wide_result = wide.applied_with_timeout(HANDLE_WAIT);
    classify(hw, "op.zoom_wide.outcome", &wide_result);

    // Physical rest: first wait for the protocol idle condition (recorded,
    // not asserted), then sample both axes.
    let idle = camera.motion().wait_until_idle(IdleWait::new(
        AffectedAxes::PAN_TILT.union(AffectedAxes::ZOOM),
        Duration::from_secs(3),
    ));
    observe(hw, "wait_until_idle", &idle);
    let stable = sample_rest(hw, "pan_tilt+zoom", REST_SAMPLES, REST_INTERVAL, || {
        Ok((camera.pan_tilt().position()?, camera.zoom().position()?))
    });

    // A submission after the halt runs normally.
    let fresh = camera
        .zoom()
        .stop()
        .and_then(|mut operation| operation.applied());
    observe(hw, "fresh.zoom_stop.applied", &fresh);

    restore_and_report(camera, hw, &recall_attempted);

    checks.check(hw, "halt_returned_ok_report", report.is_ok());
    checks.check(hw, "halt_within_bound", halt_elapsed <= bound);
    if let Ok(report) = &report {
        checks.check(
            hw,
            "halt_pan_tilt_supported",
            !matches!(report.pan_tilt, HaltOutcome::Unsupported),
        );
        checks.check(
            hw,
            "halt_zoom_supported",
            !matches!(report.zoom, HaltOutcome::Unsupported),
        );
    }
    checks.check(hw, "rest_stable", stable);
    checks.check(hw, "fresh_stop_after_halt_applied", fresh.is_ok());
    Ok(checks)
}

// ---------------------------------------------------------------------------
// HW-04: async G2 cancel refusal
// ---------------------------------------------------------------------------

/// Async guard. `Drop` cannot await, so it drives the same best-effort STOPs
/// and preset recall to completion on the test's multi-thread runtime from a
/// scoped helper thread. The runtime's workers keep the owner progressing
/// while this thread waits, and the whole guard is bounded by a timeout.
struct AsyncMotionGuard {
    camera: AsyncCamera<PtzOpticsG2>,
    hw: Hw,
    handle: tokio::runtime::Handle,
    recall_attempted: Arc<AtomicBool>,
}

impl Drop for AsyncMotionGuard {
    fn drop(&mut self) {
        let camera = &self.camera;
        let hw = &self.hw;
        let handle = &self.handle;
        let attempted = &self.recall_attempted;
        hw!(hw, "guard.begin");
        let joined = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    handle.block_on(async {
                        let work = async {
                            let pan_tilt = camera.pan_tilt().stop().await;
                            let zoom = camera.zoom().stop().await;
                            for (label, submitted) in
                                [("guard.pan_tilt_stop", pan_tilt), ("guard.zoom_stop", zoom)]
                            {
                                match submitted {
                                    Ok(mut operation) => {
                                        let result =
                                            operation.applied_with_timeout(HANDLE_WAIT).await;
                                        observe(hw, label, &result);
                                    }
                                    Err(error) => {
                                        observe_error(hw, &format!("{label}.submit"), &error);
                                    }
                                }
                            }
                            if !attempted.swap(true, Ordering::SeqCst) {
                                async_restore_preset_one(camera, hw, "guard.recall_preset1").await;
                            }
                        };
                        if tokio::time::timeout(Duration::from_secs(90), work)
                            .await
                            .is_err()
                        {
                            hw!(hw, "guard.timeout");
                        }
                    });
                })
                .join()
        });
        if joined.is_err() {
            hw!(hw, "guard.helper_thread_panicked");
        }
        hw!(hw, "guard.end");
    }
}

async fn async_restore_preset_one(camera: &AsyncCamera<PtzOpticsG2>, hw: &Hw, label: &str) {
    let result = match PresetNumber::new(1) {
        Ok(preset) => match camera.presets().recall(preset).await {
            Ok(mut operation) => operation.settled().await,
            Err(error) => Err(error),
        },
        Err(error) => Err(error),
    };
    observe(hw, &format!("{label}.settled"), &result);
}

async fn async_report_restored(camera: &AsyncCamera<PtzOpticsG2>, hw: &Hw) {
    let pan_tilt = camera.pan_tilt().position().await;
    let zoom = camera.zoom().position().await;
    observe(hw, "restored.pan_tilt", &pan_tilt);
    observe(hw, "restored.zoom", &zoom);
    let (pan, tilt) = pan_tilt.map_or_else(
        |_| ("ERR".to_owned(), "ERR".to_owned()),
        |position| (position.pan.to_string(), position.tilt.to_string()),
    );
    let zoom = zoom.map_or_else(
        |_| "ERR".to_owned(),
        |position| position.value().to_string(),
    );
    hw!(hw, "RESTORED pan={pan} tilt={tilt} zoom={zoom}");
}

/// Mirrors `examples/cancellation.rs::cancel_g2_drive`, but always sends the
/// zoom STOP whichever way `cancel()` resolves. Requires the multi-thread
/// runtime so the guard's helper thread can drive the owner during `Drop`.
///
/// Deviation from the example: the cancel wait is bounded to 100 ms
/// (`cancel_with_timeout`) so the drive stays within the 300 ms safety limit
/// even if the refusal were slow. A G2 refuses a post-send cancel
/// immediately with `Error::NotSupported`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn hw04_async_g2_cancel_refused_then_stop() {
    let hw = Hw::new();
    let Some(ip) = motion_gate(&hw, "hw04_async_g2_cancel_refused_then_stop") else {
        return;
    };
    let address = format!("{ip}:{TCP_PORT}");
    hw!(hw, "connect.tcp={address}");
    let runtime = TokioRuntime::from_current().expect("tokio runtime");
    let session = AsyncConnect::open_tcp::<PtzOpticsG2, _>(address, runtime)
        .await
        .expect("open async TCP session");

    let outcome = async_cancel_refused_then_stop(session.camera(), &hw).await;
    let close = session.close().await;
    observe(&hw, "session.close", &close);
    outcome.expect("async cancel test").assert_all();
}

async fn async_cancel_refused_then_stop(
    camera: &AsyncCamera<PtzOpticsG2>,
    hw: &Hw,
) -> Result<Checks, Error> {
    let mut checks = Checks::default();
    let recall_attempted = Arc::new(AtomicBool::new(false));

    let _guard = AsyncMotionGuard {
        camera: camera.clone(),
        hw: *hw,
        handle: tokio::runtime::Handle::current(),
        recall_attempted: Arc::clone(&recall_attempted),
    };

    let drive_started = Instant::now();
    let mut drive = camera.zoom().tele().await?;
    hw!(hw, "zoom.tele.submitted id={:?}", drive.id());
    tokio::time::sleep(Duration::from_millis(100)).await;

    // On a G2 a post-send cancel is refused with `Error::NotSupported` and the
    // drive keeps running; a drive that already concluded returns Ok. Either
    // way the STOP below is sent.
    let cancel = drive.cancel_with_timeout(Duration::from_millis(100)).await;
    observe(hw, "zoom.tele.cancel", &cancel);
    hw!(
        hw,
        "zoom.tele.cancel.class={}",
        match &cancel {
            Ok(_) => "CONCLUDED",
            Err(Error::NotSupported) => "REFUSED_NOT_SUPPORTED",
            Err(_) => "OTHER_ERROR",
        }
    );

    let stop = camera.zoom().stop().await;
    hw!(
        hw,
        "zoom.drive_wall_ms_before_stop_submitted={}",
        drive_started.elapsed().as_millis()
    );
    let stop_result = match stop {
        Ok(mut operation) => operation.applied().await,
        Err(error) => Err(error),
    };
    observe(hw, "zoom.stop.applied", &stop_result);

    let drive_result = drive.applied_with_timeout(HANDLE_WAIT).await;
    observe(hw, "zoom.tele.final", &drive_result);

    let idle = camera
        .motion()
        .wait_until_idle(IdleWait::new(AffectedAxes::ZOOM, Duration::from_secs(2)))
        .await;
    observe(hw, "zoom.wait_until_idle", &idle);

    recall_attempted.store(true, Ordering::SeqCst);
    async_restore_preset_one(camera, hw, "restore.preset1").await;
    async_report_restored(camera, hw).await;

    checks.check(hw, "zoom_stop_applied", stop_result.is_ok());
    Ok(checks)
}

// ---------------------------------------------------------------------------
// HW-04: recovery after a client-side disconnect
// ---------------------------------------------------------------------------

/// The operator injects a client-side fault externally between
/// `READY_FOR_FAULT` and removing the flag file. No motion is issued.
#[test]
#[ignore]
fn hw04_recover_after_disconnect() {
    const FAULT_APPEAR_TIMEOUT: Duration = Duration::from_secs(60);
    const FAULT_OBSERVE_WINDOW: Duration = Duration::from_secs(15);
    const FAULT_POLL: Duration = Duration::from_millis(500);
    const RECOVERY_WINDOW: Duration = Duration::from_secs(30);
    const RECOVERY_POLL: Duration = Duration::from_secs(1);

    let hw = Hw::new();
    let Some(ip) = camera_ip(&hw, "hw04_recover_after_disconnect") else {
        return;
    };
    let Some(flag) = env::var_os("VISCA_HW_FAULT_FLAG").map(PathBuf::from) else {
        hw!(
            hw,
            "SKIP hw04_recover_after_disconnect: VISCA_HW_FAULT_FLAG not set"
        );
        eprintln!("Skipped: VISCA_HW_FAULT_FLAG not set");
        return;
    };
    assert!(
        !flag.exists(),
        "fault flag {} already exists; remove it before starting",
        flag.display()
    );
    let address = format!("{ip}:{TCP_PORT}");
    hw!(hw, "connect.tcp={address}");

    let session = Connect::open_tcp::<PtzOpticsG2>(address.clone()).expect("open TCP session");
    let first = session.camera().power().state();
    observe(&hw, "inquiry.before_fault", &first);
    if first.is_err() {
        close_session(&hw, session);
        panic!("baseline inquiry failed before any fault was injected");
    }

    hw!(hw, "READY_FOR_FAULT flag={}", flag.display());
    let wait_started = Instant::now();
    while !flag.exists() {
        if wait_started.elapsed() >= FAULT_APPEAR_TIMEOUT {
            close_session(&hw, session);
            panic!("fault flag {} did not appear within 60 s", flag.display());
        }
        sleep(Duration::from_millis(100));
    }
    hw!(hw, "fault.flag_seen");

    // Observe the session while the fault is active.
    let observe_started = Instant::now();
    let mut trigger = "window_elapsed";
    let mut index = 0usize;
    while observe_started.elapsed() < FAULT_OBSERVE_WINDOW {
        let result = session.camera().power().state();
        observe(&hw, &format!("inquiry.during_fault[{index}]"), &result);
        index += 1;
        if let Err(error) = &result {
            if error.requires_new_session() {
                trigger = "requires_new_session";
                break;
            }
        }
        if !flag.exists() {
            trigger = "flag_removed";
            break;
        }
        sleep(FAULT_POLL);
    }
    hw!(hw, "fault.observation_ended trigger={trigger}");
    if trigger == "window_elapsed" {
        hw!(hw, "NO_SESSION_DEATH_OBSERVED");
    }

    let close = session.close();
    observe(&hw, "session.close", &close);

    // Replace the session: fresh connections until an inquiry succeeds.
    let recovery_started = Instant::now();
    let mut attempt = 0usize;
    let mut recovered = false;
    while recovery_started.elapsed() < RECOVERY_WINDOW {
        match Connect::open_tcp::<PtzOpticsG2>(address.clone()) {
            Ok(fresh) => {
                let result = fresh.camera().power().state();
                observe(&hw, &format!("recovery.inquiry[{attempt}]"), &result);
                recovered = result.is_ok();
                let close = fresh.close();
                observe(&hw, &format!("recovery.close[{attempt}]"), &close);
            }
            Err(error) => observe_error(&hw, &format!("recovery.open[{attempt}]"), &error),
        }
        attempt += 1;
        if recovered {
            hw!(hw, "RECOVERED attempts={attempt}");
            break;
        }
        sleep(RECOVERY_POLL);
    }
    assert!(recovered, "no fresh session recovered within 30 s");
}
