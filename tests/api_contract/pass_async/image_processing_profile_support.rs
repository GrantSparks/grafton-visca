#![cfg(feature = "async")]

use grafton_visca::{
    command::NoiseReduction2DMode,
    profiles::{PtzOptics30X, PtzOpticsG2, PtzOpticsG3},
    types::{NoiseReduction2DLevel, NoiseReduction3DLevel},
    Camera,
};

fn image_surface<P>(camera: &Camera<P>)
where
    P: grafton_visca::CompileTimeProfile
        + grafton_visca::capabilities::HasImageProcessing
        + grafton_visca::capabilities::HasNoiseReduction2D
        + grafton_visca::capabilities::HasNoiseReduction3D
        + grafton_visca::capabilities::HasNoiseReduction2DControl
        + grafton_visca::capabilities::HasNoiseReduction3DControl,
{
    let _ = camera.image().freeze_on();
    let _ = camera.image().freeze_off();
    let _ = camera.image().defog_level();
    let _ = camera.image().noise_reduction_2d();
    let _ = camera.image().noise_reduction_2d_mode();
    let _ = camera.image().noise_reduction_3d();
    let _ = camera
        .image()
        .set_noise_reduction_2d_mode(NoiseReduction2DMode::Manual);
    let _ = camera
        .image()
        .set_noise_reduction_2d(NoiseReduction2DLevel::MAX);
    let _ = camera.image().disable_noise_reduction_2d();
    let _ = camera
        .image()
        .set_noise_reduction_3d(NoiseReduction3DLevel::MAX);
    let _ = camera.image().disable_noise_reduction_3d();
}

fn main() {
    let _: fn(&Camera<PtzOpticsG2>) = image_surface::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG3>) = image_surface::<PtzOpticsG3>;
    let _: fn(&Camera<PtzOptics30X>) = image_surface::<PtzOptics30X>;
}
