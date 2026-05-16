use grafton_visca::{
    profiles::{SonyBRCH900, SonyEVIH100, SonyFR7},
    transport::BlockingTransportHandle,
    types::{BrightnessLevel, ColorTemp, ContrastLevel, IrisLevel, SharpnessLevel},
    BlockingCamera, Error, Normalized, ZoomDomain,
};

fn use_sony_fr7(camera: BlockingCamera<SonyFR7, BlockingTransportHandle>) -> Result<(), Error> {
    camera.set_digital_zoom(true)?;
    camera.zoom_absolute_normalized(Normalized::new(0.75)?, ZoomDomain::OpticalPlusDigital)?;
    camera.exposure().iris_priority()?;
    camera.set_iris(IrisLevel::new(1)?)?;
    camera.exposure().set_brightness(BrightnessLevel::new(1)?)?;
    camera.image().set_contrast(ContrastLevel::new(1)?)?;
    camera.image().set_sharpness(SharpnessLevel::new(1)?)?;
    let _ = camera.exposure().brightness()?;
    let _ = camera.image().contrast()?;
    let _ = camera.image().sharpness_level()?;
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
    camera.exposure().set_brightness(BrightnessLevel::new(1)?)?;
    camera.image().set_contrast(ContrastLevel::new(1)?)?;
    camera.image().set_sharpness(SharpnessLevel::new(1)?)?;
    camera.white_balance().color_temperature_mode()?;
    camera.set_color_temperature(ColorTemp::from_kelvin(5600)?)?;
    camera.tally().red_on()?;
    let _ = camera.exposure().iris()?;
    let _ = camera.white_balance().color_temperature()?;
    Ok(())
}

fn use_ptzoptics_quality(
    camera: BlockingCamera<grafton_visca::profiles::PtzOpticsG2, BlockingTransportHandle>,
) -> Result<(), Error> {
    camera.exposure().bright_mode()?;
    camera.exposure().set_brightness(BrightnessLevel::new(1)?)?;
    camera.image().set_contrast(ContrastLevel::new(1)?)?;
    camera.image().set_sharpness(SharpnessLevel::new(1)?)?;
    let _ = camera.exposure().brightness()?;
    let _ = camera.image().contrast()?;
    let _ = camera.image().sharpness_mode()?;
    let _ = camera.image().sharpness_level()?;
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
    let _ = use_ptzoptics_quality;
}
