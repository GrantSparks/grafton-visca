#![deny(unused_must_use)]

use grafton_visca::dynapi::DynPanTilt;

async fn ignore_dyn_operation_handle(pan_tilt: &dyn DynPanTilt) {
    pan_tilt.home().await.expect("dyn operation submission");
}

fn main() {
    let _ = ignore_dyn_operation_handle;
}
