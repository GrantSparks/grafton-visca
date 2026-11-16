#[allow(unused_macros)]
macro_rules! assert {
    ($($tokens:tt)*) => {};
}

#[grafton_visca::value(min = -1, max = 1, wire = biased(0))]
struct Invalid(i8);

fn main() {}

//~ E0080
//~ "value wire representation underflows"
