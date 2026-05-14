use grafton_visca::{
    command::VariableSpeedMode, profiles::PtzOpticsG2, transport::BlockingTransportHandle,
    BlockingCamera,
};

fn main() {
    let camera: Option<BlockingCamera<PtzOpticsG2, BlockingTransportHandle>> = None;
    let _ = camera
        .unwrap()
        .set_variable_speed_mode(VariableSpeedMode::Fine50);
}
