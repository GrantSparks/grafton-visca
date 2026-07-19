#![deny(unused_must_use)]

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, AsyncCamera},
    command::PanTilt,
    runtime::{TokioRuntime, TransportHandle},
};

async fn ignore_operation_handle(
    camera: &AsyncCamera<PtzOpticsG2, TransportHandle<TokioRuntime>, TokioRuntime>,
) {
    camera.submit(&PanTilt::Home).await.expect("submission");
}

fn main() {
    let _ = ignore_operation_handle;
}
