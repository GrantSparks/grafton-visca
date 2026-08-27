//! Tokio operation handles: applied, settled, detached, and dropped.
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

use std::{env, time::Duration};

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, IdleWait},
    command::PanTiltDirection,
    runtime::TokioRuntime,
    types::{PanSpeed, TiltSpeed},
    AffectedAxes, Connect, Error, Session,
};

use support::finish_session;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = env::args()
        .nth(1)
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .ok_or("camera address required (argument or VISCA_CAMERA_ADDR)")?;

    let runtime = TokioRuntime::from_current()?;
    let session = Connect::open_tcp::<PtzOpticsG2, _>(&address, runtime).await?;
    let result = movement(&session).await;
    finish_session(result, session.close().await)?;
    Ok(())
}

async fn movement(session: &Session) -> Result<(), Error> {
    let camera = session.camera::<PtzOpticsG2>()?;

    // Targeted movement exposes physical settling; applied-only movement has
    // no settled state and is observed only at protocol application. Both
    // waits consume the handle, so neither adds a stop of its own.
    camera.pan_tilt().home().await?.settled().await?;
    camera.zoom().tele().await?.applied().await?;
    camera.zoom().stop().await?.applied().await?;

    // A handle held across fallible work is the case the drop stop exists for:
    // if `applied` below returned an error, `drive` would already be gone and
    // pan/tilt would already have been stopped.
    let drive = camera
        .pan_tilt()
        .move_direction(PanTiltDirection::Up, PanSpeed::new(6)?, TiltSpeed::new(6)?)
        .await?;
    tokio::time::sleep(Duration::from_millis(250)).await;
    drive.applied().await?;
    camera.pan_tilt().stop().await?.applied().await?;

    // `detach` is the deliberate opt-out. The zoom keeps driving after this
    // line, so it must be paired with an explicit bounded stop.
    camera.zoom().tele().await?.detach();
    tokio::time::sleep(Duration::from_millis(250)).await;
    camera.zoom().stop().await?.applied().await?;

    camera
        .motion()
        .wait_until_idle(IdleWait::new(
            AffectedAxes::PAN_TILT,
            Duration::from_secs(30),
        ))
        .await?;
    Ok(())
}
