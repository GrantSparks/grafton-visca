use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0b1_0000_0000, response = Power)]
struct WideOpcodeInquiry;

fn main() {}

//~ "`opcode` must be an unsuffixed integer literal in 0..=255"
