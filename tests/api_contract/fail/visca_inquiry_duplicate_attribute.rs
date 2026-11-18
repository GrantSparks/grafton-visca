use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power)]
//~^ "first `opcode` specified here"
#[visca(opcode = 0x01)]
//~^ "duplicate `visca` attribute `opcode`"
struct DuplicateOpcodeInquiry;

fn main() {}
