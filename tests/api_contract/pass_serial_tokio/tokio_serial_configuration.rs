#![allow(dead_code)]
#![cfg(feature = "transport-serial-tokio")]

use grafton_visca::{
    camera::CameraConfig, profiles::PtzOpticsG2, runtime::TokioRuntime, transport::TransportConfig,
    Error,
};

async fn tokio_serial_contract(runtime: TokioRuntime) -> Result<(), Error> {
    let camera = CameraConfig::<PtzOpticsG2>::serial("/dev/ttyUSB0", 9600)
        .transport_config(TransportConfig::default())
        .open_serial_async(runtime)
        .await?;

    let _ = camera.close().await?;

    Ok(())
}

fn contract() {
    let _ = tokio_serial_contract;
}

fn main() {}
