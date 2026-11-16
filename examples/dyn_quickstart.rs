//! Runtime-selected profiles use the same concrete camera and operation types.
//!
//! `Camera<AnyProfile>` retains the registered profile's runtime validation,
//! owner, pacing, cancellation, and state cache. Capability discovery replaces
//! compile-time `Has*` bounds. Targeted operations expose `settled`; applied-only
//! operations expose `applied`.
//!
//! Requires `--features runtime-tokio`. Moves real hardware. Set
//! `VISCA_CAMERA_ADDR` or pass an address on the command line.

mod support;

use std::{env, time::Duration};

use grafton_visca::{
    capabilities::TypedSupportSurface,
    config::{CameraConfig, TransportOptions},
    operation::{AppliedOnly, Targeted},
    profiles::{AnyProfile, ProfileId},
    r#async::{Camera, CameraSession, Operation},
    runtime::TokioRuntime,
    Error,
};

use support::finish_session;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = env::args()
        .nth(1)
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .ok_or("camera address required (argument or VISCA_CAMERA_ADDR)")?;

    let config = CameraConfig::new(ProfileId::PtzOpticsG2, TransportOptions::tcp(address));
    let runtime = TokioRuntime::from_current()?;
    let session = CameraSession::connect(&config, runtime).await?;
    let result = runtime_control(session.camera()).await;
    finish_session(result, session.close().await)?;
    Ok(())
}

async fn runtime_control(camera: &Camera<AnyProfile>) -> Result<(), Error> {
    println!(
        "direct-zoom supported: {}",
        camera
            .capabilities()
            .supports(TypedSupportSurface::DirectZoom)
    );

    let mut home: Operation<Targeted> = camera.pan_tilt().home().await?;
    home.settled_with_timeout(Duration::from_secs(20)).await?;

    let mut stop: Operation<AppliedOnly> = camera.zoom().stop().await?;
    stop.applied_with_timeout(Duration::from_secs(2)).await?;
    Ok(())
}
