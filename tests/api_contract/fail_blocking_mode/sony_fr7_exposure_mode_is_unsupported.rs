#![cfg(feature = "blocking")]

use grafton_visca::{blocking::Camera, profiles::SonyFR7};

fn main() {
    let camera: Option<Camera<'static, SonyFR7>> = None;
    let camera = camera.as_ref().unwrap();

    let _ = camera.exposure().mode();
}

//~ E0277
//~ "profile `SonyFR7` does not declare shared exposure-mode support"
