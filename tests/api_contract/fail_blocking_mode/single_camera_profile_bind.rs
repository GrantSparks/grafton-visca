// The session path admits a second, runtime-checked profile naming:
// `Connect::open_tcp::<PtzOpticsG2>(..)` followed by `session.camera::<PtzOpticsG3>()`
// compiles and fails at run time. The single-camera constructors name the
// profile once and bind it at compile time, so neither the constructor's
// result nor the camera view it hands out can change profile.

use grafton_visca::{
    blocking::{Camera, CameraSession, Connect},
    profiles::{PtzOpticsG2, PtzOpticsG3},
};

fn constructor_result_cannot_change_profile() -> grafton_visca::Result<CameraSession<PtzOpticsG3>> {
    Connect::open_tcp_camera::<PtzOpticsG2>("192.168.0.110")
}

fn view_cannot_change_profile(session: &CameraSession<PtzOpticsG2>) -> Camera<'_, PtzOpticsG3> {
    session.camera()
}

fn main() {
    let _ = constructor_result_cannot_change_profile;
    let _ = view_cannot_change_profile;
}
