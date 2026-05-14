use grafton_visca::{
    command::MotionSyncMode, profiles::PtzOpticsG2, transport::BlockingTransportHandle,
    BlockingCamera,
};

fn main() {
    let camera: Option<BlockingCamera<PtzOpticsG2, BlockingTransportHandle>> = None;
    let _ = camera.unwrap().set_motion_sync_mode(MotionSyncMode::On);
}
