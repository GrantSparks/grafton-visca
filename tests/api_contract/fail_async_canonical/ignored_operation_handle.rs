#![deny(unused_must_use)]

use grafton_visca::{profiles::PtzOpticsG2, request::builtin::PanTiltHome, Camera};

async fn ignore_operation_handle(camera: &Camera<PtzOpticsG2>) {
    camera.submit(&PanTiltHome).await.expect("submission");
}

fn main() {
    let _ = ignore_operation_handle;
}

// This fixture submitted `command::PanTilt::Home` — a wire-value enum, not a
// typed request — so it was rejected by `submit`'s `OperationCommand` bound
// (E0277) and never reached the `#[must_use]` lint it exists to pin. The
// submission now type-checks, and the dropped handle is the only error.

//~ "unused `grafton_visca::Operation` that must be used"
//~ "await, cancel, or explicitly detach this operation"
