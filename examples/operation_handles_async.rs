//! Tokio operation handles with explicit applied and settled observations.
//!
//! This example moves real hardware. Set `VISCA_CAMERA_ADDR` or pass an
//! address on the command line.

use std::{env, time::Duration};

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, IdleWait},
    runtime::TokioRuntime,
    AffectedAxes, Connect,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = env::args()
        .nth(1)
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .ok_or("camera address required (argument or VISCA_CAMERA_ADDR)")?;

    let runtime = TokioRuntime::from_current()?;
    let session = Connect::open_tcp::<PtzOpticsG2, _>(&address, runtime).await?;
    let camera = session.camera::<PtzOpticsG2>()?;

    camera.pan_tilt().home().await?.settled().await?;
    camera.zoom().tele().await?.applied().await?;
    camera.zoom().stop().await?.applied().await?;
    camera
        .motion()
        .wait_until_idle(IdleWait::new(
            AffectedAxes::PAN_TILT,
            Duration::from_secs(30),
        ))
        .await?;

    session.close().await?;
    Ok(())
}
