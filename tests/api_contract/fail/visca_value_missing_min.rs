use grafton_visca::value;

#[value(max = 4)]
struct MissingMin(u8);

fn main() {}

//~ "`max` requires `min`; specify both bounds or use `valid_values`"
