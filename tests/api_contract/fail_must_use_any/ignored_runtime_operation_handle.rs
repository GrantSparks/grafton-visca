#![deny(unused_must_use)]

use grafton_visca::{profiles::AnyProfile, r#async::Camera};

async fn ignore_runtime_operation_handle(camera: &Camera<AnyProfile>) {
    camera
        .pan_tilt()
        .home()
        .await
        .expect("runtime-profile operation submission");
}

fn main() {
    let _ = ignore_runtime_operation_handle;
}

//~ "Operation` that must be used"
