use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = ExposureMode, parser = Mode, value_type = ExposureMode, data_variant = Power)]
struct InvalidInquiry;

fn main() {}

//~ "`data_variant` has no effect with `parser = Mode`"
