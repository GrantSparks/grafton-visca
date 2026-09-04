#![cfg(feature = "async")]

use grafton_visca::{command::ExposureMode, profiles::SonyFR7, Camera};

fn main() {
    let camera: Option<Camera<SonyFR7>> = None;
    let camera = camera.as_ref().unwrap();

    let _ = camera.exposure().set_mode(ExposureMode::Auto);
}

//~ E0277
//~ "profile `SonyFR7` does not declare shared exposure-mode support"
