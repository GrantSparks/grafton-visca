#![cfg(feature = "blocking")]
//! Hardware tests for the high-risk corrected and profile-specific wire rows
//! (#715) and the PTZOptics G2 bench facts recorded in
//! `docs/camera_profile_support.md` (#795: the G2 shutter codes and G2 tilt
//! polarity), run against a real PTZOptics camera by an operator who can see
//! it.
//!
//! **WARNING: the motion tests physically move the camera and the settings
//! tests briefly change image, focus and exposure settings.** Every test is
//! ignored by default.
//!
//! Gating, in order, for every test:
//! 1. `#[ignore]`: the test runs only with `--ignored`.
//! 2. `VISCA_CAMERA_IP` must be set, otherwise the test prints a skip line and
//!    returns (passing).
//! 3. Tests that change settings additionally need `VISCA_HW_ALLOW_SETTINGS=1`;
//!    tests that can move the camera need `VISCA_HW_ALLOW_MOTION=1`. A missing
//!    flag prints a skip line and returns (passing).
//! 4. `VISCA_PROFILE` selects the compile-time profile the test body is
//!    instantiated for: `g2` (default) selects `PtzOpticsG2`, `30x` selects
//!    `PtzOptics30X`. Anything else is a hard failure before any I/O.
//!
//! Transport: raw TCP `{VISCA_CAMERA_IP}:5678` through the blocking facade.
//!
//! Tests and the wire rows they exercise (cross-checked against
//! `tests/fixtures/issue_715_wire_golden.txt`):
//!
//! | Test | Rows |
//! |---|---|
//! | `hw05_corrected_inquiries` | `81 09 04 AA FF`, `81 09 04 63 FF`, `81 09 04 53 FF`, `81 09 04 50 FF`, `81 09 04 54 FF` |
//! | `hw05_nr_level_zero_round_trip` | `81 01 04 53 00 FF`, `81 01 04 54 00 FF`, restore `04 53 0n`/`04 54 0n`, optional restore of `81 01 04 50 02/03 FF` |
//! | `hw05_focus_zone_round_trip` | `81 01 04 AA 00/01/02 FF` and `81 09 04 AA FF` |
//! | `hw05_direct_zoom_positions` | `81 01 04 47 00 08 00 00 FF` and `81 01 04 47 01 00 00 00 FF` |
//! | `hw05_absolute_pan_tilt_small_offset` | `81 01 06 02 01 01 <pan x4> <tilt x4> FF` |
//! | `hw05_shutter_round_trip` | `81 09 04 39 FF`, `81 01 04 39 0A FF` (only when the mode has no settable shutter) and its restore, `81 01 04 4A 00 00 0p 0q FF` for one step from the current code and the table minimum, middle and maximum, `81 09 04 4A FF` |
//! | `hw05_tilt_polarity` | `81 01 06 01 01 01 03 01 FF` (up), `81 01 06 01 01 01 03 02 FF` (down), `81 01 06 01 <vv> <ww> 03 03 FF` (stop), `81 09 06 12 FF` |
//!
//! Safety rules the tests keep:
//! - No preset set/reset, power, menu, settings save, limit set, IR/address,
//!   USB/NDI audio, tally, multicast/NDI quality, flip/mirror, picture-effect
//!   SET, white-balance or AF-sensitivity request is ever issued. No home,
//!   reset or relative move is issued either. The only exposure-mode change is
//!   `hw05_shutter_round_trip` selecting shutter priority, which its guard
//!   reverts.
//! - Motion tests drive only through one absolute pan/tilt target (slowest
//!   speed the API allows), absolute zoom targets, or (`hw05_tilt_polarity`
//!   only) `move_direction` up or down at pan speed 1 and tilt speed 1 with
//!   each drive stopped within about 300 ms of wall time. A drop guard sends
//!   pan/tilt STOP and zoom STOP and then recalls preset 1 (best effort) on
//!   every exit path, including a panic or an early `?`. Each motion test
//!   ends by explicitly recalling preset 1 and printing
//!   `RESTORED pan=.. tilt=.. zoom=..`. Preset 1 must therefore be a safe,
//!   previously stored position on the camera under test.
//! - Settings tests read the original value first, and a drop guard restores
//!   it on every exit path (including a panic or an early `?`) after any
//!   attempt to change it. Each prints a `RESTORED ...` line.
//! - A command that returned an `Unconfirmed` error is never retried by a test
//!   body. A restore command is attempted at most once per guard.
//!
//! Output: every observation is one greppable line on stderr of the form
//! `HW|t=<elapsed ms since test start>|<observation>`.
//!
//! Run every row with:
//! ```sh
//! VISCA_CAMERA_IP=192.0.2.10 VISCA_PROFILE=g2 \
//!   VISCA_HW_ALLOW_SETTINGS=1 VISCA_HW_ALLOW_MOTION=1 \
//!   cargo test --test hardware_wire_rows_test --features runtime-tokio \
//!   -- --ignored --nocapture --test-threads=1
//! ```
//!
//! The G2 shutter table round trip alone (settings, no motion):
//! ```sh
//! VISCA_CAMERA_IP=192.0.2.10 VISCA_PROFILE=g2 VISCA_HW_ALLOW_SETTINGS=1 \
//!   cargo test --test hardware_wire_rows_test --features runtime-tokio \
//!   -- --ignored --nocapture --test-threads=1 --exact hw05_shutter_round_trip
//! ```
//!
//! The G2 tilt polarity row alone (motion; watch the camera and confirm the
//! direction when the `OPERATOR` line asks):
//! ```sh
//! VISCA_CAMERA_IP=192.0.2.10 VISCA_PROFILE=g2 VISCA_HW_ALLOW_MOTION=1 \
//!   cargo test --test hardware_wire_rows_test --features runtime-tokio \
//!   -- --ignored --nocapture --test-threads=1 --exact hw05_tilt_polarity
//! ```

#[macro_use]
#[path = "common/hardware.rs"]
mod hardware;
#[path = "common/hardware_rest.rs"]
mod hardware_rest;

use std::{
    cell::Cell,
    env,
    thread::sleep,
    time::{Duration, Instant},
};

use grafton_visca::{
    blocking::{Camera, CameraSession, Connect},
    camera::{
        profiles::{PtzOptics30X, PtzOpticsG2},
        IdleWait, PanTiltPosition,
    },
    capabilities::{
        Capabilities, HasDirectZoom, HasExposure, HasExposureMode, HasFocus, HasFocusZone,
        HasFocusZoneInquiry, HasImageProcessing, HasIrisControl, HasNoiseReduction2D,
        HasNoiseReduction2DControl, HasNoiseReduction2DMode, HasNoiseReduction3D,
        HasNoiseReduction3DControl, HasPanTilt, HasPictureEffect, HasPresets, HasZoom, PanTilt,
        ShutterSpeedEntry, SupportsTcp,
    },
    command::{
        ExposureCommand, ExposureMode, FocusZone, NoiseReduction2DMode, PanTiltDirection, Shutter,
        ShutterInquiry,
    },
    raw,
    types::{
        NoiseReduction2DLevel, NoiseReduction3DLevel, PanSpeed, ShutterSpeed, SpeedLevel,
        TiltSpeed, ZoomPosition,
    },
    units::{Degrees, UnitInterval},
    AffectedAxes, CameraId, CompileTimeProfile, ControlClass, Error, InquiryRoute,
    PanTiltCoordinateConversion, Request, RetryClass, TimeoutClass, ZoomDomain,
};

use hardware::{
    close_session, observe, opted_in, report_restored, restore_and_report, Checks, Hw, HwLog,
    MotionGuard, HANDLE_WAIT,
};
use hardware_rest::{sample_rest, DRIVE, REST_INTERVAL, REST_SAMPLES};

const TCP_PORT: u16 = 5678;
/// Delay between a settings write and its readback.
const SETTLE_DELAY: Duration = Duration::from_millis(500);
/// Raw pan units added to the current pan position by the absolute-move test.
const PAN_OFFSET_RAW: i32 = 20;
/// Raw direct-zoom target used by the absolute-zoom test.
const RAW_ZOOM_TARGET: u16 = 0x0800;
/// Normalized optical zoom target used by the absolute-zoom test.
const NORMALIZED_ZOOM_TARGET: f32 = 0.25;
/// Pause after the `OBSERVE` line so the operator can watch the tilt drive.
const OBSERVE_PAUSE: Duration = Duration::from_secs(2);
/// Largest raw pan change accepted across a tilt-only drive (encoder jitter).
const PAN_TOLERANCE_RAW: i32 = 2;
/// Bound for the protocol idle wait after each tilt STOP.
const IDLE_WAIT: Duration = Duration::from_secs(2);

// ---------------------------------------------------------------------------
// Gating helpers
// ---------------------------------------------------------------------------

/// Magnification of a raw zoom position, when the profile fixes its lens.
fn magnification(capabilities: &Capabilities, units: u16) -> Option<f32> {
    capabilities
        .zoom_scale()
        .ok()
        .and_then(|scale| scale.magnification(units))
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

/// Gate for tests that need the camera IP and one extra `=1` opt-in flag.
fn flag_gate(hw: &Hw, test: &str, flag: &str) -> Option<String> {
    let ip = camera_ip(hw, test)?;
    opted_in(hw, test, flag).then_some(ip)
}

/// Gate for tests that may move the camera.
fn motion_gate(hw: &Hw, test: &str) -> Option<String> {
    flag_gate(hw, test, "VISCA_HW_ALLOW_MOTION")
}

/// Gate for tests that change camera settings (and restore them).
fn settings_gate(hw: &Hw, test: &str) -> Option<String> {
    flag_gate(hw, test, "VISCA_HW_ALLOW_SETTINGS")
}

// ---------------------------------------------------------------------------
// Profile selection
// ---------------------------------------------------------------------------

/// The profile the test bodies are instantiated for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Selected {
    G2,
    X30,
}

/// Reads `VISCA_PROFILE` (`g2` default, or `30x`). An unknown value is a hard
/// failure before any camera I/O.
fn selected_profile(hw: &Hw) -> Selected {
    let raw = env::var("VISCA_PROFILE").unwrap_or_default();
    let selected = match raw.trim().to_ascii_lowercase().as_str() {
        "" | "g2" => Selected::G2,
        "30x" => Selected::X30,
        other => panic!("VISCA_PROFILE must be `g2` or `30x`, got `{other}`"),
    };
    hw!(hw, "profile.selected={selected:?} VISCA_PROFILE={raw:?}");
    selected
}

/// Everything the wire-row tests need from a profile. The blanket impl means
/// only profiles that statically declare every row can be selected.
trait WireRowProfile:
    CompileTimeProfile
    + SupportsTcp
    + HasPanTilt
    + HasZoom
    + HasFocus
    + HasPresets
    + HasImageProcessing
    + HasExposure
    + HasExposureMode
    + HasIrisControl
    + HasDirectZoom
    + HasFocusZone
    + HasFocusZoneInquiry
    + HasNoiseReduction2D
    + HasNoiseReduction2DControl
    + HasNoiseReduction2DMode
    + HasNoiseReduction3D
    + HasNoiseReduction3DControl
    + HasPictureEffect
{
}

impl<P> WireRowProfile for P where
    P: CompileTimeProfile
        + SupportsTcp
        + HasPanTilt
        + HasZoom
        + HasFocus
        + HasPresets
        + HasImageProcessing
        + HasExposure
        + HasExposureMode
        + HasIrisControl
        + HasDirectZoom
        + HasFocusZone
        + HasFocusZoneInquiry
        + HasNoiseReduction2D
        + HasNoiseReduction2DControl
        + HasNoiseReduction2DMode
        + HasNoiseReduction3D
        + HasNoiseReduction3DControl
        + HasPictureEffect
{
}

/// Calls the generic runner `$run::<P>($hw, $ip)` for the selected profile.
macro_rules! run_for_profile {
    ($selected:expr, $run:ident, $hw:expr, $ip:expr) => {
        match $selected {
            Selected::G2 => $run::<PtzOpticsG2>($hw, $ip),
            Selected::X30 => $run::<PtzOptics30X>($hw, $ip),
        }
    };
}

/// Prints the chosen profile and its capability model name, then opens a raw
/// TCP session.
fn open_session<P: WireRowProfile>(hw: &Hw, ip: &str) -> CameraSession<P> {
    let capabilities = Capabilities::from_profile::<P>();
    hw!(
        hw,
        "profile.type={} model_name={:?}",
        std::any::type_name::<P>(),
        capabilities.model_name
    );
    let address = format!("{ip}:{TCP_PORT}");
    hw!(hw, "connect.tcp={address}");
    Connect::open_tcp::<P>(address).expect("open TCP session")
}

// ---------------------------------------------------------------------------
// HW-05.1: corrected inquiries (no motion, no settings change)
// ---------------------------------------------------------------------------

/// Reads the five corrected inquiries and asserts each returns `Ok`:
/// `focus().zone()` -> `81 09 04 AA FF`,
/// `image().picture_effect()` -> `81 09 04 63 FF`,
/// `image().noise_reduction_2d()` -> `81 09 04 53 FF`,
/// `image().noise_reduction_2d_mode()` -> `81 09 04 50 FF`,
/// `image().noise_reduction_3d()` -> `81 09 04 54 FF`.
#[test]
#[ignore]
fn hw05_corrected_inquiries() {
    let hw = Hw::new();
    let Some(ip) = camera_ip(&hw, "hw05_corrected_inquiries") else {
        return;
    };
    let selected = selected_profile(&hw);
    run_for_profile!(selected, corrected_inquiries, &hw, &ip);
}

fn corrected_inquiries<P: WireRowProfile>(hw: &Hw, ip: &str) {
    let session = open_session::<P>(hw, ip);
    let camera = session.camera();

    let zone = camera.focus().zone();
    observe(hw, "inquiry.focus_zone", &zone);
    let effect = camera.image().picture_effect();
    observe(hw, "inquiry.picture_effect", &effect);
    let nr2d = camera.image().noise_reduction_2d();
    observe(hw, "inquiry.nr2d_level", &nr2d);
    let nr2d_mode = camera.image().noise_reduction_2d_mode();
    observe(hw, "inquiry.nr2d_mode", &nr2d_mode);
    let nr3d = camera.image().noise_reduction_3d();
    observe(hw, "inquiry.nr3d_level", &nr3d);

    close_session(hw, session);

    let mut checks = Checks::default();
    checks.check(hw, "focus_zone_inquiry_ok", zone.is_ok());
    checks.check(hw, "picture_effect_inquiry_ok", effect.is_ok());
    checks.check(hw, "nr2d_level_inquiry_ok", nr2d.is_ok());
    checks.check(hw, "nr2d_mode_inquiry_ok", nr2d_mode.is_ok());
    checks.check(hw, "nr3d_level_inquiry_ok", nr3d.is_ok());
    checks.assert_all();
}

// ---------------------------------------------------------------------------
// HW-05.2: NR level zero round trip (settings)
// ---------------------------------------------------------------------------

fn level2(result: &Result<NoiseReduction2DLevel, Error>) -> String {
    result
        .as_ref()
        .map_or_else(|_| "ERR".to_owned(), |level| level.value().to_string())
}

fn level3(result: &Result<NoiseReduction3DLevel, Error>) -> String {
    result
        .as_ref()
        .map_or_else(|_| "ERR".to_owned(), |level| level.value().to_string())
}

fn mode2(result: &Result<NoiseReduction2DMode, Error>) -> String {
    result
        .as_ref()
        .map_or_else(|_| "ERR".to_owned(), |mode| format!("{mode:?}"))
}

/// Restores the original 2D/3D noise-reduction values. The originals are
/// captured before any change; a dimension is marked dirty before its first
/// set attempt, so `Drop` restores it on every exit path, including a panic or
/// an early `?`. Each restore is attempted at most once.
struct NrGuard<'a, P: WireRowProfile> {
    camera: &'a Camera<P>,
    hw: Hw,
    orig_2d: Option<NoiseReduction2DLevel>,
    orig_mode: Option<NoiseReduction2DMode>,
    orig_3d: Option<NoiseReduction3DLevel>,
    dirty_2d: Cell<bool>,
    dirty_3d: Cell<bool>,
}

impl<P: WireRowProfile> NrGuard<'_, P> {
    /// Restores the 2D level (`81 01 04 53 0n FF`) and, only when the mode
    /// reading no longer equals the original, the 2D mode (`81 01 04 50 02/03
    /// FF`). The mode is restored last because the level setter is the only
    /// thing the test used to change anything.
    fn restore_2d(&self) {
        if !self.dirty_2d.replace(false) {
            return;
        }
        let hw = &self.hw;
        if let Some(level) = self.orig_2d {
            let result = self.camera.image().set_noise_reduction_2d(level);
            observe(hw, "restore.nr2d.set", &result);
            sleep(SETTLE_DELAY);
        }
        if let Some(original) = self.orig_mode {
            let current = self.camera.image().noise_reduction_2d_mode();
            observe(hw, "restore.nr2d_mode.read", &current);
            if !matches!(&current, Ok(mode) if *mode == original) {
                let result = self.camera.image().set_noise_reduction_2d_mode(original);
                observe(hw, "restore.nr2d_mode.set", &result);
                sleep(SETTLE_DELAY);
            }
        }
    }

    /// Restores the 3D level (`81 01 04 54 0n FF`).
    fn restore_3d(&self) {
        if !self.dirty_3d.replace(false) {
            return;
        }
        let hw = &self.hw;
        if let Some(level) = self.orig_3d {
            let result = self.camera.image().set_noise_reduction_3d(level);
            observe(hw, "restore.nr3d.set", &result);
            sleep(SETTLE_DELAY);
        }
    }

    /// Reads all three values back and prints the `RESTORED` line.
    #[allow(clippy::type_complexity)]
    fn report_restored(
        &self,
    ) -> (
        Result<NoiseReduction2DLevel, Error>,
        Result<NoiseReduction2DMode, Error>,
        Result<NoiseReduction3DLevel, Error>,
    ) {
        let hw = &self.hw;
        let nr2d = self.camera.image().noise_reduction_2d();
        let nr2d_mode = self.camera.image().noise_reduction_2d_mode();
        let nr3d = self.camera.image().noise_reduction_3d();
        observe(hw, "restored.nr2d", &nr2d);
        observe(hw, "restored.nr2d_mode", &nr2d_mode);
        observe(hw, "restored.nr3d", &nr3d);
        hw!(
            hw,
            "RESTORED nr2d={} nr2d_mode={} nr3d={}",
            level2(&nr2d),
            mode2(&nr2d_mode),
            level3(&nr3d)
        );
        (nr2d, nr2d_mode, nr3d)
    }
}

impl<P: WireRowProfile> Drop for NrGuard<'_, P> {
    fn drop(&mut self) {
        let any = self.dirty_2d.get() || self.dirty_3d.get();
        if any {
            hw!(self.hw, "guard.begin nr");
        }
        self.restore_2d();
        self.restore_3d();
        if any {
            let _ = self.report_restored();
            hw!(self.hw, "guard.end nr");
        }
    }
}

/// Sets 2D NR level 0 (`81 01 04 53 00 FF`) then 3D NR level 0
/// (`81 01 04 54 00 FF`) through the typed setters, records the readbacks,
/// restores the originals and asserts the restored values equal them.
#[test]
#[ignore]
fn hw05_nr_level_zero_round_trip() {
    let hw = Hw::new();
    let Some(ip) = settings_gate(&hw, "hw05_nr_level_zero_round_trip") else {
        return;
    };
    let selected = selected_profile(&hw);
    run_for_profile!(selected, nr_level_zero_round_trip, &hw, &ip);
}

fn nr_level_zero_round_trip<P: WireRowProfile>(hw: &Hw, ip: &str) {
    let session = open_session::<P>(hw, ip);
    let outcome = nr_zero_body(session.camera(), hw);
    close_session(hw, session);
    outcome.expect("NR level zero round trip").assert_all();
}

fn nr_zero_body<P: WireRowProfile>(camera: &Camera<P>, hw: &Hw) -> Result<Checks, Error> {
    let mut checks = Checks::default();

    let orig_2d = camera.image().noise_reduction_2d();
    observe(hw, "original.nr2d", &orig_2d);
    let orig_mode = camera.image().noise_reduction_2d_mode();
    observe(hw, "original.nr2d_mode", &orig_mode);
    let orig_3d = camera.image().noise_reduction_3d();
    observe(hw, "original.nr3d", &orig_3d);
    checks.check(hw, "original_nr2d_readable", orig_2d.is_ok());
    checks.check(hw, "original_nr2d_mode_readable", orig_mode.is_ok());
    checks.check(hw, "original_nr3d_readable", orig_3d.is_ok());

    let guard = NrGuard {
        camera,
        hw: *hw,
        orig_2d: orig_2d.as_ref().ok().copied(),
        orig_mode: orig_mode.as_ref().ok().copied(),
        orig_3d: orig_3d.as_ref().ok().copied(),
        dirty_2d: Cell::new(false),
        dirty_3d: Cell::new(false),
    };

    // 2D first. The mode is never changed by the test body itself; whether the
    // level setter changes it is recorded, not asserted.
    if guard.orig_2d.is_some() {
        let zero = NoiseReduction2DLevel::new(0)?;
        guard.dirty_2d.set(true);
        let set = camera.image().set_noise_reduction_2d(zero);
        observe(hw, "nr2d.set_level_zero", &set);
        sleep(SETTLE_DELAY);
        let level = camera.image().noise_reduction_2d();
        observe(hw, "nr2d.after_zero.level", &level);
        let mode = camera.image().noise_reduction_2d_mode();
        observe(hw, "nr2d.after_zero.mode", &mode);
        hw!(
            hw,
            "observation.nr2d_readback_equals_zero={}",
            matches!(&level, Ok(value) if value.value() == 0)
        );
        hw!(
            hw,
            "observation.nr2d_mode_before={} after={} changed_by_level_set={}",
            mode2(&orig_mode),
            mode2(&mode),
            orig_mode.as_ref().ok() != mode.as_ref().ok()
        );
        guard.restore_2d();
    }

    // Then 3D.
    if guard.orig_3d.is_some() {
        let zero = NoiseReduction3DLevel::new(0)?;
        guard.dirty_3d.set(true);
        let set = camera.image().set_noise_reduction_3d(zero);
        observe(hw, "nr3d.set_level_zero", &set);
        sleep(SETTLE_DELAY);
        let level = camera.image().noise_reduction_3d();
        observe(hw, "nr3d.after_zero.level", &level);
        hw!(
            hw,
            "observation.nr3d_readback_equals_zero={}",
            matches!(&level, Ok(value) if value.value() == 0)
        );
        guard.restore_3d();
    }

    let (nr2d, nr2d_mode, nr3d) = guard.report_restored();
    checks.check(
        hw,
        "restored_nr2d_equals_original",
        nr2d.as_ref().ok() == orig_2d.as_ref().ok() && nr2d.is_ok(),
    );
    checks.check(
        hw,
        "restored_nr2d_mode_equals_original",
        nr2d_mode.as_ref().ok() == orig_mode.as_ref().ok() && nr2d_mode.is_ok(),
    );
    checks.check(
        hw,
        "restored_nr3d_equals_original",
        nr3d.as_ref().ok() == orig_3d.as_ref().ok() && nr3d.is_ok(),
    );
    Ok(checks)
}

// ---------------------------------------------------------------------------
// HW-05.3: focus zone round trip (settings)
// ---------------------------------------------------------------------------

/// Restores the original focus zone. Marked dirty before the first set
/// attempt, so `Drop` restores it on every exit path. Attempted at most once.
struct FocusZoneGuard<'a, P: WireRowProfile> {
    camera: &'a Camera<P>,
    hw: Hw,
    original: Option<FocusZone>,
    dirty: Cell<bool>,
}

impl<P: WireRowProfile> FocusZoneGuard<'_, P> {
    /// Restores the zone (`81 01 04 AA 00/01/02 FF`).
    fn restore(&self) {
        if !self.dirty.replace(false) {
            return;
        }
        if let Some(zone) = self.original {
            let result = self.camera.focus().set_zone(zone);
            observe(&self.hw, "restore.focus_zone.set", &result);
            sleep(SETTLE_DELAY);
        }
    }

    /// Reads the zone back (`81 09 04 AA FF`) and prints the `RESTORED` line.
    fn report_restored(&self) -> Result<FocusZone, Error> {
        let zone = self.camera.focus().zone();
        observe(&self.hw, "restored.focus_zone", &zone);
        hw!(
            self.hw,
            "RESTORED focus_zone={}",
            zone.as_ref()
                .map_or_else(|_| "ERR".to_owned(), |zone| format!("{zone:?}"))
        );
        zone
    }
}

impl<P: WireRowProfile> Drop for FocusZoneGuard<'_, P> {
    fn drop(&mut self) {
        if self.dirty.get() {
            hw!(self.hw, "guard.begin focus_zone");
            self.restore();
            let _ = self.report_restored();
            hw!(self.hw, "guard.end focus_zone");
        }
    }
}

/// Reads the zone (`81 09 04 AA FF`), sets a different one
/// (`81 01 04 AA 00/02 FF`), reads it back, restores the original and asserts
/// the restored zone equals it.
#[test]
#[ignore]
fn hw05_focus_zone_round_trip() {
    let hw = Hw::new();
    let Some(ip) = settings_gate(&hw, "hw05_focus_zone_round_trip") else {
        return;
    };
    let selected = selected_profile(&hw);
    run_for_profile!(selected, focus_zone_round_trip, &hw, &ip);
}

fn focus_zone_round_trip<P: WireRowProfile>(hw: &Hw, ip: &str) {
    let session = open_session::<P>(hw, ip);
    let outcome = focus_zone_body(session.camera(), hw);
    close_session(hw, session);
    outcome.expect("focus zone round trip").assert_all();
}

fn focus_zone_body<P: WireRowProfile>(camera: &Camera<P>, hw: &Hw) -> Result<Checks, Error> {
    let mut checks = Checks::default();

    let original = camera.focus().zone();
    observe(hw, "original.focus_zone", &original);
    checks.check(hw, "original_focus_zone_readable", original.is_ok());

    let guard = FocusZoneGuard {
        camera,
        hw: *hw,
        original: original.as_ref().ok().copied(),
        dirty: Cell::new(false),
    };

    if let Some(original) = guard.original {
        let different = if original == FocusZone::Top {
            FocusZone::Center
        } else {
            FocusZone::Top
        };
        hw!(hw, "focus_zone.target={different:?}");
        guard.dirty.set(true);
        let set = camera.focus().set_zone(different);
        observe(hw, "focus_zone.set", &set);
        sleep(SETTLE_DELAY);
        let readback = camera.focus().zone();
        observe(hw, "focus_zone.after_set", &readback);
        hw!(
            hw,
            "observation.focus_zone_readback_equals_target={}",
            matches!(&readback, Ok(zone) if *zone == different)
        );
        guard.restore();
    }

    let restored = guard.report_restored();
    checks.check(
        hw,
        "restored_focus_zone_equals_original",
        restored.is_ok() && restored.as_ref().ok() == original.as_ref().ok(),
    );
    Ok(checks)
}

// ---------------------------------------------------------------------------
// HW-05.4: direct zoom positions (motion)
// ---------------------------------------------------------------------------

/// Direct zoom to raw 0x0800 (`81 01 04 47 00 08 00 00 FF`), then to the
/// normalized optical 0.25 target (`81 01 04 47 0p 0q 0r 0s FF`, `0x1000` when
/// the profile's optical maximum is `0x4000`), recording the read-back
/// positions. `settled()` documents observed stability, not arrival, so no
/// arrival tolerance is asserted.
#[test]
#[ignore]
fn hw05_direct_zoom_positions() {
    let hw = Hw::new();
    let Some(ip) = motion_gate(&hw, "hw05_direct_zoom_positions") else {
        return;
    };
    let selected = selected_profile(&hw);
    run_for_profile!(selected, direct_zoom_positions, &hw, &ip);
}

fn direct_zoom_positions<P: WireRowProfile>(hw: &Hw, ip: &str) {
    let session = open_session::<P>(hw, ip);
    let outcome = direct_zoom_body(session.camera(), hw);
    close_session(hw, session);
    outcome.expect("direct zoom positions").assert_all();
}

fn direct_zoom_body<P: WireRowProfile>(camera: &Camera<P>, hw: &Hw) -> Result<Checks, Error> {
    let mut checks = Checks::default();
    let recall_attempted = Cell::new(false);

    let before = camera.zoom().position();
    observe(hw, "zoom.before", &before);
    let before = before?;

    let capabilities = camera.capabilities();
    let optical_max = *capabilities.zoom_range_optical.end();
    hw!(
        hw,
        "profile.zoom model_name={:?} optical_range=0x{:04X}..=0x{:04X} digital_range={:?} \
         optical_zoom_ratio={:?} supports_direct={}",
        capabilities.model_name,
        capabilities.zoom_range_optical.start(),
        optical_max,
        capabilities.zoom_range_digital,
        capabilities.optical_zoom_ratio,
        capabilities.supports_direct_zoom
    );
    hw!(
        hw,
        "zoom.before.magnification={:?}",
        magnification(capabilities, before.value())
    );

    let _guard = MotionGuard {
        camera,
        hw: *hw,
        recall_attempted: &recall_attempted,
    };

    // Raw target.
    let raw_target = ZoomPosition::new(RAW_ZOOM_TARGET)?;
    let mut raw_op = camera.zoom().set_position(raw_target)?;
    hw!(
        hw,
        "zoom.raw_target=0x{RAW_ZOOM_TARGET:04X} id={:?}",
        raw_op.id()
    );
    let raw_settled = raw_op.settled();
    observe(hw, "zoom.raw.settled", &raw_settled);
    let raw_after = camera.zoom().position();
    observe(hw, "zoom.raw.readback", &raw_after);
    if let Ok(position) = &raw_after {
        let delta = i32::from(position.value()) - i32::from(RAW_ZOOM_TARGET);
        hw!(
            hw,
            "observation.zoom.raw_readback=0x{:04X} target=0x{RAW_ZOOM_TARGET:04X} delta={delta} \
             magnification={:?}",
            position.value(),
            magnification(capabilities, position.value())
        );
    }

    // Normalized optical target.
    let normalized = UnitInterval::new(NORMALIZED_ZOOM_TARGET)?;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let expected_raw = (f64::from(NORMALIZED_ZOOM_TARGET) * f64::from(optical_max)).round() as u16;
    let mut normalized_op = camera
        .zoom()
        .set_normalized(normalized, ZoomDomain::Optical)?;
    hw!(
        hw,
        "zoom.normalized_target={NORMALIZED_ZOOM_TARGET} domain=Optical \
         expected_raw_from_optical_max=0x{expected_raw:04X} id={:?}",
        normalized_op.id()
    );
    let normalized_settled = normalized_op.settled();
    observe(hw, "zoom.normalized.settled", &normalized_settled);
    let normalized_after = camera.zoom().position();
    observe(hw, "zoom.normalized.readback", &normalized_after);
    if let Ok(position) = &normalized_after {
        let delta = i32::from(position.value()) - i32::from(expected_raw);
        hw!(
            hw,
            "observation.zoom.normalized_readback=0x{:04X} expected_raw=0x{expected_raw:04X} \
             delta={delta} magnification={:?} profile_optical_max=0x{optical_max:04X} \
             profile_optical_zoom_ratio={:?} model_name={:?}",
            position.value(),
            magnification(capabilities, position.value()),
            capabilities.optical_zoom_ratio,
            capabilities.model_name
        );
    }

    restore_and_report(camera, hw, &recall_attempted);

    checks.check(hw, "zoom_before_readable", true);
    checks.check(hw, "raw_zoom_settled_ok", raw_settled.is_ok());
    checks.check(hw, "raw_zoom_readback_ok", raw_after.is_ok());
    checks.check(hw, "normalized_zoom_settled_ok", normalized_settled.is_ok());
    checks.check(hw, "normalized_zoom_readback_ok", normalized_after.is_ok());
    Ok(checks)
}

// ---------------------------------------------------------------------------
// HW-05.5: absolute pan/tilt small offset (motion)
// ---------------------------------------------------------------------------

/// Four-nibble wire spelling of a signed raw position (`0000`..`FFFF` as one
/// nibble per byte).
fn nibbles(raw: i32) -> String {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let word = (raw as i16) as u16;
    format!(
        "{:02X} {:02X} {:02X} {:02X}",
        (word >> 12) & 0xF,
        (word >> 8) & 0xF,
        (word >> 4) & 0xF,
        word & 0xF
    )
}

/// One absolute pan/tilt move: pan = current pan + 20 raw units, tilt =
/// current tilt, at `SpeedLevel::Slowest` (pan speed 1, tilt speed 1 on both
/// PTZOptics profiles). Expected wire:
/// `81 01 06 02 01 01 <pan nibbles> <tilt nibbles> FF`.
#[test]
#[ignore]
fn hw05_absolute_pan_tilt_small_offset() {
    let hw = Hw::new();
    let Some(ip) = motion_gate(&hw, "hw05_absolute_pan_tilt_small_offset") else {
        return;
    };
    let selected = selected_profile(&hw);
    run_for_profile!(selected, absolute_pan_tilt_small_offset, &hw, &ip);
}

fn absolute_pan_tilt_small_offset<P: WireRowProfile>(hw: &Hw, ip: &str) {
    let session = open_session::<P>(hw, ip);
    let outcome = absolute_pan_tilt_body(session.camera(), hw);
    close_session(hw, session);
    outcome.expect("absolute pan/tilt").assert_all();
}

fn absolute_pan_tilt_body<P: WireRowProfile>(camera: &Camera<P>, hw: &Hw) -> Result<Checks, Error> {
    let mut checks = Checks::default();
    let recall_attempted = Cell::new(false);

    let p0 = camera.pan_tilt().position();
    observe(hw, "pan_tilt.p0", &p0);
    let p0 = p0?;

    let capabilities = camera.capabilities();
    let pan_units = <P as PanTilt>::PAN_DEGREES_TO_UNITS;
    let tilt_units = <P as PanTilt>::TILT_DEGREES_TO_UNITS;
    let slowest = SpeedLevel::Slowest;
    hw!(
        hw,
        "profile.pan_tilt pan_range={:?} tilt_range={:?} pan_speed_range={:?} \
         tilt_speed_range={:?} pan_degrees_to_units={pan_units} tilt_degrees_to_units={tilt_units}",
        capabilities.pan_range,
        capabilities.tilt_range,
        capabilities.pan_speed,
        capabilities.tilt_speed
    );

    let target_pan = p0.pan + PAN_OFFSET_RAW;
    let target_tilt = p0.tilt;
    if !capabilities.pan_range.contains(&target_pan) {
        // The typed API would reject this target; never substitute another
        // direction or offset silently.
        hw!(
            hw,
            "SKIP_RANGE pan target {target_pan} is outside {:?}; no motion issued",
            capabilities.pan_range
        );
        report_restored(camera, hw);
        checks.check(hw, "pan_target_in_range", false);
        return Ok(checks);
    }

    let conversion = PanTiltCoordinateConversion::for_profile::<P>();
    let pan_degrees = conversion.pan_degrees(target_pan);
    let tilt_degrees = conversion.tilt_degrees(target_tilt);
    let pan_speed = slowest_speed(&capabilities.pan_speed, slowest.to_pan_speed());
    let tilt_speed = slowest_speed(&capabilities.tilt_speed, slowest.to_tilt_speed());
    hw!(
        hw,
        "pan_tilt.target raw=({target_pan},{target_tilt}) degrees=({pan_degrees},{tilt_degrees}) \
         speed_level=Slowest pan_speed={pan_speed} tilt_speed={tilt_speed}"
    );
    hw!(
        hw,
        "pan_tilt.expected_wire=81 01 06 02 {pan_speed:02X} {tilt_speed:02X} {} {} FF",
        nibbles(target_pan),
        nibbles(target_tilt)
    );

    let _guard = MotionGuard {
        camera,
        hw: *hw,
        recall_attempted: &recall_attempted,
    };

    let mut operation =
        camera
            .pan_tilt()
            .absolute(Degrees(pan_degrees), Degrees(tilt_degrees), slowest)?;
    hw!(hw, "pan_tilt.absolute.submitted id={:?}", operation.id());
    let settled = operation.settled();
    observe(hw, "pan_tilt.absolute.settled", &settled);
    let readback = camera.pan_tilt().position();
    observe(hw, "pan_tilt.readback", &readback);
    if let Ok(position) = &readback {
        hw!(
            hw,
            "observation.pan_tilt.p0=({},{}) target=({target_pan},{target_tilt}) \
             readback=({},{}) delta=({},{})",
            p0.pan,
            p0.tilt,
            position.pan,
            position.tilt,
            position.pan - target_pan,
            position.tilt - target_tilt
        );
    }

    restore_and_report(camera, hw, &recall_attempted);

    checks.check(hw, "absolute_settled_ok", settled.is_ok());
    checks.check(hw, "readback_ok", readback.is_ok());
    Ok(checks)
}

/// The lowest speed the profile allows for a coarse level: the level's own
/// value clamped into the profile's range.
fn slowest_speed(range: &std::ops::RangeInclusive<u8>, level_speed: u8) -> u8 {
    level_speed.clamp(*range.start(), *range.end())
}

// ---------------------------------------------------------------------------
// HW-05.6: shutter table round trip (settings)
// ---------------------------------------------------------------------------

/// Encodes `request` for `target` through the request's own wire encoder.
fn encode<R: Request>(request: &R, target: CameraId) -> Result<Vec<u8>, Error> {
    let mut buffer = vec![0; request.encoded_size()];
    let written = request.write_into(target, &mut buffer)?;
    buffer.truncate(written);
    Ok(buffer)
}

/// Space-separated upper-case hex of `bytes`.
fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The shutter position carried by a `00 00 0p 0q` inquiry payload.
fn shutter_position(payload: &[u8]) -> Option<u8> {
    match *payload {
        [0, 0, high @ 0..=0x0F, low @ 0..=0x0F] => Some(high << 4 | low),
        _ => None,
    }
}

/// Raw-inquiry decoder that keeps the reply payload bytes verbatim.
fn payload_bytes(payload: &[u8]) -> Result<Vec<u8>, Error> {
    Ok(payload.to_vec())
}

/// The exposure mode in which the shutter is settable: the current mode when
/// it already is (shutter priority or manual), otherwise shutter priority, or
/// manual when the profile lacks shutter priority. Shutter priority leaves
/// iris and gain automatic, so it changes the least.
fn settable_shutter_mode(
    capabilities: &Capabilities,
    current: ExposureMode,
) -> Option<ExposureMode> {
    if matches!(current, ExposureMode::Shutter | ExposureMode::Manual) {
        return Some(current);
    }
    [ExposureMode::Shutter, ExposureMode::Manual]
        .into_iter()
        .find(|mode| capabilities.supports_exposure_mode(*mode))
}

fn shutter_text(result: &Result<ShutterSpeed, Error>) -> String {
    result.as_ref().map_or_else(
        |_| "ERR".to_owned(),
        |speed| format!("0x{:02X}", speed.value()),
    )
}

fn mode_text(result: &Result<ExposureMode, Error>) -> String {
    result
        .as_ref()
        .map_or_else(|_| "ERR".to_owned(), |mode| format!("{mode:?}"))
}

/// Sets one shutter code and reads it back through the typed inquiry and the
/// raw inquiry of the same frame.
struct ShutterProbe<'a, P: WireRowProfile> {
    camera: &'a Camera<P>,
    target: CameraId,
    hw: Hw,
    raw_inquiry: raw::Inquiry<Vec<u8>>,
}

impl<P: WireRowProfile> ShutterProbe<'_, P> {
    /// Sets `entry`'s code (`81 01 04 4A 00 00 0p 0q FF`), waits, inquires it
    /// back (`81 09 04 4A FF`) and checks that both the typed `ShutterSpeed`
    /// and the raw reply position equal the code set.
    fn round_trip(&self, checks: &mut Checks, label: &str, entry: &ShutterSpeedEntry) {
        let hw = &self.hw;
        let speed = ShutterSpeed::new(entry.value);
        let set_frame = encode(&Shutter::SetSpeed(speed), self.target)
            .map_or_else(|_| "ERR".to_owned(), |bytes| hex(&bytes));
        hw!(
            hw,
            "shutter.{label}.target fraction={} code=0x{:02X} set_frame={set_frame}",
            entry.exposure,
            entry.value
        );
        let set = self.camera.exposure().shutter_direct(speed);
        observe(hw, &format!("shutter.{label}.set"), &set);
        sleep(SETTLE_DELAY);
        let typed = self.camera.exposure().shutter();
        observe(hw, &format!("shutter.{label}.typed"), &typed);
        let payload = self.camera.inquire(&self.raw_inquiry);
        observe(hw, &format!("shutter.{label}.raw"), &payload);
        let raw_position = payload
            .as_ref()
            .ok()
            .and_then(|payload| shutter_position(payload));
        hw!(
            hw,
            "shutter.{label}.decode fraction={} set_code=0x{:02X} inquiry_frame={} \
             reply_payload={} typed={} raw_position={}",
            entry.exposure,
            entry.value,
            hex(self.raw_inquiry.bytes()),
            payload
                .as_ref()
                .map_or_else(|_| "ERR".to_owned(), |payload| hex(payload)),
            shutter_text(&typed),
            raw_position.map_or_else(|| "NONE".to_owned(), |code| format!("0x{code:02X}"))
        );
        checks.check(hw, format!("shutter_{label}_set_ok"), set.is_ok());
        checks.check(
            hw,
            format!("shutter_{label}_typed_readback_equals_set"),
            matches!(&typed, Ok(readback) if *readback == speed),
        );
        checks.check(
            hw,
            format!("shutter_{label}_raw_position_equals_set"),
            raw_position == Some(entry.value),
        );
    }
}

/// Restores the original shutter position and then the original exposure
/// mode. Each dimension is marked dirty before its first change attempt, so
/// `Drop` restores it on every exit path, including a panic or an early `?`.
/// Each restore is attempted at most once. The shutter goes first, while the
/// test's shutter-settable mode is still active.
struct ShutterGuard<'a, P: WireRowProfile> {
    camera: &'a Camera<P>,
    hw: Hw,
    original_mode: ExposureMode,
    original_shutter: ShutterSpeed,
    mode_dirty: Cell<bool>,
    shutter_dirty: Cell<bool>,
}

impl<P: WireRowProfile> ShutterGuard<'_, P> {
    /// Restores the shutter (`81 01 04 4A 00 00 0p 0q FF`), then the exposure
    /// mode (`81 01 04 39 0m FF`).
    fn restore(&self) {
        let hw = &self.hw;
        if self.shutter_dirty.replace(false) {
            let result = self.camera.exposure().shutter_direct(self.original_shutter);
            observe(hw, "restore.shutter.set", &result);
            sleep(SETTLE_DELAY);
        }
        if self.mode_dirty.replace(false) {
            let result = self.camera.exposure().set_mode(self.original_mode);
            observe(hw, "restore.exposure_mode.set", &result);
            sleep(SETTLE_DELAY);
        }
    }

    /// Reads the mode and shutter back and prints the `RESTORED` line.
    fn report_restored(&self) -> (Result<ExposureMode, Error>, Result<ShutterSpeed, Error>) {
        let hw = &self.hw;
        let mode = self.camera.exposure().mode();
        let shutter = self.camera.exposure().shutter();
        observe(hw, "restored.exposure_mode", &mode);
        observe(hw, "restored.shutter", &shutter);
        hw!(
            hw,
            "RESTORED exposure={} shutter={}",
            mode_text(&mode),
            shutter_text(&shutter)
        );
        (mode, shutter)
    }
}

impl<P: WireRowProfile> Drop for ShutterGuard<'_, P> {
    fn drop(&mut self) {
        if self.mode_dirty.get() || self.shutter_dirty.get() {
            hw!(self.hw, "guard.begin shutter");
            self.restore();
            let _ = self.report_restored();
            hw!(self.hw, "guard.end shutter");
        }
    }
}

/// Reads the exposure mode (`81 09 04 39 FF`), shutter (`81 09 04 4A FF`),
/// iris and gain; selects a shutter-settable exposure mode; then sets and
/// inquires back one step from the current shutter code and the profile
/// table's minimum, middle and maximum codes
/// (`81 01 04 4A 00 00 0p 0q FF`). Each readback must equal the code set,
/// both through the typed inquiry and as the raw `00 00 0p 0q` reply payload
/// of the same inquiry frame. Restores the shutter and then the mode, and
/// asserts both equal the originals.
#[test]
#[ignore]
fn hw05_shutter_round_trip() {
    let hw = Hw::new();
    let Some(ip) = settings_gate(&hw, "hw05_shutter_round_trip") else {
        return;
    };
    let selected = selected_profile(&hw);
    run_for_profile!(selected, shutter_round_trip, &hw, &ip);
}

fn shutter_round_trip<P: WireRowProfile>(hw: &Hw, ip: &str) {
    let session = open_session::<P>(hw, ip);
    let outcome = shutter_body(session.camera(), session.target(), hw);
    close_session(hw, session);
    outcome.expect("shutter round trip").assert_all();
}

fn shutter_body<P: WireRowProfile>(
    camera: &Camera<P>,
    target: CameraId,
    hw: &Hw,
) -> Result<Checks, Error> {
    let mut checks = Checks::default();
    let capabilities = camera.capabilities();
    let table = &capabilities.shutter_speeds;
    for entry in table {
        hw!(
            hw,
            "profile.shutter_entry code=0x{:02X} fraction={}",
            entry.value,
            entry.exposure
        );
    }
    checks.check(
        hw,
        "profile_shutter_table_has_three_entries",
        table.len() >= 3,
    );

    let original_mode = camera.exposure().mode();
    observe(hw, "original.exposure_mode", &original_mode);
    let original_shutter = camera.exposure().shutter();
    observe(hw, "original.shutter", &original_shutter);
    observe(hw, "original.iris", &camera.exposure().iris());
    observe(hw, "original.gain", &camera.exposure().gain());
    checks.check(hw, "original_exposure_mode_readable", original_mode.is_ok());
    checks.check(hw, "original_shutter_readable", original_shutter.is_ok());
    let (Ok(original_mode), Ok(original_shutter)) = (original_mode, original_shutter) else {
        return Ok(checks);
    };
    if table.len() < 3 {
        return Ok(checks);
    }
    let Some(test_mode) = settable_shutter_mode(capabilities, original_mode) else {
        hw!(
            hw,
            "SKIP_MODE no shutter-settable exposure mode; nothing changed"
        );
        checks.check(hw, "shutter_settable_mode_available", false);
        return Ok(checks);
    };

    // The raw inquiry sends the typed inquiry's own frame and keeps the reply
    // payload, so each readback is checked both decoded and as wire bytes.
    let probe = ShutterProbe {
        camera,
        target,
        hw: *hw,
        raw_inquiry: raw::Inquiry::from_fn(
            encode(&ShutterInquiry, target)?,
            InquiryRoute::RAW,
            payload_bytes,
            TimeoutClass::Inquiry,
            RetryClass::Inquiry,
            ControlClass::Normal,
        )?,
    };

    let guard = ShutterGuard {
        camera,
        hw: *hw,
        original_mode,
        original_shutter,
        mode_dirty: Cell::new(false),
        shutter_dirty: Cell::new(false),
    };

    let mut mode_ready = true;
    if test_mode != original_mode {
        hw!(
            hw,
            "exposure_mode.target={test_mode:?} frame={}",
            hex(&encode(&ExposureCommand::new(test_mode), target)?)
        );
        guard.mode_dirty.set(true);
        let set = camera.exposure().set_mode(test_mode);
        observe(hw, "exposure_mode.set", &set);
        sleep(SETTLE_DELAY);
        let readback = camera.exposure().mode();
        observe(hw, "exposure_mode.after_set", &readback);
        mode_ready = matches!(&readback, Ok(mode) if *mode == test_mode);
        checks.check(hw, "exposure_mode_readback_equals_target", mode_ready);
    }

    if mode_ready {
        let last = table.len() - 1;
        let original_index = table
            .iter()
            .position(|entry| entry.value == original_shutter.value());
        hw!(hw, "shutter.original_table_index={original_index:?}");
        // One step from the current code (up, or down from the maximum), then
        // the table's minimum, middle and maximum. Consecutive targets always
        // differ, so every readback proves a change.
        let one_step = original_index.map(|index| {
            let neighbour = if index < last { index + 1 } else { index - 1 };
            ("one_step", neighbour)
        });
        let steps = one_step.into_iter().chain([
            ("table_min", 0),
            ("table_middle", last / 2),
            ("table_max", last),
        ]);
        guard.shutter_dirty.set(true);
        for (label, index) in steps {
            probe.round_trip(&mut checks, label, &table[index]);
        }
    }

    guard.restore();
    let (mode, shutter) = guard.report_restored();
    checks.check(
        hw,
        "restored_exposure_mode_equals_original",
        matches!(&mode, Ok(mode) if *mode == original_mode),
    );
    let restored_shutter = matches!(&shutter, Ok(speed) if *speed == original_shutter);
    if matches!(original_mode, ExposureMode::Shutter | ExposureMode::Manual) {
        checks.check(hw, "restored_shutter_equals_original", restored_shutter);
    } else {
        // An automatic mode owns the shutter again after the mode restore, so
        // the reading is recorded, not asserted.
        hw!(
            hw,
            "observation.restored_shutter_equals_original={restored_shutter} \
             (automatic mode {original_mode:?} owns the shutter)"
        );
    }
    Ok(checks)
}

// ---------------------------------------------------------------------------
// HW-05.7: tilt polarity (motion)
// ---------------------------------------------------------------------------

/// The position at rest after one tilt drive.
struct TiltDrive {
    after: PanTiltPosition,
    rest_stable: bool,
}

/// Announces the drive, pauses for the operator, drives `direction` at pan
/// speed 1 and tilt speed 1 for [`DRIVE`], sends STOP, waits for rest and
/// reads the position. An error after the drive was submitted returns through
/// `?` into the caller's [`MotionGuard`].
fn tilt_drive<P: WireRowProfile>(
    camera: &Camera<P>,
    hw: &Hw,
    label: &str,
    direction: PanTiltDirection,
) -> Result<TiltDrive, Error> {
    let pan_speed = PanSpeed::new(1)?;
    let tilt_speed = TiltSpeed::new(1)?;
    hw!(hw, "OBSERVE: camera will tilt {label} briefly");
    sleep(OBSERVE_PAUSE);

    let drive_started = Instant::now();
    let mut drive = camera
        .pan_tilt()
        .move_direction(direction, pan_speed, tilt_speed)?;
    hw!(hw, "tilt.{label}.submitted id={:?}", drive.id());
    sleep(DRIVE);
    let mut stop = camera.pan_tilt().stop()?;
    hw!(
        hw,
        "tilt.{label}.drive_wall_ms_before_stop_submitted={}",
        drive_started.elapsed().as_millis()
    );
    let stop_result = stop.applied();
    observe(hw, &format!("tilt.{label}.stop.applied"), &stop_result);
    let drive_result = drive.applied_with_timeout(HANDLE_WAIT);
    observe(hw, &format!("tilt.{label}.applied"), &drive_result);
    stop_result?;

    let idle = camera
        .motion()
        .wait_until_idle(IdleWait::new(AffectedAxes::PAN_TILT, IDLE_WAIT));
    observe(hw, &format!("tilt.{label}.wait_until_idle"), &idle);
    let rest_stable = sample_rest(
        hw,
        &format!("tilt.{label}"),
        REST_SAMPLES,
        REST_INTERVAL,
        || camera.pan_tilt().position(),
    );
    let after = camera.pan_tilt().position();
    observe(hw, &format!("tilt.{label}.after"), &after);
    Ok(TiltDrive {
        after: after?,
        rest_stable,
    })
}

/// Prints one drive's raw and degree positions and records its checks: the
/// raw and degree tilt increase for an `up` drive and decrease otherwise, and
/// pan is unchanged within [`PAN_TOLERANCE_RAW`].
fn record_tilt<P: WireRowProfile>(
    hw: &Hw,
    checks: &mut Checks,
    label: &str,
    up: bool,
    before: PanTiltPosition,
    drive: &TiltDrive,
) {
    let conversion = PanTiltCoordinateConversion::for_profile::<P>();
    let after = drive.after;
    let tilt_delta = after.tilt - before.tilt;
    let pan_delta = after.pan - before.pan;
    let before_deg = conversion.to_degrees(before);
    let after_deg = conversion.to_degrees(after);
    let tilt_delta_deg = after_deg.tilt.0 - before_deg.tilt.0;
    hw!(
        hw,
        "observation.tilt.{label} before_raw=({},{}) after_raw=({},{}) delta_raw=(pan {pan_delta}, \
         tilt {tilt_delta}) before_deg=({:.3},{:.3}) after_deg=({:.3},{:.3}) \
         tilt_delta_deg={tilt_delta_deg:.3}",
        before.pan,
        before.tilt,
        after.pan,
        after.tilt,
        before_deg.pan.0,
        before_deg.tilt.0,
        after_deg.pan.0,
        after_deg.tilt.0
    );
    checks.check(hw, format!("tilt_{label}_rest_stable"), drive.rest_stable);
    checks.check(
        hw,
        format!("tilt_{label}_raw_delta_sign"),
        if up { tilt_delta > 0 } else { tilt_delta < 0 },
    );
    checks.check(
        hw,
        format!("tilt_{label}_degree_delta_sign"),
        if up {
            tilt_delta_deg > 0.0
        } else {
            tilt_delta_deg < 0.0
        },
    );
    checks.check(
        hw,
        format!("tilt_{label}_pan_unchanged"),
        pan_delta.abs() <= PAN_TOLERANCE_RAW,
    );
}

/// Drives `PanTiltDirection::Up` then `Down` for [`DRIVE`] each at pan speed 1
/// and tilt speed 1 (`81 01 06 01 01 01 03 01/02 FF`), reading the position
/// (`81 09 06 12 FF`) at rest before and after each. Up must increase the raw
/// and degree tilt, down must decrease them, and pan must not change. Whether
/// up is physically up is confirmed by the operator.
#[test]
#[ignore]
fn hw05_tilt_polarity() {
    let hw = Hw::new();
    let Some(ip) = motion_gate(&hw, "hw05_tilt_polarity") else {
        return;
    };
    let selected = selected_profile(&hw);
    run_for_profile!(selected, tilt_polarity, &hw, &ip);
}

fn tilt_polarity<P: WireRowProfile>(hw: &Hw, ip: &str) {
    let session = open_session::<P>(hw, ip);
    let outcome = tilt_polarity_body(session.camera(), hw);
    close_session(hw, session);
    outcome.expect("tilt polarity").assert_all();
}

fn tilt_polarity_body<P: WireRowProfile>(camera: &Camera<P>, hw: &Hw) -> Result<Checks, Error> {
    let mut checks = Checks::default();
    let recall_attempted = Cell::new(false);

    let p0 = camera.pan_tilt().position();
    observe(hw, "pan_tilt.p0", &p0);
    let p0 = p0?;

    let capabilities = camera.capabilities();
    hw!(
        hw,
        "profile.pan_tilt tilt_range={:?} tilt_degrees_to_units={} pan_tolerance_raw={PAN_TOLERANCE_RAW}",
        capabilities.tilt_range,
        <P as PanTilt>::TILT_DEGREES_TO_UNITS
    );
    // An axis at a limit cannot show a change; never move it there.
    if p0.tilt <= *capabilities.tilt_range.start() || p0.tilt >= *capabilities.tilt_range.end() {
        hw!(
            hw,
            "SKIP_RANGE tilt {} is at a limit of {:?}; no motion issued",
            p0.tilt,
            capabilities.tilt_range
        );
        report_restored(camera, hw);
        checks.check(hw, "tilt_start_inside_range", false);
        return Ok(checks);
    }

    let _guard = MotionGuard {
        camera,
        hw: *hw,
        recall_attempted: &recall_attempted,
    };

    let up = tilt_drive(camera, hw, "UP", PanTiltDirection::Up)?;
    record_tilt::<P>(hw, &mut checks, "UP", true, p0, &up);
    let down = tilt_drive(camera, hw, "DOWN", PanTiltDirection::Down)?;
    record_tilt::<P>(hw, &mut checks, "DOWN", false, up.after, &down);

    restore_and_report(camera, hw, &recall_attempted);
    hw!(
        hw,
        "OPERATOR: confirm the camera physically tilted up during the UP drive and down during \
         the DOWN drive (positive tilt is up)"
    );
    Ok(checks)
}
