#![cfg(feature = "blocking")]

use grafton_visca::{
    blocking::Camera, blocking::Operation, capabilities::HasPanTilt, operation::Kind,
    profiles::CompileTimeProfile, profiles::PtzOpticsG2,
};

fn profile_gated_view<P>(camera: &Camera<P>)
where
    P: CompileTimeProfile + HasPanTilt,
{
    let _target = camera.target();
}

fn generic_operation_handle<K: Kind>(handle: Operation<K>) {
    let _ = handle;
}

fn main() {
    let _: fn(&Camera<PtzOpticsG2>) = profile_gated_view::<PtzOpticsG2>;
    let _: fn(Operation<grafton_visca::operation::AppliedOnly>) =
        generic_operation_handle::<grafton_visca::operation::AppliedOnly>;
}
