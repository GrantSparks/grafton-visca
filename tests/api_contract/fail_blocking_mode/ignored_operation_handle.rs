#![deny(unused_must_use)]

use grafton_visca::{blocking::Camera, command::PanTilt, profiles::PtzOpticsG2};

fn ignore_operation_handle(camera: &Camera<'static, PtzOpticsG2>) {
    camera.submit(&PanTilt::Home).expect("submission");
}

fn main() {
    let _ = ignore_operation_handle;
}
