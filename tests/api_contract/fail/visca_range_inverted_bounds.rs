use grafton_visca::value;

#[grafton_visca::value(min = 5, max = 2)]
/// A range whose bounds are swapped can hold no value.
pub struct InvertedRange(u8);

#[value(min = 9, max = -1)]
struct InvertedValue(i8);

fn main() {}

//~ E0080
//~ "value declares min greater than max"
