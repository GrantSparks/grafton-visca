use grafton_visca::{
    profiles::{SonyBRCH900, SonyFR7},
    transport::BlockingTransportHandle,
    types::IrisLevel,
    BlockingCamera, Error, Normalized, ZoomDomain,
};

fn use_sony_fr7(camera: BlockingCamera<SonyFR7, BlockingTransportHandle>) -> Result<(), Error> {
    camera.set_digital_zoom(true)?;
    camera.zoom_absolute_normalized(Normalized::new(0.75)?, ZoomDomain::OpticalPlusDigital)?;
    camera.exposure().iris_priority()?;
    camera.set_iris(IrisLevel::new(1)?)?;
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
    let _ = camera.exposure().iris()?;
    Ok(())
}

fn main() {
    let _ = use_sony_fr7;
    let _ = use_sony_brch900;
}
