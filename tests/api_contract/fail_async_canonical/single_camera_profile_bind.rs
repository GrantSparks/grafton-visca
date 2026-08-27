// The session path admits a second, runtime-checked profile naming:
// `Connect::open_tcp::<PtzOpticsG2, _>(..)` followed by
// `session.camera::<PtzOpticsG3>()` compiles and fails at run time. The
// single-camera constructors name the profile once and bind it at compile
// time, so neither the camera the session owns nor the camera it gives up can
// change profile.

use grafton_visca::{
    profiles::{PtzOpticsG2, PtzOpticsG3},
    Camera, CameraSession,
};

fn view_cannot_change_profile(session: &CameraSession<PtzOpticsG2>) -> &Camera<PtzOpticsG3> {
    session.camera()
}

fn owned_camera_cannot_change_profile(session: CameraSession<PtzOpticsG2>) -> Camera<PtzOpticsG3> {
    session.into_camera()
}

fn main() {
    let _ = view_cannot_change_profile;
    let _ = owned_camera_cannot_change_profile;
}
