#![cfg(feature = "blocking")]

use grafton_visca::{blocking::Camera, profiles::SonyFr7};

fn main() {
    let camera: Option<Camera<SonyFr7>> = None;
    let camera = camera.as_ref().unwrap();

    let _ = camera.exposure().mode();
}

//~ E0277
//~ "camera profile `grafton_visca::profiles::SonyFr7` does not support `grafton_visca::capabilities::gate::ExposureMode`"
