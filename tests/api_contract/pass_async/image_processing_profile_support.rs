#![cfg(feature = "async")]

use grafton_visca::{profiles::PtzOpticsG2, Camera};

fn image_surface(camera: &Camera<PtzOpticsG2>) {
    let _ = camera.image().freeze_on();
    let _ = camera.image().freeze_off();
    let _ = camera.image().defog_level();
}

fn main() {
    let _: fn(&Camera<PtzOpticsG2>) = image_surface;
}
