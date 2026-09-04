use grafton_visca::ViscaValue;

#[derive(ViscaValue)]
#[visca_value(min = "[", max = "4")]
struct MalformedMin(u8);

fn main() {}

//~ "`min` must be an unsuffixed integer literal string"
