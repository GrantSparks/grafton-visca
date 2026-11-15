use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Iris, parser = Nibble, field = position, data_variant = Power)]
struct InvalidInquiry;

fn main() {}

//~ "`data_variant` has no effect with `parser = Nibble`"
