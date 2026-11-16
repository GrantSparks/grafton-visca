#[allow(unused_macros)]
macro_rules! assert {
    ($($tokens:tt)*) => {};
}

#[grafton_visca::value(min = f32::NAN, max = 1.0)]
struct Invalid(f32);

fn main() {}

//~ E0080
//~ "value declares a non-finite bound"
