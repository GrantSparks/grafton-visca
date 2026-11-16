use grafton_visca::value;

#[value(min = 0, min = 1, max = 4)]
struct DuplicateAttribute(u8);

fn main() {}

//~ "duplicate `value` attribute `min`"
//~ "first `min` specified here"
