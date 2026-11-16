use grafton_visca::value;

#[value(min = 0)]
struct MissingMax(u8);

fn main() {}

//~ "`min` requires `max`; specify both bounds or use `valid_values`"
