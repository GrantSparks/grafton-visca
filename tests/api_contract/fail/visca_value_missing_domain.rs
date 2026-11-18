use grafton_visca::value;

#[value(display = "{:#04x}")]
//~^ "`value` requires `min` and `max`, or `valid_values`"
struct Unchecked(u8);

fn main() {}
