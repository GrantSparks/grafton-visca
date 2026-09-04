#![cfg(all(feature = "blocking", feature = "transport-serial"))]

use grafton_visca::{
    blocking::Session, camera::CameraConfig, profiles::PtzOpticsG2, transport::TransportConfig,
    Error,
};

fn serial_blocking_contract() -> Result<(), Error> {
    let camera = CameraConfig::<PtzOpticsG2>::serial("/dev/ttyUSB0", 9600)
        .transport_config(TransportConfig::default())
        .open_serial()?;

    let _: Session = camera;
    Ok(())
}

fn contract() {
    let _ = serial_blocking_contract;
}

fn main() {}
