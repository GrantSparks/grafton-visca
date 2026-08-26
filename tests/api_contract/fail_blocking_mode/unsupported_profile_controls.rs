use grafton_visca::{blocking::Camera, profiles::PtzOpticsG2};

fn main() {
    let camera: Option<Camera<'static, PtzOpticsG2>> = None;
    let _ = camera.unwrap().nd_filter();
}
