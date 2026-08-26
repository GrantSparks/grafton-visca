#![deny(unused_must_use)]

use grafton_visca::{command::PanTilt, profiles::PtzOpticsG2, Camera};

async fn ignore_operation_handle(camera: &Camera<PtzOpticsG2>) {
    camera.submit(&PanTilt::Home).await.expect("submission");
}

fn main() {
    let _ = ignore_operation_handle;
}
