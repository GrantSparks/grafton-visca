use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power, convention = OnIs02)]
struct InvalidInquiry;

fn main() {}

//~ "`convention` requires a parser that consumes it"
