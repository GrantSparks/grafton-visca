#![cfg(feature = "blocking")]

use grafton_visca::{blocking::Camera, command::NoiseReduction2DMode, profiles::SonyFR7};

fn sony_fr7_cannot_set_2d_noise_reduction_mode(camera: &Camera<'_, SonyFR7>) {
    let _ = camera
        .image()
        .set_noise_reduction_2d_mode(NoiseReduction2DMode::Manual);
}

fn main() {}

//~ E0277
//~ "profile `SonyFR7` does not declare 2D noise-reduction control support"
