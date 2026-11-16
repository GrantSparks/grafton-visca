#![allow(dead_code)]
#![cfg(feature = "transport-serial-tokio")]

//! Tokio serial construction has the same single-camera bind as TCP/UDP.
//!
//! Async serial is Tokio-only. Both forms retain the profile parameter and
//! return `CameraSession<P>` with an already-bound camera view.

use grafton_visca::{
    config::CameraConfig, profiles::PtzOpticsG2, r#async::CameraSession, runtime::TokioRuntime,
    transport::TransportConfig, Error,
};

async fn one_liner_serial_camera(runtime: TokioRuntime) -> Result<(), Error> {
    let session: CameraSession<PtzOpticsG2> =
        grafton_visca::r#async::CameraSession::<PtzOpticsG2>::connect_serial(
            "/dev/ttyUSB0",
            9600,
            runtime,
        )
        .await?;

    let camera = session.camera();
    let _target = camera.target();
    session.close().await
}

async fn configured_serial_camera(runtime: TokioRuntime) -> Result<(), Error> {
    let config = CameraConfig::<PtzOpticsG2>::serial("/dev/ttyUSB0", 9600)
        .with_transport_config(TransportConfig::for_serial());
    let session: CameraSession<PtzOpticsG2> = CameraSession::connect(&config, runtime).await?;

    let camera = session.camera();
    let _target = camera.target();
    session.close().await
}

fn contract() {
    let _ = one_liner_serial_camera;
    let _ = configured_serial_camera;
}

fn main() {}
