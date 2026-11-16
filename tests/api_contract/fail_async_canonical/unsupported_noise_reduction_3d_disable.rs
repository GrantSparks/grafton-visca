#![cfg(feature = "async")]

use grafton_visca::{profiles::SonyFr7, r#async::Camera};

fn sony_fr7_cannot_disable_3d_noise_reduction(camera: &Camera<SonyFr7>) {
    let _ = camera
        .image()
        .set_noise_reduction_3d(grafton_visca::types::NoiseReduction3D::Off);
}

fn main() {}

//~ E0277
//~ "camera profile `grafton_visca::profiles::SonyFr7` does not support `grafton_visca::capabilities::gate::NoiseReduction3D`"
