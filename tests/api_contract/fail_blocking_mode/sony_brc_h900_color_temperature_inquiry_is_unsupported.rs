#![cfg(feature = "blocking")]

use grafton_visca::{blocking::Camera, profiles::SonyBRCH900};

// The BRC-H900 has color-temperature controls, but no source documents its
// color-temperature reply layout, so the typed inquiry is not offered.
fn main() {
    let camera: Option<Camera<SonyBRCH900>> = None;
    let camera = camera.as_ref().unwrap();

    let _ = camera.white_balance().color_temperature_mode();
    let _ = camera.white_balance().color_temperature();
}

//~ E0277
//~ "profile `SonyBRCH900` does not declare a sourced color-temperature reply"
