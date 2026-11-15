use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0xFF, response = Power)]
struct InvalidInquiry;

fn main() {}

//~ "`opcode` must not be the VISCA terminator 0xFF"
