use grafton_visca::{config::CameraConfig, profiles::PtzOpticsG2, types::CameraId};

fn main() {
    let _ = CameraConfig::<PtzOpticsG2>::tcp("camera.local")
        .with_camera_id(CameraId::BROADCAST);
}

//~ E0308
//~ "expected `IndividualCameraId`, found `CameraId`"
