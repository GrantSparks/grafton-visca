use grafton_visca::prelude::blocking::*;

// Observability data has one explicit home outside the noun-signature prelude.
use grafton_visca::observability::{MetricsSnapshot, SessionStatus};

fn main() {
    let _: Option<CameraSession<PtzOpticsG2>> = None;
    let _ = std::mem::size_of::<MetricsSnapshot>();
    let _ = std::mem::size_of::<SessionStatus>();
    let _ = std::mem::size_of::<bool>();
    let _ = PowerStatus::On;
}

fn individual_target_round_trip(camera: &Camera) {
    let target: IndividualCameraId = camera.target();
    let _: IndividualCameraId = camera.state_cache().target();
    let _: IndividualCameraId = target;
}
