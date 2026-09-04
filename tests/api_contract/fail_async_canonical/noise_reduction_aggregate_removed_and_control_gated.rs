#![cfg(feature = "async")]

use grafton_visca::{profiles::PtzOpticsG2, Camera};

fn aggregate_surface_is_still_removed(camera: &Camera<PtzOpticsG2>) {
    let _ = camera.image().noise_reduction_level();
    let _ = camera.image().noise_reduction_mode();
}

fn main() {}

//~ E0599
//~ "no method named `noise_reduction_level`"
//~ "no method named `noise_reduction_mode`"
