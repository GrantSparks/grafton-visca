use grafton_visca::ViscaValue;

#[derive(ViscaValue)]
#[visca_value(min = "0", max = "[")]
struct MalformedMax(u8);

fn main() {}

//~ "`max` must be an unsuffixed integer literal string"
