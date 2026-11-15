use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0, response = Power, parser = ExtendedNibble)]
struct InvalidInquiry;

fn main() {}

//~ "parser = ExtendedNibble is unsupported"
//~ "remove parser to use the canonical inquiry decoder"
