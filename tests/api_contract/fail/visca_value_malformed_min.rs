use grafton_visca::value;

#[value(min = "[", max = 4)]
//~^ "value bounds and wire codes must be unsuffixed integer literals"
struct MalformedMin(u8);

fn main() {}
