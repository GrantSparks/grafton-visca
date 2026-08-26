use grafton_visca::{profiles::PtzOpticsG2, Camera};

fn main() {
    let camera: Option<Camera<PtzOpticsG2>> = None;
    let _ = camera.unwrap().nd_filter();
}
