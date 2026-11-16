#![cfg(all(feature = "blocking", feature = "transport-serial"))]

//! Blocking serial construction has the same single-camera bind as TCP/UDP.
//!
//! Both forms return `CameraSession<P>` and its already-bound camera view.

use grafton_visca::{
    blocking::CameraSession, config::CameraConfig, profiles::PtzOpticsG2,
    transport::TransportConfig, Error,
};

fn one_liner_serial_camera() -> Result<(), Error> {
    let session: CameraSession<PtzOpticsG2> =
        grafton_visca::blocking::CameraSession::<PtzOpticsG2>::connect_serial(
            "/dev/ttyUSB0",
            9600,
        )?;

    let camera = session.camera();
    let _target = camera.target();
    session.close()
}

fn configured_serial_camera() -> Result<(), Error> {
    let config = CameraConfig::<PtzOpticsG2>::serial("/dev/ttyUSB0", 9600)
        .with_transport_config(TransportConfig::for_serial());
    let session: CameraSession<PtzOpticsG2> = CameraSession::connect(&config)?;

    let camera = session.camera();
    let _target = camera.target();
    session.close()
}

fn contract() {
    let _ = one_liner_serial_camera;
    let _ = configured_serial_camera;
}

fn main() {
    let _ = contract;
}
