use grafton_visca::{
    command::{NdFilterMode, NdFilterStep, VariableSpeedMode},
    profiles::{PtzOpticsG2, SonyFR7},
    transport::BlockingTransportHandle,
    types::ZoomPosition,
    units::{Degrees, UnitInterval},
    BlockingCamera, Error, SpeedLevel,
};

fn use_blocking_camera(
    camera: BlockingCamera<PtzOpticsG2, BlockingTransportHandle>,
) -> Result<(), Error> {
    camera.power().on()?;
    camera.zoom().set_position(ZoomPosition::new(0x2000)?)?;
    camera.zoom().set_normalized(UnitInterval::new(0.5)?)?;
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

fn use_sony_fr7_nd_filter(
    camera: BlockingCamera<SonyFR7, BlockingTransportHandle>,
) -> Result<(), Error> {
    camera.nd_filter().set_mode(NdFilterMode::Variable)?;
    camera.nd_filter().set_value(4)?;
    camera.nd_filter().set_stops(3.0)?;
    camera.nd_filter().step(NdFilterStep::Up)?;
    camera.nd_filter().set_auto(false)?;
    camera.tally().bright_hi()?;
    camera.tally().off()?;
    camera.set_variable_speed_mode(VariableSpeedMode::Fine50)?;
    Ok(())
}

fn main() {
    let _ = use_blocking_camera;
    let _ = use_sony_fr7_nd_filter;
}
