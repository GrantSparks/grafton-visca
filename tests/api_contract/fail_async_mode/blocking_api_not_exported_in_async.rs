use grafton_visca::{
    profiles::PtzOpticsG2, transport::BlockingTransportHandle, BlockingCamera, BlockingClient,
};

fn main() {
    let _: Option<BlockingCamera<PtzOpticsG2, BlockingTransportHandle>> = None;
    let _: Option<BlockingClient<PtzOpticsG2, BlockingTransportHandle>> = None;
}
