//! Blocking operation handles: applied, settled, detached, and dropped.
//!
//! Every movement step runs inside `movement`, so an early `?` returns through
//! `finish_session` and the session is closed on the failure path as well as
//! the success path.
//!
//! Dropping an operation handle is exactly `detach`: it relinquishes the
//! observer and never stops hardware. Callers who want motion bounded by a
//! scope write their own guard; `StopPanTiltOnExit` below is the whole pattern
//! and uses nothing but the public API.
//!
//! This example moves real hardware. Set `VISCA_CAMERA_ADDR` or pass an
//! address on the command line.

mod support;

use std::{env, thread::sleep, time::Duration};

use grafton_visca::{
    blocking::{Camera, Connect, Session},
    camera::{profiles::PtzOpticsG2, IdleWait},
    command::PanTiltDirection,
    types::{PanSpeed, TiltSpeed},
    AffectedAxes, Error,
};

use support::finish_session;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = env::args()
        .nth(1)
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .ok_or("camera address required (argument or VISCA_CAMERA_ADDR)")?;

    let session = Connect::open_tcp::<PtzOpticsG2>(&address)?;
    let result = movement(&session);
    finish_session(result, session.close())?;
    Ok(())
}

/// Sends the typed pan/tilt STOP when it leaves scope, on every exit path.
///
/// This is the scoped stop-on-exit pattern from `docs/migration_2_0.md`. It is
/// caller-owned code, not library API: the library never stops hardware on
/// drop, so a caller who wants an early `?` or a panic to end motion writes a
/// guard like this and holds it for the region that must stay bounded.
///
/// `Drop` cannot report a failure and may run while unwinding, so the stop is
/// best effort here — exactly as in any scope guard.
struct StopPanTiltOnExit<'a, 'session> {
    camera: &'a Camera<'session, PtzOpticsG2>,
}

impl Drop for StopPanTiltOnExit<'_, '_> {
    fn drop(&mut self) {
        if let Ok(stop) = self.camera.pan_tilt().stop() {
            let _ = stop.applied();
        }
    }
}

fn movement(session: &Session) -> Result<(), Error> {
    let camera = session.camera::<PtzOpticsG2>()?;

    // Targeted movement exposes profile-selected protocol settlement;
    // applied-only movement has no settled state and is observed only at protocol application. Both
    // waits consume the handle, so neither adds a stop of its own.
    camera.pan_tilt().home()?.settled()?;
    camera.zoom().tele()?.applied()?;
    camera.zoom().stop()?.applied()?;

    // A handle held across fallible work is not a safety net: if `applied`
    // below returned an error, `drive` would simply be dropped and pan/tilt
    // would keep moving. The guard is what bounds the motion to this scope.
    {
        let _stop_on_exit = StopPanTiltOnExit { camera: &camera };
        let drive = camera.pan_tilt().move_direction(
            PanTiltDirection::Up,
            PanSpeed::new(6)?,
            TiltSpeed::new(6)?,
        )?;
        sleep(Duration::from_millis(250));
        drive.applied()?;
    }

    // `detach` is the explicit spelling of what drop already does. The zoom
    // keeps driving after this line, so it must be paired with an explicit
    // bounded stop.
    camera.zoom().tele()?.detach();
    sleep(Duration::from_millis(250));
    camera.zoom().stop()?.applied()?;

    camera.motion().wait_until_idle(IdleWait::new(
        AffectedAxes::PAN_TILT,
        Duration::from_secs(30),
    ))?;
    Ok(())
}
