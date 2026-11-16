#[allow(unused_macros)]
macro_rules! assert {
    ($($tokens:tt)*) => {};
}

#[grafton_visca::value(min = 4, max = 1)]
struct Invalid(u8);

fn main() {}

//~ E0080
//~ "value declares min greater than max"
