#[grafton_visca::value(min = f32::NAN, max = 1.0)]
//~^ E0080 "value declares a non-finite bound"
struct NanBound(f32);

fn main() {}
