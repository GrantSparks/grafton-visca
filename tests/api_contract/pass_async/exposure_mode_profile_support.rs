#![cfg(feature = "async")]

use grafton_visca::{
    command::ExposureMode,
    profiles::{PtzOptics30X, PtzOpticsG2, PtzOpticsG3},
    types::IrisLevel,
    Camera,
};

fn ptzoptics_g2(camera: &Camera<PtzOpticsG2>) {
    let _ = camera.exposure().mode();
    let _ = camera.exposure().set_mode(ExposureMode::Auto);
}

fn ptzoptics_g3(camera: &Camera<PtzOpticsG3>) {
    let _ = camera.exposure().mode();
    let _ = camera.exposure().set_mode(ExposureMode::Auto);
    let _ = camera.exposure().iris();
    let _ = camera.exposure().iris_direct(IrisLevel::MIN);
}

fn ptzoptics_30x(camera: &Camera<PtzOptics30X>) {
    let _ = camera.exposure().mode();
    let _ = camera.exposure().set_mode(ExposureMode::Auto);
}

fn main() {
    let _: fn(&Camera<PtzOpticsG2>) = ptzoptics_g2;
    let _: fn(&Camera<PtzOpticsG3>) = ptzoptics_g3;
    let _: fn(&Camera<PtzOptics30X>) = ptzoptics_30x;
}
