#[grafton_visca::value(min = 0.0, max = f64::INFINITY)]
//~^ E0080 "value declares a non-finite bound"
struct InfiniteBound(f64);

fn main() {}
