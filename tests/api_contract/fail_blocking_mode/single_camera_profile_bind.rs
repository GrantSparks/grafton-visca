// The session path admits a second, runtime-checked profile naming:
// `Connect::open_tcp::<PtzOpticsG2>(..)` followed by `session.camera::<PtzOpticsG3>()`
// compiles and fails at run time. The single-camera constructors name the
// profile once and bind it at compile time, so neither the constructor's
// result nor the camera view it hands out can change profile.
//
// Where the compile-time bind ends: `CameraSession::session()` hands back the
// profile-generic `&Session` the value owns, and `Session::camera::<P>()` is
// the runtime-checked projection. So
// `camera_session.session().camera::<PtzOpticsG3>()` *does* compile against a
// `CameraSession<PtzOpticsG2>` and returns `Err` at run time instead. That
// escape hatch is deliberate — it is how a caller reaches the generic session
// API from a single-camera handle — and it is exactly why the two functions
// below have to be compile-fail: the bound accessors, the ones a caller
// reaches without going through `session()`, must not offer the same
// substitution.

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

//~ E0308
//~ "found `Result<CameraSession<PtzOpticsG2>, ...>`"
//~ "expected `Camera<'_, PtzOpticsG3>`, found `Camera<'_, PtzOpticsG2>`"
