// This aggregate marker described unsupported control surface and must stay
// absent from the public API.
use grafton_visca::capabilities::HasNoiseReduction;

fn main() {}

//~ E0432
//~ "no `HasNoiseReduction` in `capabilities`"
