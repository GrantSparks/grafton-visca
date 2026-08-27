//! Tokio operation handles: applied, settled, detached, and dropped.
//!
//! Every movement step runs inside `movement`, so an early `?` returns through
//! `finish_session` and the session is closed on the failure path as well as
//! the success path.
//!
//! Dropping an operation handle is exactly `detach`: it relinquishes the
//! observer and never stops hardware. `Drop` cannot await, so the async form
//! of a scoped stop is a wrapper that runs the stop on both of the body's
//! return paths — but not on a panic or a dropped future, which a synchronous
//! `Drop` guard does cover. See `bounded_drive` below and the guard pattern in
//! `docs/migration_2_0.md`.
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
    AffectedAxes, Camera, Connect, Error, Session,
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

    // A handle held across fallible work is not a safety net: if `applied`
    // inside `drive_up` returned an error, its handle would simply be dropped
    // and pan/tilt would keep moving. `bounded_drive` is what stops it.
    bounded_drive(&camera).await?;

    // `detach` is the explicit spelling of what drop already does. The zoom
    // keeps driving after this line, so it must be paired with an explicit
    // bounded stop.
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

/// Drives pan/tilt with a STOP that runs on both of `drive_up`'s return paths.
///
/// This is the async form of the scoped stop-on-exit guard from
/// `docs/migration_2_0.md`. `Drop` cannot await, so instead of a guard type the
/// caller wraps the fallible region, stops, and only then propagates the body's
/// result. The stop itself is best effort, as in a synchronous guard.
///
/// It covers less than a synchronous `Drop` guard does. `Ok` and `Err` from
/// `drive_up` both reach the stop; a panic inside `drive_up` and a caller that
/// drops this future before it completes — a `select!` loser, an expired
/// `tokio::time::timeout`, an aborted task — do not. In those cases the stop is
/// never reached and pan/tilt keeps moving. A synchronous `Drop` guard covers
/// both, because `Drop` runs while unwinding and runs whenever the value leaves
/// scope. An async caller that must survive cancellation has to submit the STOP
/// from a `Drop` that cannot await — through a channel or a detached task — or
/// bound the movement at the camera instead of at the future.
async fn bounded_drive(camera: &Camera<PtzOpticsG2>) -> Result<(), Error> {
    let result = drive_up(camera).await;
    if let Ok(stop) = camera.pan_tilt().stop().await {
        let _ = stop.applied().await;
    }
    result
}

async fn drive_up(camera: &Camera<PtzOpticsG2>) -> Result<(), Error> {
    let drive = camera
        .pan_tilt()
        .move_direction(PanTiltDirection::Up, PanSpeed::new(6)?, TiltSpeed::new(6)?)
        .await?;
    tokio::time::sleep(Duration::from_millis(250)).await;
    drive.applied().await
}
