use grafton_visca::{CameraBuilder, CameraBuilderWithTransport, CameraBuilderWithTransportProfile};

fn main() {
    let _ = core::any::TypeId::of::<CameraBuilder>();
    let _ = core::any::TypeId::of::<CameraBuilderWithTransport<(), ()>>();
    let _ = core::any::TypeId::of::<CameraBuilderWithTransportProfile<(), (), ()>>();
}
