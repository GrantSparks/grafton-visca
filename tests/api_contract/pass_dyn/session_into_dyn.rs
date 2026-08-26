#![allow(dead_code)]

//! Issue #588: the `Connect` async helpers return a `CameraSession`, but
//! `IntoDynCamera` is implemented for the owned `Camera`. `into_inner()` is the
//! bridge between the two, so `Connect -> into_dyn()` must typecheck without
//! dropping down to `Runtime::connect_tcp` + `CameraBuilder`.

use grafton_visca::{
    camera::{Camera, CameraSession, Connect},
    dynapi::{DynCamera, DynCameraControl, IntoDynCamera},
    mode::Async,
    profiles::PtzOpticsG2,
    runtime::{Runtime, TransportHandle},
    Error,
};

/// The short form the dyn API quickstart is meant to use.
async fn connect_then_into_dyn<R>(
    runtime: R,
) -> Result<DynCamera<PtzOpticsG2, TransportHandle<R>, R>, Error>
where
    R: Runtime,
{
    let dyn_camera = Connect::open_tcp_async::<PtzOpticsG2, _>("192.168.0.110", runtime)
        .await?
        .into_inner()
        .into_dyn();

    Ok(dyn_camera)
}

/// The same conversion spelled out, pinning each intermediate type.
async fn connect_then_into_dyn_step_by_step<R>(
    runtime: R,
) -> Result<Box<dyn DynCameraControl>, Error>
where
    R: Runtime,
{
    let session: CameraSession<Async, PtzOpticsG2, TransportHandle<R>, R> =
        Connect::open_udp_async::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
    let camera: Camera<Async, PtzOpticsG2, TransportHandle<R>, R> = session.into_inner();
    let dyn_camera: DynCamera<PtzOpticsG2, TransportHandle<R>, R> = camera.into_dyn();

    Ok(Box::new(dyn_camera))
}

fn main() {}
