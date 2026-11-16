#[grafton_visca::value(min = 0, max = 70_000)]
/// `ParameterOutOfRange` reports `i32` bounds, so a wider inner type is rejected.
pub struct WideRange(u32);

fn main() {}

//~ "range bounds are reported as `i32`; use `u8`, `u16`, `i8`, `i16` or `i32`"
