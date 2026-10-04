//! Blocking motion safety and observation: `is_moving`, `is_moving_axes`,
//! `wait_until_idle`, and the `stop_all_motion` owner halt.
//!
//! `camera.motion()` is the one camera-level motion safety/observation view.
//! `stop_all_motion()` attempts pan/tilt, zoom, and focus STOP even when an
//! earlier axis fails, returning a per-axis report under one common deadline.
//! This example explicitly collapses the report to its first failure.
//! `is_moving`/`is_moving_axes`/`wait_until_idle` observe *only* the axes they
//! are given; they never settle a submitted operation.
//!
//! This example only observes and then stops motion. Set `VISCA_CAMERA_ADDR` or
//! pass an address on the command line.

mod support;

use std::{env, time::Duration};

use grafton_visca::{
    blocking::{Camera, Connect},
    camera::{profiles::PtzOpticsG2, IdleWait, MotionQuery},
    AffectedAxes, Error,
};

use support::finish_session;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = env::args()
        .nth(1)
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .ok_or("camera address required (argument or VISCA_CAMERA_ADDR)")?;

    let session = Connect::open_tcp::<PtzOpticsG2>(&address)?;
    let camera = session.camera();
    let result = observe_and_stop(camera);
    finish_session(result, session.close())?;
    Ok(())
}

fn observe_and_stop(camera: &Camera<PtzOpticsG2>) -> Result<(), Error> {
    // `is_moving()` takes no argument and samples `AffectedAxes::MOVEMENT`
    // (pan/tilt + zoom + focus).
    println!(
        "moving (any movement axis): {}",
        camera.motion().is_moving()?
    );

    // `is_moving_axes` queries exactly the selected axes. Pan/tilt and zoom both
    // have position inquiries on every built-in profile.
    let query = MotionQuery::new(AffectedAxes::PAN_TILT.union(AffectedAxes::ZOOM));
    println!(
        "moving (pan/tilt + zoom): {}",
        camera.motion().is_moving_axes(query)?
    );

    // Wait for pan/tilt to come to rest, bounded by an explicit timeout. This is
    // observation, not settling: it does not correspond to any submitted move.
    camera.motion().wait_until_idle(IdleWait::new(
        AffectedAxes::PAN_TILT,
        Duration::from_secs(10),
    ))?;
    println!("pan/tilt idle");

    // Fence older declared motion, then inspect the supported STOP outcomes.
    camera.motion().stop_all_motion()?.into_result()?;
    println!("stop_all_motion complete");

    Ok(())
}
