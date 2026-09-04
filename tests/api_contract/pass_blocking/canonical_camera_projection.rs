#![cfg(feature = "blocking")]

use grafton_visca::{
    blocking::{Camera, Operation},
    capabilities::PanTilt,
    completion::Kind,
    profiles::PtzOpticsG2,
    CompileTimeProfile,
};

fn profile_gated_view<'a, P>(camera: &Camera<'a, P>)
where
    P: CompileTimeProfile + PanTilt,
{
    let _target = camera.target();
}

fn generic_operation_handle<K: Kind>(handle: Operation<K>) {
    let _ = handle;
}

fn main() {
    let _: fn(&Camera<'static, PtzOpticsG2>) = profile_gated_view::<PtzOpticsG2>;
    let _: fn(Operation<grafton_visca::completion::AppliedOnly>) =
        generic_operation_handle::<grafton_visca::completion::AppliedOnly>;
}
