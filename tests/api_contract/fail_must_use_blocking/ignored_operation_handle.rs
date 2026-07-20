#![deny(unused_must_use)]

use grafton_visca::{
    command::PanTilt, profiles::PtzOpticsG2, transport::BlockingTransportHandle, BlockingCamera,
};

fn ignore_operation_handle(camera: &BlockingCamera<PtzOpticsG2, BlockingTransportHandle>) {
    camera.submit(&PanTilt::Home).expect("submission");
}

fn main() {
    let _ = ignore_operation_handle;
}
