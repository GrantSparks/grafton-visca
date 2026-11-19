//! Move a PTZOptics G2 using Tokio operation handles. Pass the camera
//! address on the command line or set `VISCA_CAMERA_ADDR`.

mod support;

use std::{env, time::Duration};

use grafton_visca::{
    profiles::PtzOpticsG2, r#async::Camera, r#async::CameraSession, runtime::TokioRuntime,
    types::AffectedAxes, types::IdleWait, types::PanSpeed, types::PanTiltDirection,
    types::PanTiltSpeed, types::TiltSpeed, Error,
};

use support::finish_session;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = env::args()
        .nth(1)
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .ok_or("camera address required (argument or VISCA_CAMERA_ADDR)")?;

    let runtime = TokioRuntime::from_current()?;
    let session = CameraSession::<PtzOpticsG2>::connect_tcp(&address, runtime).await?;
    let result = movement(session.camera()).await;
    finish_session(result, session.close().await)?;
    Ok(())
}

async fn movement(camera: &Camera<PtzOpticsG2>) -> Result<(), Error> {
    // Home has a target position. The zoom drive continues after application.
    camera.pan_tilt().home().await?.settled().await?;
    camera.zoom().tele().await?.applied().await?;
    camera.zoom().stop().await?.applied().await?;

    bounded_drive(camera).await?;

    // Detaching leaves the zoom running until the explicit STOP.
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

/// Attempts a STOP after `drive_up` returns either `Ok` or `Err`.
/// A panic or a dropped future can skip this cleanup. To cover those paths,
/// arrange for a separate task to send the STOP, or bound motion at the camera.
async fn bounded_drive(camera: &Camera<PtzOpticsG2>) -> Result<(), Error> {
    let result = drive_up(camera).await;
    if let Ok(mut stop) = camera.pan_tilt().stop().await {
        let _ = stop.applied().await;
    }
    result
}

async fn drive_up(camera: &Camera<PtzOpticsG2>) -> Result<(), Error> {
    let mut drive = camera
        .pan_tilt()
        .drive(
            PanTiltDirection::Up,
            PanTiltSpeed::new(PanSpeed::new(6)?, TiltSpeed::new(6)?),
        )
        .await?;
    tokio::time::sleep(Duration::from_millis(250)).await;
    drive.applied().await
}
