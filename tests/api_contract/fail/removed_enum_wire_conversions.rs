use grafton_visca::types::ExposureMode;

fn main() {
    let _ = ExposureMode::try_from(0x00_u8);
    //~^ E0277 "the trait bound `grafton_visca::types::ExposureMode: From<u8>` is not satisfied"
    //~^^ E0277 "the trait bound `grafton_visca::types::ExposureMode: TryFrom<u8>` is not satisfied"
    let _ = u8::from(ExposureMode::Auto);
    //~^ E0277 "the trait bound `u8: From<grafton_visca::types::ExposureMode>` is not satisfied"
    let _ = ExposureMode::is_valid_discriminant(0);
    //~^ E0599 "no variant, associated function, or constant named `is_valid_discriminant` found for enum `grafton_visca::types::ExposureMode` in the current scope"
}
