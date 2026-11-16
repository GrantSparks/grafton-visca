use grafton_visca::value;

#[value(min = 0, max = 10, valid_values = [3, 5])]
struct MixedValidValuesBounds(u8);

fn main() {}

//~ "`valid_values` cannot be combined with `min` or `max`"
