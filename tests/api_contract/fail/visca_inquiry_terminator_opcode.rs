use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0xFF, response = Power)]
//~^ "`opcode` must not be the VISCA terminator 0xFF"
struct InvalidInquiry;

fn main() {}
