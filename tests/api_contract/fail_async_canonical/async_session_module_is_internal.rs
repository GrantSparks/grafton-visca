// `Session`, `Camera` and `CameraSession` are root exports; the module they
// are declared in is private. This replaces the 1.x `camera::session`
// fixture, whose module does not exist in 2.0.

fn main() {
    let _ = core::any::TypeId::of::<grafton_visca::async_session::Session>();
}

//~ E0603
//~ "module `async_session` is private"
