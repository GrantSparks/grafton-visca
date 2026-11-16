use grafton_visca::{profiles::GenericVisca, types::CameraId};

#[cfg(feature = "blocking")]
fn select(session: &grafton_visca::blocking::Session) {
    let _ = session.camera::<GenericVisca>(CameraId::BROADCAST);
}

#[cfg(all(feature = "async", not(feature = "blocking")))]
fn select(session: &grafton_visca::r#async::Session) {
    let _ = session.camera::<GenericVisca>(CameraId::BROADCAST);
}

fn main() {}

//~ E0308
//~ "expected `IndividualCameraId`, found `CameraId`"
