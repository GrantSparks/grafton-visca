#![cfg(feature = "async")]

use grafton_visca::{profiles::PtzOpticsG3, Camera};

fn main() {
    let camera: Option<Camera<PtzOpticsG3>> = None;
    let camera = camera.as_ref().unwrap();

    let _ = camera.exposure().iris_control();
}

//~ E0277
//~ "profile `grafton_visca::profiles::PtzOpticsG3` does not declare iris control-status inquiry support"
