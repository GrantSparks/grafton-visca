#![cfg(feature = "async")]

use grafton_visca::{profiles::SonyBRC300, units::UnitInterval, Camera, ZoomDomain};

fn optical_normalized_zoom(camera: &Camera<SonyBRC300>) {
    let _ = camera
        .zoom()
        .set_normalized_in_domain(UnitInterval::ZERO, ZoomDomain::Optical);
}

fn main() {
    let _: fn(&Camera<SonyBRC300>) = optical_normalized_zoom;
}
