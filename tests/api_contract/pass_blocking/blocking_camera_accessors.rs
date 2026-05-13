use grafton_visca::{
    command::{NdFilterMode, NdFilterStep},
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
    camera.tally().bright_hi()?;
    camera.tally().flash()?;
    camera.tally().off()?;
    camera.nd_filter().set_mode(NdFilterMode::Variable)?;
    camera.nd_filter().set_value(4)?;
    camera.nd_filter().set_stops(3.0)?;
    camera.nd_filter().step(NdFilterStep::Up)?;
    camera.nd_filter().set_auto(false)?;
    camera.system().interface_clear()?;
    Ok(())
}

fn main() {
    let _ = use_blocking_camera;
}
