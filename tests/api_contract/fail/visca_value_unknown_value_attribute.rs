use grafton_visca::value;

#[value(bytes = 2)]
struct UnknownValueAttribute(u8);

fn main() {}

//~ "unknown `value` attribute `bytes`; expected `min`, `max`, `valid_values`, `display`, `wire`, `error`, or `crate`"
