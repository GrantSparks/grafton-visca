use grafton_visca::ViscaValue;

#[derive(ViscaValue)]
#[visca_value(min = "0")]
#[visca_value(min = "1", max = "4")]
struct DuplicateAttribute(u8);

fn main() {}

//~ "duplicate `visca_value` attribute `min`"
//~ "first `min` specified here"
