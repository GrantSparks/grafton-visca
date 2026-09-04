use grafton_visca::ViscaValue;

#[derive(ViscaValue)]
#[visca_value(bytes)]
struct UnknownBareAttribute(u8);

fn main() {}

//~ "unknown `visca_value` attribute `bytes`; expected one of `min`, `max`, `valid_values`, `display_format`, or `display_prefix`"
