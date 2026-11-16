use grafton_visca::value;

#[value(valid_values = [])]
struct EmptyValidValues(u8);

fn main() {}

//~ "`valid_values` must be a non-empty array literal"
