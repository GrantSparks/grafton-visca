use grafton_visca::prelude::blocking::*;

fn main() {
    let _: Option<CameraSession<PtzOpticsG2>> = None;
    let _ = std::mem::size_of::<MetricsSnapshot>();
    let _ = std::mem::size_of::<SessionStatus>();
    let _ = std::mem::size_of::<MotionSyncMode>();
    let _ = PowerStatus::On;
}
