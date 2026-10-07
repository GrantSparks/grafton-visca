use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00u8, response = Power)]
struct SuffixedOpcodeInquiry;

fn main() {}

//~ "`opcode` must be an unsuffixed integer literal in 0..=255"
