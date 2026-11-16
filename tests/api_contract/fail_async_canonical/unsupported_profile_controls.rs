use grafton_visca::{profiles::PtzOpticsG2, r#async::Camera};

fn main() {
    let camera: Option<Camera<PtzOpticsG2>> = None;
    let _ = camera.unwrap().nd_filter();
}

//~ E0277
//~ "camera profile `grafton_visca::profiles::PtzOpticsG2` does not support `grafton_visca::capabilities::gate::NdFilter`"
