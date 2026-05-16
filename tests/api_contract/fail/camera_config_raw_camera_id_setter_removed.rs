use grafton_visca::{camera::CameraConfig, profiles::PtzOpticsG2};

fn main() {
    let _ = CameraConfig::<PtzOpticsG2>::tcp("127.0.0.1").camera_id(1);
}
