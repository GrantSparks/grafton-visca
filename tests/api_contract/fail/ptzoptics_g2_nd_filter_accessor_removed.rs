use grafton_visca::{profiles::PtzOpticsG2, transport::BlockingTransportHandle, BlockingCamera};

fn main() {
    let camera: Option<BlockingCamera<PtzOpticsG2, BlockingTransportHandle>> = None;
    let _ = camera.unwrap().nd_filter();
}
