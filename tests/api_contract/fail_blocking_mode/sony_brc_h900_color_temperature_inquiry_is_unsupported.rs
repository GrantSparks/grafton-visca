#![cfg(feature = "blocking")]

use grafton_visca::{blocking::Camera, profiles::SonyBrcH900};

// The BRC-H900 has color-temperature controls, but no source documents its
// color-temperature reply layout, so the typed inquiry is not offered.
fn main() {
    let camera: Option<Camera<SonyBrcH900>> = None;
    let camera = camera.as_ref().unwrap();

    let _ = camera.white_balance().color_temperature_mode();
    let _ = camera.white_balance().color_temperature();
}

//~ E0277
//~ E0599
//~ "camera profile `grafton_visca::profiles::SonyBrcH900` does not support `grafton_visca::capabilities::gate::ColorTemperatureInquiry`"
//~ "no method named `color_temperature_mode`"
