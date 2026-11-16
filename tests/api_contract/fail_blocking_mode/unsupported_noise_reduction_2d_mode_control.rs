#![cfg(feature = "blocking")]

use grafton_visca::{blocking::Camera, profiles::SonyEviH100, types::NoiseReduction2DMode};

/// The EVI-H100 has the R8 `04 53` 2D level control but no `04 50` mode.
fn sony_evi_h100_cannot_set_2d_noise_reduction_mode(camera: &Camera<SonyEviH100>) {
    let _ = camera
        .image()
        .set_noise_reduction_2d_mode(NoiseReduction2DMode::Manual);
}

fn main() {}

//~ E0277
//~ "camera profile `grafton_visca::profiles::SonyEviH100` does not support `grafton_visca::capabilities::gate::NoiseReduction2DMode`"
