#![cfg(feature = "async")]

use grafton_visca::{profiles::SonyFR7, types::NoiseReduction3DLevel, Camera};

fn sony_fr7_cannot_set_3d_noise_reduction(camera: &Camera<SonyFR7>) {
    let _ = camera
        .image()
        .set_noise_reduction_3d(NoiseReduction3DLevel::MAX);
}

fn main() {}

//~ E0277
//~ "profile `SonyFR7` does not declare 3D noise-reduction control support"
