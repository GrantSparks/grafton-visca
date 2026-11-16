use grafton_visca::value;

#[value(min = "[", max = 4)]
struct MalformedMin(u8);

fn main() {}

//~ "value bounds and wire codes must be unsuffixed integer literals"
