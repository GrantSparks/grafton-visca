#![cfg(feature = "blocking")]

use grafton_visca::{
    blocking::Camera, profiles::SonyBrc300, types::UnitInterval, types::ZoomDomain,
};

fn optical_normalized_zoom(camera: &Camera<SonyBrc300>) {
    let _ = camera
        .zoom()
        .set_normalized(UnitInterval::MIN, ZoomDomain::Optical);
}

fn main() {
    let _: fn(&Camera<SonyBrc300>) = optical_normalized_zoom;
}
