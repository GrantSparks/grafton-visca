#![cfg(feature = "async")]

use grafton_visca::{
    profiles::SonyBrc300, r#async::Camera, types::UnitInterval, types::ZoomDomain,
};

fn optical_normalized_zoom(camera: &Camera<SonyBrc300>) {
    let _ = camera
        .zoom()
        .set_normalized(UnitInterval::MIN, ZoomDomain::Optical);
}

fn main() {
    let _: fn(&Camera<SonyBrc300>) = optical_normalized_zoom;
}
