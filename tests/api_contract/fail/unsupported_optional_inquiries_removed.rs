use grafton_visca::{
    profiles::{GenericVisca, PtzOpticsG2},
    transport::BlockingTransportHandle,
    BlockingCamera,
};

fn main() {
    let g2: Option<BlockingCamera<PtzOpticsG2, BlockingTransportHandle>> = None;
    let generic: Option<BlockingCamera<GenericVisca, BlockingTransportHandle>> = None;
    let _ = g2.unwrap().nd_filter_position();
    let _ = generic.unwrap().nd_filter_preset();
}
