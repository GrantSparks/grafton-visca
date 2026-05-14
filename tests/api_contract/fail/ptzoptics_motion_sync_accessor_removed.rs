use grafton_visca::{
    profiles::{PtzOptics30X, PtzOpticsG2, PtzOpticsG3},
    transport::BlockingTransportHandle,
    BlockingCamera,
};

fn main() {
    let g2: Option<BlockingCamera<PtzOpticsG2, BlockingTransportHandle>> = None;
    let g3: Option<BlockingCamera<PtzOpticsG3, BlockingTransportHandle>> = None;
    let thirty_x: Option<BlockingCamera<PtzOptics30X, BlockingTransportHandle>> = None;

    let _ = g2.unwrap().motion_sync();
    let _ = g3.unwrap().motion_sync();
    let _ = thirty_x.unwrap().motion_sync();
}
