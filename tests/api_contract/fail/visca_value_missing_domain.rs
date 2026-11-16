use grafton_visca::value;

#[value(display = "{:#04x}")]
struct Unchecked(u8);

fn main() {}

//~ "`value` requires `min` and `max`, or `valid_values`"
