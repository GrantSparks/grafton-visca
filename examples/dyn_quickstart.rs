//! Dynamic (profile-erased) API: a targeted and an applied-only handle.
//!
//! `DynSessionCamera` erases the compile-time profile but shares the very same
//! owner, timeouts, pacing, cancellation, and state cache as a static
//! `Camera<P>`. Its nouns preserve the completion distinction:
//! `pan_tilt().home()` returns a `DynTargetedOperation` (which has `settled`),
//! while `zoom().stop()` returns a `DynAppliedOperation` (which does not).
//! Capability discovery replaces the compile-time marker gates.
//!
//! Requires `--features runtime-tokio,dyn-api`. Moves real hardware. Set
//! `VISCA_CAMERA_ADDR` or pass an address on the command line.

mod support;

use std::{env, time::Duration};

use grafton_visca::{
    camera::profiles::PtzOpticsG2,
    capabilities::TypedSupportSurface,
    dynapi::{DynAppliedOperation, DynSessionCamera, DynTargetedOperation},
    runtime::TokioRuntime,
    Connect, Error,
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
    let result = dynamic_control(session.session()).await;
    finish_session(result, session.close().await)?;
    Ok(())
}

async fn dynamic_control(session: &grafton_visca::Session) -> Result<(), Error> {
    // One dynamic view onto the session's sole registered target.
    let camera = DynSessionCamera::from_session(session)?;

    // Runtime capability discovery replaces the compile-time `Has*` markers.
    println!(
        "direct-zoom supported: {}",
        camera.supports_typed(TypedSupportSurface::DirectZoom)
    );

    // A targeted dynamic operation exposes `settled` (profile-selected protocol settlement).
    let home: DynTargetedOperation = camera.pan_tilt().home().await?;
    home.settled_with_timeout(Duration::from_secs(20)).await?;

    // An applied-only dynamic operation has no `settled`, only `applied`.
    let stop: DynAppliedOperation = camera.zoom().stop().await?;
    stop.applied_with_timeout(Duration::from_secs(2)).await?;

    Ok(())
}
