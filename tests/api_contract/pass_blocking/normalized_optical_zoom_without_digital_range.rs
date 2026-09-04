#![cfg(feature = "blocking")]

use grafton_visca::{blocking::Camera, profiles::SonyBRC300, units::UnitInterval, ZoomDomain};

fn optical_normalized_zoom<'session>(camera: &Camera<'session, SonyBRC300>) {
    let _ = camera
        .zoom()
        .set_normalized_in_domain(UnitInterval::ZERO, ZoomDomain::Optical);
}

fn main() {
    let _: for<'session> fn(&'session Camera<'session, SonyBRC300>) = optical_normalized_zoom;
}
