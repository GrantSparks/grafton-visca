use grafton_visca::ViscaValue;

#[derive(ViscaValue)]
#[visca_value(display_format = "hex")]
struct Unchecked(u8);

fn main() {}

//~ "`visca_value` requires `min` and `max`, or `valid_values`"
