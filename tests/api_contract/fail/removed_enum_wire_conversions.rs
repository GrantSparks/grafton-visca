use grafton_visca::ExposureMode;

fn main() {
    let _ = ExposureMode::try_from(0x00_u8);
    let _ = u8::from(ExposureMode::Auto);
    let _ = ExposureMode::is_valid_discriminant(0);
}

//~ E0277
//~ E0599
//~ "is_valid_discriminant"
