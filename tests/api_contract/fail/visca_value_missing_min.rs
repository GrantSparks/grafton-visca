use grafton_visca::value;

#[value(max = 4)]
//~^ "`max` requires `min`; specify both bounds or use `valid_values`"
struct MissingMin(u8);

fn main() {}
