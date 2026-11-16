#[grafton_visca::value(min = f32::NAN, max = 1.0)]
struct NanBound(f32);

fn main() {}

//~ E0080
//~ "value declares a non-finite bound"
