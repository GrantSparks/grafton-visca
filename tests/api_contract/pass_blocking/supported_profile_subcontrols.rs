use grafton_visca::{
    profiles::{SonyBRCH900, SonyEVIH100, SonyFR7},
    transport::BlockingTransportHandle,
    types::{ColorTemp, IrisLevel},
    BlockingCamera, Error, Normalized, ZoomDomain,
};

fn use_sony_fr7(camera: BlockingCamera<SonyFR7, BlockingTransportHandle>) -> Result<(), Error> {
    camera.set_digital_zoom(true)?;
    camera.zoom_absolute_normalized(Normalized::new(0.75)?, ZoomDomain::OpticalPlusDigital)?;
    camera.exposure().iris_priority()?;
    camera.set_iris(IrisLevel::new(1)?)?;
    camera.tally().red_on()?;
    let _ = camera.tally().status()?;
    let _ = camera.exposure().iris()?;
    Ok(())
}

fn use_sony_brch900(
    camera: BlockingCamera<SonyBRCH900, BlockingTransportHandle>,
) -> Result<(), Error> {
    camera.set_digital_zoom(false)?;
    camera.zoom_absolute_normalized(Normalized::new(0.5)?, ZoomDomain::OpticalPlusDigital)?;
    camera.exposure().iris_priority()?;
    camera.set_iris(IrisLevel::new(1)?)?;
    camera.white_balance().color_temperature_mode()?;
    camera.set_color_temperature(ColorTemp::from_kelvin(5600)?)?;
    camera.tally().red_on()?;
    let _ = camera.exposure().iris()?;
    let _ = camera.white_balance().color_temperature()?;
    Ok(())
}

fn use_sony_evih100(
    camera: BlockingCamera<SonyEVIH100, BlockingTransportHandle>,
) -> Result<(), Error> {
    camera.white_balance().color_temperature_mode()?;
    camera.set_color_temperature(ColorTemp::from_kelvin(5600)?)?;
    let _ = camera.white_balance().color_temperature()?;
    Ok(())
}

fn main() {
    let _ = use_sony_fr7;
    let _ = use_sony_brch900;
    let _ = use_sony_evih100;
}
