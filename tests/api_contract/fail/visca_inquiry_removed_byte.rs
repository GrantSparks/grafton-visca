use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0, response = Power, parser = Byte)]
struct InvalidInquiry;

fn main() {}

//~ "parser = Byte is unsupported"
//~ "remove parser to use the canonical inquiry decoder"
