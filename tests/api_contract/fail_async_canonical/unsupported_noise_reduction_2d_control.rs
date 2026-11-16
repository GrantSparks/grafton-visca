#![cfg(feature = "async")]

use grafton_visca::{profiles::SonyFr7, r#async::Camera, types::NoiseReduction2DLevel};

fn sony_fr7_cannot_set_2d_noise_reduction(camera: &Camera<SonyFr7>) {
    let _ = camera
        .image()
        .set_noise_reduction_2d(grafton_visca::types::NoiseReduction2D::Level(
            NoiseReduction2DLevel::MAX,
        ));
}

fn main() {}

//~ E0277
//~ "camera profile `grafton_visca::profiles::SonyFr7` does not support `grafton_visca::capabilities::gate::NoiseReduction2D`"
