#![cfg(feature = "async")]

use grafton_visca::{profiles::SonyFR7, types::NoiseReduction2DLevel, Camera};

fn sony_fr7_cannot_set_2d_noise_reduction(camera: &Camera<SonyFR7>) {
    let _ = camera
        .image()
        .set_noise_reduction_2d(NoiseReduction2DLevel::MAX);
}

fn main() {}

//~ E0277
//~ "profile `SonyFR7` does not declare 2D noise-reduction control support"
