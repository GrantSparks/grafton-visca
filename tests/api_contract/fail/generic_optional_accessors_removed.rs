use grafton_visca::{profiles::GenericVisca, transport::BlockingTransportHandle, BlockingCamera};

fn main() {
    let camera: Option<BlockingCamera<GenericVisca, BlockingTransportHandle>> = None;
    let camera = camera.unwrap();
    let _ = camera.nd_filter();
    let _ = camera.motion_sync();
}
