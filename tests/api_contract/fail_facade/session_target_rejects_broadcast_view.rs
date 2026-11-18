use grafton_visca::{profiles::GenericVisca, types::CameraId};

#[cfg(feature = "blocking")]
use grafton_visca::blocking::Session;
#[cfg(all(feature = "async", not(feature = "blocking")))]
use grafton_visca::r#async::Session;

fn select(session: &Session) {
    let _ = session.camera::<GenericVisca>(CameraId::BROADCAST);
    //~^ E0308 "mismatched types"
}

fn main() {}
