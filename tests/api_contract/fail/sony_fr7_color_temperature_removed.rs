use grafton_visca::{
    profiles::SonyFR7, transport::BlockingTransportHandle, types::ColorTemp, BlockingCamera,
};

fn main() {
    let camera: Option<BlockingCamera<SonyFR7, BlockingTransportHandle>> = None;
    let camera = camera.unwrap();

    let _ = camera.white_balance().color_temperature_mode();
    let _ = camera.set_color_temperature(ColorTemp::from_kelvin(5600).unwrap());
    let _ = camera.white_balance().color_temperature();
    let _ = camera.color_temperature();
}
