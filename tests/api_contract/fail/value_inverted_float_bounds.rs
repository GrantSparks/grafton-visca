#[grafton_visca::value(min = 2.0, max = 1.0)]
struct InvertedFloatRange(f32);

fn main() {}

//~ E0080
//~ "value declares min greater than max"
