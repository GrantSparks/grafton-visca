use grafton_visca::ViscaValue;

#[derive(ViscaValue)]
#[visca_value(max = "4")]
struct MissingMin(u8);

fn main() {}

//~ "`max` requires `min`; specify both bounds or use `valid_values`"
