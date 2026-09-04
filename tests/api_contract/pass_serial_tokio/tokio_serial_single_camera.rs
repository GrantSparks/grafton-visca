#![allow(dead_code)]
#![cfg(feature = "transport-serial-tokio")]

//! Tokio serial construction retains the multi-target session path (#729).
//!
//! Async serial is Tokio-only, so both forms carry the same `RuntimeSerial`
//! bound and return a `Session`; callers select the desired address with
//! `camera_for`.

use grafton_visca::{
    camera::{CameraConfig, Connect},
    profiles::PtzOpticsG2,
    runtime::TokioRuntime,
    transport::TransportConfig,
    Error, Session,
};

async fn one_liner_serial_camera(runtime: TokioRuntime) -> Result<(), Error> {
    let session: Session =
        Connect::open_serial::<PtzOpticsG2, _>("/dev/ttyUSB0", 9600, runtime).await?;

    let camera = session.camera_for::<PtzOpticsG2>(grafton_visca::CameraId::CAMERA_1)?;
    let _target = camera.target();
    session.close().await
}

async fn configured_serial_camera(runtime: TokioRuntime) -> Result<(), Error> {
    let session: Session = CameraConfig::<PtzOpticsG2>::serial("/dev/ttyUSB0", 9600)
        .transport_config(TransportConfig::default())
        .open_serial_async(runtime)
        .await?;

    let camera = session.camera_for::<PtzOpticsG2>(grafton_visca::CameraId::CAMERA_1)?;
    let _target = camera.target();
    session.close().await
}

fn contract() {
    let _ = one_liner_serial_camera;
    let _ = configured_serial_camera;
}

fn main() {}
