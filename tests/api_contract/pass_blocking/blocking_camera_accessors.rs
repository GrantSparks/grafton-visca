use grafton_visca::{
    profiles::PtzOpticsG2,
    transport::BlockingTransportHandle,
    units::{Degrees, Normalized},
    BlockingCamera, Error, SpeedLevel,
};

fn use_blocking_camera(
    camera: BlockingCamera<PtzOpticsG2, BlockingTransportHandle>,
) -> Result<(), Error> {
    camera.power().on()?;
    camera.zoom().set_position(Normalized(0.5))?;
    camera
        .pan_tilt()
        .absolute(Degrees(0.0), Degrees(0.0), SpeedLevel::Medium)?;
    camera.focus().auto()?;
    camera.exposure().auto()?;
    camera.white_balance().auto()?;
    camera.presets().recall(1)?;
    camera.system().interface_clear()?;
    Ok(())
}

fn main() {
    let _ = use_blocking_camera;
}
