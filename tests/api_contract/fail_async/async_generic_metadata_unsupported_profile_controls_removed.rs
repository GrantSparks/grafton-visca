use grafton_visca::{
    camera::CameraSession,
    command::{ImageFlipMode, PictureEffectMode},
    mode::Async,
    profiles::GenericVisca,
    types::{
        BrightnessLevel, ColorTemp, ContrastLevel, DynamicRangeLevel, NoiseReduction2DLevel,
        RedChannel, RedTuning, SharpnessLevel,
    },
    BacklightCompensationControl, ColorTemperatureControl, Executor, RgbGainControl,
    RgbTuningControl, WideDynamicRangeControl,
};

fn use_generic<Tr, Exec>(session: &CameraSession<Async, GenericVisca, Tr, Exec>)
where
    Tr: grafton_visca::transport::AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + Clone + 'static,
{
    let _ = session.set_backlight(true);
    let _ = session.set_dynamic_range(DynamicRangeLevel::new(1).unwrap());
    let _ = session.set_color_temperature(ColorTemp::from_kelvin(5600).unwrap());
    let _ = session.set_red_gain(RedChannel::new(1).unwrap());
    let _ = session.set_red_tuning(RedTuning::NEUTRAL);
    let _ = session.image().set_flip_mode(ImageFlipMode::Both);
    let _ = session
        .image()
        .set_noise_reduction_2d(NoiseReduction2DLevel::new(1).unwrap());
    let _ = session
        .image()
        .set_picture_effect(PictureEffectMode::BlackAndWhite);
    let _ = session
        .exposure()
        .set_brightness(BrightnessLevel::new(1).unwrap());
    let _ = session.image().set_contrast(ContrastLevel::new(1).unwrap());
    let _ = session
        .image()
        .set_sharpness(SharpnessLevel::new(1).unwrap());
    let _ = session.exposure().brightness();
    let _ = session.image().contrast();
    let _ = session.image().sharpness_level();
    let _ = session.white_balance().red_gain();
    let _ = session.image().flip();
}

fn main() {}
