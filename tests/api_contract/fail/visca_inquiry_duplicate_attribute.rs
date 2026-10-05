use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power)]
#[visca(opcode = 0x01)]
struct DuplicateOpcodeInquiry;

fn main() {}

//~ "duplicate `visca` attribute `opcode`"
//~ "first `opcode` specified here"
