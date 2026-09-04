#![cfg(feature = "async")]

use grafton_visca::{profiles::SonyFR7, Camera};

fn main() {
    let camera: Option<Camera<SonyFR7>> = None;
    let camera = camera.as_ref().unwrap();

    let _ = camera.exposure().mode();
}

//~ E0277
//~ "profile `SonyFR7` does not declare shared exposure-mode support"
