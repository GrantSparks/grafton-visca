#![deny(unused_must_use)]

use grafton_visca::dynapi::DynPanTiltControl;

async fn ignore_dyn_operation_handle(pan_tilt: &dyn DynPanTiltControl) {
    pan_tilt
        .pan_tilt_home_op()
        .await
        .expect("dyn operation submission");
}

fn main() {
    let _ = ignore_dyn_operation_handle;
}
