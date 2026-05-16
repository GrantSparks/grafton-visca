use grafton_visca::{camera::CameraConfig, profiles::SonyFR7};

fn main() {
    let _ = CameraConfig::<SonyFR7>::serial("/dev/ttyUSB0", 9600);
}
