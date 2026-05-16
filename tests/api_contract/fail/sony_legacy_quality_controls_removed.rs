use grafton_visca::{
    profiles::{SonyBRC300, SonyEVIH100},
    transport::BlockingTransportHandle,
    types::{BrightnessLevel, ContrastLevel, SharpnessLevel},
    BlockingCamera,
};

fn main() {
    let brc300: Option<BlockingCamera<SonyBRC300, BlockingTransportHandle>> = None;
    let brc300 = brc300.unwrap();
    let _ = brc300
        .exposure()
        .set_brightness(BrightnessLevel::new(1).unwrap());
    let _ = brc300.exposure().brightness();
    let _ = brc300.image().set_contrast(ContrastLevel::new(1).unwrap());
    let _ = brc300.image().contrast();
    let _ = brc300
        .image()
        .set_sharpness(SharpnessLevel::new(1).unwrap());
    let _ = brc300.image().sharpness_level();

    let evih100: Option<BlockingCamera<SonyEVIH100, BlockingTransportHandle>> = None;
    let evih100 = evih100.unwrap();
    let _ = evih100
        .exposure()
        .set_brightness(BrightnessLevel::new(1).unwrap());
    let _ = evih100.exposure().brightness();
    let _ = evih100.image().set_contrast(ContrastLevel::new(1).unwrap());
    let _ = evih100.image().contrast();
    let _ = evih100
        .image()
        .set_sharpness(SharpnessLevel::new(1).unwrap());
    let _ = evih100.image().sharpness_level();
}
