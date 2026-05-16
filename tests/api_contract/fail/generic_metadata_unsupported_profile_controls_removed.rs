use grafton_visca::{
    command::{ImageFlipMode, PictureEffectMode},
    profiles::GenericVisca,
    transport::BlockingTransportHandle,
    types::{
        BrightnessLevel, ColorTemp, ContrastLevel, DynamicRangeLevel, NoiseReduction2DLevel,
        RedChannel, RedTuning, SharpnessLevel,
    },
    BlockingCamera,
};

fn main() {
    let camera: Option<BlockingCamera<GenericVisca, BlockingTransportHandle>> = None;
    let camera = camera.unwrap();

    let _ = camera.set_backlight(true);
    let _ = camera.set_dynamic_range(DynamicRangeLevel::new(1).unwrap());
    let _ = camera.set_color_temperature(ColorTemp::from_kelvin(5600).unwrap());
    let _ = camera.set_red_gain(RedChannel::new(1).unwrap());
    let _ = camera.set_red_tuning(RedTuning::NEUTRAL);
    let _ = camera.enable_flip();
    let _ = camera.set_image_flip(ImageFlipMode::Both);
    let _ = camera.set_noise_reduction_2d(NoiseReduction2DLevel::new(1).unwrap());
    let _ = camera.set_picture_effect(PictureEffectMode::BlackAndWhite);
    let _ = camera.set_brightness(BrightnessLevel::new(1).unwrap());
    let _ = camera.set_contrast(ContrastLevel::new(1).unwrap());
    let _ = camera.set_sharpness(SharpnessLevel::new(1).unwrap());

    let _ = camera.exposure().brightness();
    let _ = camera.image().contrast();
    let _ = camera.image().sharpness_level();
    let _ = camera.white_balance().red_gain();
    let _ = camera.white_balance().color_temperature();
    let _ = camera.image().flip();
    let _ = camera.image().noise_reduction_2d();
    let _ = camera.image().picture_effect();
}
