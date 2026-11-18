//! Output, classification, gating and motion-safety helpers shared by the
//! operator-run hardware tests (`hardware_motion_test`,
//! `hardware_wire_rows_test` and `hardware_concurrent_test`).
//!
//! Compiled under grafton-visca's `blocking` feature; the observation macro
//! is [`hw!`](crate::hw).
//!
//! Output convention: every observation is one greppable line on stderr of
//! the form `HW|t=<elapsed ms since test start>|<observation>`. A sink that
//! prefixes observations (for example `cam=<ip>|`) implements [`HwLog`].

use std::{
    borrow::Cow,
    cell::Cell,
    env, fmt,
    time::{Duration, Instant},
};

use grafton_visca::{
    blocking::{Camera, CameraSession},
    capabilities::{gate, Permits},
    config::{CameraConfig, TransportOptions},
    error::{Certainty, ErrorKind},
    profiles::{AnyProfile, CameraProfile, ProfileId},
    types::PresetNumber,
    Error,
};

/// Resolves `VISCA_PROFILE` through the generated built-in identifier parser.
///
/// The existing G2 bench remains the default when the variable is absent or
/// empty. Every explicit value is a canonical `snake_case` identifier; invalid
/// values fail before a session can open. This helper needs no serde feature.
pub fn selected_profile() -> ProfileId {
    let value = match env::var("VISCA_PROFILE") {
        Ok(value) => Some(value),
        Err(env::VarError::NotPresent) => None,
        Err(error) => panic!("cannot read VISCA_PROFILE: {error}"),
    };
    parse_profile(value.as_deref()).unwrap_or_else(|error| panic!("invalid VISCA_PROFILE: {error}"))
}

fn parse_profile(value: Option<&str>) -> Result<ProfileId, Error> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => value.parse(),
        None => Ok(ProfileId::PtzOpticsG2),
    }
}

/// Builds a runtime-profile camera configuration for an explicit transport.
///
/// Motion, fault and wire-regression scenarios keep their selected transport
/// and port. `CameraSession::connect` validates that choice before network I/O.
pub fn camera_config(transport: TransportOptions) -> CameraConfig<AnyProfile> {
    let id = selected_profile();
    eprintln!("Hardware profile: {}", id.as_str());
    CameraConfig::new(id, transport)
}

/// Selects an allowed network transport for general hardware inquiries.
///
/// TCP is preferred when available, otherwise UDP is used. The selected
/// profile supplies its registered port and envelope. Serial-only profiles
/// require a serial harness instead of the `VISCA_CAMERA_IP` network harness.
pub fn network_config(address: impl Into<String>) -> CameraConfig<AnyProfile> {
    let id = selected_profile();
    let profile = id.spec().expect("built-in profile validation");
    let transports = profile.transports();
    let address = address.into();
    let transport = if transports.tcp_port().is_some() {
        TransportOptions::tcp(address)
    } else if transports.udp_port().is_some() {
        TransportOptions::udp(address)
    } else {
        panic!(
            "VISCA_PROFILE={} has no network transport for VISCA_CAMERA_IP",
            id.as_str()
        );
    };
    eprintln!("Hardware profile: {}", id.as_str());
    CameraConfig::new(id, transport)
}

/// Profile permissions required by the shared hardware restoration helpers.
///
/// Typed cameras provide source-backed grants; runtime cameras validate the
/// registered profile before any STOP or preset recall reaches the wire.
pub trait HardwareProfile:
    CameraProfile
    + Permits<gate::PanTilt>
    + Permits<gate::PanTiltPositioning>
    + Permits<gate::Zoom>
    + Permits<gate::Presets>
    + Permits<gate::PresetRecallSettlement>
{
}
impl<P> HardwareProfile for P where
    P: CameraProfile
        + Permits<gate::PanTilt>
        + Permits<gate::PanTiltPositioning>
        + Permits<gate::Zoom>
        + Permits<gate::Presets>
        + Permits<gate::PresetRecallSettlement>
{
}

/// Bound for waiting on a guard STOP or another handle observation.
pub const HANDLE_WAIT: Duration = Duration::from_secs(2);

/// A sink for `HW|t=..|` observation lines.
pub trait HwLog {
    /// Writes one observation line.
    fn log(&self, args: fmt::Arguments<'_>);
}

/// Formats one observation line through an [`HwLog`] sink.
#[macro_export]
macro_rules! hw {
    ($hw:expr, $($arg:tt)*) => {
        $hw.log(format_args!($($arg)*))
    };
}

/// Test clock: every output line carries milliseconds since test start.
#[derive(Debug, Clone, Copy)]
pub struct Hw {
    start: Instant,
}

impl Hw {
    /// Starts the test clock now.
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Default for Hw {
    fn default() -> Self {
        Self::new()
    }
}

impl HwLog for Hw {
    fn log(&self, args: fmt::Arguments<'_>) {
        eprintln!("HW|t={}|{}", self.start.elapsed().as_millis(), args);
    }
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
pub fn observe_error(hw: &impl HwLog, label: &str, error: &Error) {
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
pub fn observe<T: fmt::Debug>(hw: &impl HwLog, label: &str, result: &Result<T, Error>) {
    match result {
        Ok(value) => hw!(hw, "{label}=Ok({value:?})"),
        Err(error) => observe_error(hw, label, error),
    }
}

/// Whether the operator opted in with `<flag>=1`; prints a skip line when not.
pub fn opted_in(hw: &impl HwLog, test: &str, flag: &str) -> bool {
    if env::var(flag).as_deref() == Ok("1") {
        return true;
    }
    hw!(hw, "SKIP {test}: {flag}=1 not set");
    eprintln!("Skipped: {flag}=1 not set");
    false
}

/// Named boolean checks collected during a test and asserted only after the
/// camera has been restored and the session closed.
#[derive(Debug, Default)]
pub struct Checks(Vec<(Cow<'static, str>, bool)>);

impl Checks {
    pub fn check(&mut self, hw: &impl HwLog, name: impl Into<Cow<'static, str>>, ok: bool) {
        let name = name.into();
        hw!(hw, "check.{name}={ok}");
        self.0.push((name, ok));
    }

    pub fn assert_all(&self) {
        let failed: Vec<&str> = self
            .0
            .iter()
            .filter(|(_, ok)| !ok)
            .map(|(name, _)| name.as_ref())
            .collect();
        assert!(failed.is_empty(), "failed checks: {failed:?}");
    }
}

/// Closes a blocking session and prints the result.
pub fn close_session<P: CameraProfile>(hw: &impl HwLog, session: CameraSession<P>) {
    let result = session.close();
    observe(hw, "session.close", &result);
}

/// Sends pan/tilt STOP and zoom STOP, waits for each to be applied, then
/// recalls preset 1 unless the test already attempted that recall. Every step
/// is best effort: `Drop` cannot report and may run while unwinding.
pub struct MotionGuard<'a, P: HardwareProfile, L: HwLog> {
    pub camera: &'a Camera<P>,
    pub hw: L,
    pub recall_attempted: &'a Cell<bool>,
}

impl<P: HardwareProfile, L: HwLog> Drop for MotionGuard<'_, P, L> {
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
fn restore_preset_one<P: HardwareProfile>(camera: &Camera<P>, hw: &impl HwLog, label: &str) {
    let result = PresetNumber::new(1)
        .and_then(|preset| camera.presets().recall(preset))
        .and_then(|mut operation| operation.settled());
    observe(hw, &format!("{label}.settled"), &result);
}

/// Reads pan/tilt and zoom positions and prints the `RESTORED` line.
pub fn report_restored<P: HardwareProfile>(camera: &Camera<P>, hw: &impl HwLog) {
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
pub fn restore_and_report<P: HardwareProfile>(
    camera: &Camera<P>,
    hw: &impl HwLog,
    attempted: &Cell<bool>,
) {
    attempted.set(true);
    restore_preset_one(camera, hw, "restore.preset1");
    report_restored(camera, hw);
}

#[cfg(test)]
mod tests {
    use super::{parse_profile, HardwareProfile};
    use grafton_visca::profiles::ProfileId;

    #[test]
    fn hardware_profile_selection_uses_every_generated_id_without_serde() {
        for id in ProfileId::all() {
            assert_eq!(parse_profile(Some(id.as_str())).unwrap(), *id);
        }
    }

    #[test]
    fn hardware_profile_default_and_invalid_inputs_stay_explicit() {
        for input in [None, Some(""), Some("  ")] {
            assert_eq!(parse_profile(input).unwrap(), ProfileId::PtzOpticsG2);
        }
        for invalid in ["g2", "30x", "not_a_profile", "PtzOpticsG2"] {
            assert!(parse_profile(Some(invalid)).is_err(), "{invalid}");
        }
    }

    #[test]
    fn hardware_helpers_accept_typed_and_runtime_profile_permissions() {
        fn has_restoration_permissions<P: HardwareProfile>() {}
        has_restoration_permissions::<grafton_visca::profiles::PtzOpticsG2>();
        has_restoration_permissions::<grafton_visca::profiles::AnyProfile>();
    }
}
