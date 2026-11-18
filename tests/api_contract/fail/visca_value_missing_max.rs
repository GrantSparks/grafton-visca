use grafton_visca::value;

#[value(min = 0)]
//~^ "`min` requires `max`; specify both bounds or use `valid_values`"
struct MissingMax(u8);

fn main() {}
