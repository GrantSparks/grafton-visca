#![cfg(feature = "blocking")]

//! The single-camera constructor names the profile once: the camera view it
//! hands out is already bound to that profile, with no second turbofish and no
//! fallible projection.

use grafton_visca::{
    blocking::{Camera, CameraSession},
    capabilities::PanTilt,
    profiles::PtzOpticsG2,
    CompileTimeProfile,
};

fn bound_view<P>(session: &CameraSession<P>) -> Camera<'_, P>
where
    P: CompileTimeProfile + PanTilt,
{
    let camera = session.camera();
    let _target = camera.target();
    camera
}

fn main() {
    let _: fn(&CameraSession<PtzOpticsG2>) -> Camera<'_, PtzOpticsG2> = bound_view::<PtzOpticsG2>;
}
