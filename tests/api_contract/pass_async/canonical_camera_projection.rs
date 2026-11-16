#![cfg(feature = "async")]

use grafton_visca::{
    capabilities::HasPanTilt, operation::Kind, profiles::CompileTimeProfile, profiles::PtzOpticsG2,
    r#async::Camera, r#async::Operation,
};

fn profile_gated_view<P>(camera: &Camera<P>)
where
    P: CompileTimeProfile + HasPanTilt,
{
    let _clone: Camera<P> = camera.clone();
}

fn generic_operation_handle<K: Kind>(handle: Operation<K>) {
    let _ = handle;
}

fn main() {
    let _: fn(&Camera<PtzOpticsG2>) = profile_gated_view::<PtzOpticsG2>;
    let _: fn(Operation<grafton_visca::operation::AppliedOnly>) =
        generic_operation_handle::<grafton_visca::operation::AppliedOnly>;
}
