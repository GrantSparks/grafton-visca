use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, subcode = 0xFF, response = Power)]
//~^ "`subcode` must not be the VISCA terminator 0xFF"
struct InvalidInquiry;

fn main() {}
