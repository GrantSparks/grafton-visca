use grafton_visca::{
    command::{ImageFlipMode, PictureEffectMode},
    profiles::{PtzOpticsG2, SonyFR7},
    transport::BlockingTransportHandle,
    types::{
        ColorTemp, DynamicRangeLevel, HueLevel, NoiseReduction2DLevel, NoiseReduction3DLevel,
        RedChannel, RedTuning, SaturationLevel,
    },
    BlockingCamera, Error,
};

fn use_ptzoptics_g2(
    camera: BlockingCamera<PtzOpticsG2, BlockingTransportHandle>,
) -> Result<(), Error> {
    camera.set_backlight(true)?;
    camera.set_dynamic_range(DynamicRangeLevel::new(1)?)?;
    camera.white_balance().one_push()?;
    camera.white_balance().one_push_trigger()?;
    camera.white_balance().color_temperature_mode()?;
    camera.set_color_temperature(ColorTemp::from_kelvin(5600)?)?;
    camera.set_red_gain(RedChannel::new(1)?)?;
    camera.set_red_tuning(RedTuning::NEUTRAL)?;
    camera.set_saturation(SaturationLevel::new(1)?)?;
    camera.set_hue(HueLevel::new(1)?)?;
    camera.set_image_flip(ImageFlipMode::Both)?;
    camera.set_noise_reduction_2d(NoiseReduction2DLevel::new(1)?)?;
    camera.set_noise_reduction_3d(NoiseReduction3DLevel::new(1)?)?;
    camera.set_picture_effect(PictureEffectMode::BlackAndWhite)?;
    let _ = camera.white_balance().red_gain()?;
    let _ = camera.image().noise_reduction_2d()?;
    let _ = camera.image().picture_effect()?;
    Ok(())
}

fn use_sony_fr7(camera: BlockingCamera<SonyFR7, BlockingTransportHandle>) -> Result<(), Error> {
    camera.set_picture_effect(PictureEffectMode::BlackAndWhite)?;
    let _ = camera.image().picture_effect()?;
    Ok(())
}

fn main() {
    let _ = use_ptzoptics_g2;
    let _ = use_sony_fr7;
}
