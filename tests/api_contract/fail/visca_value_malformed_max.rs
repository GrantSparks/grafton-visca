use grafton_visca::value;

#[value(min = 0, max = "[")]
//~^ "value bounds and wire codes must be unsuffixed integer literals"
struct MalformedMax(u8);

fn main() {}
