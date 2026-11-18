use grafton_visca::value;

#[value(bytes = 2)]
//~^ "unknown `value` attribute `bytes`; expected `min`, `max`, `valid_values`, `display`, `wire`, `error`, or `crate`"
struct UnknownValueAttribute(u8);

fn main() {}
