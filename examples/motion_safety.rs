//! Blocking motion safety and observation: `is_moving`, `wait_until_idle`,
//! and the `stop_all_motion` owner halt.
//!
//! `camera.motion()` is the one camera-level motion safety/observation view.
//! `stop_all_motion()` attempts pan/tilt, zoom, and focus STOP even when an
//! earlier axis fails, returning a per-axis report under one common deadline.
//! This example prints every axis outcome and treats a pan/tilt or zoom failure
//! as its error.
//!
//! A PTZOptics G2 in auto-focus mode (the factory default) rejects the focus
//! STOP as not executable (`90 6y 41 FF`), which the library reports as a
//! focus failure (`UnsequencedCommandUnconfirmed`) while pan/tilt and zoom are
//! `Applied`. In auto focus the camera owns the focus motor, so the example
//! shows that rejection but does not fail on it; in manual focus a focus STOP
//! failure is treated as an error like the other axes. With manual focus all
//! three axes are `Applied`.
//! `is_moving`/`wait_until_idle` observe *only* the axes they are given; they
//! never settle a submitted operation.
//!
//! This example only observes and then stops motion. Set `VISCA_CAMERA_ADDR` or
//! pass an address on the command line.

mod support;

use std::{env, time::Duration};

use grafton_visca::{
    blocking::{Camera, Connect},
    camera::{profiles::PtzOpticsG2, IdleWait, MotionQuery},
    AffectedAxes, Error, FocusMode, HaltOutcome,
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
    // `MotionQuery::default()` samples `AffectedAxes::MOVEMENT`
    // (pan/tilt + zoom + focus).
    println!(
        "moving (any movement axis): {}",
        camera.motion().is_moving(MotionQuery::default())?
    );

    // `is_moving` queries exactly the selected axes. Pan/tilt and zoom both
    // have position inquiries on every built-in profile.
    let query = MotionQuery::new(AffectedAxes::PAN_TILT.union(AffectedAxes::ZOOM));
    println!(
        "moving (pan/tilt + zoom): {}",
        camera.motion().is_moving(query)?
    );

    // Wait for pan/tilt to come to rest, bounded by an explicit timeout. This is
    // observation, not settling: it does not correspond to any submitted move.
    camera.motion().wait_until_idle(IdleWait::new(
        AffectedAxes::PAN_TILT,
        Duration::from_secs(10),
    ))?;
    println!("pan/tilt idle");

    // Fence older declared motion, then inspect the supported STOP outcomes.
    let report = camera.motion().stop_all_motion()?;
    println!("stop_all_motion pan/tilt: {}", describe(&report.pan_tilt));
    println!("stop_all_motion zoom: {}", describe(&report.zoom));
    println!("stop_all_motion focus: {}", describe(&report.focus));

    // Pan/tilt and zoom failures are real motion hazards. A G2 in auto focus
    // rejects the focus STOP as not executable, so that one focus failure is
    // expected and printed above, not hidden; in manual focus it is an error.
    let auto_focus = matches!(camera.focus().mode(), Ok(FocusMode::Auto));
    let focus = if auto_focus { None } else { Some(report.focus) };
    for outcome in [Some(report.pan_tilt), Some(report.zoom), focus]
        .into_iter()
        .flatten()
    {
        if let HaltOutcome::Failed(error) = outcome {
            return Err(error);
        }
    }
    println!("stop_all_motion complete");

    Ok(())
}

fn describe(outcome: &HaltOutcome) -> String {
    match outcome {
        HaltOutcome::Unsupported => "unsupported".to_owned(),
        HaltOutcome::Applied => "applied".to_owned(),
        HaltOutcome::Failed(error) => format!("failed ({error})"),
        other => format!("{other:?}"),
    }
}
