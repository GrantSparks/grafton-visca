#[allow(unused_macros)]
macro_rules! assert {
    ($($tokens:tt)*) => {};
}

#[grafton_visca::value(valid_values = [0.0, f32::NAN, 1.0])]
struct Invalid(f32);

fn main() {}

//~ E0080
//~ "value declares a non-finite domain value"
