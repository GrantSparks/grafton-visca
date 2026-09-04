#![cfg(feature = "async")]

use grafton_visca::{
    capabilities::PanTilt, completion::Kind, profiles::PtzOpticsG2, Camera, CompileTimeProfile,
    Operation,
};

fn profile_gated_view<P>(camera: &Camera<P>)
where
    P: CompileTimeProfile + PanTilt,
{
    let _clone: Camera<P> = camera.clone();
}

fn generic_operation_handle<K: Kind>(handle: Operation<K>) {
    let _ = handle;
}

fn main() {
    let _: fn(&Camera<PtzOpticsG2>) = profile_gated_view::<PtzOpticsG2>;
    let _: fn(Operation<grafton_visca::completion::AppliedOnly>) =
        generic_operation_handle::<grafton_visca::completion::AppliedOnly>;
}
