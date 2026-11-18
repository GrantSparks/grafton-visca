#![cfg(all(feature = "blocking", feature = "transport-serial"))]

use grafton_visca::{
    blocking::CameraSession, config::CameraConfig, profiles::PtzOpticsG2,
    Error,
};

fn serial_blocking_contract() -> Result<(), Error> {
    let config = CameraConfig::<PtzOpticsG2>::serial("/dev/ttyUSB0", 9600);
    let camera: CameraSession<PtzOpticsG2> = CameraSession::connect(&config)?;

    let _: CameraSession<PtzOpticsG2> = camera;
    Ok(())
}

fn contract() {
    let _ = serial_blocking_contract;
}

fn main() {}
