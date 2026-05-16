use grafton_visca::{
    camera::{AsyncCamera, CameraConfig, Connect},
    profiles::PtzOpticsG2,
};

fn main() {
    let _ = core::any::type_name::<AsyncCamera<PtzOpticsG2, (), ()>>();
    let _ = Connect::open_tcp_async::<PtzOpticsG2, _>;
    let _ = CameraConfig::<PtzOpticsG2>::new().tcp().open_async;
}
