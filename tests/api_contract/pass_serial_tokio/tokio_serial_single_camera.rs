#![allow(dead_code)]
#![cfg(feature = "transport-serial-tokio")]

//! The Tokio serial single-camera constructors name the profile once (#650).
//!
//! Async serial is Tokio-only, so both forms carry the same `RuntimeSerial`
//! bound as the owner-backed `open_serial` path; the returned
//! `CameraSession<P>` is bound at compile time.

use grafton_visca::{
    camera::{CameraConfig, Connect},
    profiles::PtzOpticsG2,
    runtime::TokioRuntime,
    transport::TransportConfig,
    CameraSession, Error,
};

async fn one_liner_serial_camera(runtime: TokioRuntime) -> Result<(), Error> {
    let session: CameraSession<PtzOpticsG2> =
        Connect::open_serial_camera::<PtzOpticsG2, _>("/dev/ttyUSB0", 9600, runtime).await?;

    let camera = session.camera();
    let _target = camera.target();
    session.close().await
}

async fn configured_serial_camera(runtime: TokioRuntime) -> Result<(), Error> {
    let session: CameraSession<PtzOpticsG2> =
        CameraConfig::<PtzOpticsG2>::serial("/dev/ttyUSB0", 9600)
            .transport_config(TransportConfig::default())
            .open_serial_camera_async(runtime)
            .await?;

    let camera = session.camera();
    let _target = camera.target();
    session.close().await
}

fn contract() {
    let _ = one_liner_serial_camera;
    let _ = configured_serial_camera;
}

fn main() {}
