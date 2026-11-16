#[grafton_visca::value(min = 0.0, max = f64::INFINITY)]
struct InfiniteBound(f64);

fn main() {}

//~ E0080
//~ "value declares a non-finite bound"
