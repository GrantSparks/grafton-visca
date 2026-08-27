//! Blocking operation handles: applied, settled, detached, and dropped.
//!
//! Every movement step runs inside `movement`, so an early `?` returns through
//! `finish_session` and the session is closed on the failure path as well as
//! the success path. That same early return drops any operation handle still
//! alive, which stops the axes it was driving — the drop is the safety net,
//! not a substitute for the explicit steps below.
//!
//! This example moves real hardware. Set `VISCA_CAMERA_ADDR` or pass an
//! address on the command line.

mod support;

use std::{env, thread::sleep, time::Duration};

use grafton_visca::{
    blocking::{Connect, Session},
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

fn movement(session: &Session) -> Result<(), Error> {
    let camera = session.camera::<PtzOpticsG2>()?;

    // Targeted movement exposes physical settling; applied-only movement has
    // no settled state and is observed only at protocol application. Both
    // waits consume the handle, so neither adds a stop of its own.
    camera.pan_tilt().home()?.settled()?;
    camera.zoom().tele()?.applied()?;
    camera.zoom().stop()?.applied()?;

    // A handle held across fallible work is the case the drop stop exists for:
    // if `applied` below returned an error, `drive` would already be gone and
    // pan/tilt would already have been stopped.
    let drive = camera.pan_tilt().move_direction(
        PanTiltDirection::Up,
        PanSpeed::new(6)?,
        TiltSpeed::new(6)?,
    )?;
    sleep(Duration::from_millis(250));
    drive.applied()?;
    camera.pan_tilt().stop()?.applied()?;

    // `detach` is the deliberate opt-out. The zoom keeps driving after this
    // line, so it must be paired with an explicit bounded stop.
    camera.zoom().tele()?.detach();
    sleep(Duration::from_millis(250));
    camera.zoom().stop()?.applied()?;

    camera.motion().wait_until_idle(IdleWait::new(
        AffectedAxes::PAN_TILT,
        Duration::from_secs(30),
    ))?;
    Ok(())
}
