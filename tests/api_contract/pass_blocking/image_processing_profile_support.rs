#![cfg(feature = "blocking")]

use grafton_visca::{blocking::Camera, profiles::PtzOpticsG2};

fn image_surface<'session>(camera: &Camera<'session, PtzOpticsG2>) {
    let _ = camera.image().contrast();
}

fn main() {
    let _: for<'session> fn(&'session Camera<'session, PtzOpticsG2>) = image_surface;
}
