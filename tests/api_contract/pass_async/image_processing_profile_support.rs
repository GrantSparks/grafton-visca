#![cfg(feature = "async")]

use grafton_visca::{
    profiles::PtzOptics30x, profiles::PtzOpticsG2, profiles::PtzOpticsG3, r#async::Camera,
    types::NoiseReduction2DLevel, types::NoiseReduction2DMode, types::NoiseReduction3DLevel,
};

fn image_surface<P>(camera: &Camera<P>)
where
    P: grafton_visca::profiles::CompileTimeProfile
        + grafton_visca::capabilities::HasImage
        + grafton_visca::capabilities::HasNoiseReduction2D
        + grafton_visca::capabilities::HasNoiseReduction3D
        + grafton_visca::capabilities::HasNoiseReduction2D
        + grafton_visca::capabilities::HasNoiseReduction2DMode
        + grafton_visca::capabilities::HasNoiseReduction3D,
{
    let _ = camera.image().noise_reduction_2d();
    let _ = camera.image().noise_reduction_2d_mode();
    let _ = camera.image().noise_reduction_3d();
    let _ = camera
        .image()
        .set_noise_reduction_2d_mode(NoiseReduction2DMode::Manual);
    let _ = camera
        .image()
        .set_noise_reduction_2d(grafton_visca::types::NoiseReduction2D::Level(
            NoiseReduction2DLevel::MAX,
        ));
    let _ = camera
        .image()
        .set_noise_reduction_2d(grafton_visca::types::NoiseReduction2D::Off);
    let _ = camera
        .image()
        .set_noise_reduction_3d(grafton_visca::types::NoiseReduction3D::Level(
            NoiseReduction3DLevel::MAX,
        ));
    let _ = camera
        .image()
        .set_noise_reduction_3d(grafton_visca::types::NoiseReduction3D::Off);
}

fn main() {
    let _: fn(&Camera<PtzOpticsG2>) = image_surface::<PtzOpticsG2>;
    let _: fn(&Camera<PtzOpticsG3>) = image_surface::<PtzOpticsG3>;
    let _: fn(&Camera<PtzOptics30x>) = image_surface::<PtzOptics30x>;
}
