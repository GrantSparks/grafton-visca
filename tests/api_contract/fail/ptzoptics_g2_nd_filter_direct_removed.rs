use grafton_visca::{
    command::NdFilterMode, profiles::PtzOpticsG2, transport::BlockingTransportHandle,
    BlockingCamera,
};

fn main() {
    let camera: Option<BlockingCamera<PtzOpticsG2, BlockingTransportHandle>> = None;
    let _ = camera.unwrap().set_nd_filter_mode(NdFilterMode::Variable);
}
