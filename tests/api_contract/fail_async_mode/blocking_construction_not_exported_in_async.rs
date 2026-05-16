use grafton_visca::{
    camera::{CameraConfig, Connect},
    profiles::PtzOpticsG2,
};

fn main() {
    let _ = Connect::open_tcp_blocking::<PtzOpticsG2>("127.0.0.1");

    let config = CameraConfig::<PtzOpticsG2>::new().tcp();
    let _ = config.open_blocking();
}
