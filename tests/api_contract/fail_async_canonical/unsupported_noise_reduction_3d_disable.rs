#![cfg(feature = "async")]

use grafton_visca::{profiles::SonyFR7, Camera};

fn sony_fr7_cannot_disable_3d_noise_reduction(camera: &Camera<SonyFR7>) {
    let _ = camera.image().disable_noise_reduction_3d();
}

fn main() {}

//~ E0277
//~ "profile `SonyFR7` does not declare 3D noise-reduction control support"
