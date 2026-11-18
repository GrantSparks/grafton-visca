use grafton_visca::{CameraBuilder, CameraBuilderWithTransport, CameraBuilderWithTransportProfile};
//~^ E0432 "unresolved imports `grafton_visca::CameraBuilder`"

fn main() {
    let _ = core::any::TypeId::of::<CameraBuilder>();
    let _ = core::any::TypeId::of::<CameraBuilderWithTransport<(), ()>>();
    let _ = core::any::TypeId::of::<CameraBuilderWithTransportProfile<(), (), ()>>();
}
