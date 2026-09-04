#![cfg(all(feature = "blocking", feature = "transport-serial"))]

//! Blocking serial construction retains the multi-target session path (#729).
//!
//! Both the one-liner and configured form return a `Session`; callers select
//! the desired address with `camera_for`.

use grafton_visca::{
    blocking::{CameraConfig, Connect, Session},
    profiles::PtzOpticsG2,
    transport::TransportConfig,
    Error,
};

fn one_liner_serial_camera() -> Result<(), Error> {
    let session: Session = Connect::open_serial::<PtzOpticsG2>("/dev/ttyUSB0", 9600)?;

    let camera = session.camera_for::<PtzOpticsG2>(grafton_visca::CameraId::CAMERA_1)?;
    let _target = camera.target();
    session.close()
}

fn configured_serial_camera() -> Result<(), Error> {
    let session: Session = CameraConfig::<PtzOpticsG2>::serial("/dev/ttyUSB0", 9600)
        .transport_config(TransportConfig::default())
        .open_serial()?;

    let camera = session.camera_for::<PtzOpticsG2>(grafton_visca::CameraId::CAMERA_1)?;
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
