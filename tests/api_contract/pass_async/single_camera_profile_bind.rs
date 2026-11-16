#![cfg(feature = "async")]

//! The single-camera constructor names the profile once: the camera it owns is
//! already bound to that profile, with no second turbofish and no fallible
//! projection.

use grafton_visca::{
    capabilities::HasPanTilt, profiles::CompileTimeProfile, profiles::PtzOpticsG2, r#async::Camera,
    r#async::CameraSession,
};

fn bound_view<P>(session: &CameraSession<P>) -> Camera<P>
where
    P: CompileTimeProfile + HasPanTilt,
{
    let camera = session.camera();
    let _target = camera.target();
    camera.clone()
}

fn owned_camera<P>(session: CameraSession<P>) -> Camera<P>
where
    P: CompileTimeProfile + HasPanTilt,
{
    session.into_camera()
}

fn main() {
    let _: fn(&CameraSession<PtzOpticsG2>) -> Camera<PtzOpticsG2> = bound_view::<PtzOpticsG2>;
    let _: fn(CameraSession<PtzOpticsG2>) -> Camera<PtzOpticsG2> = owned_camera::<PtzOpticsG2>;
}
