use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0, response = Power, parser = FocusRange)]
struct InvalidInquiry;

fn main() {}

//~ "parser = FocusRange is unsupported"
//~ "remove parser to use the canonical inquiry decoder"
