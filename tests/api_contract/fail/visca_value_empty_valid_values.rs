use grafton_visca::ViscaValue;

#[derive(ViscaValue)]
#[visca_value(valid_values = "[]")]
struct EmptyValidValues(u8);

fn main() {}

//~ "`valid_values` must be a non-empty array literal"
