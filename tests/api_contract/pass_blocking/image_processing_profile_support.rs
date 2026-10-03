#![cfg(feature = "blocking")]

use grafton_visca::{blocking::Camera, profiles::PtzOpticsG2};

fn image_surface(camera: &Camera<PtzOpticsG2>) {
    let _ = camera.image().contrast();
}

fn main() {
    let _: fn(&Camera<PtzOpticsG2>) = image_surface;
}
