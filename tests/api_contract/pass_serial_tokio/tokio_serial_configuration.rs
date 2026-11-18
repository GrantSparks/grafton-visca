#![allow(dead_code)]
#![cfg(feature = "transport-serial-tokio")]

use grafton_visca::{
    config::CameraConfig, profiles::PtzOpticsG2, r#async::CameraSession, runtime::TokioRuntime,
    Error,
};

async fn tokio_serial_contract(runtime: TokioRuntime) -> Result<(), Error> {
    let config = CameraConfig::<PtzOpticsG2>::serial("/dev/ttyUSB0", 9600);
    let camera: CameraSession<PtzOpticsG2> = CameraSession::connect(&config, runtime).await?;

    let _ = camera.close().await?;

    Ok(())
}

fn contract() {
    let _ = tokio_serial_contract;
}

fn main() {}
