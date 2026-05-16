use grafton_visca::{camera::CameraConfig, profiles::SonyFR7};

fn main() {
    let _ = CameraConfig::<SonyFR7>::tcp("127.0.0.1");
}
