#[allow(unused_macros)]
macro_rules! assert {
    ($($tokens:tt)*) => {};
}

#[grafton_visca::value(min = 0, max = 255, wire = biased(1))]
struct Invalid(u8);

fn main() {}

//~ E0080
//~ "value wire representation overflows"
