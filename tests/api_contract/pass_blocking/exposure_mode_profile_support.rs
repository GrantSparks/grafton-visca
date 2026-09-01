#![cfg(feature = "blocking")]

use grafton_visca::{
    blocking::Camera,
    command::ExposureMode,
    profiles::{PtzOptics30X, PtzOpticsG2, PtzOpticsG3},
    types::IrisLevel,
};

fn ptzoptics_g2<'session>(camera: &Camera<'session, PtzOpticsG2>) {
    let _ = camera.exposure().mode();
    let _ = camera.exposure().set_mode(ExposureMode::Auto);
}

fn ptzoptics_g3<'session>(camera: &Camera<'session, PtzOpticsG3>) {
    let _ = camera.exposure().mode();
    let _ = camera.exposure().set_mode(ExposureMode::Auto);
    let _ = camera.exposure().iris();
    let _ = camera.exposure().iris_direct(IrisLevel::MIN);
}

fn ptzoptics_30x<'session>(camera: &Camera<'session, PtzOptics30X>) {
    let _ = camera.exposure().mode();
    let _ = camera.exposure().set_mode(ExposureMode::Auto);
}

fn main() {
    let _: for<'session> fn(&'session Camera<'session, PtzOpticsG2>) = ptzoptics_g2;
    let _: for<'session> fn(&'session Camera<'session, PtzOpticsG3>) = ptzoptics_g3;
    let _: for<'session> fn(&'session Camera<'session, PtzOptics30X>) = ptzoptics_30x;
}
