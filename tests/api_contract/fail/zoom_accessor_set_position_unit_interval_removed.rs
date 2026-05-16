use grafton_visca::{
    profiles::PtzOpticsG2, transport::BlockingTransportHandle, BlockingCamera, UnitInterval,
};

fn main() {
    let camera: Option<BlockingCamera<PtzOpticsG2, BlockingTransportHandle>> = None;
    let camera = camera.unwrap();

    let _ = camera.zoom().set_position(UnitInterval::new(0.5).unwrap());
}
