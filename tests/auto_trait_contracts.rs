//! Compile-time contracts for public auto traits promised by the 1.x API.
//!
//! These bounds are observable by downstream generic code even though Rustdoc
//! does not list auto-trait implementations alongside ordinary methods.

#[cfg(not(feature = "mode-async"))]
#[test]
fn blocking_camera_surface_remains_unwind_safe() {
    use std::panic::UnwindSafe;

    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, BlockingClient, Camera, CameraSession},
        mode::Blocking,
    };

    struct Transport;
    fn assert_unwind_safe<T: UnwindSafe>() {}

    assert_unwind_safe::<Camera<Blocking, PtzOpticsG2, Transport>>();
    assert_unwind_safe::<BlockingClient<PtzOpticsG2, Transport>>();
    assert_unwind_safe::<CameraSession<Blocking, PtzOpticsG2, Transport>>();
}

#[cfg(feature = "runtime-tokio")]
#[test]
fn async_operation_handle_remains_unwind_safe() {
    use std::panic::{RefUnwindSafe, UnwindSafe};

    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, InFlight},
        TokioExecutor,
    };

    type Handle = InFlight<'static, (), PtzOpticsG2, TokioExecutor>;
    fn assert_unwind_safe<T: UnwindSafe>() {}
    fn assert_ref_unwind_safe<T: RefUnwindSafe>() {}

    assert_unwind_safe::<Handle>();
    assert_ref_unwind_safe::<Handle>();
}
