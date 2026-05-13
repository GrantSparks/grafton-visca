#![allow(dead_code)]

use grafton_visca::{
    camera::CameraConfig, profiles::PtzOpticsG2, runtime::TokioRuntime, transport::TransportConfig,
    Error,
};

async fn tokio_serial_contract(runtime: TokioRuntime) -> Result<(), Error> {
    let camera = CameraConfig::<PtzOpticsG2>::new()
        .serial("/dev/ttyUSB0", 9600)
        .transport_config(TransportConfig::default())
        .open_serial_async(runtime)
        .await?;

    let _ = camera.close().await?;

    Ok(())
}

fn main() {
    let _ = tokio_serial_contract;
}
