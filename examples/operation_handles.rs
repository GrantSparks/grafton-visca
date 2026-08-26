//! Blocking operation handles with explicit applied and settled observations.
//!
//! This example moves real hardware. Set `VISCA_CAMERA_ADDR` or pass an
//! address on the command line.

use std::{env, time::Duration};

use grafton_visca::{
    blocking::Connect,
    camera::{profiles::PtzOpticsG2, IdleWait},
    AffectedAxes,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = env::args()
        .nth(1)
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .ok_or("camera address required (argument or VISCA_CAMERA_ADDR)")?;

    let session = Connect::open_tcp::<PtzOpticsG2>(&address)?;
    let camera = session.camera::<PtzOpticsG2>()?;

    // Targeted movement exposes physical settling; applied-only movement has
    // no settled state and is observed only at protocol application.
    camera.pan_tilt().home()?.settled()?;
    camera.zoom().tele()?.applied()?;
    camera.zoom().stop()?.applied()?;
    camera.motion().wait_until_idle(IdleWait::new(
        AffectedAxes::PAN_TILT,
        Duration::from_secs(30),
    ))?;

    session.close()?;
    Ok(())
}
