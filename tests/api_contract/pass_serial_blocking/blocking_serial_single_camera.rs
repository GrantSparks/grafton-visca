#![cfg(all(feature = "blocking", feature = "transport-serial"))]

//! The blocking serial single-camera constructors name the profile once (#650).
//!
//! Both the one-liner and the configured form return a `CameraSession<P>`
//! bound at compile time, so the camera view needs no second turbofish and no
//! fallible projection.

use grafton_visca::{
    blocking::{CameraConfig, CameraSession, Connect},
    profiles::PtzOpticsG2,
    transport::TransportConfig,
    Error,
};

fn one_liner_serial_camera() -> Result<(), Error> {
    let session: CameraSession<PtzOpticsG2> =
        Connect::open_serial_camera::<PtzOpticsG2>("/dev/ttyUSB0", 9600)?;

    let camera = session.camera();
    let _target = camera.target();
    session.close()
}

fn configured_serial_camera() -> Result<(), Error> {
    let session: CameraSession<PtzOpticsG2> =
        CameraConfig::<PtzOpticsG2>::serial("/dev/ttyUSB0", 9600)
            .transport_config(TransportConfig::default())
            .open_serial_camera()?;

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
