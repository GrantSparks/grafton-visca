use grafton_visca::{profiles::SonyFR7, transport::BlockingTransportHandle, BlockingCamera};

fn main() {
    let camera: Option<BlockingCamera<SonyFR7, BlockingTransportHandle>> = None;
    let _ = camera.unwrap().motion_sync();
}
