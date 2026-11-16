use grafton_visca::value;

#[value(min = 0, max = "[")]
struct MalformedMax(u8);

fn main() {}

//~ "value bounds and wire codes must be unsuffixed integer literals"
