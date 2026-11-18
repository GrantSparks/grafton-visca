use grafton_visca::value;

#[value(valid_values = [])]
//~^ "`valid_values` must be a non-empty array literal"
struct EmptyValidValues(u8);

fn main() {}
