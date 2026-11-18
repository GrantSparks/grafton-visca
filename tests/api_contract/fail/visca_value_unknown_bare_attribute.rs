use grafton_visca::value;

#[value(bytes)]
//~^ "unknown `value` attribute `bytes`; expected `min`, `max`, `valid_values`, `display`, `wire`, `error`, or `crate`"
struct UnknownBareAttribute(u8);

fn main() {}
